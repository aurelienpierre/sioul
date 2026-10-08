// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Texts on your computers, read and sent through your phone, without Sioul
//! being the phone's SMS app (docs/texts.md; research: docs/research/sms.md,
//! phase b). The phone's SMS app stays the record: a fault of Sioul's can
//! delay a text on the computer, never lose one on the phone.
//!
//! **Reading, the whole history.** With `READ_SMS`, the phone reads its
//! texts from Android's provider (the inbox and the sent box, multimedia
//! messages and group threads included: Android shows no other app the
//! drafts, the failed or queued texts, nor a multimedia message still to
//! download) and writes a line per message in its own log,
//! `texts/log/<phone>.jsonl` (`Text`), every column the provider gives kept
//! for a faithful restore. The first import goes a batch at a time. The log
//! only grows: a text deleted on the phone gets a line of its own saying so
//! (`Text::deleted`), and every device keeps what it holds.
//!
//! **Media.** Each part of a multimedia message the phone has downloaded
//! (pictures, sound, video, cards, words) under a size you set travels as a
//! sealed blob of the sharing (`blobs/`), never inside a line (`Part`).
//!
//! **An archive.** The texts and their media are kept in Sioul's data
//! folder on every device that shares the part, never trimmed by age, with
//! every column a later restore of a new phone would need; that restore is
//! not built.
//!
//! **Sending.** A computer writes a `Request` in its own file
//! (`texts/send/<device>.jsonl`): a random key, the phone it must leave from,
//! the number, the words, the SIM, when it was written. The phone decides at
//! its step (`verdict`), claims the key in its private `Ledger` (written and
//! flushed before Android is asked), hands the text to Android, and says what
//! happened in its own `texts/outcome/<phone>.jsonl` (`Outcome`).
//!
//! **Never twice** (`verdict`): a key already in the ledger is never sent
//! again, whatever the sharing reads again; a request written before the
//! ledger's mark is refused (a ledger lost with the phone's data cannot make
//! old requests new); a request names its phone, and another phone never
//! sends it; one older than `EXPIRY_MS` on the phone's clock, corrected by
//! how far the writer's clock may run ahead, is not sent; Sioul never
//! retries, Android does (three times, "reject duplicates"); after a crash
//! between the claim and the result, Sioul looks for the row Android filed
//! for it, and says "Your phone may not have sent it" when there is none
//! (`doubt_after`), never sending again by itself. **Send again** makes a new key.
//!
//! **Sealed at rest**: every line of these stores, and every file of their
//! media, is sealed on the device that writes it (`Sealer`, with a key made
//! from the sharing key, in each device's keyring), lines padded to steps of
//! `PAD_STEP` bytes inside: a computer's copy is unreadable without the key.

use crate::calls;
use crate::i18n::Translator;
use crate::phones::{self, Region};
use jiff::Zoned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// The sharing's part that carries them.
pub const PART: &str = "texts";
/// Their folder, in the data folder (an archive, never a cache).
pub const FOLDER: &str = "texts";
/// Each phone's texts, a line each, the whole history.
pub const LOG: &str = "log";
/// Each computer's texts to send.
pub const SEND: &str = "send";
/// Each phone's word on each text it was asked to send.
pub const OUTCOME: &str = "outcome";
/// The shared stores, in that order.
pub const STORES: [&str; 3] = [LOG, SEND, OUTCOME];
/// The media of multimedia messages, each sealed at rest by its content's hash; never shared as a store (they travel as blobs).
pub const MEDIA: &str = "media";
/// The phone's private files, in the state folder, never shared: the ledger,
/// its mark, the reader's marks and the import's progress, Java's results.
pub const LEDGER: &str = "ledger.jsonl";
pub const MARK: &str = "ledger-mark.json";
pub const READER: &str = "reader.json";
pub const RESULTS: &str = "results.jsonl";
/// Requests and outcomes are taken out by their device after this many days…
pub const KEPT_DAYS: i64 = 31;
/// …and by any device this many days later (45 in all). Texts never are.
pub const LATE_DAYS: i64 = 14;
/// A request older than this on the phone's clock is not sent.
pub const EXPIRY_MS: i64 = 15 * 60_000;
/// A request dated further ahead of the phone's clock than this is refused: the clocks disagree.
pub const AHEAD_MS: i64 = 2 * 60_000;
/// The ledger keeps a key this long, past any request's life.
pub const LEDGER_KEPT_MS: i64 = 30 * DAY_MS;
/// A claim with no word from Android this long after is looked for in the provider.
pub const LOOK_AFTER_MS: i64 = 2 * 60_000;
/// A text written on a computer: at most this many characters.
pub const WRITE_MAX: usize = 1600;
/// Each line's plain length is a multiple of this before it is sealed.
pub const PAD_STEP: usize = 256;
/// The first import, and each step after: at most this many texts at a time.
pub const BATCH: usize = 500;
/// …and this many bytes of media.
pub const MEDIA_BATCH: u64 = 24 << 20;
/// A part over this many bytes stays on the phone, said, unless you set another size (MB, `texts.toml`).
pub const MEDIA_CAP_MB: u64 = 10;
const DAY_MS: i64 = 86_400_000;

/// The folder of the texts: the phones' logs, the requests and outcomes, the media; in the data folder.
pub fn folder() -> PathBuf {
    crate::config::data_dir().join(FOLDER)
}

/// The phone's private texts folder, in the state folder: never shared.
pub fn private_folder() -> PathBuf {
    crate::config::state_dir().join(FOLDER)
}

/// What seals a line at rest and opens it: the caller's, made from the sharing key.
pub trait Sealer {
    /// A line, sealed: no newline in it.
    fn seal(&self, plain: &str) -> String;
    /// A sealed line opened; none when it does not open (another key, broken).
    fn open(&self, sealed: &str) -> Option<String>;
}

// ---------------------------------------------------------------- the lines

/// One part of a multimedia message: its own columns, and where its content is.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Part {
    /// Its place in the message (`seq`; -1 for the SMIL that lays it out).
    #[serde(default)]
    pub seq: i64,
    /// Its content type: "image/jpeg", "text/plain", "application/smil"…
    #[serde(default)]
    pub ct: String,
    /// Its name, as the message gives one (`name`, else `cl`, else `fn`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// Its size in bytes, as Android says it; 0 unknown.
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub size: u64,
    /// Its words, for a text part and the SMIL.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
    /// Its content's hash (SHA-256, hex): its blob in the sharing, its sealed file here.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub hash: String,
    /// "here" (its content travels), "text" (its words are in the line), "too-big" (over your size: on your phone), "not-downloaded" (Android has not fetched it).
    #[serde(default)]
    pub state: String,
    /// Every other column of the part as the provider gives it (`chset`, `cd`, `fn`, `cid`, `cl`, `ctt_s`, `ctt_t`…).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, Value>,
}

fn is_zero_u64(value: &u64) -> bool {
    *value == 0
}

/// A text as the phone's provider holds it, as it travels.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Text {
    /// "sms-<_id>", "mms-<_id>": the provider's own row on that phone.
    pub id: String,
    /// When (ms): received, or sent.
    pub at: i64,
    /// The provider's conversation, on that phone.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub thread: String,
    /// "in" or "out".
    #[serde(rename = "box")]
    pub direction: String,
    /// The conversation's other people's numbers, as Sioul keys them (`phones::key`); several: a group.
    #[serde(default)]
    pub with: Vec<String>,
    /// Who wrote it, in a group: their number's key.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub from: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub body: String,
    /// The SIM it came by or left from (Android's `sub_id`); -1 unknown.
    #[serde(default = "no_sim")]
    pub sub: i32,
    /// It carries a picture.
    #[serde(default, skip_serializing_if = "is_false")]
    pub picture: bool,
    /// A multimedia message's parts, in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<Part>,
    /// Every column of the provider's row, as it gives them (a faithful restore needs them): docs/texts.md, "What is kept".
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, Value>,
    /// A multimedia message's addresses, each row as the provider gives it (`address`, `type`, `charset`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub addrs: Vec<BTreeMap<String, Value>>,
    /// Sioul sent it, for this key (the provider's `creator`, matched).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub key: String,
    /// When the phone found it deleted (ms): a line of its own, beside the text's; the archive keeps the text.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub deleted: i64,
    /// The phone that wrote it (its file's name). Never written.
    #[serde(skip)]
    pub device: String,
}

fn no_sim() -> i32 {
    -1
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn is_zero(value: &i64) -> bool {
    *value == 0
}

impl Text {
    /// Its name in its phone's log: the deletion's line is another.
    pub fn line_id(&self) -> String {
        if self.deleted > 0 { format!("{}#deleted", self.id) } else { self.id.clone() }
    }

    /// The media it holds here, in bytes.
    pub fn media_bytes(&self) -> u64 {
        self.parts.iter().filter(|p| p.state == "here").map(|p| p.size).sum()
    }
}

/// A text to send, as a computer writes it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    /// Random, 32 hex digits: the text's one name everywhere.
    pub key: String,
    /// The phone it must leave from (its name in the sharing).
    pub phone: String,
    /// The number, as written; the phone keys it.
    pub to: String,
    pub body: String,
    /// The SIM (`sub_id`); -1: the conversation's last, else the phone's default.
    #[serde(default = "no_sim")]
    pub sub: i32,
    /// When it was written (ms, the writer's clock).
    pub written: i64,
    /// How far the writer's clock may run ahead of the phone's (ms): the
    /// least of the gaps the writer saw between the phone's entry's time and
    /// its own clock when that entry came, over the last day; an upper bound,
    /// transit included, so a request looks older, never younger. 0 unknown.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub ahead: i64,
    /// The key of the request it sends again, when it does.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub again: String,
    /// The device that wrote it (its file's name). Never written.
    #[serde(skip)]
    pub device: String,
}

/// What became of a request, as its phone says it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outcome {
    pub key: String,
    /// "taken" (claimed, about to be handed to Android), "handed", "sent",
    /// "delivered", "failed", "expired", "refused", "clocks", "doubt".
    pub state: String,
    /// When (ms, the phone's clock).
    pub at: i64,
    /// Why, for "failed" and "refused": "radio-off", "no-service", "limit",
    /// "short-number", "several", "empty", "too-long", "before-mark"…
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reason: String,
    /// The provider's row Android filed it as ("sms-123"), when known.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub row: String,
    /// The phone that wrote it. Never written.
    #[serde(skip)]
    pub device: String,
}

// ---------------------------------------------------------------- the lines, padded and sealed

/// A value as written: its JSON padded with spaces inside a `pad` field to a
/// multiple of `PAD_STEP` bytes, then sealed. No line says how long its words are.
pub fn written<T: Serialize>(value: &T, sealer: &dyn Sealer) -> String {
    let text = serde_json::to_string(value).unwrap_or_default();
    sealer.seal(&padded(&text))
}

/// A JSON object padded inside: `{…,"pad":"   "}`, its length a multiple of `PAD_STEP`.
pub fn padded(json: &str) -> String {
    let Some(body) = json.strip_suffix('}') else { return json.to_string() };
    let joint = if body.ends_with('{') { "" } else { "," };
    let bare = format!("{body}{joint}\"pad\":\"\"}}");
    let spaces = (PAD_STEP - bare.len() % PAD_STEP) % PAD_STEP;
    format!("{body}{joint}\"pad\":\"{}\"}}", " ".repeat(spaces))
}

/// A device's name as its files name it (a line's `device`).
pub fn file_name(device: &str) -> String {
    calls::file_safe(device)
}

/// "14:02": a moment (ms) as a time of day, in `now`'s zone.
pub fn time_of_day(at_ms: i64, now: &Zoned) -> String {
    calls::local(at_ms, now).strftime("%H:%M").to_string()
}

/// A device's file in one of the stores under `root`: `<store>/<device>.jsonl`.
pub fn own_file(root: &Path, store: &str, device: &str) -> PathBuf {
    calls::own_file(root, store, device)
}

/// Every line of a file that opens and reads, in order; the rest passed over.
pub fn read_file<T: for<'de> Deserialize<'de>>(path: &Path, sealer: &dyn Sealer) -> Vec<T> {
    std::fs::read_to_string(path).unwrap_or_default().lines().filter(|l| !l.trim().is_empty()).filter_map(|l| sealer.open(l.trim())).filter_map(|plain| serde_json::from_str(&plain).ok()).collect()
}

/// Every device's lines of a store under `root`, each with its device.
fn read_store<T: for<'de> Deserialize<'de>>(root: &Path, store: &str, sealer: &dyn Sealer, mut set: impl FnMut(&mut T, &str)) -> Vec<T> {
    let mut all = Vec::new();
    for (device, path) in calls::device_files(&root.join(store)) {
        for mut value in read_file::<T>(&path, sealer) {
            set(&mut value, &device);
            all.push(value);
        }
    }
    all
}

/// Every phone's texts, oldest first, each once (by phone and row), a
/// deletion's line folded into its text (`deleted`); the whole history.
pub fn read_logs(root: &Path, sealer: &dyn Sealer) -> Vec<Text> {
    let lines: Vec<Text> = read_store(root, LOG, sealer, |t: &mut Text, d| t.device = d.to_string());
    let deletions: BTreeMap<(String, String), i64> = lines.iter().filter(|t| t.deleted > 0).map(|t| ((t.device.clone(), t.id.clone()), t.deleted)).collect();
    let mut seen = BTreeSet::new();
    let mut texts: Vec<Text> = lines
        .into_iter()
        .filter(|t| t.deleted == 0 && t.at > 0 && seen.insert((t.device.clone(), t.id.clone())))
        .map(|t| {
            let deleted = deletions.get(&(t.device.clone(), t.id.clone())).copied().unwrap_or(0);
            Text { deleted, ..t }
        })
        .collect();
    texts.sort_by(|a, b| (a.at, &a.device, &a.id).cmp(&(b.at, &b.device, &b.id)));
    texts
}

pub fn read_requests(root: &Path, sealer: &dyn Sealer) -> Vec<Request> {
    read_store(root, SEND, sealer, |r: &mut Request, d| r.device = d.to_string())
}

pub fn read_outcomes(root: &Path, sealer: &dyn Sealer) -> Vec<Outcome> {
    let mut all: Vec<Outcome> = read_store(root, OUTCOME, sealer, |o: &mut Outcome, d| o.device = d.to_string());
    all.sort_by_key(|o| o.at);
    all
}

/// Lines added at the end of a device's own file, under its lock, each
/// written whole (padded, sealed), those whose `id` the file holds already
/// left out (`id_of`; none: every line written). Returns how many were written.
pub fn append<T: Serialize + for<'de> Deserialize<'de>>(own: &Path, values: &[T], sealer: &dyn Sealer, id_of: Option<&dyn Fn(&T) -> String>) -> Result<usize, String> {
    if values.is_empty() {
        return Ok(0);
    }
    crate::filelock::with_lock(own, || {
        let mut known: BTreeSet<String> = match id_of {
            Some(id) => read_file::<T>(own, sealer).iter().map(id).collect(),
            None => BTreeSet::new(),
        };
        let mut text = String::new();
        let mut count = 0;
        for value in values {
            if let Some(id) = id_of
                && !known.insert(id(value))
            {
                continue;
            }
            text.push_str(&written(value, sealer));
            text.push('\n');
            count += 1;
        }
        if count > 0 {
            calls::append_private(own, &text)?;
        }
        Ok(count)
    })
}

/// The time a line says (`at`, else `written`), opened; none when it does not open.
fn time_of(sealer: &dyn Sealer, line: &str) -> Option<i64> {
    let value: Value = serde_json::from_str(&sealer.open(line.trim())?).ok()?;
    value["at"].as_i64().or_else(|| value["written"].as_i64())
}

/// This device's own requests and outcomes past `KEPT_DAYS` taken out; its
/// texts never (an archive). A line that does not open stays. Returns how many went.
pub fn trim_own(root: &Path, device: &str, sealer: &dyn Sealer, now_ms: i64) -> Result<usize, String> {
    let mut gone = 0;
    for store in [SEND, OUTCOME] {
        gone += calls::rewrite_lines(&own_file(root, store, device), |line| time_of(sealer, line).is_some_and(|at| now_ms - at > KEPT_DAYS * DAY_MS))?;
    }
    Ok(gone)
}

/// Another device's requests and outcomes `LATE_DAYS` past their time taken out here: texts never.
pub fn trim_others(root: &Path, here: &str, sealer: &dyn Sealer, now_ms: i64) -> Result<usize, String> {
    let mut gone = 0;
    for store in [SEND, OUTCOME] {
        for (device, path) in calls::device_files(&root.join(store)) {
            if device != calls::file_safe(here) {
                gone += calls::rewrite_lines(&path, |line| time_of(sealer, line).is_some_and(|at| now_ms - at > (KEPT_DAYS + LATE_DAYS) * DAY_MS))?;
            }
        }
    }
    Ok(gone)
}

/// What a device keeps: how many texts, and how many bytes of media here (`media`: those whose sealed file is on this device).
pub fn kept(texts: &[Text], media_here: &dyn Fn(&str) -> bool) -> (usize, u64) {
    let mut hashes = BTreeSet::new();
    let bytes = texts.iter().flat_map(|t| t.parts.iter()).filter(|p| p.state == "here" && !p.hash.is_empty() && media_here(&p.hash) && hashes.insert(p.hash.clone())).map(|p| p.size).sum();
    (texts.len(), bytes)
}

// ---------------------------------------------------------------- the phone's ledger

/// One line of the phone's private ledger: a key, when, what was done.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claim {
    pub key: String,
    /// When (ms, the phone's clock).
    pub at: i64,
    /// "claimed" (written before Android is asked), "handed" (Android took
    /// it), "done" (its result came, or it was looked for), and the decisions
    /// that end a request: "expired", "refused", "clocks", "before-mark".
    pub state: String,
    /// What was sent, to find its row after a crash: the number's key and the words' hash.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub to: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub hash: String,
}

/// The ledger's mark: requests written before it are never sent (a ledger lost cannot make them new).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mark {
    pub since: i64,
}

/// The phone's private ledger, never shared: every key it decided, and its mark.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ledger {
    pub mark: Mark,
    pub claims: Vec<Claim>,
}

impl Ledger {
    /// The ledger in `dir` (the phone's private texts folder): its mark made
    /// now when there is none (a first ledger, or one lost: requests written
    /// before now are refused).
    pub fn open(dir: &Path, now_ms: i64) -> Result<Ledger, String> {
        let mark_path = dir.join(MARK);
        let mark = match std::fs::read_to_string(&mark_path).ok().and_then(|t| serde_json::from_str::<Mark>(&t).ok()) {
            Some(mark) => mark,
            None => {
                let mark = Mark { since: now_ms };
                write_flushed(&mark_path, &serde_json::to_string(&mark).map_err(|e| e.to_string())?, false)?;
                mark
            }
        };
        let claims = std::fs::read_to_string(dir.join(LEDGER)).unwrap_or_default().lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
        Ok(Ledger { mark, claims })
    }

    /// Whether a key was decided already, whatever was decided.
    pub fn has(&self, key: &str) -> bool {
        self.claims.iter().any(|c| c.key == key)
    }

    /// A key's last state.
    pub fn state(&self, key: &str) -> Option<&str> {
        self.claims.iter().rev().find(|c| c.key == key).map(|c| c.state.as_str())
    }
}

/// A line added to the ledger and flushed to the disk before this returns:
/// the claim exists before Android is asked to send.
pub fn record(dir: &Path, claim: &Claim) -> Result<(), String> {
    let line = serde_json::to_string(claim).map_err(|e| e.to_string())? + "\n";
    write_flushed(&dir.join(LEDGER), &line, true)
}

/// Written (or appended) and flushed to the disk, yours alone.
fn write_flushed(path: &Path, text: &str, append: bool) -> Result<(), String> {
    use std::io::Write;
    let fail = |e: std::io::Error| format!("{}: {e}", path.display());
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(fail)?;
    }
    let mut options = std::fs::OpenOptions::new();
    options.create(true).write(true);
    if append {
        options.append(true);
    } else {
        options.truncate(true);
    }
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options.open(path).map_err(fail)?;
    file.write_all(text.as_bytes()).map_err(fail)?;
    file.sync_all().map_err(fail)
}

/// The ledger's old keys taken out, `LEDGER_KEPT_MS` on (any request expired
/// long before; the mark refuses those written before it anyway).
pub fn trim_ledger(dir: &Path, now_ms: i64) -> Result<usize, String> {
    calls::rewrite_lines(&dir.join(LEDGER), |line| serde_json::from_str::<Claim>(line).is_ok_and(|c| now_ms - c.at > LEDGER_KEPT_MS))
}

/// A text's words, hashed as the ledger keeps them (FNV-1a, 16 hex digits).
pub fn words_hash(body: &str) -> String {
    crate::appnotes::talk_key("texts", body)
}

// ---------------------------------------------------------------- never twice

/// What the phone does with a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Claim it, then hand it to Android.
    Send,
    /// Another phone's: nothing done, nothing said.
    NotMine,
    /// Decided already (sent, or not): nothing done again.
    Decided,
    /// Written before the ledger's mark: refused, said.
    BeforeMark,
    /// Older than `EXPIRY_MS`: not sent, said.
    Expired,
    /// Dated ahead of the phone's clock: the clocks disagree; not sent, said.
    Clocks,
    /// Not a text Sioul sends from a computer: said why.
    Refused(&'static str),
}

/// Why a number or words are refused from a computer, if they are: several
/// recipients (an MMS group), a short number (premium risk; Android would ask
/// on the phone), no words, too many. None: it may go.
pub fn refusal(to: &str, body: &str, region: Option<&Region>) -> Option<&'static str> {
    if body.trim().is_empty() {
        return Some("empty");
    }
    if body.chars().count() > WRITE_MAX {
        return Some("too-long");
    }
    if to.contains([',', ';']) || to.chars().filter(char::is_ascii_digit).count() > 15 {
        return Some("several");
    }
    let key = phones::key(to, region);
    if !phones::is_whole(&key) || key.trim_start_matches('+').len() < 8 {
        return Some("short-number");
    }
    None
}

/// The never-twice decision, step for step: another phone's; decided already
/// (the ledger holds the key, whatever the sharing read again); refused
/// (`refusal`); written before the ledger's mark; dated ahead of the phone's
/// clock; older than `EXPIRY_MS`; else sent. The time written is corrected by
/// how far the writer's clock may run ahead (`Request::ahead`, measured by
/// the writer as an upper bound: a request looks older, never younger).
pub fn verdict(request: &Request, here: &str, now_ms: i64, ledger: &Ledger, region: Option<&Region>) -> Verdict {
    if request.phone != here || request.key.is_empty() {
        return Verdict::NotMine;
    }
    if ledger.has(&request.key) {
        return Verdict::Decided;
    }
    if let Some(why) = refusal(&request.to, &request.body, region) {
        return Verdict::Refused(why);
    }
    let written = request.written - request.ahead;
    if written < ledger.mark.since {
        return Verdict::BeforeMark;
    }
    if written > now_ms + AHEAD_MS {
        return Verdict::Clocks;
    }
    if now_ms - written > EXPIRY_MS {
        return Verdict::Expired;
    }
    Verdict::Send
}

/// After a crash between the claim and Android's word: what to say of a
/// claim `LOOK_AFTER_MS` old with no result, given whether the provider holds
/// a sent row Sioul filed for it (`found`: its row). Found: sent. Not found:
/// "doubt", never sent again.
pub fn doubt_after(claim: &Claim, found: Option<String>, now_ms: i64) -> Option<Outcome> {
    if now_ms - claim.at < LOOK_AFTER_MS {
        return None;
    }
    Some(match found {
        Some(row) => Outcome { key: claim.key.clone(), state: "sent".into(), at: now_ms, reason: String::new(), row, device: String::new() },
        None => Outcome { key: claim.key.clone(), state: "doubt".into(), at: now_ms, reason: String::new(), row: String::new(), device: String::new() },
    })
}

/// A random key, 32 hex digits, from the system's randomness (`/dev/urandom`); the clock's when none.
pub fn new_key() -> String {
    use std::io::Read;
    let mut bytes = [0u8; 16];
    let read = std::fs::File::open("/dev/urandom").and_then(|mut f| f.read_exact(&mut bytes));
    if read.is_err() {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
        bytes = (now ^ u128::from(std::process::id()).rotate_left(64)).to_le_bytes();
    }
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// ---------------------------------------------------------------- parts

/// GSM 03.38's basic alphabet: one septet each.
const GSM_BASIC: &str = "@£$¥èéùìòÇ\nØø\rÅåΔ_ΦΓΛΩΠΨΣΘΞÆæßÉ !\"#¤%&'()*+,-./0123456789:;<=>?¡ABCDEFGHIJKLMNOPQRSTUVWXYZÄÖÑÜ§¿abcdefghijklmnopqrstuvwxyzäöñüà";
/// Its extension table: two septets each.
const GSM_EXTENDED: &str = "^{}\\[~]|€\u{c}";

/// How many parts a text takes, and in which alphabet: GSM's 7 bits (160
/// characters in one part, 153 in each of several) or UCS-2 (70, 67) as soon
/// as one character is outside it. Android chooses alike (`divideMessage`).
pub fn parts(body: &str) -> (usize, bool) {
    let gsm = body.chars().all(|c| GSM_BASIC.contains(c) || GSM_EXTENDED.contains(c));
    if gsm {
        let septets: usize = body.chars().map(|c| if GSM_EXTENDED.contains(c) { 2 } else { 1 }).sum();
        (if septets <= 160 { usize::from(septets > 0) } else { septets.div_ceil(153) }, true)
    } else {
        let units: usize = body.chars().map(char::len_utf16).sum();
        (if units <= 70 { 1 } else { units.div_ceil(67) }, false)
    }
}

// ---------------------------------------------------------------- the view

/// A request's state, as the computer says it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Said {
    /// "waiting", "sending", "sent", "delivered", "failed", "expired", "refused", "clocks", "doubt".
    pub state: String,
    /// "Sent at 14:03."
    pub text: String,
    /// Send again offered (a new key).
    pub again: bool,
}

/// What a request's outcomes say: its latest meaningful one (delivered over sent over handed…).
pub fn state_of<'a>(key: &str, outcomes: &'a [Outcome]) -> Option<&'a Outcome> {
    let rank = |s: &str| match s {
        "taken" => 1,
        "handed" => 2,
        "doubt" => 3,
        "sent" => 4,
        "delivered" => 5,
        "failed" | "expired" | "refused" | "clocks" => 6,
        _ => 0,
    };
    outcomes.iter().filter(|o| o.key == key).max_by_key(|o| (rank(&o.state), o.at))
}

/// A request's state in words, never red, never "late": waiting for the phone
/// (and when it last shared), sending, sent at, delivered at (only when the
/// carrier said so), not sent and why, expired, may not have been sent.
pub fn said(outcome: Option<&Outcome>, phone_shared: Option<i64>, tr: &Translator, now: &Zoned) -> Said {
    let time = |ms: i64| calls::local(ms, now).strftime("%H:%M").to_string();
    let with = |id: &str, key: &str, value: String| {
        let mut args = crate::i18n::args();
        args.set(key, value);
        tr.text(id, Some(&args))
    };
    let (state, text, again) = match outcome {
        None => match phone_shared {
            Some(at) => ("waiting", with("texts-waiting-since", "time", time(at)), false),
            None => ("waiting", tr.text("texts-waiting", None), false),
        },
        Some(o) => match o.state.as_str() {
            "taken" | "handed" => ("sending", tr.text("texts-sending", None), false),
            "sent" => ("sent", with("texts-sent-at", "time", time(o.at)), false),
            "delivered" => ("delivered", with("texts-delivered-at", "time", time(o.at)), false),
            "failed" => ("failed", with("texts-failed", "why", tr.text(&format!("texts-why-{}", known_reason(&o.reason)), None)), true),
            "expired" => ("expired", tr.text("texts-expired", None), true),
            "refused" => ("refused", tr.text(&format!("texts-refused-{}", known_refusal(&o.reason)), None), o.reason == "before-mark"),
            "clocks" => ("clocks", tr.text("texts-clocks", None), true),
            _ => ("doubt", tr.text("texts-doubt", None), true),
        },
    };
    Said { state: state.into(), text, again }
}

/// A failure's reason as the words know it.
fn known_reason(reason: &str) -> &'static str {
    match reason {
        "radio-off" => "radio-off",
        "no-service" => "no-service",
        "limit" => "limit",
        "no-sim" => "no-sim",
        _ => "other",
    }
}

/// A refusal's reason as the words know it.
fn known_refusal(reason: &str) -> &'static str {
    match reason {
        "short-number" => "short-number",
        "several" => "several",
        "empty" => "empty",
        "too-long" => "too-long",
        "before-mark" => "before-mark",
        _ => "other",
    }
}

/// A conversation, as the Texts page lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Conversation {
    /// Its people's numbers' keys, joined by ",": its name on the page.
    pub id: String,
    pub with: Vec<String>,
    /// Your address books' names, else the numbers as you write them.
    pub title: String,
    /// The last message's first words, and when.
    pub last: String,
    pub when: String,
    pub at: i64,
    pub group: bool,
}

/// A part of a message, as the page shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Shown {
    /// "picture", "sound", "video", "card", "file", "text".
    pub kind: String,
    pub name: String,
    /// "A picture (220 KB)"; "Too large to bring here: it is on your phone."; "Your phone has not downloaded it yet."
    pub said: String,
    /// Its content's hash, to open it (`texts::call` "media"); "" when it is not here.
    pub hash: String,
    pub ct: String,
}

/// One message of a conversation, or a text being sent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Message {
    pub id: String,
    pub at: i64,
    /// "09:41", or "Yesterday 09:41".
    pub when: String,
    /// Its day, for the thread's separators: "Today", "Yesterday", "Monday 5
    /// October", the year added when it is another; and its time, "09:41".
    pub day: String,
    pub time: String,
    /// "in" or "out".
    pub direction: String,
    /// Who wrote it, in a group.
    pub from: String,
    pub body: String,
    pub picture: bool,
    pub parts: Vec<Shown>,
    /// Deleted on the phone, kept here: said quietly.
    pub deleted: bool,
    /// A text sent from a computer: its key, and its state.
    pub key: String,
    pub said: Option<Said>,
    /// What it was written to, for Send again.
    pub to: String,
}

/// What the view needs besides the lines.
pub struct Viewer<'a> {
    pub now: &'a Zoned,
    pub tr: &'a Translator,
    pub region: Option<&'static Region>,
    /// Your address books' name for a number's key.
    pub name_of: &'a dyn Fn(&str) -> Option<String>,
    /// When the sending phone last shared (ms), for "waiting since".
    pub phone_shared: Option<i64>,
    /// Whether a part's sealed file is on this device.
    pub media_here: &'a dyn Fn(&str) -> bool,
}

/// A conversation's id: its people's keys, sorted, joined.
pub fn conversation_id(with: &[String]) -> String {
    let mut keys: Vec<&str> = with.iter().map(String::as_str).filter(|k| !k.is_empty()).collect();
    keys.sort_unstable();
    keys.dedup();
    keys.join(",")
}

/// "Dr Martin", "Dr Martin, Alice", "01 99 00 12 34".
fn title_of(with: &[String], v: &Viewer) -> String {
    with.iter().map(|k| (v.name_of)(k).unwrap_or_else(|| calls::shown_number(k, v.region))).collect::<Vec<_>>().join(", ")
}

/// A moment's day and time, for a thread: ("Today", "14:02"), ("Monday 5
/// October", "14:02"), ("Friday 3 May 2024", "09:15").
fn day_and_time(at: i64, v: &Viewer) -> (String, String) {
    let local = calls::local(at, v.now);
    let mut day = calls::capitalized(&calls::day_words(v.tr, local.date(), v.now.date()));
    if local.year() != v.now.year() && local.date() < v.now.date().yesterday().unwrap_or(local.date()) {
        day = format!("{day} {}", local.year());
    }
    (day, local.strftime("%H:%M").to_string())
}

/// "14:02" today, "Yesterday 14:02", "Monday 5 October 14:02".
fn when_words(at: i64, v: &Viewer) -> String {
    let local = calls::local(at, v.now);
    let time = local.strftime("%H:%M").to_string();
    if local.date() == v.now.date() {
        time
    } else {
        format!("{} {time}", calls::capitalized(&calls::day_words(v.tr, local.date(), v.now.date())))
    }
}

/// What a part is, in a word for the page.
fn kind_of(ct: &str) -> &'static str {
    match ct.split('/').next().unwrap_or("") {
        "image" => "picture",
        "audio" => "sound",
        "video" => "video",
        "text" if ct.contains("vcard") || ct.contains("x-vcard") => "card",
        "text" => "text",
        _ if ct.contains("vcard") => "card",
        _ => "file",
    }
}

/// A part as the page shows it: what it is and its size; too large, or not
/// downloaded, said; on its way when its file has not come yet. The SMIL,
/// which only lays the message out, and the text parts (in the words) are not shown apart.
fn shown(part: &Part, v: &Viewer) -> Option<Shown> {
    if part.ct == "application/smil" || part.state == "text" || (kind_of(&part.ct) == "text" && part.hash.is_empty()) {
        return None;
    }
    let kind = kind_of(&part.ct);
    let size = crate::view::size(v.tr, usize::try_from(part.size).unwrap_or(usize::MAX));
    let mut args = crate::i18n::args();
    args.set("size", size);
    let (said, hash) = match part.state.as_str() {
        "too-big" => (v.tr.text("texts-part-too-big", Some(&args)), String::new()),
        "not-downloaded" => (v.tr.text("texts-part-not-downloaded", None), String::new()),
        _ if !(v.media_here)(&part.hash) => (v.tr.text("texts-part-coming", Some(&args)), String::new()),
        _ => (v.tr.text(&format!("texts-part-{kind}"), Some(&args)), part.hash.clone()),
    };
    Some(Shown { kind: kind.into(), name: part.name.clone(), said, hash, ct: part.ct.clone() })
}

/// The conversations, newest first: every text's, and those you wrote to from a computer.
pub fn conversations(texts: &[Text], requests: &[Request], v: &Viewer) -> Vec<Conversation> {
    let mut by: BTreeMap<String, Conversation> = BTreeMap::new();
    let mut add = |with: Vec<String>, at: i64, last: String| {
        let id = conversation_id(&with);
        if id.is_empty() {
            return;
        }
        let entry = by.entry(id.clone()).or_insert_with(|| Conversation { id: id.clone(), group: with.len() > 1, title: title_of(&with, v), with, last: String::new(), when: String::new(), at: 0 });
        if at >= entry.at {
            entry.at = at;
            entry.last = last;
            entry.when = when_words(at, v);
        }
    };
    for t in texts {
        let first: String = if t.body.is_empty() && (t.picture || !t.parts.is_empty()) { v.tr.text("phonemsgs-picture", None) } else { t.body.chars().take(80).collect() };
        add(t.with.clone(), t.at, first);
    }
    for r in requests {
        add(vec![phones::key(&r.to, v.region)], r.written, r.body.chars().take(80).collect());
    }
    let mut list: Vec<Conversation> = by.into_values().collect();
    list.sort_by(|a, b| b.at.cmp(&a.at).then_with(|| a.id.cmp(&b.id)));
    list
}

/// A conversation's messages, oldest first: its texts, deleted ones said, and
/// the texts you wrote to it from a computer with their states; a request
/// Android filed is said on its row, once.
pub fn messages(id: &str, texts: &[Text], requests: &[Request], outcomes: &[Outcome], v: &Viewer) -> Vec<Message> {
    let key_of_row: BTreeMap<&str, &str> = outcomes.iter().filter(|o| !o.row.is_empty()).map(|o| (o.row.as_str(), o.key.as_str())).collect();
    let mut out: Vec<Message> = Vec::new();
    let mut shown_keys = BTreeSet::new();
    for t in texts.iter().filter(|t| conversation_id(&t.with) == id) {
        let key = if t.key.is_empty() { key_of_row.get(t.id.as_str()).map(|k| k.to_string()).unwrap_or_default() } else { t.key.clone() };
        let said = (!key.is_empty() && requests.iter().any(|r| r.key == key)).then(|| self::said(state_of(&key, outcomes), v.phone_shared, v.tr, v.now));
        if !key.is_empty() {
            shown_keys.insert(key.clone());
        }
        let (day, time) = day_and_time(t.at, v);
        out.push(Message {
            id: format!("{}/{}", t.device, t.id),
            at: t.at,
            when: when_words(t.at, v),
            day,
            time,
            direction: t.direction.clone(),
            from: if t.with.len() > 1 && t.direction == "in" && !t.from.is_empty() { (v.name_of)(&t.from).unwrap_or_else(|| calls::shown_number(&t.from, v.region)) } else { String::new() },
            body: t.body.clone(),
            picture: t.picture,
            parts: t.parts.iter().filter_map(|p| shown(p, v)).collect(),
            deleted: t.deleted > 0,
            key,
            said,
            to: String::new(),
        });
    }
    for r in requests.iter().filter(|r| conversation_id(&[phones::key(&r.to, v.region)]) == id && !shown_keys.contains(&r.key)) {
        let outcome = state_of(&r.key, outcomes);
        let (day, time) = day_and_time(r.written, v);
        out.push(Message {
            id: format!("request/{}", r.key),
            at: r.written,
            when: when_words(r.written, v),
            day,
            time,
            direction: "out".into(),
            from: String::new(),
            body: r.body.clone(),
            picture: false,
            parts: Vec::new(),
            deleted: false,
            key: r.key.clone(),
            said: Some(said(outcome, v.phone_shared, v.tr, v.now)),
            to: r.to.clone(),
        });
    }
    out.sort_by(|a, b| (a.at, &a.id).cmp(&(b.at, &b.id)));
    out
}

/// One text found by `search`, with its conversation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Found {
    /// Its conversation's id (`conversation_id`), as `messages` takes it, and its title.
    pub conversation: String,
    pub title: String,
    pub message: Message,
}

/// The texts holding every word of `query`, case and accents aside, in their
/// words, a part's name, or their people's names or numbers; the newest
/// first, `limit` at most. For a search of the archive (the MCP server's).
pub fn search(query: &str, texts: &[Text], requests: &[Request], outcomes: &[Outcome], v: &Viewer, limit: usize) -> Vec<Found> {
    let words: Vec<String> = crate::rules::fold(query).split(' ').filter(|w| !w.is_empty()).map(str::to_string).collect();
    if words.is_empty() {
        return Vec::new();
    }
    // Each conversation's people, said once: names and numbers as written and as keyed.
    let mut people: BTreeMap<String, (String, String)> = BTreeMap::new();
    let mut found: Vec<&Text> = texts
        .iter()
        .filter(|t| {
            let id = conversation_id(&t.with);
            let (_, said) = people.entry(id).or_insert_with(|| {
                let title = title_of(&t.with, v);
                let numbers = t.with.iter().map(|k| format!("{k} {}", calls::shown_number(k, v.region))).collect::<Vec<_>>().join(" ");
                let said = crate::rules::fold(&format!("{title} {numbers}"));
                (title, said)
            });
            let own = crate::rules::fold(&std::iter::once(t.body.as_str()).chain(t.parts.iter().map(|p| p.name.as_str())).collect::<Vec<_>>().join(" "));
            words.iter().all(|w| own.contains(w.as_str()) || said.contains(w.as_str()))
        })
        .collect();
    found.sort_by(|a, b| b.at.cmp(&a.at).then_with(|| a.id.cmp(&b.id)));
    found.truncate(limit);
    found
        .into_iter()
        .filter_map(|t| {
            let id = conversation_id(&t.with);
            let row = format!("{}/{}", t.device, t.id);
            let message = messages(&id, std::slice::from_ref(t), requests, outcomes, v).into_iter().find(|m| m.id == row)?;
            Some(Found { title: people.get(&id).map(|(title, _)| title.clone()).unwrap_or_default(), conversation: id, message })
        })
        .collect()
}

/// The SIM a conversation last used, on the phone that sends: its texts' `sub`, the newest; -1 none.
pub fn last_sim(id: &str, phone: &str, texts: &[Text]) -> i32 {
    texts.iter().rev().find(|t| t.device == calls::file_safe(phone) && conversation_id(&t.with) == id && t.sub >= 0).map_or(-1, |t| t.sub)
}

/// A part's words for its kind: "texts-part-picture" and the others take a `size`.
pub const PART_KINDS: [&str; 6] = ["picture", "sound", "video", "card", "file", "text"];

#[cfg(test)]
mod tests {
    use super::*;

    /// A stand-in for the sealer: reversible, and visibly not plain.
    struct Stand;

    impl Sealer for Stand {
        fn seal(&self, plain: &str) -> String {
            format!("sealed:{}", plain.bytes().map(|b| format!("{:02x}", b ^ 0x5a)).collect::<String>())
        }
        fn open(&self, sealed: &str) -> Option<String> {
            let hex = sealed.strip_prefix("sealed:")?;
            let bytes: Option<Vec<u8>> = (0..hex.len()).step_by(2).map(|i| hex.get(i..i + 2).and_then(|h| u8::from_str_radix(h, 16).ok()).map(|b| b ^ 0x5a)).collect();
            String::from_utf8(bytes?).ok()
        }
    }

    /// Thursday 8 October 2026, 10:00 in Paris.
    const NOW: i64 = 1_791_446_400_000;
    const MINUTE: i64 = 60_000;

    fn fr() -> Option<&'static Region> {
        phones::region_named("FR")
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sioul-texts-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn request(key: &str, phone: &str, written: i64) -> Request {
        Request { key: key.into(), phone: phone.into(), to: "01 99 00 12 34".into(), body: "J'arrive dans dix minutes.".into(), sub: -1, written, ahead: 0, again: String::new(), device: "desk".into() }
    }

    /// The phone's steps, as `texts.rs` in the app takes them: read every
    /// request, decide, claim before sending; returns the keys sent.
    fn step(dir: &Path, requests: &[Request], here: &str, now: i64, ahead: i64) -> Vec<String> {
        let ledger = Ledger::open(dir, now).unwrap();
        let mut sent = Vec::new();
        for r in requests {
            if verdict(&Request { ahead, ..r.clone() }, here, now, &ledger, fr()) == Verdict::Send {
                record(dir, &Claim { key: r.key.clone(), at: now, state: "claimed".into(), to: phones::key(&r.to, fr()), hash: words_hash(&r.body) }).unwrap();
                sent.push(r.key.clone());
            }
        }
        sent
    }

    #[test]
    fn a_request_is_sent_once_whatever_is_read_again() {
        let dir = scratch("once");
        Ledger::open(&dir, NOW - 10 * MINUTE).unwrap();
        let r = request("a1", "phone", NOW - MINUTE);
        assert_eq!(step(&dir, std::slice::from_ref(&r), "phone", NOW, 0), ["a1"]);
        for later in [NOW + MINUTE, NOW + 2 * MINUTE] {
            assert!(step(&dir, &[r.clone(), r.clone()], "phone", later, 0).is_empty());
        }
        assert_eq!(Ledger::open(&dir, NOW).unwrap().state("a1"), Some("claimed"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_lost_ledger_s_mark_refuses_the_requests_written_before_it() {
        let dir = scratch("lost");
        Ledger::open(&dir, NOW - 10 * MINUTE).unwrap();
        let r = request("b1", "phone", NOW - 3 * MINUTE);
        assert_eq!(step(&dir, std::slice::from_ref(&r), "phone", NOW - 2 * MINUTE, 0), ["b1"]);
        std::fs::remove_dir_all(&dir).unwrap();
        let ledger = Ledger::open(&dir, NOW).unwrap();
        assert_eq!(verdict(&r, "phone", NOW, &ledger, fr()), Verdict::BeforeMark, "within its 15 minutes, still never again");
        assert!(step(&dir, &[r], "phone", NOW, 0).is_empty());
        assert_eq!(step(&dir, &[request("b2", "phone", NOW + MINUTE)], "phone", NOW + MINUTE, 0), ["b2"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn two_phones_each_send_only_their_own_and_two_computers_both_send() {
        let (a, b) = (scratch("phone-a"), scratch("phone-b"));
        Ledger::open(&a, NOW - 10 * MINUTE).unwrap();
        Ledger::open(&b, NOW - 10 * MINUTE).unwrap();
        let from_desk = request("c1", "phone-a", NOW - MINUTE);
        let from_laptop = Request { device: "laptop".into(), ..request("c2", "phone-a", NOW - MINUTE) };
        let to_b = request("c3", "phone-b", NOW - MINUTE);
        let all = [from_desk, from_laptop, to_b];
        assert_eq!(step(&a, &all, "phone-a", NOW, 0), ["c1", "c2"], "two computers, two keys, two texts");
        assert_eq!(step(&b, &all, "phone-b", NOW, 0), ["c3"]);
        let _ = (std::fs::remove_dir_all(&a), std::fs::remove_dir_all(&b));
    }

    #[test]
    fn an_expired_request_and_clocks_apart() {
        let dir = scratch("expired");
        Ledger::open(&dir, NOW - DAY_MS).unwrap();
        let ledger = Ledger::open(&dir, NOW).unwrap();
        assert_eq!(verdict(&request("d1", "phone", NOW - 16 * MINUTE), "phone", NOW, &ledger, fr()), Verdict::Expired);
        assert_eq!(verdict(&request("d2", "phone", NOW - 14 * MINUTE), "phone", NOW, &ledger, fr()), Verdict::Send);
        // The computer's clock an hour ahead, as it measured: written "15:00" is 14:00 here.
        let ahead = 60 * MINUTE;
        assert_eq!(verdict(&Request { ahead, ..request("d3", "phone", NOW + ahead - 20 * MINUTE) }, "phone", NOW, &ledger, fr()), Verdict::Expired);
        assert_eq!(verdict(&Request { ahead, ..request("d4", "phone", NOW + ahead - 5 * MINUTE) }, "phone", NOW, &ledger, fr()), Verdict::Send);
        // Not measured, and dated well ahead: the clocks disagree, never sent late.
        assert_eq!(verdict(&request("d5", "phone", NOW + 30 * MINUTE), "phone", NOW, &ledger, fr()), Verdict::Clocks);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn short_numbers_groups_and_empty_words_are_refused() {
        let ledger = Ledger { mark: Mark { since: 0 }, claims: Vec::new() };
        let with = |to: &str, body: &str| verdict(&Request { to: to.into(), body: body.into(), ..request("e", "phone", NOW) }, "phone", NOW, &ledger, fr());
        assert_eq!(with("36179", "STOP"), Verdict::Refused("short-number"));
        assert_eq!(with("01 99 00 12 34, 04 65 71 00 42", "Bonjour"), Verdict::Refused("several"));
        assert_eq!(with("01 99 00 12 34", "   "), Verdict::Refused("empty"));
        assert_eq!(with("01 99 00 12 34", &"x".repeat(WRITE_MAX + 1)), Verdict::Refused("too-long"));
        assert_eq!(with("+33 1 99 00 12 34", "Bonjour"), Verdict::Send);
    }

    #[test]
    fn a_crash_between_the_claim_and_the_result_is_said_never_sent_again() {
        let claim = Claim { key: "f1".into(), at: NOW, state: "claimed".into(), to: "+33199001234".into(), hash: words_hash("Bonjour") };
        assert!(doubt_after(&claim, None, NOW + MINUTE).is_none(), "Android's word may still come");
        let found = doubt_after(&claim, Some("sms-412".into()), NOW + 3 * MINUTE).unwrap();
        assert_eq!((found.state.as_str(), found.row.as_str()), ("sent", "sms-412"));
        assert_eq!(doubt_after(&claim, None, NOW + 3 * MINUTE).unwrap().state, "doubt");
        let ledger = Ledger { mark: Mark { since: 0 }, claims: vec![claim] };
        assert_eq!(verdict(&request("f1", "phone", NOW + 3 * MINUTE), "phone", NOW + 3 * MINUTE, &ledger, fr()), Verdict::Decided);
    }

    #[test]
    fn the_ledger_is_flushed_before_android_is_asked_and_trimmed_after_a_month() {
        let dir = scratch("ledger");
        record(&dir, &Claim { key: "g1".into(), at: NOW - 31 * DAY_MS, state: "done".into(), ..Claim::default() }).unwrap();
        record(&dir, &Claim { key: "g2".into(), at: NOW, state: "claimed".into(), ..Claim::default() }).unwrap();
        assert!(std::fs::read_to_string(dir.join(LEDGER)).unwrap().contains("\"g2\""));
        assert_eq!(trim_ledger(&dir, NOW).unwrap(), 1);
        let ledger = Ledger::open(&dir, NOW).unwrap();
        assert!(!ledger.has("g1") && ledger.has("g2"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn message(id: &str, at: i64, body: &str) -> Text {
        Text { id: id.into(), at, thread: "7".into(), direction: "in".into(), with: vec!["+33199001234".into()], body: body.into(), sub: 1, ..Text::default() }
    }

    #[test]
    fn the_history_is_sealed_padded_kept_whole_and_a_deletion_marks_it() {
        let dir = scratch("lines");
        let own = own_file(&dir, LOG, "phone");
        let id = |t: &Text| t.line_id();
        let old = NOW - 3 * 365 * DAY_MS;
        assert_eq!(append(&own, &[message("sms-1", old, "Bonjour"), message("sms-2", NOW, "Votre rendez-vous est déplacé.")], &Stand, Some(&id)).unwrap(), 2);
        assert_eq!(append(&own, &[message("sms-2", NOW, "Votre rendez-vous est déplacé.")], &Stand, Some(&id)).unwrap(), 0, "read again: once");
        let raw = std::fs::read_to_string(&own).unwrap();
        assert!(!raw.contains("Bonjour") && !raw.contains("199001234"), "sealed at rest");
        for line in raw.lines() {
            assert_eq!(Stand.open(line).unwrap().len() % PAD_STEP, 0);
        }
        // Three years old, and still there: no window, no trim by age.
        let later = NOW + 400 * DAY_MS;
        assert_eq!(trim_own(&dir, "phone", &Stand, later).unwrap() + trim_others(&dir, "desk", &Stand, later).unwrap(), 0);
        assert_eq!(read_logs(&dir, &Stand).len(), 2);
        // Deleted on the phone: a line of its own; the text stays, marked.
        let deletion = Text { deleted: NOW + MINUTE, ..message("sms-1", old, "") };
        assert_eq!(append(&own, &[deletion], &Stand, Some(&id)).unwrap(), 1);
        let back = read_logs(&dir, &Stand);
        assert_eq!(back.iter().map(|t| (t.id.as_str(), t.body.as_str(), t.deleted > 0)).collect::<Vec<_>>(), [("sms-1", "Bonjour", true), ("sms-2", "Votre rendez-vous est déplacé.", false)]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parts_are_counted_as_android_counts_them() {
        assert_eq!(parts(""), (0, true));
        assert_eq!(parts("Bonjour, à demain !"), (1, true));
        assert_eq!(parts(&"a".repeat(160)), (1, true));
        assert_eq!(parts(&"a".repeat(161)), (2, true));
        assert_eq!(parts(&"€".repeat(80)), (1, true), "two septets each: 160");
        assert_eq!(parts(&"€".repeat(81)), (2, true));
        assert_eq!(parts("Ça va ? 🙂"), (1, false), "an emoji: UCS-2");
        assert_eq!(parts(&"ç".repeat(71)), (2, false), "ç is no GSM letter (Ç is)");
    }

    /// A thread's separators: each message's day in words, the year only when another, and its time.
    #[test]
    fn a_thread_says_each_message_s_day_and_time() {
        let tr = Translator::new("en");
        let now = paris(NOW);
        let names = |_: &str| None;
        let here = |_: &str| false;
        let v = viewer(&now, &tr, &names, &here);
        let row = |id: &str, at: i64| Text { id: id.into(), at, direction: "in".into(), with: vec!["+33199001234".into()], body: "Bonjour".into(), device: "phone".into(), ..Text::default() };
        let texts = [row("sms-1", NOW - 400 * 86_400_000), row("sms-2", NOW - 86_400_000), row("sms-3", NOW - MINUTE)];
        let said: Vec<(String, String)> = messages("+33199001234", &texts, &[], &[], &v).into_iter().map(|m| (m.day, m.time)).collect();
        assert_eq!(said[1], ("Yesterday".to_string(), "10:00".to_string()));
        assert_eq!(said[2], ("Today".to_string(), "09:59".to_string()));
        assert!(said[0].0.ends_with(" 2025") && said[0].0.contains("September"), "another year said: {:?}", said[0]);
    }

    #[test]
    fn the_archive_is_searched_by_words_names_and_numbers_newest_first() {
        let tr = Translator::new("en");
        let now = paris(NOW);
        let names = |k: &str| (k == "+33199001234").then(|| "Pharmacie Sainte-Hélène".to_string());
        let here = |_: &str| false;
        let v = viewer(&now, &tr, &names, &here);
        let row = |id: &str, at: i64, with: &str, body: &str| Text { id: id.into(), at, direction: "in".into(), with: vec![with.into()], body: body.into(), device: "phone".into(), ..Text::default() };
        let texts = [
            row("sms-1", NOW - 3 * MINUTE, "+33199001234", "Votre ordonnance est prête."),
            row("sms-2", NOW - 2 * MINUTE, "+33465710042", "Le colis est arrivé au relais."),
            row("sms-3", NOW - MINUTE, "+33199001234", "Ordonnance renouvelée, à retirer demain."),
        ];
        let ids = |query: &str, limit: usize| search(query, &texts, &[], &[], &v, limit).into_iter().map(|f| f.message.id).collect::<Vec<_>>();
        assert_eq!(ids("ORDONNANCE", 10), ["phone/sms-3", "phone/sms-1"], "case aside, newest first");
        assert_eq!(ids("arrive", 10), ["phone/sms-2"], "accents aside");
        assert_eq!(ids("helene pret", 10), ["phone/sms-1"], "a name and a word, both");
        assert_eq!(ids("04 65 71", 10), ["phone/sms-2"], "a number as written");
        assert_eq!(ids("ordonnance", 1), ["phone/sms-3"]);
        assert!(ids("  ", 10).is_empty());
        let found = &search("colis", &texts, &[], &[], &v, 10)[0];
        assert_eq!((found.conversation.as_str(), found.title.as_str()), ("+33465710042", "04 65 71 00 42"));
    }

    fn viewer<'a>(now: &'a Zoned, tr: &'a Translator, names: &'a dyn Fn(&str) -> Option<String>, here: &'a dyn Fn(&str) -> bool) -> Viewer<'a> {
        Viewer { now, tr, region: fr(), name_of: names, phone_shared: Some(NOW - 2 * MINUTE), media_here: here }
    }

    fn paris(ms: i64) -> Zoned {
        jiff::Timestamp::from_millisecond(ms).unwrap().to_zoned(jiff::tz::TimeZone::get("Europe/Paris").unwrap())
    }

    #[test]
    fn the_page_shows_conversations_and_each_request_s_state_in_words() {
        let tr = Translator::new("en");
        let now = paris(NOW);
        let names = |k: &str| (k == "+33199001234").then(|| "Dr Martin".to_string());
        let here = |_: &str| true;
        let v = viewer(&now, &tr, &names, &here);
        let texts = vec![
            Text { id: "sms-1".into(), at: NOW - 60 * MINUTE, direction: "in".into(), with: vec!["+33199001234".into()], body: "Votre rendez-vous est déplacé.".into(), sub: 2, device: "phone".into(), ..Text::default() },
            Text { id: "sms-2".into(), at: NOW - 30 * MINUTE, direction: "in".into(), with: vec!["+33465710042".into()], body: "Le colis est arrivé.".into(), sub: 1, device: "phone".into(), ..Text::default() },
            Text { id: "sms-9".into(), at: NOW - 5 * MINUTE, direction: "out".into(), with: vec!["+33199001234".into()], body: "Merci, c'est noté.".into(), sub: 2, device: "phone".into(), ..Text::default() },
        ];
        let requests = vec![Request { body: "Merci, c'est noté.".into(), ..request("k1", "phone", NOW - 6 * MINUTE) }, request("k2", "phone", NOW - MINUTE), Request { to: "04 65 71 00 42".into(), ..request("k3", "phone", NOW - 20 * MINUTE) }];
        let outcomes = vec![
            Outcome { key: "k1".into(), state: "handed".into(), at: NOW - 5 * MINUTE, ..Outcome::default() },
            Outcome { key: "k1".into(), state: "sent".into(), at: NOW - 5 * MINUTE, row: "sms-9".into(), ..Outcome::default() },
            Outcome { key: "k3".into(), state: "expired".into(), at: NOW - 2 * MINUTE, ..Outcome::default() },
        ];
        let list = conversations(&texts, &requests, &v);
        assert_eq!(list.iter().map(|c| c.title.as_str()).collect::<Vec<_>>(), ["Dr Martin", "04 65 71 00 42"]);
        let shown = messages("+33199001234", &texts, &requests, &outcomes, &v);
        assert_eq!(shown.iter().map(|m| (m.direction.as_str(), m.key.as_str())).collect::<Vec<_>>(), [("in", ""), ("out", "k1"), ("out", "k2")]);
        assert_eq!(shown[1].said.as_ref().unwrap().text, "Sent at 09:55.");
        assert_eq!(shown[2].said.as_ref().unwrap().text, "Waiting for your phone (it last shared at 09:58).");
        let other = messages("+33465710042", &texts, &requests, &outcomes, &v);
        let expired = other.iter().find(|m| m.key == "k3").unwrap().said.clone().unwrap();
        assert!(expired.again && expired.state == "expired", "{expired:?}");
        assert_eq!(last_sim("+33199001234", "phone", &texts), 2);
    }

    #[test]
    fn a_message_s_media_are_said_with_their_sizes_or_why_they_are_not_here() {
        let tr = Translator::new("en");
        let now = paris(NOW);
        let names = |_: &str| None;
        let here = |hash: &str| hash == "aa";
        let v = viewer(&now, &tr, &names, &here);
        let part = |seq: i64, ct: &str, size: u64, hash: &str, state: &str| Part { seq, ct: ct.into(), size, hash: hash.into(), state: state.into(), ..Part::default() };
        let mms = Text {
            id: "mms-31".into(),
            at: NOW - MINUTE,
            direction: "in".into(),
            with: vec!["+33465710042".into(), "+33199001234".into()],
            from: "+33465710042".into(),
            body: "Le colis.".into(),
            parts: vec![part(-1, "application/smil", 300, "", "text"), part(0, "image/jpeg", 220_000, "aa", "here"), part(1, "video/mp4", 30_000_000, "", "too-big"), part(2, "audio/amr", 9_000, "bb", "here"), part(3, "image/png", 0, "", "not-downloaded"), part(4, "text/plain", 9, "", "text")],
            device: "phone".into(),
            ..Text::default()
        };
        let shown = messages(&conversation_id(&mms.with), std::slice::from_ref(&mms), &[], &[], &v);
        let said: Vec<(&str, &str, bool)> = shown[0].parts.iter().map(|p| (p.kind.as_str(), p.said.as_str(), !p.hash.is_empty())).collect();
        assert_eq!(said.len(), 4, "the SMIL and the words are not shown apart: {said:?}");
        assert_eq!(said[0].0, "picture");
        assert!(said[0].2, "a picture here opens");
        assert!(said[1].1.contains("on your phone") && !said[1].2, "{said:?}");
        assert!(said[2].1.contains("its way") && !said[2].2, "its file not come yet: {said:?}");
        assert!(said[3].1.contains("not downloaded"), "{said:?}");
        assert_eq!(shown[0].from, "04 65 71 00 42", "a group says who wrote");
        assert_eq!(kept(std::slice::from_ref(&mms), &here), (1, 220_000), "only what is here is counted");
    }

    #[test]
    fn delivered_is_said_only_when_the_carrier_says_so() {
        let tr = Translator::new("en");
        let now = paris(NOW);
        let sent = [Outcome { key: "h1".into(), state: "sent".into(), at: NOW + MINUTE, ..Outcome::default() }];
        assert_eq!(said(state_of("h1", &sent), None, &tr, &now).state, "sent", "no report: sent, never 'not delivered'");
        let delivered = [sent[0].clone(), Outcome { key: "h1".into(), state: "delivered".into(), at: NOW + 2 * MINUTE, ..Outcome::default() }];
        assert_eq!(said(state_of("h1", &delivered), None, &tr, &now).text, "Delivered at 10:02.");
    }
}
