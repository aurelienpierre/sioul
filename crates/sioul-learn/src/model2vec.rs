// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! A static embedding model of Model2Vec's kind, read from its folder
//! (`model.safetensors` and `tokenizer.json`), for the one alternative
//! `sioul topics eval --model2vec <folder>` measures beside fastText: a
//! nearest centroid over the mean of a message's token vectors
//! (docs/research/mail-topics.md, §4.1). Nothing is downloaded here: the
//! folder is one the person fetched, such as Minish Lab's
//! potion-multilingual-128M (101 languages, 256 dimensions, 512 MB).
//!
//! A message's vector is the plain mean of its tokens' rows, unknown tokens
//! left out and 512 tokens at most, as Model2Vec encodes; never normalized
//! here, so that a centroid's score `v · c` is the mean of each token's
//! `e_t · c`, one number per token and topic: it folds as fastText's table does.
//!
//! The tokenizer is read from `tokenizer.json`: a Unigram model (its pieces
//! and their log probabilities), the text split on spaces, each word marked
//! with `▁` as SentencePiece's Metaspace does, each word cut by Viterbi into
//! the most likely pieces. It is close to Hugging Face's, not the same: its
//! normalizer (a precompiled NFKC table) is left out, which mail written in
//! composed characters (NFC, the usual) does not notice. Measured as such.

use std::collections::HashMap;
use std::io::{BufReader, Read};
use std::path::Path;

/// Tokens a message is read with at most, as Model2Vec's `encode` cuts them.
pub const MOST_TOKENS: usize = 512;
/// Characters of a word cut into pieces at most; the rest of a longer word is left out.
const LONGEST_WORD: usize = 100;
/// What an unknown character costs below the least likely piece (SentencePiece's penalty).
const UNKNOWN_PENALTY: f64 = 10.0;

/// The model: its pieces, its rows, and the pieces of the words already cut.
pub struct StaticModel {
    pieces: HashMap<String, (u32, f64)>,
    longest: usize,
    unknown: Option<u32>,
    unknown_score: f64,
    lowercase: bool,
    /// Whether the first word of a text is marked too (Metaspace's "always" and "first").
    mark_first: bool,
    pub dim: usize,
    pub rows: usize,
    embeddings: Vec<f32>,
    cut: HashMap<String, Vec<u32>>,
    /// The tokenizer file's size in bytes: what a phone would carry beside the table.
    pub tokenizer_bytes: u64,
}

#[derive(serde::Deserialize)]
struct TokenizerFile {
    model: ModelPart,
    #[serde(default)]
    normalizer: Option<serde_json::Value>,
    #[serde(default)]
    pre_tokenizer: Option<serde_json::Value>,
}

#[derive(serde::Deserialize)]
struct ModelPart {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    vocab: Vec<(String, f64)>,
    #[serde(default)]
    unk_id: Option<u32>,
}

/// Whether a part of the tokenizer's description holds an object of this type
/// (with `field` true when given), at any depth.
fn holds(value: &serde_json::Value, kind: &str, field: Option<&str>) -> bool {
    match value {
        serde_json::Value::Object(map) => {
            let here = map.get("type").and_then(|t| t.as_str()) == Some(kind) && field.is_none_or(|f| map.get(f).and_then(serde_json::Value::as_bool) == Some(true));
            here || map.values().any(|v| holds(v, kind, field))
        }
        serde_json::Value::Array(items) => items.iter().any(|v| holds(v, kind, field)),
        _ => false,
    }
}

/// Metaspace's prepend scheme, wherever it is: "always" when unsaid.
fn prepend_scheme(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Object(map) => {
            if map.get("type").and_then(|t| t.as_str()) == Some("Metaspace") {
                if let Some(scheme) = map.get("prepend_scheme").and_then(|s| s.as_str()) {
                    return Some(scheme.to_string());
                }
                if map.get("add_prefix_space").and_then(serde_json::Value::as_bool) == Some(false) {
                    return Some("never".into());
                }
                return Some("always".into());
            }
            map.values().find_map(prepend_scheme)
        }
        serde_json::Value::Array(items) => items.iter().find_map(prepend_scheme),
        _ => None,
    }
}

/// A bfloat16 number's value.
fn bf16(bits: u16) -> f32 {
    f32::from_bits(u32::from(bits) << 16)
}

impl StaticModel {
    /// Reads a Model2Vec folder: `tokenizer.json` (a Unigram model) and
    /// `model.safetensors` (its tensor `embeddings`, else its only
    /// two-dimensional one, in F32, F16 or BF16).
    pub fn load(folder: &Path) -> Result<StaticModel, String> {
        let tokenizer_path = folder.join("tokenizer.json");
        let file = std::fs::File::open(&tokenizer_path).map_err(|e| format!("{}: {e}", tokenizer_path.display()))?;
        let tokenizer_bytes = file.metadata().map_or(0, |m| m.len());
        let parsed: TokenizerFile = serde_json::from_reader(BufReader::new(file)).map_err(|e| format!("{}: {e}", tokenizer_path.display()))?;
        if parsed.model.kind != "Unigram" {
            return Err(format!("{}: a {} model; only Unigram ones are read", tokenizer_path.display(), parsed.model.kind));
        }
        let lowercase = parsed.normalizer.as_ref().is_some_and(|n| holds(n, "Lowercase", None) || holds(n, "BertNormalizer", Some("lowercase")));
        let mark_first = parsed.pre_tokenizer.as_ref().and_then(prepend_scheme).is_none_or(|s| s != "never");
        let unknown_score = parsed.model.vocab.iter().map(|(_, s)| *s).fold(f64::INFINITY, f64::min) - UNKNOWN_PENALTY;
        let mut pieces = HashMap::with_capacity(parsed.model.vocab.len());
        let mut longest = 1;
        for (id, (piece, score)) in parsed.model.vocab.into_iter().enumerate() {
            longest = longest.max(piece.chars().count());
            pieces.entry(piece).or_insert((id as u32, score));
        }
        let (dim, rows, embeddings) = read_embeddings(&folder.join("model.safetensors"))?;
        Ok(StaticModel { pieces, longest: longest.min(32), unknown: parsed.model.unk_id, unknown_score: if unknown_score.is_finite() { unknown_score } else { -UNKNOWN_PENALTY }, lowercase, mark_first, dim, rows, embeddings, cut: HashMap::new(), tokenizer_bytes })
    }

    /// Pieces in the tokenizer.
    pub fn vocabulary(&self) -> usize {
        self.pieces.len()
    }

    /// A token's row.
    pub fn row(&self, token: u32) -> &[f32] {
        let at = token as usize * self.dim;
        &self.embeddings[at..at + self.dim]
    }

    /// A text's tokens: unknown ones and those without a row left out, `MOST_TOKENS` at most.
    pub fn tokens(&mut self, text: &str) -> Vec<u32> {
        let mut out = Vec::new();
        for (i, word) in text.split_whitespace().enumerate() {
            if out.len() >= MOST_TOKENS {
                break;
            }
            let word: String = if self.lowercase { word.to_lowercase() } else { word.to_string() };
            let marked = if i > 0 || self.mark_first { format!("\u{2581}{word}") } else { word };
            let marked: String = marked.chars().take(LONGEST_WORD).collect();
            if !self.cut.contains_key(&marked) {
                if self.cut.len() > 2_000_000 {
                    self.cut.clear();
                }
                let pieces = self.viterbi(&marked);
                self.cut.insert(marked.clone(), pieces);
            }
            let unknown = self.unknown;
            let rows = self.rows as u32;
            out.extend(self.cut[&marked].iter().copied().filter(|&t| Some(t) != unknown && t < rows));
        }
        out.truncate(MOST_TOKENS);
        out
    }

    /// The most likely pieces of a word (Viterbi over its characters); an
    /// uncovered character is the unknown piece.
    fn viterbi(&self, word: &str) -> Vec<u32> {
        let bounds: Vec<usize> = word.char_indices().map(|(i, _)| i).chain(std::iter::once(word.len())).collect();
        let n = bounds.len() - 1;
        let mut best: Vec<(f64, usize, Option<u32>)> = vec![(f64::NEG_INFINITY, 0, None); n + 1];
        best[0].0 = 0.0;
        for i in 0..n {
            if best[i].0 == f64::NEG_INFINITY {
                continue;
            }
            let mut single = false;
            for len in 1..=self.longest.min(n - i) {
                if let Some(&(id, score)) = self.pieces.get(&word[bounds[i]..bounds[i + len]]) {
                    single |= len == 1;
                    let total = best[i].0 + score;
                    if total > best[i + len].0 {
                        best[i + len] = (total, i, Some(id));
                    }
                }
            }
            if !single {
                let total = best[i].0 + self.unknown_score;
                if total > best[i + 1].0 {
                    best[i + 1] = (total, i, self.unknown);
                }
            }
        }
        let mut pieces = Vec::new();
        let mut at = n;
        while at > 0 {
            let (_, from, id) = best[at];
            if let Some(id) = id {
                pieces.push(id);
            }
            at = from;
        }
        pieces.reverse();
        pieces
    }

    /// The mean of some tokens' rows; none without tokens.
    pub fn mean(&self, tokens: &[u32]) -> Option<Vec<f32>> {
        if tokens.is_empty() {
            return None;
        }
        let mut sum = vec![0f64; self.dim];
        for &t in tokens {
            for (s, x) in sum.iter_mut().zip(self.row(t)) {
                *s += f64::from(*x);
            }
        }
        Some(sum.into_iter().map(|s| (s / tokens.len() as f64) as f32).collect())
    }
}

/// A safetensors file's embeddings: their dimension, row count and values.
fn read_embeddings(path: &Path) -> Result<(usize, usize, Vec<f32>), String> {
    let fail = |e: &dyn std::fmt::Display| format!("{}: {e}", path.display());
    let mut file = BufReader::new(std::fs::File::open(path).map_err(|e| fail(&e))?);
    let mut length = [0u8; 8];
    file.read_exact(&mut length).map_err(|e| fail(&e))?;
    let length = u64::from_le_bytes(length);
    if length > 100 << 20 {
        return Err(fail(&"not a safetensors file"));
    }
    let mut header = vec![0u8; length as usize];
    file.read_exact(&mut header).map_err(|e| fail(&e))?;
    let header: serde_json::Map<String, serde_json::Value> = serde_json::from_slice(&header).map_err(|e| fail(&e))?;
    let matrices: Vec<(&String, &serde_json::Value)> = header.iter().filter(|(name, t)| *name != "__metadata__" && t["shape"].as_array().is_some_and(|s| s.len() == 2)).collect();
    let (_, tensor) = matrices.iter().find(|(name, _)| *name == "embeddings").or_else(|| (matrices.len() == 1).then(|| &matrices[0])).ok_or_else(|| fail(&"no tensor named embeddings"))?;
    let shape: Vec<usize> = tensor["shape"].as_array().into_iter().flatten().filter_map(serde_json::Value::as_u64).map(|n| n as usize).collect();
    let offsets: Vec<u64> = tensor["data_offsets"].as_array().into_iter().flatten().filter_map(serde_json::Value::as_u64).collect();
    let (&[rows, dim], &[start, end]) = (shape.as_slice(), offsets.as_slice()) else { return Err(fail(&"its embeddings have no shape")) };
    let width = match tensor["dtype"].as_str() {
        Some("F32") => 4,
        Some("F16") | Some("BF16") => 2,
        other => return Err(fail(&format!("embeddings in {}", other.unwrap_or("?")))),
    };
    if (end - start) as usize != rows * dim * width {
        return Err(fail(&"its embeddings' size does not match their shape"));
    }
    // Skip to the tensor, then read it in pieces.
    std::io::copy(&mut (&mut file).take(start), &mut std::io::sink()).map_err(|e| fail(&e))?;
    let mut values = Vec::with_capacity(rows * dim);
    let mut chunk = vec![0u8; 1 << 20];
    let mut left = (end - start) as usize;
    let dtype = tensor["dtype"].as_str().unwrap_or_default().to_string();
    while left > 0 {
        let n = left.min(chunk.len());
        file.read_exact(&mut chunk[..n]).map_err(|e| fail(&e))?;
        match dtype.as_str() {
            "F32" => values.extend(chunk[..n].chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))),
            "F16" => values.extend(chunk[..n].chunks_exact(2).map(|b| sioul_core::topics::table::from_f16(u16::from_le_bytes([b[0], b[1]])))),
            _ => values.extend(chunk[..n].chunks_exact(2).map(|b| bf16(u16::from_le_bytes([b[0], b[1]])))),
        }
        left -= n;
    }
    Ok((dim, rows, values))
}

/// A stand-in model in a folder, for tests: these pieces (with their log
/// probabilities) and their rows, F32.
#[doc(hidden)]
pub fn write_stand_in(folder: &Path, pieces: &[(&str, f64)], rows: &[Vec<f32>], unk_id: u32) -> std::io::Result<()> {
    std::fs::create_dir_all(folder)?;
    let tokenizer = serde_json::json!({
        "version": "1.0",
        "normalizer": null,
        "pre_tokenizer": { "type": "Metaspace", "replacement": "\u{2581}", "prepend_scheme": "always", "split": true },
        "model": { "type": "Unigram", "unk_id": unk_id, "vocab": pieces.iter().map(|(p, s)| serde_json::json!([p, s])).collect::<Vec<_>>() },
    });
    std::fs::write(folder.join("tokenizer.json"), serde_json::to_vec(&tokenizer)?)?;
    let dim = rows.first().map_or(0, Vec::len);
    let data: Vec<u8> = rows.iter().flatten().flat_map(|x| x.to_le_bytes()).collect();
    let header = serde_json::to_vec(&serde_json::json!({ "embeddings": { "dtype": "F32", "shape": [rows.len(), dim], "data_offsets": [0, data.len()] } }))?;
    let mut bytes = (header.len() as u64).to_le_bytes().to_vec();
    bytes.extend(header);
    bytes.extend(data);
    std::fs::write(folder.join("model.safetensors"), bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stand-in model read back; words cut into their most likely pieces,
    /// unknown characters left out; the mean of their rows.
    #[test]
    fn a_stand_in_model_cuts_and_averages() {
        let root = std::env::temp_dir().join(format!("sioul-model2vec-{}", std::process::id()));
        let pieces = [("<unk>", 0.0), ("\u{2581}", -3.0), ("\u{2581}chor", -2.0), ("ale", -2.0), ("\u{2581}chorale", -3.0), ("\u{2581}c", -4.0), ("h", -4.0), ("o", -4.0), ("r", -4.0), ("a", -4.0), ("l", -4.0), ("e", -4.0), ("\u{2581}tax", -1.0)];
        let rows: Vec<Vec<f32>> = (0..pieces.len()).map(|i| vec![i as f32, 1.0]).collect();
        write_stand_in(&root, &pieces, &rows, 0).unwrap();
        let mut model = StaticModel::load(&root).unwrap();
        assert_eq!((model.dim, model.rows, model.vocabulary()), (2, pieces.len(), pieces.len()));
        // "▁chorale" whole (-3) beats "▁chor" + "ale" (-4).
        assert_eq!(model.tokens("chorale"), vec![4]);
        // "x" is no piece: unknown, left out; "tax" whole.
        assert_eq!(model.tokens("chorx tax"), vec![2, 12]);
        let mean = model.mean(&[4, 12]).unwrap();
        assert_eq!(mean, vec![8.0, 1.0]);
        assert!(model.mean(&[]).is_none());
        let long = "tax ".repeat(MOST_TOKENS + 10);
        assert_eq!(model.tokens(&long).len(), MOST_TOKENS);
        let _ = std::fs::remove_dir_all(root);
    }
}
