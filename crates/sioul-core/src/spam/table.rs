// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The reduced table: the spam filter as every device runs it, a megabyte or
//! two, with no word of your mail in it: each word under a 64-bit hash of it
//! (`word_hash`), never the word itself, and only words seen five times or
//! more.
//!
//! Two kinds (`Kind`), both linear, so that they fold (docs/spam-filter.md,
//! "The model, and how it folds"):
//!
//! - **The classifier's** (`Kind::Rows`, what a training makes): fastText's
//!   supervised model, two labels, reads a message as its words, then its
//!   header facts as words (`features::header_words`), then the end of the
//!   line. Each word brings its input rows (its own, when fastText knows it,
//!   and those of its character n-grams' buckets), and the model's score is
//!   the mean of every row's scalar `s_r = (w_spam − w_ham) · row_r`, the
//!   softmax of two labels being the sigmoid of that. The table keeps, for
//!   each word fastText knows, the sum of its rows' scalars; for each bucket,
//!   its scalar; for the end of the line (`bias`), its own. A message's
//!   score: `f = (bias + Σ its words' sums) / (1 + Σ their rows)`, a word
//!   having `1 + ` its n-grams' count of rows when the table knows it, else
//!   its n-grams' (`ngram_buckets`), each counted as often as it comes. No
//!   header weights: the facts are words.
//! - **The centroid's** (`Kind::Centroid`, format 1's only kind, still made
//!   on request): fastText learns a vector for each word (the language
//!   model, private, never shared), and a linear SVM weighs the mean of a
//!   message's word vectors with its header features (`features.rs`). With
//!   `u = w / σ` the SVM's embedding weights over the standardization's
//!   deviations, a word scores `s_w = u · v_w`, an n-gram bucket `s_b = u ·
//!   row_b`, and the mean of a message's word scores is the SVM's score of the
//!   mean of its vectors. `f = bias + (mean of its words' scores, or 0
//!   without words) − text_mean + Σ weightₕ·(xₕ − meanₕ)`, where a word
//!   missing from the table scores the mean of its character n-grams'
//!   buckets, as fastText builds the vector of a word it never saw. The
//!   standardization's mean terms stay beside the weights rather than folded
//!   into the bias, so that the explanation can say what departs from the
//!   usual; the score is the same.
//!
//! Its probability of being spam, either kind: Platt's `p = 1 / (1 + exp(A·f + B))`.
//!
//! The file (little-endian), `$XDG_DATA_HOME/sioul/spam/table.bin`:
//!
//! | bytes | what |
//! |---|---|
//! | 8 | magic `SIOULSPM` |
//! | 4 × 12 | format version (2), tokenizer version, features version, embedding dimension, minn, maxn, bucket count B, word count W, header feature count H (0: the classifier's kind; `features::N`: the centroid's), metadata length L, two zeros (so that the hashes start on 8 bytes) |
//! | 4 × 4 | bias (the classifier's: the end of the line's scalar), text_mean (the classifier's: 0), Platt A, Platt B (f32) |
//! | 8 × W | the words' hashes, ascending (u64) |
//! | 4 × W | their scores (f32): the classifier's, the sum of each word's rows' scalars |
//! | 4 × B | the buckets' scores (f32) |
//! | 4 × H, 4 × H | the header features' weights, then their means (f32), the centroid's alone |
//! | L | metadata, JSON (`Meta`) |
//! | 8 | FNV-1a 64 of every byte before it |

use super::features;
use super::tokenize::TOKENIZER;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// The file's first bytes.
pub const MAGIC: [u8; 8] = *b"SIOULSPM";
/// The file format's version: 2 since the classifier's kind (`Kind`); a
/// Sioul that reads 1 refuses it, rather than reading its sums as scores.
pub const VERSION: u32 = 2;
/// Bytes before the words: the magic, twelve counts, four numbers.
const HEADER: usize = 8 + 12 * 4 + 4 * 4;

/// What the settings say of the training that made the table.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Meta {
    /// When it was trained, Unix seconds.
    #[serde(default)]
    pub trained_at: i64,
    /// Messages it learned from, by label.
    #[serde(default)]
    pub ham: u64,
    #[serde(default)]
    pub spam: u64,
    /// Messages it was tested on (the newest), by label.
    #[serde(default)]
    pub test_ham: u64,
    #[serde(default)]
    pub test_spam: u64,
    /// Aggregate measures on the test messages, by name ("auc", "ham_called_spam"…).
    #[serde(default)]
    pub metrics: BTreeMap<String, f64>,
    /// The device it was trained on, as the sharing's devices name it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub device: String,
}

/// How a table scores a message (see the module's documentation).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// fastText's classifier: the mean of every input row's scalar, the header facts as words.
    Rows,
    /// The language model's centroid, weighed by an SVM with the header features.
    Centroid,
}

/// The spam filter, folded (see the module's documentation).
#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    /// The tokenizer's version it was trained with (`tokenize::TOKENIZER`).
    pub tokenizer: u32,
    /// The header features' version (`features::FEATURES`).
    pub features: u32,
    /// The language model's dimension, for the record.
    pub dim: u32,
    /// fastText's character n-grams: shortest, longest, and how many buckets they hash into.
    pub minn: u32,
    pub maxn: u32,
    pub bucket: u32,
    /// Each vocabulary word's score, under its hash (`word_hash`), ascending:
    /// scoring looks words up by halves. A table read from its file is so; one
    /// made in memory is once `sort`ed (writing sorts it too).
    pub words: Vec<(u64, f32)>,
    /// Each n-gram bucket's score, `bucket` of them.
    pub buckets: Vec<f32>,
    /// Each header feature's weight, over its deviation, and its mean
    /// (`features::NAMES`' order); none in the classifier's table, which
    /// reads the header facts as words: that is how its kind is told (`kind`).
    pub weights: Vec<f32>,
    pub means: Vec<f32>,
    /// The SVM's intercept; the classifier's: the end of the line's scalar.
    pub bias: f32,
    /// The score of the mean message's text (`u · μ`); the classifier's: 0.
    pub text_mean: f32,
    /// Platt's A and B.
    pub platt_a: f32,
    pub platt_b: f32,
    pub meta: Meta,
}

/// A message's score, and what made it.
#[derive(Debug, Clone, PartialEq)]
pub struct Score {
    /// The SVM's decision value: above zero, spam.
    pub f: f32,
    /// The probability it is spam.
    pub p: f32,
    /// Each word's share of `f` (all its occurrences), the most toward spam first.
    pub words: Vec<(String, f32)>,
    /// Each header feature's share of `f`, by its place in `features::NAMES`, the most toward spam first.
    pub signs: Vec<(usize, f32)>,
}

/// A word's key in the table: FNV-1a, 64 bits, over its UTF-8 bytes.
pub fn word_hash(word: &str) -> u64 {
    fnv64(word.as_bytes())
}

fn fnv64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// fastText's hash: 32-bit FNV-1a, each byte sign-extended first, as the C++
/// casts it (`uint32_t(int8_t(c))`): bytes above 127 differ from plain FNV-1a.
fn fasttext_hash(bytes: &[u8]) -> u32 {
    let mut h: u32 = 2_166_136_261;
    for b in bytes {
        h ^= *b as i8 as i32 as u32;
        h = h.wrapping_mul(16_777_619);
    }
    h
}

/// The buckets of a word's character n-grams, as fastText 0.8.0 computes them
/// (<https://github.com/messense/fasttext-rs>, tag v0.8.0: `src/dictionary.rs`,
/// `for_each_ngram` and `get_subwords_for_string`; `src/utils.rs`, `hash`):
/// the word between `<` and `>`; from each character (not each byte: UTF-8
/// continuation bytes start nothing), every n-gram of `minn` to `maxn`
/// characters (a single character never at either end); each hashed
/// (`fasttext_hash`) modulo `bucket`; in that order, repeats kept. fastText's
/// input row for one is `nwords + bucket`, and the vector of a word it never
/// saw is the mean of those rows (`get_word_vector_into`); of a word it knows,
/// the mean of its own row and those.
pub fn ngram_buckets(word: &str, minn: u32, maxn: u32, bucket: u32) -> Vec<u32> {
    let mut out = Vec::new();
    if maxn == 0 || bucket == 0 {
        return out;
    }
    let marked = format!("<{word}>");
    let bytes = marked.as_bytes();
    let continuation = |b: u8| (b & 0xc0) == 0x80;
    for i in 0..bytes.len() {
        if continuation(bytes[i]) {
            continue;
        }
        let mut j = i;
        let mut n = 1;
        while j < bytes.len() && n <= maxn {
            j += 1;
            while j < bytes.len() && continuation(bytes[j]) {
                j += 1;
            }
            if n >= minn && !(n == 1 && (i == 0 || j == bytes.len())) {
                out.push(fasttext_hash(&bytes[i..j]) % bucket);
            }
            n += 1;
        }
    }
    out
}

/// Words sorted by their hash; two words of one hash (never seen yet: 64 bits)
/// share their mean score.
fn sorted(mut words: Vec<(u64, f32)>) -> Vec<(u64, f32)> {
    words.sort_by_key(|(h, _)| *h);
    let mut merged: Vec<(u64, f32, u32)> = Vec::with_capacity(words.len());
    for (h, s) in words {
        match merged.last_mut() {
            Some((last, sum, n)) if *last == h => {
                *sum += s;
                *n += 1;
            }
            _ => merged.push((h, s, 1)),
        }
    }
    merged.into_iter().map(|(h, sum, n)| (h, sum / n as f32)).collect()
}

impl Table {
    /// Its kind: no header weights, the classifier's, which reads the header
    /// facts as words; else the centroid's.
    pub fn kind(&self) -> Kind {
        if self.weights.is_empty() && self.means.is_empty() { Kind::Rows } else { Kind::Centroid }
    }

    /// Sorts the words by their hash, as scoring needs them (`words`).
    pub fn sort(&mut self) {
        self.words = sorted(std::mem::take(&mut self.words));
    }

    /// A word's rows in the classifier's table: the sum of their scalars and
    /// how many they are. Known: its own row and its n-grams'; else its
    /// n-grams' alone (none without n-grams: nothing).
    fn rows_of(&self, word: &str) -> (f64, usize) {
        let buckets = ngram_buckets(word, self.minn, self.maxn, self.bucket);
        match self.words.binary_search_by_key(&word_hash(word), |(h, _)| *h) {
            Ok(at) => (f64::from(self.words[at].1), 1 + buckets.len()),
            Err(_) => (buckets.iter().map(|b| f64::from(self.buckets.get(*b as usize).copied().unwrap_or(0.0))).sum(), buckets.len()),
        }
    }

    /// A word's score in the centroid's table: its own, else the mean of its n-grams' buckets; 0 when it has none.
    pub fn word_score(&self, word: &str) -> f32 {
        if let Ok(at) = self.words.binary_search_by_key(&word_hash(word), |(h, _)| *h) {
            return self.words[at].1;
        }
        let buckets = ngram_buckets(word, self.minn, self.maxn, self.bucket);
        if buckets.is_empty() {
            return 0.0;
        }
        let sum: f64 = buckets.iter().map(|b| f64::from(self.buckets.get(*b as usize).copied().unwrap_or(0.0))).sum();
        (sum / buckets.len() as f64) as f32
    }

    /// A message's score from its words (`tokenize::tokens`) and its header
    /// features (`features::features`), with what made it. The table's words
    /// must be sorted (`sort`): one read from its file is. A header feature
    /// that is missing (`f32::NAN`) weighs nothing: the classifier reads no
    /// word for it; the centroid stands it at the training's mean, which is
    /// over the messages that had it.
    pub fn score(&self, words: &[String], features: &[f32]) -> Score {
        match self.kind() {
            Kind::Rows => self.score_rows(words, features),
            Kind::Centroid => self.score_centroid(words, features),
        }
    }

    /// The classifier's score: the mean of the rows' scalars of its words,
    /// its header facts' words and the end of the line. Each word's share is
    /// its rows' sum over every row; a header fact's, its word's.
    fn score_rows(&self, words: &[String], features: &[f32]) -> Score {
        // Each word once, with how often it comes, its rows' sum and their count.
        let mut seen: BTreeMap<&str, (usize, f64, usize)> = BTreeMap::new();
        for word in words {
            seen.entry(word.as_str()).or_insert_with(|| {
                let (sum, rows) = self.rows_of(word);
                (0, sum, rows)
            }).0 += 1;
        }
        let facts: Vec<(usize, f64, usize)> = features::header_words(features).iter().map(|(word, h)| {
            let (sum, rows) = self.rows_of(word);
            (*h, sum, rows)
        }).collect();
        let rows = 1 + seen.values().map(|(count, _, rows)| count * rows).sum::<usize>() + facts.iter().map(|(_, _, rows)| rows).sum::<usize>();
        let rows = rows as f64;
        let sum = f64::from(self.bias) + seen.values().map(|(count, sum, _)| *count as f64 * sum).sum::<f64>() + facts.iter().map(|(_, sum, _)| sum).sum::<f64>();
        let f = (sum / rows) as f32;
        let mut shares: Vec<(String, f32)> = seen.iter().map(|(word, (count, sum, _))| (word.to_string(), (*count as f64 * sum / rows) as f32)).collect();
        let mut signs: Vec<(usize, f32)> = (0..features.len().min(features::N)).map(|h| (h, 0.0)).collect();
        for (h, sum, _) in &facts {
            if let Some(sign) = signs.get_mut(*h) {
                sign.1 += (sum / rows) as f32;
            }
        }
        let by_share = |a: &f32, b: &f32| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal);
        shares.sort_by(|a, b| by_share(&a.1, &b.1).then_with(|| a.0.cmp(&b.0)));
        signs.sort_by(|a, b| by_share(&a.1, &b.1).then_with(|| a.0.cmp(&b.0)));
        Score { f, p: platt(f, self.platt_a, self.platt_b), words: shares, signs }
    }

    /// The centroid's score (see the module's documentation).
    fn score_centroid(&self, words: &[String], features: &[f32]) -> Score {
        // Each word once, with how often it comes and its score.
        let mut seen: BTreeMap<&str, (usize, f32)> = BTreeMap::new();
        for word in words {
            seen.entry(word.as_str()).or_insert_with(|| (0, self.word_score(word))).0 += 1;
        }
        let total = words.len();
        let mut text = -f64::from(self.text_mean);
        let mut shares: Vec<(String, f32)> = Vec::with_capacity(seen.len());
        if total > 0 {
            let sum: f64 = seen.values().map(|(count, s)| *count as f64 * f64::from(*s)).sum();
            text += sum / total as f64;
            shares = seen.iter().map(|(word, (count, s))| (word.to_string(), (*count as f64 * (f64::from(*s) - f64::from(self.text_mean)) / total as f64) as f32)).collect();
        }
        let mut signs: Vec<(usize, f32)> = self.weights.iter().zip(&self.means).zip(features).enumerate().map(|(h, ((w, m), x))| (h, if x.is_nan() { 0.0 } else { w * (x - m) })).collect();
        let f = (f64::from(self.bias) + text + signs.iter().map(|(_, c)| f64::from(*c)).sum::<f64>()) as f32;
        let by_share = |a: &f32, b: &f32| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal);
        shares.sort_by(|a, b| by_share(&a.1, &b.1).then_with(|| a.0.cmp(&b.0)));
        signs.sort_by(|a, b| by_share(&a.1, &b.1).then_with(|| a.0.cmp(&b.0)));
        Score { f, p: platt(f, self.platt_a, self.platt_b), words: shares, signs }
    }

    /// The table as its file holds it, words sorted (`sort`), checksummed.
    pub fn to_bytes(&self) -> Vec<u8> {
        let merged = sorted(self.words.clone());
        let meta = serde_json::to_vec(&self.meta).unwrap_or_default();
        let header = self.weights.len().min(self.means.len());
        let mut out = Vec::with_capacity(HEADER + merged.len() * 12 + self.buckets.len() * 4 + header * 8 + meta.len() + 8);
        out.extend_from_slice(&MAGIC);
        let counts = [VERSION, self.tokenizer, self.features, self.dim, self.minn, self.maxn, self.buckets.len() as u32, merged.len() as u32, header as u32, meta.len() as u32, 0, 0];
        for n in counts {
            out.extend_from_slice(&n.to_le_bytes());
        }
        for x in [self.bias, self.text_mean, self.platt_a, self.platt_b] {
            out.extend_from_slice(&x.to_le_bytes());
        }
        for (h, _) in &merged {
            out.extend_from_slice(&h.to_le_bytes());
        }
        for (_, s) in &merged {
            out.extend_from_slice(&s.to_le_bytes());
        }
        for x in self.buckets.iter().chain(&self.weights[..header]).chain(&self.means[..header]) {
            out.extend_from_slice(&x.to_le_bytes());
        }
        out.extend_from_slice(&meta);
        let sum = fnv64(&out);
        out.extend_from_slice(&sum.to_le_bytes());
        out
    }

    /// Reads a table, refusing one that is damaged, made by another tokenizer
    /// or with other header features than this Sioul's: its scores would mean nothing.
    pub fn from_bytes(bytes: &[u8]) -> Result<Table, String> {
        if bytes.len() < HEADER + 8 || bytes[..8] != MAGIC {
            return Err("not a spam table".into());
        }
        let (body, sum) = bytes.split_at(bytes.len() - 8);
        if fnv64(body) != u64::from_le_bytes(sum.try_into().map_err(|_| "no checksum")?) {
            return Err("damaged: its checksum does not match".into());
        }
        let u32_at = |i: usize| u32::from_le_bytes([body[8 + 4 * i], body[9 + 4 * i], body[10 + 4 * i], body[11 + 4 * i]]);
        let f32_at = |at: usize| f32::from_le_bytes([body[at], body[at + 1], body[at + 2], body[at + 3]]);
        let [version, tokenizer, features, dim, minn, maxn, bucket, words, header, meta, _, _] = std::array::from_fn(u32_at);
        if version != VERSION {
            return Err(format!("format {version}, this Sioul reads {VERSION}"));
        }
        if tokenizer != TOKENIZER {
            return Err(format!("made with tokenizer {tokenizer}, this Sioul has {TOKENIZER}: train again"));
        }
        // No header weights: the classifier's kind; all of them: the centroid's.
        if features != features::FEATURES || !(header == 0 || header as usize == features::N) {
            return Err(format!("made with header features {features} ({header}), this Sioul has {} ({}): train again", features::FEATURES, features::N));
        }
        if minn > maxn || maxn > 64 {
            return Err(format!("n-grams of {minn} to {maxn} characters"));
        }
        let (bucket, words, header, meta) = (bucket as usize, words as usize, header as usize, meta as usize);
        let expected = (|| HEADER.checked_add(words.checked_mul(12)?)?.checked_add(bucket.checked_mul(4)?)?.checked_add(header * 8)?.checked_add(meta))();
        if expected != Some(body.len()) {
            return Err("damaged: its parts do not add up to its length".into());
        }
        let floats = |start: usize, n: usize| (0..n).map(|i| f32_at(start + 4 * i)).collect::<Vec<f32>>();
        let hashes_at = HEADER;
        let scores_at = hashes_at + 8 * words;
        let buckets_at = scores_at + 4 * words;
        let weights_at = buckets_at + 4 * bucket;
        let means_at = weights_at + 4 * header;
        let meta_at = means_at + 4 * header;
        let hashes: Vec<u64> = (0..words).map(|i| u64::from_le_bytes(body[hashes_at + 8 * i..hashes_at + 8 * i + 8].try_into().unwrap_or_default())).collect();
        if hashes.windows(2).any(|w| w[0] >= w[1]) {
            return Err("damaged: its words are out of order".into());
        }
        let scores = floats(scores_at, words);
        let table = Table {
            tokenizer,
            features,
            dim,
            minn,
            maxn,
            bucket: bucket as u32,
            words: hashes.into_iter().zip(scores).collect(),
            buckets: floats(buckets_at, bucket),
            weights: floats(weights_at, header),
            means: floats(means_at, header),
            bias: f32_at(HEADER - 16),
            text_mean: f32_at(HEADER - 12),
            platt_a: f32_at(HEADER - 8),
            platt_b: f32_at(HEADER - 4),
            meta: serde_json::from_slice(&body[meta_at..]).map_err(|e| format!("damaged: its metadata: {e}"))?,
        };
        let finite = table.words.iter().map(|(_, s)| s).chain(&table.buckets).chain(&table.weights).chain(&table.means).chain([&table.bias, &table.text_mean, &table.platt_a, &table.platt_b]).all(|x| x.is_finite());
        if !finite {
            return Err("damaged: a score is not a number".into());
        }
        Ok(table)
    }

    /// Reads the table's file.
    pub fn read(path: &Path) -> Result<Table, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Table::from_bytes(&bytes).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Writes the table where it goes, whole or not at all (written beside,
    /// flushed, then moved); the one it replaces, when this Sioul can use it,
    /// is kept as `table.prev.bin` (`keep_previous`).
    pub fn write(&self, path: &Path) -> Result<(), String> {
        use std::io::Write;
        let fail = |e: std::io::Error| format!("{}: {e}", path.display());
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(fail)?;
        }
        let temporary = path.with_extension("bin.new");
        let mut file = std::fs::File::create(&temporary).map_err(fail)?;
        file.write_all(&self.to_bytes()).and_then(|()| file.sync_all()).map_err(fail)?;
        drop(file);
        // `cached` sees a new table by its size and time. A clock that ticks
        // coarsely (Windows: every 15.6 ms) can give the new file the time of the
        // one it replaces, and a new table often has the same size: the new one is
        // made a millisecond younger than the one it replaces, at least.
        let modified = |at: &Path| std::fs::metadata(at).ok().and_then(|m| m.modified().ok());
        if let Some(old) = modified(path)
            && modified(&temporary).is_none_or(|new| new <= old)
        {
            let _ = std::fs::File::options().write(true).open(&temporary).and_then(|f| f.set_modified(old + std::time::Duration::from_millis(1)));
        }
        keep_previous(path);
        std::fs::rename(&temporary, path).map_err(fail)
    }

    /// The table at `path`, read once per process and again only when its
    /// file changes (its size and time); none when there is none. One this
    /// Sioul refuses (`read` says why: made by a newer Sioul, damaged) leaves
    /// the one before it in use (`table.prev.bin`), when there is one it
    /// reads. The phone's background process reads it once, then keeps it.
    pub fn cached(path: &Path) -> Option<Arc<Table>> {
        type Kept = Option<(PathBuf, Option<(u64, u128)>, Option<Arc<Table>>)>;
        static KEPT: Mutex<Kept> = Mutex::new(None);
        let seal = std::fs::metadata(path).ok().map(|m| (m.len(), m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos())));
        let mut kept = KEPT.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((at, known, table)) = kept.as_ref()
            && at == path
            && *known == seal
        {
            return table.clone();
        }
        let table = seal.and_then(|_| Table::read(path).or_else(|_| Table::read(&previous(path))).ok()).map(Arc::new);
        *kept = Some((path.to_path_buf(), seal, table.clone()));
        table
    }
}

/// Platt's probability for a decision value.
pub fn platt(f: f32, a: f32, b: f32) -> f32 {
    let z = f64::from(a) * f64::from(f) + f64::from(b);
    // The same value, written to never overflow.
    let p = if z >= 0.0 { (-z).exp() / (1.0 + (-z).exp()) } else { 1.0 / (1.0 + z.exp()) };
    p as f32
}

/// Where the table replaced by `Table::write`, or by another device's
/// through the sharing, is kept.
pub fn previous(path: &Path) -> PathBuf {
    path.with_file_name("table.prev.bin")
}

/// Before another table takes `path`'s place (a training here, another
/// device's table through the sharing): the one there, when this Sioul can
/// use it, copied beside as `table.prev.bin` (written beside, then moved).
/// A table this Sioul refuses then leaves the last good one in use
/// (`Table::cached`), and never takes its place there. Best effort: a copy
/// that fails leaves the one before as it was.
pub fn keep_previous(path: &Path) {
    if Table::read(path).is_err() {
        return;
    }
    let kept = previous(path);
    let temporary = kept.with_extension("bin.new");
    if std::fs::copy(path, &temporary).and_then(|_| std::fs::rename(&temporary, &kept)).is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spam::features::N;

    /// A table small enough to score by hand: two words, eight buckets, the header's first feature weighed.
    fn small() -> Table {
        let mut weights = vec![0.0; N];
        let mut means = vec![0.0; N];
        weights[0] = 2.0;
        means[0] = 0.25;
        let mut table = Table {
            tokenizer: TOKENIZER,
            features: features::FEATURES,
            dim: 4,
            minn: 3,
            maxn: 4,
            bucket: 8,
            words: vec![(word_hash("gratuit"), 3.0), (word_hash("reunion"), -1.0)],
            buckets: vec![0.5, -0.5, 1.0, 0.0, 2.0, -1.0, 0.25, 0.75],
            weights,
            means,
            bias: -0.5,
            text_mean: 0.5,
            platt_a: -2.0,
            platt_b: 0.0,
            meta: Meta { trained_at: 1_791_000_000, ham: 900, spam: 100, test_ham: 225, test_spam: 25, metrics: BTreeMap::from([("auc".to_string(), 0.99)]), device: "desk".into() },
        };
        table.sort();
        table
    }

    #[test]
    fn ngrams_as_fasttext_cuts_them() {
        // "<ab>": from each character, n-grams of three and four characters.
        let wanted: Vec<u32> = ["<ab", "<ab>", "ab>"].iter().map(|g| fasttext_hash(g.as_bytes()) % 1_000_003).collect();
        assert_eq!(ngram_buckets("ab", 3, 4, 1_000_003), wanted);
        // A character is one, however many bytes: "é" is two, "<é>" one n-gram of three.
        assert_eq!(ngram_buckets("é", 3, 6, 1_000_003), vec![fasttext_hash("<é>".as_bytes()) % 1_000_003]);
        // The hash sign-extends bytes above 127, as fastText's C++ does: not plain FNV-1a.
        let plain = |bytes: &[u8]| bytes.iter().fold(2_166_136_261u32, |h, b| (h ^ u32::from(*b)).wrapping_mul(16_777_619));
        assert_eq!(fasttext_hash(b"<ab"), plain(b"<ab"));
        assert_ne!(fasttext_hash("é".as_bytes()), plain("é".as_bytes()));
        // fastText's own value for "a" (FNV-1a of 0x61), and none without buckets.
        assert_eq!(fasttext_hash(b"a"), 0xe40c_292c);
        assert!(ngram_buckets("abc", 3, 6, 0).is_empty());
        assert_eq!(ngram_buckets("abcdefgh", 3, 6, 97).len(), 8 + 7 + 6 + 5);
    }

    #[test]
    fn a_score_computed_by_hand() {
        let table = small();
        let unknown = ngram_buckets("offre", 3, 4, 8);
        let offre: f32 = unknown.iter().map(|b| table.buckets[*b as usize]).sum::<f32>() / unknown.len() as f32;
        assert!((table.word_score("offre") - offre).abs() < 1e-6);
        let words: Vec<String> = ["gratuit", "gratuit", "reunion", "offre"].iter().map(|w| w.to_string()).collect();
        let mut x = [0.0; N];
        x[0] = 1.0;
        let score = table.score(&words, &x);
        // f = bias + mean(3, 3, -1, offre) − text_mean + 2·(1 − 0.25)
        let f = -0.5 + (3.0 + 3.0 - 1.0 + offre) / 4.0 - 0.5 + 2.0 * 0.75;
        assert!((score.f - f).abs() < 1e-5, "{} {f}", score.f);
        assert!((score.p - 1.0 / (1.0 + (-2.0 * f).exp())).abs() < 1e-6);
        // "gratuit" pushes most: twice (3 − 0.5) / 4; the header feature 2·0.75.
        assert_eq!(score.words[0].0, "gratuit");
        assert!((score.words[0].1 - 1.25).abs() < 1e-6);
        assert_eq!(score.signs[0], (0, 1.5));
        // The shares add up to the score, the bias aside.
        let shares: f32 = score.words.iter().map(|(_, s)| s).sum::<f32>() + score.signs.iter().map(|(_, s)| s).sum::<f32>();
        assert!((score.f - (shares - 0.5)).abs() < 1e-5);
        // No word at all: the text is the mean message's, and the bias and the header decide.
        let empty = table.score(&[], &x);
        assert!((empty.f - (-0.5 - 0.5 + 1.5)).abs() < 1e-6 && empty.words.is_empty());
        // The header feature missing: it stands at its mean and weighs nothing.
        let mut missing = x;
        missing[0] = f32::NAN;
        let unknown = table.score(&words, &missing);
        assert!((unknown.f - (f - 1.5)).abs() < 1e-5 && unknown.f.is_finite(), "{}", unknown.f);
        assert_eq!(unknown.signs[0].1, 0.0);
    }

    /// The classifier's table, small enough to score by hand: two words, a
    /// header fact's word, eight buckets, the end of the line's scalar.
    fn small_rows() -> Table {
        let mut table = Table {
            weights: Vec::new(),
            means: Vec::new(),
            words: vec![(word_hash("gratuit"), 6.0), (word_hash("reunion"), -4.0), (word_hash("H:DKIM_SIGNED_BY_SENDER"), -20.0)],
            bias: 0.5,
            text_mean: 0.0,
            ..small()
        };
        table.sort();
        table
    }

    #[test]
    fn the_classifier_scored_by_hand() {
        let table = small_rows();
        assert_eq!((table.kind(), small().kind()), (Kind::Rows, Kind::Centroid));
        let rows = |word: &str| ngram_buckets(word, 3, 4, 8);
        // A word it knows: its sum, over its own row and its n-grams'; one it does not: its n-grams' alone.
        let offre: f64 = rows("offre").iter().map(|b| f64::from(table.buckets[*b as usize])).sum();
        let words: Vec<String> = ["gratuit", "gratuit", "reunion", "offre"].iter().map(|w| w.to_string()).collect();
        let none = [f32::NAN; N];
        let score = table.score(&words, &none);
        let count = 1 + 2 * (1 + rows("gratuit").len()) + (1 + rows("reunion").len()) + rows("offre").len();
        let f = (0.5 + 2.0 * 6.0 - 4.0 + offre) / count as f64;
        assert!((f64::from(score.f) - f).abs() < 1e-6, "{} {f}", score.f);
        assert!((score.p - 1.0 / (1.0 + (-2.0 * f as f32).exp())).abs() < 1e-6);
        // "gratuit" pushes most, twice its sum over every row; nothing known of the headers: no fact weighs.
        assert_eq!(score.words[0].0, "gratuit");
        assert!((f64::from(score.words[0].1) - 12.0 / count as f64).abs() < 1e-6);
        assert!(score.signs.iter().all(|(_, s)| *s == 0.0) && score.signs.len() == N);
        // A fact that holds: its word read after the text's, its rows counted; its share its feature's.
        let mut signed = [f32::NAN; N];
        let by_sender = features::index("dkim_signed_by_sender").unwrap();
        signed[by_sender] = 1.0;
        let vouched = table.score(&words, &signed);
        let fact = 1 + rows("H:DKIM_SIGNED_BY_SENDER").len();
        let g = (0.5 + 2.0 * 6.0 - 4.0 + offre - 20.0) / (count + fact) as f64;
        assert!((f64::from(vouched.f) - g).abs() < 1e-6 && vouched.f < score.f, "{} {g}", vouched.f);
        let last = vouched.signs.last().unwrap();
        assert_eq!(last.0, by_sender);
        assert!((f64::from(last.1) - (-20.0 / (count + fact) as f64)).abs() < 1e-6);
        // The shares add up to the score, the end of the line's aside.
        let shares: f64 = vouched.words.iter().map(|(_, s)| f64::from(*s)).sum::<f64>() + vouched.signs.iter().map(|(_, s)| f64::from(*s)).sum::<f64>();
        assert!((f64::from(vouched.f) - (shares + 0.5 / (count + fact) as f64)).abs() < 1e-5);
        // No word at all and nothing known: the end of the line alone.
        assert!((table.score(&[], &none).f - 0.5).abs() < 1e-6);
        // Written and read back: the same, the same kind.
        let back = Table::from_bytes(&table.to_bytes()).unwrap();
        assert_eq!((back.kind(), &back), (Kind::Rows, &table));
    }

    #[test]
    fn written_and_read_back() {
        let table = small();
        let back = Table::from_bytes(&table.to_bytes()).unwrap();
        let sorted = table.clone();
        assert_eq!(back, sorted);
        // Made in memory in any order: sorted for scoring, two words of one hash merged.
        let mut loose = Table { words: vec![(9, 1.0), (3, 2.0), (9, 3.0)], ..table.clone() };
        loose.sort();
        assert_eq!(loose.words, vec![(3, 2.0), (9, 2.0)]);
        let dir = std::env::temp_dir().join(format!("sioul-spam-table-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("spam").join("table.bin");
        table.write(&path).unwrap();
        assert_eq!(Table::read(&path).unwrap(), sorted);
        assert_eq!(Table::cached(&path).as_deref(), Some(&sorted));
        // A new one replaces it; the one before is kept. The one in place dated
        // ahead, as a coarse clock can leave it (Windows): still seen as new.
        std::fs::File::options().write(true).open(&path).unwrap().set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(60)).unwrap();
        let newer = Table { bias: 1.0, ..table.clone() };
        newer.write(&path).unwrap();
        assert_eq!(Table::read(&previous(&path)).unwrap().bias, -0.5);
        assert_eq!(Table::cached(&path).map(|t| t.bias), Some(1.0));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A table this Sioul refuses (another device's, made by a newer Sioul)
    /// leaves the last good one in use, and never takes its place beside.
    #[test]
    fn a_refused_table_leaves_the_last_good_one_in_use() {
        let dir = std::env::temp_dir().join(format!("sioul-spam-previous-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("table.bin");
        // Each file its own time: the cache knows a file by its size and time.
        let dated = |path: &Path, second: u64| std::fs::File::options().write(true).open(path).unwrap().set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(second)).unwrap();
        let good = small();
        good.write(&path).unwrap();
        dated(&path, 1_791_000_000);
        assert!(!previous(&path).exists(), "nothing before the first");
        // A newer Sioul's table comes: the good one kept beside, and still in use.
        let newer = |tokenizer: u32| Table { tokenizer, bias: 9.0, ..small() }.to_bytes();
        keep_previous(&path);
        std::fs::write(&path, newer(TOKENIZER + 1)).unwrap();
        dated(&path, 1_791_000_100);
        assert!(Table::read(&path).is_err());
        assert_eq!(Table::cached(&path).map(|t| t.bias), Some(-0.5));
        // Another one this Sioul refuses: the good one stays the one before.
        keep_previous(&path);
        std::fs::write(&path, newer(TOKENIZER + 2)).unwrap();
        dated(&path, 1_791_000_200);
        assert_eq!(Table::read(&previous(&path)).unwrap(), good);
        assert_eq!(Table::cached(&path).map(|t| t.bias), Some(-0.5));
        // One it reads again: in use, the good one before it.
        let fresh = Table { bias: 0.25, ..small() };
        fresh.write(&path).unwrap();
        dated(&path, 1_791_000_300);
        assert_eq!(Table::cached(&path).map(|t| t.bias), Some(0.25));
        assert_eq!(Table::read(&previous(&path)).unwrap(), good);
        // None at all: nothing, whatever is beside it.
        std::fs::remove_file(&path).unwrap();
        assert!(Table::cached(&path).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_damaged_or_foreign_table_is_refused() {
        let bytes = small().to_bytes();
        // One byte changed anywhere: the checksum says so.
        for at in [10, HEADER + 3, bytes.len() / 2, bytes.len() - 9] {
            let mut damaged = bytes.clone();
            damaged[at] ^= 0x40;
            assert!(Table::from_bytes(&damaged).is_err(), "byte {at}");
        }
        assert!(Table::from_bytes(&bytes[..bytes.len() - 1]).is_err());
        assert!(Table::from_bytes(b"not a table at all").is_err());
        // Another tokenizer, other header features: refused, even sound.
        let other = Table { tokenizer: TOKENIZER + 1, ..small() };
        assert!(Table::from_bytes(&other.to_bytes()).unwrap_err().contains("tokenizer"));
        let fewer = Table { weights: vec![0.0; N - 1], means: vec![0.0; N - 1], ..small() };
        assert!(Table::from_bytes(&fewer.to_bytes()).unwrap_err().contains("header features"));
        // The version before (features 3: the provider's checks alone, none of Sioul's): refused.
        let older = Table { features: features::FEATURES - 1, ..small() };
        assert!(Table::from_bytes(&older.to_bytes()).unwrap_err().contains(&format!("header features {}", features::FEATURES - 1)));
        let broken = Table { buckets: vec![f32::NAN; 8], ..small() };
        assert!(Table::from_bytes(&broken.to_bytes()).is_err());
        // Format 1 (made before the classifier's kind): refused, its checksum made good again.
        let mut first = bytes.clone();
        first[8..12].copy_from_slice(&1u32.to_le_bytes());
        let body = first.len() - 8;
        let sum = fnv64(&first[..body]);
        first[body..].copy_from_slice(&sum.to_le_bytes());
        assert!(Table::from_bytes(&first).unwrap_err().contains("format 1"));
    }
}
