// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The topic table: the topics' model as every device runs it, with no word
//! of the person's mail in it, each word under a 64-bit hash
//! (`spam::table::word_hash`), only words seen five times or more.
//!
//! The model is fastText's classifier with softmax over every topic at once
//! (one topic per message), and, when the training learned one, a last class
//! for "no topic". It reads a message as one line ([`super::line`]): the
//! sender's words, then the message's. Each word brings its input rows (its
//! own when fastText knows it, those of its character n-grams' buckets), and
//! class `c`'s score is the mean of every row's scalar `s_rc = w_c · row_r`,
//! the end of the line's row included. So the table keeps, for each word
//! fastText knows, the sum of its rows' scalars, one per class; for each
//! bucket, its scalars; for the end of the line, its own (`bias`). A
//! message's scores: `f_c = (bias_c + Σ its words' sums_c) / (1 + Σ their
//! rows)`, as the spam filter's table counts rows (`spam::table`, the
//! classifier's kind). Its probabilities: softmax of `f / temperature`,
//! the temperature fitted on mail the model never learned from; the
//! uncalibrated ones (temperature 1) are fastText's own.
//!
//! The bands apply to the top topic's probability: sure from its topic's
//! threshold (`sure`), maybe from `maybe`, no topic below.
//!
//! The file (little-endian), format 1:
//!
//! | bytes | what |
//! |---|---|
//! | 8 | magic `SIOULTOP` |
//! | 4 × 12 | format version (1), tokenizer version, sender words' version, minn, maxn, bucket count B, word count W, class count C, topic count K (C = K, or K + 1 with "no topic" last), bytes per score P (2: f16, 4: f32), metadata length L, a zero |
//! | 4 × 2 | temperature, the maybe band's start (f32) |
//! | 8 × W | the words' hashes, ascending (u64) |
//! | 4 × C | the end of the line's scalars (f32) |
//! | 4 × K | each topic's sure threshold (f32; above 1: never sure) |
//! | P × C × W | each word's sums, class by class |
//! | P × C × B | each bucket's scalars |
//! | L | metadata, JSON (`Meta`) |
//! | 8 | FNV-1a 64 of every byte before it |

use crate::spam::table::{ngram_buckets, word_hash};
use crate::spam::tokenize::TOKENIZER;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The file's first bytes.
pub const MAGIC: [u8; 8] = *b"SIOULTOP";
/// The file format's version.
pub const VERSION: u32 = 1;
/// Bytes before the words' hashes.
const HEADER: usize = 8 + 12 * 4 + 2 * 4;

/// What the settings say of the training that made the table.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Meta {
    /// When it was trained, Unix seconds.
    #[serde(default)]
    pub trained_at: i64,
    /// The topics' ids, in the classes' order.
    #[serde(default)]
    pub topics: Vec<String>,
    /// Messages it learned from.
    #[serde(default)]
    pub messages: u64,
    /// Aggregate measures (`sioul topics eval`), by name.
    #[serde(default)]
    pub metrics: BTreeMap<String, f64>,
    /// The device it was trained on.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub device: String,
    /// The tokenizer's words it reads every message with (`tokenize::Lexicon`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lexicon: Option<crate::words::SpamWords>,
}

/// How a table's scores are kept in its file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Precision {
    /// IEEE 754 half precision: half the size, about three significant digits.
    F16,
    F32,
}

impl Precision {
    fn bytes(self) -> usize {
        match self {
            Precision::F16 => 2,
            Precision::F32 => 4,
        }
    }
}

/// How sure a table is of a message's topic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Band {
    /// Shown on the row and in the topic's filter.
    Sure,
    /// Shown only at the end of the topic's filter, under "Perhaps"; never counted as a label.
    Maybe,
    /// No topic.
    No,
}

/// A message's topic as a table judges it: its top topic (an index into
/// `Meta::topics`), that topic's probability, and the band it falls in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Judged {
    pub topic: Option<usize>,
    pub p: f64,
    pub band: Band,
}

/// The topics' model, folded (see the module's documentation).
#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    pub tokenizer: u32,
    pub senders: u32,
    pub minn: u32,
    pub maxn: u32,
    pub bucket: u32,
    /// The topics, K of them; a last class for "no topic" when `none`.
    pub topics: usize,
    pub none: bool,
    /// The words' hashes, ascending (`sort`; a table read from its file is so).
    pub words: Vec<u64>,
    /// Each word's sums, C per word, in `words`' order.
    pub word_scores: Vec<f32>,
    /// Each bucket's scalars, C per bucket.
    pub buckets: Vec<f32>,
    /// The end of the line's scalars, C.
    pub bias: Vec<f32>,
    pub temperature: f32,
    /// Each topic's sure threshold; above 1, never sure.
    pub sure: Vec<f32>,
    pub maybe: f32,
    pub precision: Precision,
    pub meta: Meta,
}

impl Table {
    /// The classes: the topics, and "no topic" when learned.
    pub fn classes(&self) -> usize {
        self.topics + usize::from(self.none)
    }

    /// Sorts the words by their hash, their scores with them; two words of
    /// one hash (never seen: 64 bits) share their mean.
    pub fn sort(&mut self) {
        let c = self.classes();
        let mut order: Vec<usize> = (0..self.words.len()).collect();
        order.sort_by_key(|&i| self.words[i]);
        let mut words: Vec<u64> = Vec::with_capacity(order.len());
        let mut scores: Vec<f32> = Vec::with_capacity(order.len() * c);
        let mut counts: Vec<u32> = Vec::with_capacity(order.len());
        for i in order {
            let row = &self.word_scores[i * c..(i + 1) * c];
            if words.last() == Some(&self.words[i]) {
                let at = scores.len() - c;
                for (s, x) in scores[at..].iter_mut().zip(row) {
                    *s += x;
                }
                if let Some(n) = counts.last_mut() {
                    *n += 1;
                }
            } else {
                words.push(self.words[i]);
                scores.extend_from_slice(row);
                counts.push(1);
            }
        }
        for (w, n) in counts.iter().enumerate().filter(|(_, n)| **n > 1) {
            for s in &mut scores[w * c..(w + 1) * c] {
                *s /= *n as f32;
            }
        }
        self.words = words;
        self.word_scores = scores;
    }

    /// Each class's score for a line (`topics::line`): the mean of its rows' scalars.
    pub fn logits(&self, line: &[String]) -> Vec<f64> {
        let c = self.classes();
        let mut sums: Vec<f64> = self.bias.iter().map(|&b| f64::from(b)).collect();
        sums.resize(c, 0.0);
        let mut rows = 1usize;
        let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
        for word in line {
            *seen.entry(word.as_str()).or_default() += 1;
        }
        for (word, count) in seen {
            let buckets = ngram_buckets(word, self.minn, self.maxn, self.bucket);
            match self.words.binary_search(&word_hash(word)) {
                Ok(at) => {
                    for (s, x) in sums.iter_mut().zip(&self.word_scores[at * c..(at + 1) * c]) {
                        *s += count as f64 * f64::from(*x);
                    }
                    rows += count * (1 + buckets.len());
                }
                Err(_) => {
                    for b in &buckets {
                        if let Some(row) = self.buckets.get(*b as usize * c..(*b as usize + 1) * c) {
                            for (s, x) in sums.iter_mut().zip(row) {
                                *s += count as f64 * f64::from(*x);
                            }
                        }
                    }
                    rows += count * buckets.len();
                }
            }
        }
        sums.into_iter().map(|s| s / rows as f64).collect()
    }

    /// Each class's probability for a line, at this temperature (1: fastText's own).
    pub fn probabilities(&self, line: &[String], temperature: f64) -> Vec<f64> {
        softmax(&self.logits(line), temperature)
    }

    /// A line's topic: its top topic, that topic's calibrated probability,
    /// and its band (see the module's documentation).
    pub fn judge(&self, line: &[String]) -> Judged {
        let p = self.probabilities(line, f64::from(self.temperature));
        judged(&p[..self.topics.min(p.len())], &self.sure, f64::from(self.maybe))
    }

    /// The table as its file holds it, words sorted, its scores at its precision, checksummed.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut table = self.clone();
        table.sort();
        let c = table.classes();
        let meta = serde_json::to_vec(&table.meta).unwrap_or_default();
        let p = table.precision.bytes();
        let mut out = Vec::with_capacity(HEADER + table.words.len() * (8 + p * c) + table.buckets.len() * p + 4 * (c + table.topics) + meta.len() + 8);
        out.extend_from_slice(&MAGIC);
        let counts = [VERSION, table.tokenizer, table.senders, table.minn, table.maxn, table.bucket, table.words.len() as u32, c as u32, table.topics as u32, p as u32, meta.len() as u32, 0];
        for n in counts {
            out.extend_from_slice(&n.to_le_bytes());
        }
        for x in [table.temperature, table.maybe] {
            out.extend_from_slice(&x.to_le_bytes());
        }
        for h in &table.words {
            out.extend_from_slice(&h.to_le_bytes());
        }
        let mut bias = table.bias.clone();
        bias.resize(c, 0.0);
        let mut sure = table.sure.clone();
        sure.resize(table.topics, 2.0);
        for x in bias.iter().chain(&sure) {
            out.extend_from_slice(&x.to_le_bytes());
        }
        let mut buckets = table.buckets.clone();
        buckets.resize(table.bucket as usize * c, 0.0);
        for x in table.word_scores.iter().chain(&buckets) {
            match table.precision {
                Precision::F16 => out.extend_from_slice(&to_f16(*x).to_le_bytes()),
                Precision::F32 => out.extend_from_slice(&x.to_le_bytes()),
            }
        }
        out.extend_from_slice(&meta);
        let sum = fnv64(&out);
        out.extend_from_slice(&sum.to_le_bytes());
        out
    }

    /// Reads a table, refusing one that is damaged, or made by another
    /// tokenizer or other sender words than this Sioul's.
    pub fn from_bytes(bytes: &[u8]) -> Result<Table, String> {
        if bytes.len() < HEADER + 8 || bytes[..8] != MAGIC {
            return Err("not a topic table".into());
        }
        let (body, sum) = bytes.split_at(bytes.len() - 8);
        if fnv64(body) != u64::from_le_bytes(sum.try_into().map_err(|_| "no checksum")?) {
            return Err("damaged: its checksum does not match".into());
        }
        let u32_at = |i: usize| u32::from_le_bytes([body[8 + 4 * i], body[9 + 4 * i], body[10 + 4 * i], body[11 + 4 * i]]);
        let f32_at = |at: usize| f32::from_le_bytes([body[at], body[at + 1], body[at + 2], body[at + 3]]);
        let [version, tokenizer, senders, minn, maxn, bucket, words, classes, topics, p, meta, _] = std::array::from_fn(u32_at);
        if version != VERSION {
            return Err(format!("format {version}, this Sioul reads {VERSION}"));
        }
        if tokenizer != TOKENIZER || senders != super::SENDERS {
            return Err(format!("made with tokenizer {tokenizer} and sender words {senders}, this Sioul has {TOKENIZER} and {}: train again", super::SENDERS));
        }
        if minn > maxn || maxn > 64 {
            return Err(format!("n-grams of {minn} to {maxn} characters"));
        }
        if topics == 0 || !(classes == topics || classes == topics + 1) {
            return Err(format!("{classes} classes for {topics} topics"));
        }
        let precision = match p {
            2 => Precision::F16,
            4 => Precision::F32,
            other => return Err(format!("scores of {other} bytes")),
        };
        let (bucket, words, c, k, p, meta) = (bucket as usize, words as usize, classes as usize, topics as usize, p as usize, meta as usize);
        let expected = (|| HEADER.checked_add(words.checked_mul(8)?)?.checked_add(4 * (c + k))?.checked_add(words.checked_mul(c)?.checked_mul(p)?)?.checked_add(bucket.checked_mul(c)?.checked_mul(p)?)?.checked_add(meta))();
        if expected != Some(body.len()) {
            return Err("damaged: its parts do not add up to its length".into());
        }
        let hashes_at = HEADER;
        let bias_at = hashes_at + 8 * words;
        let sure_at = bias_at + 4 * c;
        let scores_at = sure_at + 4 * k;
        let buckets_at = scores_at + p * c * words;
        let meta_at = buckets_at + p * c * bucket;
        let hashes: Vec<u64> = (0..words).map(|i| u64::from_le_bytes(body[hashes_at + 8 * i..hashes_at + 8 * i + 8].try_into().unwrap_or_default())).collect();
        if hashes.windows(2).any(|w| w[0] >= w[1]) {
            return Err("damaged: its words are out of order".into());
        }
        let scores = |start: usize, n: usize| -> Vec<f32> {
            (0..n)
                .map(|i| match precision {
                    Precision::F16 => from_f16(u16::from_le_bytes([body[start + 2 * i], body[start + 2 * i + 1]])),
                    Precision::F32 => f32_at(start + 4 * i),
                })
                .collect()
        };
        let table = Table {
            tokenizer,
            senders,
            minn,
            maxn,
            bucket: bucket as u32,
            topics: k,
            none: c == k + 1,
            words: hashes,
            word_scores: scores(scores_at, words * c),
            buckets: scores(buckets_at, bucket * c),
            bias: (0..c).map(|i| f32_at(bias_at + 4 * i)).collect(),
            temperature: f32_at(HEADER - 8),
            sure: (0..k).map(|i| f32_at(sure_at + 4 * i)).collect(),
            maybe: f32_at(HEADER - 4),
            precision,
            meta: serde_json::from_slice(&body[meta_at..]).map_err(|e| format!("damaged: its metadata: {e}"))?,
        };
        let finite = table.word_scores.iter().chain(&table.buckets).chain(&table.bias).chain(&table.sure).chain([&table.temperature, &table.maybe]).all(|x| x.is_finite());
        if !finite || table.temperature <= 0.0 {
            return Err("damaged: a score is not a number".into());
        }
        Ok(table)
    }
}

/// Softmax of scores divided by a temperature, computed from their maximum.
pub fn softmax(logits: &[f64], temperature: f64) -> Vec<f64> {
    let t = if temperature > 0.0 { temperature } else { 1.0 };
    let top = logits.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let exp: Vec<f64> = logits.iter().map(|x| ((x - top) / t).exp()).collect();
    let sum: f64 = exp.iter().sum();
    exp.into_iter().map(|e| e / sum).collect()
}

/// The top topic among the topics' probabilities (`p`, "no topic" left
/// out), and its band: sure from its threshold, maybe from `maybe`.
pub fn judged(p: &[f64], sure: &[f32], maybe: f64) -> Judged {
    let Some((topic, &top)) = p.iter().enumerate().max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal)) else {
        return Judged { topic: None, p: 0.0, band: Band::No };
    };
    let threshold = sure.get(topic).map_or(2.0, |&s| f64::from(s));
    let band = if top >= threshold && threshold <= 1.0 {
        Band::Sure
    } else if top >= maybe {
        Band::Maybe
    } else {
        Band::No
    };
    Judged { topic: (band != Band::No).then_some(topic), p: top, band }
}

/// A float as IEEE 754 half precision, rounded to the nearest, ties to even;
/// too large, infinite.
pub fn to_f16(value: f32) -> u16 {
    let x = value.to_bits();
    let sign = ((x >> 16) & 0x8000) as u16;
    let exponent = ((x >> 23) & 0xff) as i32;
    let mantissa = x & 0x007f_ffff;
    if exponent == 0xff {
        return sign | 0x7c00 | if mantissa != 0 { 0x0200 } else { 0 };
    }
    let e = exponent - 127 + 15;
    if e >= 0x1f {
        return sign | 0x7c00;
    }
    if e <= 0 {
        // Below the smallest normal: a subnormal half, or zero.
        if e < -10 {
            return sign;
        }
        let m = mantissa | 0x0080_0000;
        let shift = (14 - e) as u32;
        let half = m >> shift;
        let rest = m & ((1 << shift) - 1);
        let middle = 1 << (shift - 1);
        let rounded = if rest > middle || (rest == middle && half & 1 == 1) { half + 1 } else { half };
        return sign | rounded as u16;
    }
    let half = ((e as u32) << 10) | (mantissa >> 13);
    let rest = mantissa & 0x1fff;
    // A carry into the exponent is right, up to infinity.
    let rounded = if rest > 0x1000 || (rest == 0x1000 && half & 1 == 1) { half + 1 } else { half };
    sign | rounded as u16
}

/// A half-precision float's value.
pub fn from_f16(bits: u16) -> f32 {
    let sign = u32::from(bits & 0x8000) << 16;
    let exponent = u32::from((bits >> 10) & 0x1f);
    let mantissa = u32::from(bits & 0x03ff);
    let value = match (exponent, mantissa) {
        (0, 0) => sign,
        (0, m) => {
            // Subnormal: shifted until its leading bit is the implicit one.
            let mut e: u32 = 127 - 15 + 1;
            let mut m = m;
            while m & 0x0400 == 0 {
                m <<= 1;
                e -= 1;
            }
            sign | (e << 23) | ((m & 0x03ff) << 13)
        }
        (0x1f, m) => sign | 0x7f80_0000 | (m << 13),
        (e, m) => sign | ((e + 127 - 15) << 23) | (m << 13),
    };
    f32::from_bits(value)
}

fn fnv64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_precision_rounds_to_nearest() {
        assert_eq!(to_f16(1.0), 0x3c00);
        assert_eq!(to_f16(-2.0), 0xc000);
        assert_eq!(to_f16(65504.0), 0x7bff);
        assert_eq!(to_f16(70000.0), 0x7c00, "too large: infinite");
        assert_eq!(to_f16(0.1), 0x2e66);
        assert_eq!(to_f16(5.960_464_5e-8), 0x0001, "the smallest subnormal");
        assert_eq!(to_f16(1e-9), 0x0000);
        assert_eq!(to_f16(-0.0), 0x8000);
        for bits in [0x0001u16, 0x03ff, 0x0400, 0x3555, 0x3c00, 0x7bff, 0xbc01] {
            assert_eq!(to_f16(from_f16(bits)), bits, "{bits:#06x} back and forth");
        }
        let mut rng = 0x1234_5678u64;
        for _ in 0..10_000 {
            rng = rng.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            let x = ((rng >> 33) as f32 / (1u64 << 31) as f32 - 0.5) * 20.0;
            if x.abs() > 1e-4 {
                let back = from_f16(to_f16(x));
                assert!(((back - x) / x).abs() <= 1.0 / 2048.0, "{x} → {back}");
            }
        }
    }

    fn table(precision: Precision) -> Table {
        // Two topics and "no topic"; three words; buckets of 7.
        let words = ["chorale", "impots", "from-dom:chorale.example"];
        let mut table = Table {
            tokenizer: TOKENIZER,
            senders: super::super::SENDERS,
            minn: 3,
            maxn: 4,
            bucket: 7,
            topics: 2,
            none: true,
            words: words.iter().map(|w| word_hash(w)).collect(),
            word_scores: vec![2.0, -1.0, 0.0, -1.0, 2.5, 0.0, 3.0, -2.0, 0.1],
            buckets: (0..21).map(|i| (i as f32 - 10.0) / 20.0).collect(),
            bias: vec![-0.2, -0.1, 0.4],
            temperature: 0.8,
            sure: vec![0.7, 2.0],
            maybe: 0.5,
            precision,
            meta: Meta { trained_at: 1, topics: vec!["chorale".into(), "impots".into()], messages: 10, ..Meta::default() },
        };
        table.sort();
        table
    }

    /// A word's sums in a sorted table.
    fn sums<'a>(t: &'a Table, word: &str) -> &'a [f32] {
        let at = t.words.binary_search(&word_hash(word)).unwrap();
        &t.word_scores[at * 3..at * 3 + 3]
    }

    /// The scores are the mean of the rows' scalars, known words and n-grams of unknown ones alike.
    #[test]
    fn scores_count_rows_as_fasttext_does() {
        let t = table(Precision::F32);
        let line: Vec<String> = ["from-dom:chorale.example", "chorale", "chorale", "zzz"].iter().map(|w| w.to_string()).collect();
        let f = t.logits(&line);
        let rows = |w: &str| ngram_buckets(w, 3, 4, 7);
        let known = |w: &str| 1 + rows(w).len();
        let unknown: Vec<u32> = rows("zzz");
        let n = 1 + known("from-dom:chorale.example") + 2 * known("chorale") + unknown.len();
        for c in 0..3 {
            let mut sum = f64::from(t.bias[c]) + f64::from(sums(&t, "from-dom:chorale.example")[c]) + 2.0 * f64::from(sums(&t, "chorale")[c]);
            sum += unknown.iter().map(|&b| f64::from(t.buckets[b as usize * 3 + c])).sum::<f64>();
            assert!((f[c] - sum / n as f64).abs() < 1e-9, "class {c}");
        }
        let p = t.probabilities(&line, 1.0);
        assert!((p.iter().sum::<f64>() - 1.0).abs() < 1e-12);
        let judged = t.judge(&line);
        assert_eq!(judged.topic.is_some(), judged.band != Band::No);
    }

    /// Written and read again: the same table, f32 exactly, f16 within its precision.
    #[test]
    fn a_table_reads_back() {
        let t = table(Precision::F32);
        assert_eq!(Table::from_bytes(&t.to_bytes()).unwrap(), t);
        assert_eq!(sums(&t, "impots"), &[-1.0, 2.5, 0.0]);
        let half = Table::from_bytes(&table(Precision::F16).to_bytes()).unwrap();
        assert_eq!(half.precision, Precision::F16);
        let line = vec!["impots".to_string(), "unknown".to_string()];
        let (a, b) = (t.probabilities(&line, 0.8), half.probabilities(&line, 0.8));
        assert!(a.iter().zip(&b).all(|(x, y)| (x - y).abs() < 1e-3), "{a:?} {b:?}");
        // Damaged, another format: refused.
        let mut bytes = t.to_bytes();
        bytes[HEADER + 3] ^= 1;
        assert!(Table::from_bytes(&bytes).is_err());
        assert!(Table::from_bytes(b"SIOULSPM").is_err());
    }

    /// Bands: sure from the topic's threshold, maybe from 0.5, never sure above 1.
    #[test]
    fn bands_of_the_top_topic() {
        assert_eq!(judged(&[0.8, 0.1], &[0.7, 0.9], 0.5).band, Band::Sure);
        assert_eq!(judged(&[0.6, 0.1], &[0.7, 0.9], 0.5).band, Band::Maybe);
        let none = judged(&[0.3, 0.2], &[0.7, 0.9], 0.5);
        assert_eq!((none.band, none.topic), (Band::No, None));
        assert_eq!(judged(&[0.1, 0.99], &[0.7, 2.0], 0.5).band, Band::Maybe, "never sure");
        let s = softmax(&[1.0, 2.0, 3.0], 1.0);
        assert!((s[2] - 0.665_240_955_774_821_6).abs() < 1e-12);
        assert!(softmax(&[1.0, 2.0], 0.5)[1] > softmax(&[1.0, 2.0], 1.0)[1]);
    }
}
