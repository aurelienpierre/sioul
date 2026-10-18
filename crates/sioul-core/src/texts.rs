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
    /// Its row's name on that phone (`Row::name`): `sms-<_id>@<date>`,
    /// `mms-<_id>@<date>`; `sms-<_id>`, `mms-<_id>` for a line written
    /// before 16 October 2026.
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
    /// On a deletion's line: the name of the line that holds the same
    /// message under the date its provider rewrote (an MMS downloaded or sent
    /// writes its date after its row; a resend): computers show it once,
    /// under that name (`fold_logs`). "" for a message deleted on the phone.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub moved: String,
    /// The phone that wrote it (its file's name). Never written.
    #[serde(skip)]
    pub device: String,
    /// The same text in other phones' logs (`<phone>/<row>`), when `one_each`
    /// kept this copy for them all (a restore onto a new phone). Never written.
    #[serde(skip)]
    pub also: Vec<String>,
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

    /// Its row on its phone: table, id and the row's own date (`Row`).
    pub fn row(&self) -> Option<Row> {
        Row::of(&self.id, self.at)
    }
}

/// A text's row in its phone's provider of texts: its table, Android's
/// `_id`, and the row's own date there (milliseconds for a plain text,
/// seconds for a multimedia message, as the provider counts them): what
/// tells two texts apart on one phone. The plain texts' table has no
/// AUTOINCREMENT, so the next text takes a deleted newest text's id again,
/// and a provider emptied starts again at 1; the date tells those rows
/// apart (docs/texts.md, "A row's name").
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Row {
    pub mms: bool,
    pub id: i64,
    pub date: i64,
}

impl Row {
    /// Its name in the log, the text's `id`: `sms-12@1791446340000`, `mms-31@1791446340`.
    pub fn name(&self) -> String {
        format!("{}-{}@{}", if self.mms { "mms" } else { "sms" }, self.id, self.date)
    }

    /// A line's row, from its name and its time (`at`, ms): `sms-12@1791…`;
    /// or, for a line named before 16 October 2026 (`sms-12`, `mms-31`), the
    /// date its time holds (a multimedia message's in seconds). None for a
    /// name of no row.
    pub fn of(name: &str, at: i64) -> Option<Row> {
        let (table, rest) = name.split_once('-')?;
        let mms = match table {
            "sms" => false,
            "mms" => true,
            _ => return None,
        };
        let (id, date) = match rest.split_once('@') {
            Some((id, date)) => (id.parse().ok()?, date.parse().ok()?),
            None => (rest.parse().ok()?, if mms { at.div_euclid(1000) } else { at }),
        };
        Some(Row { mms, id, date })
    }

    /// Its time, in milliseconds: a multimedia message's seconds made so.
    pub fn at(&self) -> i64 {
        if self.mms { self.date.saturating_mul(1000) } else { self.date }
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
    /// The provider's row Android filed it as, by its name in the log
    /// (`Row::name`, "sms-123@1791446340000"; "sms-123" before 16 October
    /// 2026), when known: never a bare id an older text may hold.
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
    read_lines(&std::fs::read_to_string(path).unwrap_or_default(), sealer)
}

/// Every line of a file's text that opens and reads, in order; the rest passed over.
pub fn read_lines<T: for<'de> Deserialize<'de>>(text: &str, sealer: &dyn Sealer) -> Vec<T> {
    text.lines().filter(|l| !l.trim().is_empty()).filter_map(|l| sealer.open(l.trim())).filter_map(|plain| serde_json::from_str(&plain).ok()).collect()
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
    fold_logs(read_store(root, LOG, sealer, |t: &mut Text, d| t.device = d.to_string()))
}

/// A line's place in its phone's log: its row (`Row::of`), so that a line
/// named by its id alone (before 16 October 2026) and its row's name now
/// are one text; its name for a line of no row.
fn place_of(name: &str, at: i64) -> String {
    Row::of(name, at).map_or_else(|| name.to_string(), |r| r.name())
}

/// The phones' log lines (each with its `device`) as `read_logs` gives
/// them: oldest first, each text once by phone and row (a line named by
/// its id alone and the same row named with its date are one: a phone put
/// back on an older Sioul, which writes again under the old names what a
/// newer one read, never doubles a text on the computers; the review of 16
/// October 2026, MEDIUM 1), a deletion's line folded into its text, a
/// line moved under its new name (its row's date rewritten) shown under
/// that name alone, its old name kept beside it (`also`), so that a request
/// sent from a computer still finds its text (review of 18 October 2026, L-4).
pub fn fold_logs(lines: Vec<Text>) -> Vec<Text> {
    let deletions: BTreeMap<(String, String), i64> = lines.iter().filter(|t| t.deleted > 0).map(|t| ((t.device.clone(), place_of(&t.id, t.at)), t.deleted)).collect();
    let places: BTreeSet<(String, String)> = lines.iter().filter(|t| t.deleted == 0).map(|t| (t.device.clone(), place_of(&t.id, t.at))).collect();
    let mut moved: BTreeSet<(String, String)> = BTreeSet::new();
    let mut old_names: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
    for t in lines.iter().filter(|t| t.deleted > 0 && !t.moved.is_empty()) {
        let to = (t.device.clone(), place_of(&t.moved, 0));
        if places.contains(&to) {
            moved.insert((t.device.clone(), place_of(&t.id, t.at)));
            old_names.entry(to).or_default().push(format!("{}/{}", t.device, t.id));
        }
    }
    let mut seen = BTreeSet::new();
    let mut texts: Vec<Text> = lines
        .into_iter()
        .filter(|t| t.deleted == 0 && t.at > 0)
        .filter_map(|t| {
            let place = (t.device.clone(), place_of(&t.id, t.at));
            if moved.contains(&place) || !seen.insert(place.clone()) {
                return None;
            }
            let deleted = deletions.get(&place).copied().unwrap_or(0);
            let mut also = t.also.clone();
            also.extend(old_names.get(&place).into_iter().flatten().cloned());
            Some(Text { deleted, also, ..t })
        })
        .collect();
    texts.sort_by(|a, b| (a.at, &a.device, &a.id).cmp(&(b.at, &b.device, &b.id)));
    texts
}

/// A phone's log as `Logs` read it: its lines, how far, the bytes just
/// before that place (to tell the same file grown from another one), and
/// the file's stamp then (`Stamp`).
#[derive(Debug, Default)]
struct LogRead {
    /// Its lines without their raw columns, and what each text's fingerprint is made of (`Print`), by its name.
    lines: Vec<Text>,
    plain: BTreeMap<String, Print>,
    upto: u64,
    before: Vec<u8>,
    seen: Stamp,
}

/// What tells a file unchanged since it was read: its size and time, and on
/// Unix its inode and change time, which no tool sets back (a file of the
/// same size whose time was put back, by rsync -t or a restore, is read
/// again: the review of 16 October 2026, LOW 6). Windows gives neither: such
/// a file is read again there only once its size or time changes (NTFS's
/// tunnelling also keeps a recreated file's creation time, so `created()`
/// tells nothing either).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct Stamp {
    len: u64,
    modified: Option<std::time::SystemTime>,
    inode: u64,
    changed: i128,
}

impl Stamp {
    fn of(meta: &std::fs::Metadata) -> Stamp {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            Stamp { len: meta.len(), modified: meta.modified().ok(), inode: meta.ino(), changed: i128::from(meta.ctime()) * 1_000_000_000 + i128::from(meta.ctime_nsec()) }
        }
        #[cfg(not(unix))]
        {
            Stamp { len: meta.len(), modified: meta.modified().ok(), inode: 0, changed: 0 }
        }
    }

    /// Read up to `len` with a line still being written after it: never taken for unchanged.
    fn partial(len: u64) -> Stamp {
        Stamp { len, ..Stamp::default() }
    }
}

/// How many bytes before the place a log was read to tell the same file grown.
const LOG_CHECK: u64 = 64;

/// Every phone's log under a root as `read_logs` gives it, kept between two
/// reads and read again only where a file grew (a log only grows: a line is
/// added at its end, never changed); a file that changed otherwise (another
/// file under its name, cut short) is read again whole. With each text's
/// fingerprint kept by phone and row (`one_each`): the window's refresh after
/// an exchange that brought one text opened 20,000 sealed lines and hashed
/// them all, 0.3 s on its thread (the review of 16 October 2026, LOW 3).
#[derive(Debug, Default)]
pub struct Logs {
    files: BTreeMap<String, (PathBuf, LogRead)>,
    prints: BTreeMap<(String, String), String>,
    region: Option<&'static str>,
}

impl Logs {
    /// Each phone's log under `root` read again where it changed; whether one did.
    pub fn refresh(&mut self, root: &Path, sealer: &dyn Sealer) -> bool {
        let found = calls::device_files(&root.join(LOG));
        let mut changed = self.files.len() != found.len() || self.files.keys().zip(&found).any(|(kept, (device, _))| kept != device);
        self.files.retain(|device, _| found.iter().any(|(d, _)| d == device));
        for (device, path) in found {
            let entry = self.files.entry(device.clone()).or_insert_with(|| (path.clone(), LogRead::default()));
            if entry.0 != path {
                *entry = (path.clone(), LogRead::default());
            }
            changed |= grow(&path, &device, &mut entry.1, sealer);
        }
        changed
    }

    /// Every phone's texts, oldest first, each once by phone and row, a
    /// deletion folded into its text (`fold_logs`), without their raw columns
    /// (`fields`, `addrs`): only the fingerprints read those, and each is made
    /// as its line is read (review of 16 October 2026, LOW 7: cloning them at
    /// each refresh cost the window's thread 0.14 s at 20,000 lines).
    pub fn texts(&self) -> Vec<Text> {
        fold_logs(self.files.values().flat_map(|(_, read)| read.lines.iter().cloned()).collect())
    }

    /// `texts`, each text once across phones (`one_each`), the numbers keyed
    /// for `region`: each fingerprint made once for a phone's row and kept.
    pub fn one_each(&mut self, region: Option<&'static Region>) -> Vec<Text> {
        let code = region.map(|r| r.code);
        if code != self.region {
            self.prints.clear();
            self.region = code;
        }
        let (files, prints) = (&self.files, &mut self.prints);
        let texts = fold_logs(files.values().flat_map(|(_, read)| read.lines.iter().cloned()).collect());
        one_each_by(texts, &mut |t: &Text| {
            prints
                .entry((t.device.clone(), t.id.clone()))
                .or_insert_with(|| files.get(&t.device).and_then(|(_, read)| read.plain.get(&t.id)).map_or_else(|| Print::of(t), Clone::clone).key(region))
                .clone()
        })
    }
}

/// A log read on from where it was read (`LogRead`), or again whole when it
/// is not that file grown; only whole lines. Whether its lines changed.
fn grow(path: &Path, device: &str, read: &mut LogRead, sealer: &dyn Sealer) -> bool {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(meta) = std::fs::metadata(path) else {
        let had = !read.lines.is_empty();
        *read = LogRead::default();
        return had;
    };
    let seen = Stamp::of(&meta);
    if seen == read.seen {
        return false;
    }
    let Ok(mut file) = std::fs::File::open(path) else { return false };
    // The bytes before the place it was read to, as they were: the same file, grown.
    let from = read.upto - read.before.len() as u64;
    let mut before = vec![0u8; read.before.len()];
    let same = seen.len > read.upto && file.seek(SeekFrom::Start(from)).is_ok() && file.read_exact(&mut before).is_ok() && before == read.before;
    let mut changed = false;
    if !same {
        changed = !read.lines.is_empty();
        *read = LogRead::default();
        if file.seek(SeekFrom::Start(0)).is_err() {
            return changed;
        }
    }
    let mut bytes = Vec::new();
    if file.take(seen.len.saturating_sub(read.upto)).read_to_end(&mut bytes).is_err() {
        return changed;
    }
    // Whole lines only: one being written is read at the next look.
    let Some(end) = bytes.iter().rposition(|b| *b == b'\n') else {
        read.seen = Stamp::partial(read.upto);
        return changed;
    };
    let whole = &bytes[..=end];
    let lines: Vec<Text> = read_lines(&String::from_utf8_lossy(whole), sealer);
    changed |= !lines.is_empty();
    for t in lines {
        if t.deleted == 0 {
            read.plain.entry(t.id.clone()).or_insert_with(|| Print::of(&t));
        }
        read.lines.push(Text { device: device.to_string(), fields: BTreeMap::new(), addrs: Vec::new(), ..t });
    }
    let mut tail = std::mem::take(&mut read.before);
    tail.extend_from_slice(whole);
    read.before = tail[tail.len().saturating_sub(LOG_CHECK as usize)..].to_vec();
    read.upto += whole.len() as u64;
    // A line still being written: the next look reads on.
    read.seen = if read.upto == seen.len { seen } else { Stamp::partial(read.upto) };
    changed
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

// ---------------------------------------------------------------- each text once

/// What Android writes for "this phone" among a multimedia message's addresses.
pub const OWN_ADDRESS: &str = "insert-address-token";
/// A multimedia message's address types (`addr.type`, OMA's PduHeaders): from, to, copy, hidden copy.
pub const ADDR_FROM: i64 = 137;
pub const ADDR_TO: i64 = 151;
pub const ADDR_CC: i64 = 130;
pub const ADDR_BCC: i64 = 129;

/// What a message is, whichever phone or backup holds it: what SMS Backup &
/// Restore carries from one phone to another unchanged, and what its own
/// backups hold of the rows Sioul's archive keeps (docs/texts.md, "Each text
/// once"). Never the provider's row or conversation, which another phone
/// numbers its own way.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Print {
    /// A multimedia message (else a plain text).
    pub mms: bool,
    pub outgoing: bool,
    /// When, to the second: the multimedia provider counts in seconds.
    pub second: i64,
    /// The other people, as written: a text's address; a multimedia
    /// message's sender when received, those it went to when sent (this
    /// phone's own address left out, as `OWN_ADDRESS` or as its number).
    pub people: Vec<String>,
    /// Its words: a text's; each text part's of a multimedia message.
    pub words: Vec<String>,
    /// Its media's content types ("image/jpeg").
    pub media: Vec<String>,
}

/// Whether a multimedia message's addresses of this type are its other
/// people for `Print`: a received one's sender; a sent one's recipients.
pub fn print_people(outgoing: bool, kind: i64) -> bool {
    if outgoing { matches!(kind, ADDR_TO | ADDR_CC | ADDR_BCC) } else { kind == ADDR_FROM }
}

/// A part's content type that is words, never a medium: the text and the SMIL that lays it out.
pub fn is_words(ct: &str) -> bool {
    ct.starts_with("text/plain") || ct == "application/smil"
}

/// Words as XML 1.0 can carry them: the control characters it refuses (all
/// below a space but the tab, the line feed and the carriage return, and
/// U+FFFE, U+FFFF) left out. SMS Backup & Restore's backups are XML, so a
/// text restored from one never holds them.
pub fn carried(words: &str) -> std::borrow::Cow<'_, str> {
    if words.chars().any(|c| !xml_char(c)) { std::borrow::Cow::Owned(words.chars().filter(|c| xml_char(*c)).collect()) } else { std::borrow::Cow::Borrowed(words) }
}

/// A character XML 1.0 can carry (its `Char`; Rust's strings hold no lone surrogate).
pub fn xml_char(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\r') || (c >= ' ' && c != '\u{fffe}' && c != '\u{ffff}')
}

impl Print {
    /// A message of a phone's log: its kind, way, second, other people, words and media.
    pub fn of(t: &Text) -> Print {
        let mms = t.id.starts_with("mms-");
        let outgoing = t.direction == "out";
        let field = |name: &str| t.fields.get(name).and_then(Value::as_str).unwrap_or_default().to_string();
        let (people, words, media) = if mms {
            let people: Vec<String> = t.addrs.iter().filter(|a| print_people(outgoing, number(a.get("type")))).filter_map(|a| a.get("address").and_then(Value::as_str)).filter(|a| !a.trim().is_empty() && *a != OWN_ADDRESS).map(str::to_string).collect();
            let people = if people.is_empty() { t.with.clone() } else { people };
            let words = t.parts.iter().filter(|p| p.ct.starts_with("text/plain")).map(|p| p.text.clone()).collect();
            let media = t.parts.iter().filter(|p| !is_words(&p.ct)).map(|p| p.ct.clone()).collect();
            (people, words, media)
        } else {
            let address = field("address");
            let people = if address.trim().is_empty() { t.with.clone() } else { vec![address] };
            (people, vec![t.body.clone()], Vec::new())
        };
        Print { mms, outgoing, second: t.at.div_euclid(1000), people, words, media }
    }

    /// Its fingerprint: the same for the same message on any phone and in any
    /// backup, whatever their rows and conversations (SHA-256, hex). The
    /// people keyed as Sioul keys numbers (`phones::key`), in any order; the
    /// words as XML carries them; an empty text part, or one SMS Backup &
    /// Restore wrote "null", is none.
    pub fn key(&self, region: Option<&Region>) -> String {
        use sha2::{Digest, Sha256};
        let mut people: Vec<String> = self.people.iter().flat_map(|p| p.split('~')).map(|p| phones::key(p, region)).filter(|k| !k.is_empty()).collect();
        people.sort_unstable();
        people.dedup();
        let mut words: Vec<String> = self.words.iter().map(|w| carried(w).into_owned()).filter(|w| !w.is_empty() && !(self.mms && w == "null")).collect();
        words.sort_unstable();
        let mut media: Vec<String> = self.media.iter().map(|m| m.trim().to_ascii_lowercase()).collect();
        media.sort_unstable();
        let mut hasher = Sha256::new();
        for piece in [if self.mms { "mms" } else { "sms" }, if self.outgoing { "out" } else { "in" }, &self.second.to_string(), &people.join("\u{1f}"), &words.join("\u{1e}"), &media.join("\u{1f}")] {
            hasher.update(piece.as_bytes());
            hasher.update([0u8]);
        }
        hasher.finalize().iter().map(|b| format!("{b:02x}")).collect()
    }
}

/// How far apart a received text's two dates may be for a re-date (a
/// provider re-dates its own sending, an MMS downloaded or sent, a resend;
/// never a text received hours later under a deleted text's id).
pub const MOVED_WITHIN_MS: i64 = 10 * 60_000;

/// Whether `new` is the message of `old` under a date its provider
/// rewrote: the same table and row id, another date, the same way and
/// people; a text's words the same, and either sent from the phone or
/// dated within `MOVED_WITHIN_MS` of the other (the plain texts' table
/// gives a deleted newest text's id to the next one: Alice's "Oui." deleted,
/// her next "Oui." is another text, said deleted as before: review of 18
/// October 2026, L-3); a multimedia message's media the same by their
/// hashes wherever the older has them, its words and media as complete or
/// more (one read before its parts were attached comes whole).
pub fn moved(old: &Text, new: &Text) -> bool {
    let (Some(a), Some(b)) = (old.row(), new.row()) else { return false };
    if a.mms != b.mms || a.id != b.id || a.date == b.date || old.direction != new.direction {
        return false;
    }
    let (was, is) = (Print::of(old), Print::of(new));
    let people = |p: &Print| {
        let mut people: Vec<String> = p.people.iter().map(|w| w.trim().to_string()).collect();
        people.sort_unstable();
        people.dedup();
        people
    };
    if people(&was) != people(&is) {
        return false;
    }
    if a.mms {
        let hashes = |t: &Text| t.parts.iter().filter(|p| !p.hash.is_empty()).map(|p| p.hash.clone()).collect::<BTreeSet<String>>();
        hashes(old).is_subset(&hashes(new)) && was.words.iter().all(|w| is.words.contains(w)) && was.media.iter().all(|m| is.media.contains(m))
    } else {
        old.body == new.body && (old.direction == "out" || (b.date - a.date).abs() <= MOVED_WITHIN_MS)
    }
}

/// The marks of the lines whose row its provider re-dated (`moved`): each
/// line of a phone's log not marked yet whose row is no longer listed, while
/// its row id is listed under another date with a line of the log holding
/// the same message (the newest such): a deletion's line naming it
/// (`Text::moved`), never "deleted on your phone".
pub fn moved_marks(log: &[Text], listed: &BTreeSet<Row>, now: i64) -> Vec<Text> {
    let marked: BTreeSet<&str> = log.iter().filter(|t| t.deleted > 0).map(|t| t.id.as_str()).collect();
    let mut by_id: BTreeMap<(bool, i64), Vec<(&Text, Row)>> = BTreeMap::new();
    for t in log.iter().filter(|t| t.deleted == 0) {
        if let Some(row) = t.row().filter(|r| listed.contains(r)) {
            by_id.entry((row.mms, row.id)).or_default().push((t, row));
        }
    }
    log.iter()
        .filter(|t| t.deleted == 0 && !marked.contains(t.id.as_str()))
        .filter_map(|old| {
            let row = old.row().filter(|r| !listed.contains(r))?;
            let new = by_id.get(&(row.mms, row.id))?.iter().filter(|(t, _)| moved(old, t)).max_by_key(|(_, r)| r.date).map(|(t, _)| *t)?;
            Some(Text { id: old.id.clone(), at: old.at, direction: old.direction.clone(), with: old.with.clone(), deleted: now, moved: new.id.clone(), ..Text::default() })
        })
        .collect()
}

/// A column's number, written as a number or as words.
pub fn number(value: Option<&Value>) -> i64 {
    value.and_then(|v| v.as_i64().or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))).unwrap_or(0)
}

/// Every phone's texts with each text once (docs/texts.md, "Each text
/// once"): a text found in several phones' logs, as after a restore onto a
/// new phone, once; a text there twice in one phone's log (truly sent
/// twice), twice: for each fingerprint (`Print::key`), as many as the phone
/// holding most, its copies paired with the others' in their logs' order.
/// The copy kept of each pair has the most media here, else is not deleted,
/// else is the first phone's by name; it is said deleted only when every
/// copy is, and names the others (`Text::also`). Oldest first, as
/// `read_logs`. For what the page, the search and agents read; the phones'
/// logs themselves keep every copy (nothing is lost if one goes).
pub fn one_each(texts: Vec<Text>, region: Option<&Region>) -> Vec<Text> {
    one_each_by(texts, &mut |t: &Text| Print::of(t).key(region))
}

/// `one_each` with each text's fingerprint as `print` gives it (`Print::key`
/// with the region of the numbers, kept by phone and row between two reads:
/// `Logs`).
pub fn one_each_by(texts: Vec<Text>, print: &mut dyn FnMut(&Text) -> String) -> Vec<Text> {
    let phones: BTreeSet<&str> = texts.iter().map(|t| t.device.as_str()).collect();
    if phones.len() < 2 {
        return texts;
    }
    // By fingerprint, then by phone: each copy's place, in its log's order.
    let mut groups: BTreeMap<String, BTreeMap<String, Vec<usize>>> = BTreeMap::new();
    for (i, t) in texts.iter().enumerate() {
        groups.entry(print(t)).or_default().entry(t.device.clone()).or_default().push(i);
    }
    let media = |t: &Text| t.parts.iter().filter(|p| p.state == "here" && !p.hash.is_empty()).count();
    let mut slots: Vec<Option<Text>> = texts.into_iter().map(Some).collect();
    let mut out: Vec<Text> = Vec::with_capacity(slots.len());
    for by_phone in groups.into_values() {
        let most = by_phone.values().map(Vec::len).max().unwrap_or(0);
        for k in 0..most {
            let copies: Vec<usize> = by_phone.values().filter_map(|list| list.get(k).copied()).collect();
            let best = copies
                .iter()
                .copied()
                .max_by(|&a, &b| {
                    let (a, b) = (slots[a].as_ref(), slots[b].as_ref());
                    let rank = |t: Option<&Text>| t.map(|t| (media(t), t.deleted == 0, !t.key.is_empty()));
                    // On a tie, the first phone's by name.
                    rank(a).cmp(&rank(b)).then_with(|| b.map(|t| &t.device).cmp(&a.map(|t| &t.device)))
                })
                .unwrap_or(copies[0]);
            let others: Vec<Text> = copies.iter().filter(|&&i| i != best).filter_map(|&i| slots[i].take()).collect();
            let Some(mut kept) = slots[best].take() else { continue };
            if others.iter().any(|t| t.deleted == 0) {
                kept.deleted = 0;
            } else if kept.deleted > 0 {
                kept.deleted = others.iter().map(|t| t.deleted).fold(kept.deleted, i64::max);
            }
            if kept.key.is_empty() {
                kept.key = others.iter().map(|t| t.key.clone()).find(|k| !k.is_empty()).unwrap_or_default();
            }
            kept.also.extend(others.iter().map(|t| format!("{}/{}", t.device, t.id)));
            out.push(kept);
        }
    }
    out.sort_by(|a, b| (a.at, &a.device, &a.id).cmp(&(b.at, &b.device, &b.id)));
    out
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
    /// A reply's conversation (`replies::claim_of`, its key): the same words
    /// to it are held back whichever of its notifications they go through.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub talk: String,
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
    /// Every key the claims hold (`has`), once: a step judges thousands of
    /// requests already decided without a search each.
    keys: std::collections::BTreeSet<String>,
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
        let claims: Vec<Claim> = std::fs::read_to_string(dir.join(LEDGER)).unwrap_or_default().lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
        let keys = claims.iter().map(|c| c.key.clone()).collect();
        Ok(Ledger { mark, claims, keys })
    }

    /// Whether a key was decided already, whatever was decided.
    pub fn has(&self, key: &str) -> bool {
        self.keys.contains(key)
    }

    /// A ledger of these lines and this mark, held in memory (tests, stand-ins).
    pub fn with(mark: Mark, claims: Vec<Claim>) -> Ledger {
        let keys = claims.iter().map(|c| c.key.clone()).collect();
        Ledger { mark, claims, keys }
    }

    /// A line added in memory, as the ledger will hold it once written (`record_all`).
    pub fn add(&mut self, claim: Claim) {
        self.keys.insert(claim.key.clone());
        self.claims.push(claim);
    }

    /// A key's last state.
    pub fn state(&self, key: &str) -> Option<&str> {
        self.claims.iter().rev().find(|c| c.key == key).map(|c| c.state.as_str())
    }
}

/// A line added to the ledger and flushed to the disk before this returns:
/// the claim exists before Android is asked to send.
pub fn record(dir: &Path, claim: &Claim) -> Result<(), String> {
    record_all(dir, std::slice::from_ref(claim))
}

/// Lines added to the ledger together and flushed once before this returns
/// (a step's decisions: one flush, not one each); nothing for none.
pub fn record_all(dir: &Path, claims: &[Claim]) -> Result<(), String> {
    if claims.is_empty() {
        return Ok(());
    }
    let mut text = String::new();
    for claim in claims {
        text.push_str(&serde_json::to_string(claim).map_err(|e| e.to_string())?);
        text.push('\n');
    }
    write_flushed(&dir.join(LEDGER), &text, true)
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

/// The key of the request a text is, by the row its phone filed it as
/// (`Outcome::row`): its own row or one of its copies' (`Text::also`), each
/// on its own phone, never another phone's row of the same number. An
/// outcome that names no phone matches the row on any (as before phones were
/// two). A row is its name with its date (`Row::name`); an outcome written
/// before 16 October 2026 names it by its id alone, and matches that id's
/// text only from when its request could be sent (`from`) to a little after
/// the outcome itself (Android files the row before its result comes:
/// `AHEAD_MS`), never an older text whose id the text sent took again, nor
/// a text received days later under that id (review of 16 October 2026, LOW 5).
fn row_key(t: &Text, key_of_row: &BTreeMap<String, Vec<(&str, &str, i64, i64)>>) -> String {
    let own = (t.device.as_str(), t.id.as_str());
    std::iter::once(own)
        .chain(t.also.iter().filter_map(|copy| copy.split_once('/')))
        .find_map(|(phone, id)| {
            let row = Row::of(id, t.at)?;
            let bare = format!("{}-{}", if row.mms { "mms" } else { "sms" }, row.id);
            [row.name(), bare].iter().find_map(|name| key_of_row.get(name)?.iter().find(|(device, _, from, until)| (device.is_empty() || *device == phone) && t.at >= *from && t.at <= *until).map(|(_, key, _, _)| key.to_string()))
        })
        .unwrap_or_default()
}

/// A conversation's messages, oldest first: its texts, deleted ones said, and
/// the texts you wrote to it from a computer with their states; a request
/// Android filed is said on its row, once. A row is its phone's own: another
/// phone's row of the same number is another text (`row_key`).
pub fn messages(id: &str, texts: &[Text], requests: &[Request], outcomes: &[Outcome], v: &Viewer) -> Vec<Message> {
    // A text Android filed for a request is dated no earlier than the
    // request's time on the phone's clock, less what `verdict` lets pass.
    let from: BTreeMap<&str, i64> = requests.iter().map(|r| (r.key.as_str(), r.written - r.ahead - AHEAD_MS)).collect();
    let mut key_of_row: BTreeMap<String, Vec<(&str, &str, i64, i64)>> = BTreeMap::new();
    for o in outcomes.iter().filter(|o| !o.row.is_empty()) {
        let named = o.row.contains('@');
        let (since, until) = if named { (i64::MIN, i64::MAX) } else { (from.get(o.key.as_str()).copied().unwrap_or(i64::MIN), o.at.saturating_add(AHEAD_MS)) };
        let name = if named { Row::of(&o.row, 0).map_or_else(|| o.row.clone(), |r| r.name()) } else { o.row.clone() };
        key_of_row.entry(name).or_default().push((o.device.as_str(), o.key.as_str(), since, until));
    }
    let mut out: Vec<Message> = Vec::new();
    let mut shown_keys = BTreeSet::new();
    for t in texts.iter().filter(|t| conversation_id(&t.with) == id) {
        let key = if t.key.is_empty() { row_key(t, &key_of_row) } else { t.key.clone() };
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
                record(dir, &Claim { key: r.key.clone(), at: now, state: "claimed".into(), to: phones::key(&r.to, fr()), hash: words_hash(&r.body), ..Claim::default() }).unwrap();
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
        let ledger = Ledger::with(Mark { since: 0 }, Vec::new());
        let with = |to: &str, body: &str| verdict(&Request { to: to.into(), body: body.into(), ..request("e", "phone", NOW) }, "phone", NOW, &ledger, fr());
        assert_eq!(with("36179", "STOP"), Verdict::Refused("short-number"));
        assert_eq!(with("01 99 00 12 34, 04 65 71 00 42", "Bonjour"), Verdict::Refused("several"));
        assert_eq!(with("01 99 00 12 34", "   "), Verdict::Refused("empty"));
        assert_eq!(with("01 99 00 12 34", &"x".repeat(WRITE_MAX + 1)), Verdict::Refused("too-long"));
        assert_eq!(with("+33 1 99 00 12 34", "Bonjour"), Verdict::Send);
    }

    #[test]
    fn a_crash_between_the_claim_and_the_result_is_said_never_sent_again() {
        let claim = Claim { key: "f1".into(), at: NOW, state: "claimed".into(), to: "+33199001234".into(), hash: words_hash("Bonjour"), ..Claim::default() };
        assert!(doubt_after(&claim, None, NOW + MINUTE).is_none(), "Android's word may still come");
        let found = doubt_after(&claim, Some("sms-412".into()), NOW + 3 * MINUTE).unwrap();
        assert_eq!((found.state.as_str(), found.row.as_str()), ("sent", "sms-412"));
        assert_eq!(doubt_after(&claim, None, NOW + 3 * MINUTE).unwrap().state, "doubt");
        let ledger = Ledger::with(Mark { since: 0 }, vec![claim]);
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

    /// A text of a phone's log with its provider columns, as Java reads them.
    fn row(phone: &str, id: &str, at: i64, direction: &str, address: &str, body: &str) -> Text {
        let fields = [("address", serde_json::json!(address)), ("body", serde_json::json!(body)), ("date", serde_json::json!(at))].into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        Text { id: id.into(), at, thread: "7".into(), direction: direction.into(), with: vec![phones::key(address, fr())], body: body.into(), sub: 1, fields, device: phone.into(), ..Text::default() }
    }

    /// A multimedia message's address row, as the provider gives it.
    fn addr(address: &str, kind: i64) -> BTreeMap<String, Value> {
        [("address", serde_json::json!(address)), ("type", serde_json::json!(kind)), ("charset", serde_json::json!(106))].into_iter().map(|(k, v)| (k.to_string(), v)).collect()
    }

    /// A courier's multimedia message with its picture (here, or over the
    /// phone's size), its words, and this phone's own address as given.
    fn picture_message(phone: &str, id: &str, at: i64, own: &str, state: &str) -> Text {
        let part = |seq: i64, ct: &str, text: &str, hash: &str, state: &str| Part { seq, ct: ct.into(), text: text.into(), hash: hash.into(), state: state.into(), size: if text.is_empty() { 220_000 } else { 0 }, ..Part::default() };
        let picture = if state == "here" { "aa".repeat(32) } else { String::new() };
        Text {
            id: id.into(),
            at,
            thread: "9".into(),
            direction: "in".into(),
            with: vec!["+33465710042".into()],
            body: "Le colis.".into(),
            picture: true,
            parts: vec![part(-1, "application/smil", "<smil/>", "", "text"), part(0, "image/jpeg", "", &picture, state), part(1, "text/plain", "Le colis.", "", "text")],
            addrs: vec![addr("+33465710042", ADDR_FROM), addr(own, ADDR_TO)],
            device: phone.into(),
            ..Text::default()
        }
    }

    /// Two phones' logs as `read_logs` gives them: oldest first.
    fn logs(mut all: Vec<Text>) -> Vec<Text> {
        all.sort_by(|a, b| (a.at, &a.device, &a.id).cmp(&(b.at, &b.device, &b.id)));
        all
    }

    /// The old phone's history, and the new phone's after SMS Backup &
    /// Restore put it back there (other rows, other conversations; a picture
    /// over the new phone's size; a text deleted on the old phone, there
    /// again), with texts of the new phone's own: one row number the same as
    /// an old one's, for another text.
    fn two_phones(old: &str, new: &str) -> (Vec<Text>, Vec<Text>) {
        let second = |s: i64| NOW - 2 * DAY_MS + s * 1000;
        let at = NOW - 3 * DAY_MS + 123;
        let old_log = vec![
            row(old, "sms-10", at, "in", "01 99 00 12 34", "Le rendez-vous est à 9 h."),
            // The same words sent twice within one second: two texts.
            row(old, "sms-11", second(0) + 100, "out", "0199001234", "OK"),
            row(old, "sms-12", second(0) + 400, "out", "0199001234", "OK"),
            picture_message(old, "mms-3", second(60), "+33536490000", "here"),
            Text { deleted: NOW - DAY_MS, ..row(old, "sms-13", second(120), "in", "+33465710042", "Votre colis arrive demain.") },
        ];
        let new_log = vec![
            Text { thread: "2".into(), ..row(new, "sms-501", at, "in", "01 99 00 12 34", "Le rendez-vous est à 9 h.") },
            row(new, "sms-502", second(0) + 100, "out", "0199001234", "OK"),
            row(new, "sms-503", second(0) + 400, "out", "0199001234", "OK"),
            picture_message(new, "mms-77", second(60), OWN_ADDRESS, "too-big"),
            row(new, "sms-504", second(120), "in", "+33465710042", "Votre colis arrive demain."),
            row(new, "sms-600", NOW - MINUTE, "in", "0199001234", "À tout à l'heure."),
            row(new, "sms-11", second(30), "out", "0199001234", "OK"),
        ];
        (old_log, new_log)
    }

    /// The matching rule: what a restore keeps (the way, the second, the
    /// other people however written, the words, the media's types), never the
    /// rows, the conversations, this phone's own address, nor a character XML
    /// cannot carry.
    #[test]
    fn the_matching_rule_is_what_a_restore_keeps() {
        let key = |t: &Text| Print::of(t).key(fr());
        let text = row("a", "sms-1", NOW + 250, "in", "01 99 00 12 34", "Bonjour");
        assert_eq!(key(&text), key(&Text { id: "sms-900".into(), thread: "41".into(), device: "b".into(), ..row("b", "sms-900", NOW + 900, "in", "+33 1 99 00 12 34", "Bonjour") }), "another row and number form, the same second");
        for other in [row("a", "sms-1", NOW + 1000, "in", "01 99 00 12 34", "Bonjour"), row("a", "sms-1", NOW, "out", "01 99 00 12 34", "Bonjour"), row("a", "sms-1", NOW, "in", "01 99 00 12 35", "Bonjour"), row("a", "sms-1", NOW, "in", "01 99 00 12 34", "Bonjour !")] {
            assert_ne!(key(&text), key(&other), "{other:?}");
        }
        assert_eq!(key(&row("a", "sms-1", NOW, "in", "0199001234", "Page\u{c}break")), key(&row("b", "sms-2", NOW, "in", "0199001234", "Pagebreak")), "a form feed no backup can hold");
        // A multimedia message: this phone's own address as a token or as its number; the media's types in any case.
        let picture = picture_message("a", "mms-3", NOW, "+33536490000", "here");
        let restored = picture_message("b", "mms-77", NOW, OWN_ADDRESS, "too-big");
        assert_eq!(key(&picture), key(&restored), "here on one phone, too large on the other: the same");
        let upper = Text { parts: picture.parts.iter().map(|p| if p.ct == "image/jpeg" { Part { ct: "IMAGE/JPEG".into(), ..p.clone() } } else { p.clone() }).collect(), ..picture.clone() };
        assert_eq!(key(&upper), key(&picture), "a type's case aside");
        let other_words = Text { parts: picture.parts.iter().map(|p| if p.ct == "text/plain" { Part { text: "Le colis est là.".into(), ..p.clone() } } else { p.clone() }).collect(), ..picture.clone() };
        assert_ne!(key(&other_words), key(&picture));
        assert_eq!(Print::of(&picture).people, ["+33465710042"], "received: its sender");
        // Sent: those it went to, this phone's own address aside however written.
        let sent = |own: &str| Text { direction: "out".into(), addrs: vec![addr(own, ADDR_FROM), addr("04 65 71 00 42", ADDR_TO), addr("+33199001234", ADDR_CC)], ..picture.clone() };
        assert_eq!(Print::of(&sent(OWN_ADDRESS)).people, ["04 65 71 00 42", "+33199001234"]);
        assert_eq!(key(&sent(OWN_ADDRESS)), key(&sent("+33536490000")));
        assert_ne!(key(&picture), key(&sent(OWN_ADDRESS)), "sent, it is another");
        let backup_null = Text { parts: [picture.parts.clone(), vec![Part { seq: 2, ct: "text/plain".into(), text: "null".into(), state: "text".into(), ..Part::default() }]].concat(), ..picture.clone() };
        assert_eq!(key(&backup_null), key(&picture), "a text part SMS Backup & Restore wrote \"null\": none");
    }

    /// The same history in two phones' logs, either way round: each text
    /// once, a text truly sent twice twice, the copy with its picture kept,
    /// deleted only when every copy is; what was sent from a computer said on
    /// its own phone's row, never on another phone's row of the same number.
    #[test]
    fn a_text_in_two_phones_logs_shows_once_and_one_sent_twice_stays_twice() {
        for (old, new) in [("old-phone", "new-phone"), ("zz-old", "new-phone")] {
            let (old_log, new_log) = two_phones(old, new);
            assert_eq!(one_each(logs(old_log.clone()), fr()), logs(old_log.clone()), "one phone: as it is");
            let shown = one_each(logs([old_log.clone(), new_log.clone()].concat()), fr());
            assert_eq!(shown.len(), 7, "{old}: {:?}", shown.iter().map(|t| format!("{}/{}", t.device, t.id)).collect::<Vec<_>>());
            let twice: Vec<&Text> = shown.iter().filter(|t| t.body == "OK" && t.at < NOW - 2 * DAY_MS + 1000).collect();
            assert_eq!(twice.len(), 2, "sent twice, twice");
            assert!(twice.iter().all(|t| t.also.len() == 1));
            let mms = shown.iter().find(|t| t.id.starts_with("mms-")).unwrap();
            assert_eq!((mms.device.as_str(), mms.parts[1].state.as_str()), (old, "here"), "the copy with its picture");
            assert_eq!(mms.also, [format!("{new}/mms-77")]);
            let deleted = shown.iter().find(|t| t.body.starts_with("Votre colis")).unwrap();
            assert_eq!(deleted.deleted, 0, "still on the new phone");
            assert_eq!(shown.iter().filter(|t| t.body == "OK").count(), 3, "the new phone's own OK, another second");
            // Deleted on both: said so.
            let both_gone = one_each(logs(vec![Text { deleted: NOW - DAY_MS, ..row(old, "sms-1", NOW, "in", "0199001234", "x") }, Text { deleted: NOW - MINUTE, ..row(new, "sms-9", NOW, "in", "0199001234", "x") }]), fr());
            assert_eq!(both_gone.iter().map(|t| t.deleted).collect::<Vec<_>>(), [NOW - MINUTE]);
            // Twice in one log, once in the other: twice.
            let once_there = one_each(logs([old_log.clone(), vec![row(new, "sms-502", NOW - 2 * DAY_MS + 100, "out", "0199001234", "OK")]].concat()), fr());
            assert_eq!(once_there.iter().filter(|t| t.body == "OK").count(), 2);
        }
        // The page: one conversation each; a text sent from here said on its own phone's row.
        let (old_log, new_log) = two_phones("old-phone", "new-phone");
        let shown = one_each(logs([old_log, new_log].concat()), fr());
        let tr = Translator::new("en");
        let now = paris(NOW);
        let names = |_: &str| None;
        let here = |_: &str| true;
        let v = viewer(&now, &tr, &names, &here);
        let requests = [Request { to: "0199001234".into(), body: "OK".into(), ..request("k1", "old-phone", NOW - 2 * DAY_MS - 30_000) }];
        let outcomes = [Outcome { key: "k1".into(), state: "sent".into(), at: NOW - 2 * DAY_MS + 100, row: "sms-11".into(), device: "old-phone".into(), ..Outcome::default() }];
        assert_eq!(conversations(&shown, &requests, &v).len(), 2);
        let thread = messages("+33199001234", &shown, &requests, &outcomes, &v);
        assert_eq!(thread.len(), 5, "{:?}", thread.iter().map(|m| (&m.id, &m.body, &m.key)).collect::<Vec<_>>());
        let sent: Vec<&Message> = thread.iter().filter(|m| m.key == "k1").collect();
        assert_eq!(sent.len(), 1, "the request said once, on its row");
        assert!(sent[0].id.starts_with("new-phone/sms-50") && sent[0].said.as_ref().is_some_and(|s| s.state == "sent"), "{:?}", sent[0]);
        assert!(thread.iter().any(|m| m.id == "new-phone/sms-11" && m.key.is_empty()), "another phone's row of that number: another text");
        assert_eq!(search("rendez", &shown, &requests, &outcomes, &v, 10).len(), 1);
    }

    /// A row's name: the provider's id with the row's own date, so that a
    /// text taking a deleted newest text's id (the plain texts' table has no
    /// AUTOINCREMENT) is a line of its own; a line named before (`sms-12`)
    /// reads its date from its time; a request's state goes on its own row.
    #[test]
    fn a_row_is_its_id_with_its_date() {
        let first = Row { mms: false, id: 12, date: 1_791_446_340_000 };
        assert_eq!(first.name(), "sms-12@1791446340000");
        assert_eq!(Row::of("sms-12@1791446340000", 0), Some(first));
        assert_eq!(Row::of("sms-12", 1_791_446_340_000), Some(first), "a line named before: its date from its time");
        let picture = Row { mms: true, id: 31, date: 1_791_446_340 };
        assert_eq!((Row::of("mms-31", 1_791_446_340_000), Row::of(&picture.name(), 0), picture.at()), (Some(picture), Some(picture), 1_791_446_340_000));
        assert_ne!(Row::of("sms-12@1791446400000", 0), Some(first), "the same id, another text");
        assert_eq!((Row::of("request/abc", 0), Row::of("sms-x", 0), Row::of("sms-12@x", 0)), (None, None, None));
        // Two texts of one id: each its line; the first's deletion its own.
        let second = Row { mms: false, id: 12, date: 1_791_446_400_000 };
        let line = |row: Row, deleted: i64| Text { id: row.name(), at: row.at(), device: "p".into(), deleted, with: vec!["+33199001234".into()], ..Text::default() };
        let folded = fold_logs(vec![line(first, 0), line(second, 0), line(first, 1_791_446_500_000)]);
        assert_eq!(folded.iter().map(|t| (t.id.as_str(), t.deleted > 0)).collect::<Vec<_>>(), [("sms-12@1791446340000", true), ("sms-12@1791446400000", false)]);
        // The state of a text sent from here names its own row, never the older text of that id.
        let tr = Translator::new("en");
        let now = paris(NOW);
        let names = |_: &str| None;
        let here = |_: &str| true;
        let v = viewer(&now, &tr, &names, &here);
        let requests = [request("k9", "p", 1_791_446_390_000)];
        let outcomes = [Outcome { key: "k9".into(), state: "sent".into(), at: 1_791_446_400_000, row: second.name(), device: "p".into(), ..Outcome::default() }];
        let thread = messages("+33199001234", &folded, &requests, &outcomes, &v);
        assert_eq!(thread.iter().map(|m| (m.id.as_str(), m.key.as_str())).collect::<Vec<_>>(), [("p/sms-12@1791446340000", ""), ("p/sms-12@1791446400000", "k9")]);
        // An outcome written before (its row by its id alone): never the older text of that id
        // (named before too), whether the text sent is read yet or not.
        let late = Row { mms: false, id: 12, date: 1_791_446_530_000 };
        let requests = [request("k8", "p", 1_791_446_520_000)];
        let bare = [Outcome { key: "k8".into(), state: "sent".into(), at: 1_791_446_535_000, row: "sms-12".into(), device: "p".into(), ..Outcome::default() }];
        let older = Text { id: "sms-12".into(), ..line(first, 0) };
        let thread = messages("+33199001234", &fold_logs(vec![older.clone(), line(late, 0)]), &requests, &bare, &v);
        assert_eq!(thread.iter().map(|m| (m.id.as_str(), m.key.as_str())).collect::<Vec<_>>(), [("p/sms-12", ""), ("p/sms-12@1791446530000", "k8")]);
        let thread = messages("+33199001234", &fold_logs(vec![older]), &requests, &bare, &v);
        assert!(thread.iter().find(|m| m.id == "p/sms-12").is_some_and(|m| m.key.is_empty()), "the text sent not read yet: not the older one's");
    }

    /// The logs read again only where they grew: the lines added read alone,
    /// a line being written left for the next look, a file put in another's
    /// place or cut short read again whole, a phone's file gone taken out;
    /// each time as `read_logs` and `one_each` give them.
    #[test]
    fn the_logs_are_read_again_only_where_they_grew() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        /// The stand-in sealer, counting the lines it opens.
        struct Counted(AtomicUsize);
        impl Sealer for Counted {
            fn seal(&self, plain: &str) -> String {
                Stand.seal(plain)
            }
            fn open(&self, sealed: &str) -> Option<String> {
                self.0.fetch_add(1, Ordering::Relaxed);
                Stand.open(sealed)
            }
        }
        let dir = scratch("logs");
        let seal = Counted(AtomicUsize::new(0));
        let opened = || seal.0.swap(0, Ordering::Relaxed);
        let text = |id: i64, body: &str| Text { id: Row { mms: false, id, date: NOW + id }.name(), at: NOW + id, direction: "in".into(), with: vec!["+33199001234".into()], body: body.into(), ..Text::default() };
        let (a, b) = (own_file(&dir, LOG, "a"), own_file(&dir, LOG, "b"));
        append(&a, &[text(1, "Un."), text(2, "Deux.")], &seal, None).unwrap();
        // A text as Java reads it, with its raw columns: the fingerprint made of them, the columns not kept.
        let mut raw = text(8, "Deux.");
        raw.fields.insert("address".into(), Value::from("04 65 71 00 42"));
        append(&b, &[text(7, "Un."), raw], &seal, None).unwrap();
        opened();
        let mut logs = Logs::default();
        let bare = |texts: Vec<Text>| texts.into_iter().map(|t| Text { fields: BTreeMap::new(), addrs: Vec::new(), ..t }).collect::<Vec<_>>();
        let same = |logs: &mut Logs| {
            let n = seal.0.load(Ordering::Relaxed);
            assert_eq!(logs.texts(), bare(read_logs(&dir, &seal)));
            assert_eq!(logs.one_each(fr()), bare(one_each(read_logs(&dir, &seal), fr())));
            seal.0.store(n, Ordering::Relaxed);
        };
        assert!(logs.refresh(&dir, &seal));
        assert_eq!(opened(), 4);
        assert!(logs.texts().iter().all(|t| t.fields.is_empty() && t.addrs.is_empty()), "no raw columns kept");
        // The same texts and fingerprints as a whole read, the raw columns aside.
        same(&mut logs);
        assert!(!logs.refresh(&dir, &seal), "nothing changed");
        assert_eq!(opened(), 0);
        // Two lines added, then half a line being written: the two alone, then the rest.
        append(&a, &[text(3, "Trois."), Text { deleted: NOW + 9, ..text(1, "") }], &seal, None).unwrap();
        let line = format!("{}\n", seal.seal(&serde_json::to_string(&text(4, "Quatre.")).unwrap()));
        let (half, rest) = line.split_at(line.len() / 2);
        calls::append_private(&a, half).unwrap();
        assert!(logs.refresh(&dir, &seal));
        assert_eq!(opened(), 2, "the lines added, alone");
        same(&mut logs);
        calls::append_private(&a, rest).unwrap();
        assert!(logs.refresh(&dir, &seal));
        assert_eq!(opened(), 1);
        same(&mut logs);
        assert_eq!(logs.texts().iter().filter(|t| t.device == "a").count(), 4);
        // Another file put in its place, longer; then cut short: read again whole.
        std::fs::remove_file(&a).unwrap();
        append(&a, &(10..16).map(|id| text(id, "Autre.")).collect::<Vec<_>>(), &seal, None).unwrap();
        assert!(logs.refresh(&dir, &seal));
        assert_eq!(opened(), 6);
        same(&mut logs);
        std::fs::remove_file(&a).unwrap();
        append(&a, &[text(20, "Court.")], &seal, None).unwrap();
        assert!(logs.refresh(&dir, &seal));
        same(&mut logs);
        // The same size, other words, its time put back (rsync -t, a restore): read again all the same, by
        // its inode and change time, which Unix alone gives (on Windows, once its size or time changes).
        #[cfg(unix)]
        {
            let modified = std::fs::metadata(&a).unwrap().modified().unwrap();
            let body = std::fs::read(&a).unwrap();
            std::fs::remove_file(&a).unwrap();
            append(&a, &[text(20, "Kourt.")], &seal, None).unwrap();
            assert_eq!(std::fs::read(&a).unwrap().len(), body.len(), "the same size");
            std::fs::File::options().write(true).open(&a).unwrap().set_modified(modified).unwrap();
            assert!(logs.refresh(&dir, &seal), "another file, though its size and time are the old one's");
            same(&mut logs);
            assert!(logs.texts().iter().any(|t| t.body == "Kourt."));
        }
        // A phone's file gone: its texts too.
        std::fs::remove_file(&b).unwrap();
        assert!(logs.refresh(&dir, &seal));
        same(&mut logs);
        assert!(logs.texts().iter().all(|t| t.device == "a"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Each row once on a computer: a line named by its id alone and the
    /// same row named with its date are one text (a phone put back on an
    /// older Sioul writes again under the old names: MEDIUM 1); a row its
    /// provider re-dated is one text, its old name beside it, so that the
    /// request it was still finds it (L-4); a reused id with the same words,
    /// received hours later, is another text, the older one said deleted
    /// (L-3); an outcome named by its id alone binds no text received after
    /// it (LOW 5).
    #[test]
    fn each_row_is_one_text_on_a_computer() {
        const HOUR: i64 = 60 * MINUTE;
        let tr = Translator::new("en");
        let now = paris(NOW);
        let names = |_: &str| None;
        let here = |_: &str| true;
        let v = viewer(&now, &tr, &names, &here);
        let line = |id: &str, at: i64, body: &str| Text { id: id.into(), at, direction: "in".into(), with: vec!["+33199001234".into()], body: body.into(), device: "phone".into(), ..Text::default() };
        // MEDIUM 1: the same row under its old name and its name now, a deletion under the old name.
        let t5 = NOW - 9 * HOUR;
        let t7 = NOW - 8 * HOUR;
        let lines = vec![line("sms-5", t5, "Bonjour."), line(&format!("sms-5@{t5}"), t5, "Bonjour."), line(&format!("sms-7@{t7}"), t7, "Au revoir."), Text { deleted: NOW - HOUR, ..line("sms-7", t7, "") }];
        let folded = fold_logs(lines);
        assert_eq!(folded.iter().map(|t| (t.body.as_str(), t.deleted > 0)).collect::<Vec<_>>(), [("Bonjour.", false), ("Au revoir.", true)]);
        // L-4: a text sent from a computer (r1, its outcome naming its first row), then re-dated: one text, bound to r1.
        let t9 = NOW - 2 * HOUR;
        let first = Text { direction: "out".into(), ..line(&format!("sms-9@{t9}"), t9, "J'arrive.") };
        let then = Text { id: format!("sms-9@{}", t9 + 1), at: t9 + 1, ..first.clone() };
        let mark = Text { id: first.id.clone(), at: first.at, deleted: NOW - HOUR, moved: then.id.clone(), direction: "out".into(), with: first.with.clone(), device: "phone".into(), ..Text::default() };
        let folded = fold_logs(vec![first.clone(), then.clone(), mark]);
        assert_eq!(folded.len(), 1);
        assert_eq!((folded[0].id.as_str(), folded[0].also.clone()), (then.id.as_str(), vec![format!("phone/{}", first.id)]));
        let request = Request { key: "r1".into(), phone: "phone".into(), to: "+33199001234".into(), body: "J'arrive.".into(), written: t9 - 1_000, ..Request::default() };
        let sent = Outcome { key: "r1".into(), state: "sent".into(), at: t9 + 5_000, row: first.id.clone(), device: "phone".into(), ..Outcome::default() };
        let shown = messages(&conversation_id(&first.with), &folded, std::slice::from_ref(&request), std::slice::from_ref(&sent), &v);
        assert_eq!(shown.iter().map(|m| (m.id.clone(), m.key.clone())).collect::<Vec<_>>(), [(format!("phone/{}", then.id), "r1".to_string())], "one text, the request found by its old name");
        // L-3: Alice's "Oui." deleted, her next "Oui." hours later under its id: another text; a millisecond later, or sent from the phone: the same.
        let oui = line(&format!("sms-11@{}", NOW - 6 * HOUR), NOW - 6 * HOUR, "Oui.");
        let later = line(&format!("sms-11@{}", NOW - 3 * HOUR), NOW - 3 * HOUR, "Oui.");
        assert!(!moved(&oui, &later), "a reused id, hours later: another text");
        assert!(moved(&oui, &line(&format!("sms-11@{}", NOW - 6 * HOUR + 1), NOW - 6 * HOUR + 1, "Oui.")), "re-dated by a millisecond");
        let out = |t: &Text| Text { direction: "out".into(), ..t.clone() };
        assert!(moved(&out(&oui), &out(&later)), "sent from the phone: its provider re-dates its own sending");
        let picture = |hash: &str| Text { id: format!("mms-3@{}", (NOW - 7 * HOUR) / 1000), at: NOW - 7 * HOUR, direction: "in".into(), with: vec!["+33199001234".into()], parts: vec![Part { ct: "image/jpeg".into(), hash: hash.into(), state: "here".into(), ..Part::default() }], device: "phone".into(), ..Text::default() };
        let again = |hash: &str| Text { id: format!("mms-3@{}", (NOW - 7 * HOUR) / 1000 + 1), at: NOW - 7 * HOUR + 1_000, ..picture(hash) };
        assert!(moved(&picture("aa11"), &again("aa11")), "the same picture re-dated");
        assert!(!moved(&picture("aa11"), &again("bb22")), "another picture under a reused id");
        assert!(moved(&picture(""), &again("bb22")), "read before its parts were attached: comes whole");
        // LOW 5: an outcome of before 16 October naming its row by its id alone binds no text received days after it.
        let bare = Outcome { key: "r2".into(), state: "sent".into(), at: NOW - 5 * HOUR, row: "sms-12".into(), device: "phone".into(), ..Outcome::default() };
        let asked = Request { key: "r2".into(), phone: "phone".into(), to: "+33199001234".into(), body: "Merci.".into(), written: NOW - 5 * HOUR - 30_000, ..Request::default() };
        let filed = Text { direction: "out".into(), ..line("sms-12", NOW - 5 * HOUR - 10_000, "Merci.") };
        let reused = line("sms-12", NOW + 3 * DAY_MS, "Autre.");
        let bound = |t: &Text| messages(&conversation_id(&t.with), std::slice::from_ref(t), std::slice::from_ref(&asked), std::slice::from_ref(&bare), &v).into_iter().find(|m| m.id.starts_with("phone/")).map(|m| m.key).unwrap_or_default();
        assert_eq!(bound(&filed), "r2", "the text it sent, filed before its result");
        assert_eq!(bound(&reused), "", "a text received days later under its id");
    }
}
