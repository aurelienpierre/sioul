// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Texts read and sent through your phone, as this device does its part
//! (docs/texts.md; the rules are `sioul_core::texts`, the reading and the
//! sending Java's: android/…/Texts.java, TextSend.java).
//!
//! - **On the phone, at each exchange** (`light_step`, under one lock for its
//!   processes): Android's word on each text sent copied into its outcomes;
//!   each request decided (`texts::verdict`), claimed in the private ledger,
//!   flushed, before Java hands it to Android; a claim with no word from
//!   Android looked for among the texts Sioul sent, else said in doubt.
//! - **On the phone, never inside an exchange** (`heavy_step`: after the
//!   background service's exchange, or in a thread of the window's): the
//!   whole history read a batch at a time (`BATCH` texts, `MEDIA_BATCH`
//!   bytes of media put as sealed blobs), every column kept, while the phone
//!   charges; then what the phone's texts changed, when they did
//!   (`import_now`); the texts deleted on the phone marked so, once a day;
//!   the sizes the settings say.
//! - **On a computer**: the Texts page (`view`, `conversation`), a text
//!   written for the phone to send (`send`, `again`); the media's blobs
//!   brought and kept sealed (`fetch_media`, in a thread, never inside an
//!   exchange); how far this computer's clock may run ahead of the phone's.
//! - Every line, and every media file kept here, sealed at rest with a key
//!   made from the sharing key (`sioul_sync::textseal`): no sharing key here,
//!   nothing is read or written.

use crate::backend::{load_config, tr};
use jiff::Zoned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sioul_core::config::{Config, config_dir, state_dir};
use sioul_core::phones;
use sioul_core::porch::Senders;
use sioul_core::texts::{self as rules, Claim, Outcome, Part, Request, Text, Verdict};
use sioul_sync::textseal::TextSeal;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

/// Whether this device reads and sends texts: a phone. SIOUL_TEXTS makes a computer's step run too, to look at it.
fn phone() -> bool {
    cfg!(target_os = "android") || std::env::var_os("SIOUL_TEXTS").is_some()
}

/// The demo profile (`SIOUL_DEMO=1`, which tools/demo/run.sh sets in its
/// sandbox with no network, and nothing else does): its texts are stand-ins
/// written plain by tools/demo/make-demo.py, its phone a stand-in.
fn demo() -> bool {
    std::env::var_os("SIOUL_DEMO").is_some()
}

/// What seals the lines and the media: the sharing key's, or the demo's stand-in (plain; a line marked).
enum Seal {
    Key(TextSeal),
    Demo,
}

impl rules::Sealer for Seal {
    fn seal(&self, plain: &str) -> String {
        match self {
            Seal::Key(seal) => seal.seal(plain),
            Seal::Demo => format!("demo:{plain}"),
        }
    }

    fn open(&self, sealed: &str) -> Option<String> {
        match self {
            Seal::Key(seal) => seal.open(sealed),
            Seal::Demo => sealed.strip_prefix("demo:").map(str::to_string),
        }
    }
}

impl Seal {
    fn seal_bytes(&self, plain: &[u8]) -> Vec<u8> {
        match self {
            Seal::Key(seal) => seal.seal_bytes(plain),
            Seal::Demo => plain.to_vec(),
        }
    }

    fn open_bytes(&self, sealed: &[u8]) -> Option<Vec<u8>> {
        match self {
            Seal::Key(seal) => seal.open_bytes(sealed),
            Seal::Demo => Some(sealed.to_vec()),
        }
    }
}

/// The sharing's folder and key: where the media's blobs go and come from.
type Vault = (PathBuf, [u8; 32]);

/// This device's name in the sharing, the sealer, and the sharing's folder
/// and key, while the part "texts" is on here (the demo: no folder).
fn sealed() -> Option<(String, Seal, Option<Vault>)> {
    if demo() && !phone() {
        return Some(("demo-desk".into(), Seal::Demo, None));
    }
    let (here, vault) = crate::share::vault()?;
    let (folder, key) = vault?;
    crate::share::shares(rules::PART).then(|| (here, Seal::Key(TextSeal::new(&key)), Some((folder, key))))
}

fn root() -> PathBuf {
    rules::folder()
}

fn private() -> PathBuf {
    rules::private_folder()
}

fn now_ms() -> i64 {
    jiff::Timestamp::now().as_millisecond()
}

/// A private file written whole beside, then put in place.
fn write_whole(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let fail = |e: std::io::Error| format!("{}: {e}", path.display());
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(fail)?;
    }
    let fresh = path.with_extension(format!("{}.new", std::process::id()));
    std::fs::write(&fresh, bytes).map_err(fail)?;
    std::fs::rename(&fresh, path).map_err(|e| {
        let _ = std::fs::remove_file(&fresh);
        fail(e)
    })
}

// ---------------------------------------------------------------- this device's choices

/// This device's own texts settings (`texts.toml` in the configuration, never
/// shared): on a computer, the phone that sends; on a phone, the largest
/// media file it brings to your computers (MB; none: `MEDIA_CAP_MB`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Choice {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    phone: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    media_cap_mb: Option<u64>,
}

fn choice_path() -> PathBuf {
    config_dir().join("texts.toml")
}

fn choice() -> Choice {
    std::fs::read_to_string(choice_path()).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default()
}

/// The sizes offered for the largest media file (MB): the blobs take 64 MB at most.
const CAPS_MB: [u64; 5] = [1, 5, 10, 25, 50];

/// The largest media file brought to your computers, in MB.
fn cap_mb() -> u64 {
    choice().media_cap_mb.unwrap_or(rules::MEDIA_CAP_MB).clamp(1, sioul_sync::blobs::LARGEST >> 20)
}

// ---------------------------------------------------------------- the phone: the whole history

/// The reader's marks and the import's measures, private to the phone
/// (`texts/reader.json` in the state folder): the provider's last rows read,
/// when the import began, what the phone holds and how much has gone.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct Reader {
    /// `READER_VERSION`; 0, the first version's (30 days, before the archive).
    version: u32,
    sms: i64,
    mms: i64,
    started: i64,
    /// What the phone holds, as last measured (texts; media bytes), and when.
    total_texts: u64,
    total_media: u64,
    measured: i64,
    /// What has gone to your computers: texts written, media bytes put in the sharing.
    done_texts: u64,
    done_media: u64,
    /// When the deletions were last looked for.
    looked: i64,
    /// The last batch left texts to read: full, or a message's media that did
    /// not fit. The import goes on only while the phone charges (`import_now`).
    behind: bool,
}

/// The reader of the archive. The first version read 30 days: its marks
/// would pass the history, so its reader starts again from the first row.
const READER_VERSION: u32 = 2;

impl Reader {
    fn path() -> PathBuf {
        private().join(rules::READER)
    }

    fn load() -> Reader {
        let fresh = || Reader { version: READER_VERSION, ..Reader::default() };
        match std::fs::read_to_string(Reader::path()).ok().and_then(|t| serde_json::from_str::<Reader>(&t).ok()) {
            Some(reader) if reader.version == READER_VERSION => reader,
            Some(_) => {
                // The first version's stores, in the state folder then: sealed copies of 30 days, read again whole now.
                for old in ["log", "send", "outcome", "ask", "answer"] {
                    let _ = std::fs::remove_dir_all(private().join(old));
                }
                eprintln!("sioul: texts: the reader starts again from the first text, for the archive");
                fresh()
            }
            None => fresh(),
        }
    }

    fn save(&self) -> Result<(), String> {
        write_whole(&Reader::path(), serde_json::to_string(self).map_err(|e| e.to_string())?.as_bytes())
    }

    /// What `texts-ids` measured: how many texts, how many media bytes.
    fn measure(&mut self, answer: &Value, now: i64) {
        let count = |name: &str| answer[name].as_array().map_or(0, Vec::len) as u64;
        self.total_texts = count("sms") + count("mms");
        self.total_media = answer["media"].as_u64().unwrap_or(0);
        self.measured = now;
    }
}

/// A row as Java read it (Texts.sms, Texts.mms; android/jvm-checks/TextsCheck.java): every column.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct Raw {
    kind: String,
    fields: BTreeMap<String, Value>,
    addrs: Vec<BTreeMap<String, Value>>,
    parts: Vec<BTreeMap<String, Value>>,
}

fn int(fields: &BTreeMap<String, Value>, name: &str) -> i64 {
    fields.get(name).and_then(|v| v.as_i64().or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))).unwrap_or(0)
}

fn string(fields: &BTreeMap<String, Value>, name: &str) -> String {
    fields.get(name).and_then(Value::as_str).unwrap_or_default().to_string()
}

/// A multimedia message's address types (`addr.type`): who sent it, to whom.
const MMS_FROM: i64 = 137;
const MMS_TO: i64 = 151;
/// A multimedia message still to download (`m_type`: Android has only its notification).
const MMS_NOTIFICATION: i64 = 130;
/// What Android writes for "this phone" in a message's addresses.
const OWN_ADDRESS: &str = "insert-address-token";

/// A part as it travels: its words in the line; its content said, to be put
/// in the sharing (`bring_media`) when it is not over `cap` bytes.
fn part_of(columns: &BTreeMap<String, Value>, waiting: bool, cap: u64) -> Part {
    let ct = string(columns, "ct");
    let words = ct.starts_with("text/plain") || ct == "application/smil";
    let size = int(columns, "size");
    let state = if words {
        "text"
    } else if waiting || size < 0 {
        "not-downloaded"
    } else if u64::try_from(size).unwrap_or(0) > cap {
        "too-big"
    } else {
        "here"
    };
    let name = ["name", "cl", "fn"].iter().map(|n| string(columns, n)).find(|n| !n.is_empty()).unwrap_or_default();
    // The other columns as they came; the size is Android's measure, not a column.
    let mut fields = columns.clone();
    for held in ["seq", "ct", "text", "size"] {
        fields.remove(held);
    }
    Part { seq: int(columns, "seq"), ct, name, size: u64::try_from(size).unwrap_or(0), text: if words { string(columns, "text") } else { String::new() }, hash: String::new(), state: state.into(), fields }
}

/// A row Java read, as it travels: its people keyed (its conversation's,
/// else its own addresses), its words, its parts said (their content left for
/// `bring_media`), every column kept as it came.
fn text_of(raw: &Raw, threads: &BTreeMap<String, Vec<String>>, region: Option<&phones::Region>, cap: u64) -> Text {
    let key = |a: &str| phones::key(a, region);
    let f = &raw.fields;
    let thread = int(f, "thread_id").to_string();
    let people: Vec<String> = threads.get(&thread).map(|list| list.iter().map(|a| key(a)).filter(|k| !k.is_empty()).collect()).unwrap_or_default();
    let sub = i32::try_from(int(f, "sub_id")).ok().filter(|_| f.contains_key("sub_id")).unwrap_or(-1);
    if raw.kind == "mms" {
        let outgoing = matches!(int(f, "msg_box"), 2 | 4);
        let addresses = |kind: i64| -> Vec<String> { raw.addrs.iter().filter(|a| int(a, "type") == kind).map(|a| string(a, "address")).filter(|a| !a.is_empty() && a != OWN_ADDRESS).map(|a| key(&a)).filter(|k| !k.is_empty()).collect() };
        let from = addresses(MMS_FROM).into_iter().next().unwrap_or_default();
        let with = if !people.is_empty() {
            people
        } else if outgoing {
            addresses(MMS_TO)
        } else {
            [from.clone()].into_iter().filter(|k| !k.is_empty()).collect()
        };
        let waiting = int(f, "m_type") == MMS_NOTIFICATION;
        let parts: Vec<Part> = raw.parts.iter().map(|p| part_of(p, waiting, cap)).collect();
        let body = parts.iter().filter(|p| p.ct.starts_with("text/plain")).map(|p| p.text.as_str()).collect::<Vec<_>>().join("\n");
        Text {
            id: format!("mms-{}", int(f, "_id")),
            // The multimedia provider counts in seconds.
            at: int(f, "date") * 1000,
            thread,
            direction: if outgoing { "out".into() } else { "in".into() },
            from: if outgoing || with.len() < 2 { String::new() } else { from },
            with,
            body,
            sub,
            picture: parts.iter().any(|p| p.ct.starts_with("image/")),
            parts,
            fields: f.clone(),
            addrs: raw.addrs.clone(),
            ..Text::default()
        }
    } else {
        let address = key(&string(f, "address"));
        let with = if !people.is_empty() { people } else { [address.clone()].into_iter().filter(|k| !k.is_empty()).collect() };
        let outgoing = matches!(int(f, "type"), 2 | 4 | 5 | 6);
        Text {
            id: format!("sms-{}", int(f, "_id")),
            at: int(f, "date"),
            thread,
            direction: if outgoing { "out".into() } else { "in".into() },
            from: if outgoing || with.len() < 2 { String::new() } else { address },
            with,
            body: string(f, "body"),
            sub,
            fields: f.clone(),
            ..Text::default()
        }
    }
}

fn raws(answer: &Value) -> Vec<Raw> {
    answer["texts"].as_array().map(|a| a.iter().filter_map(|v| serde_json::from_value(v.clone()).ok()).collect()).unwrap_or_default()
}

/// Each part of a message that travels, copied from Android into the
/// private folder, hashed, put in the sharing as a sealed blob (never inside
/// a line), the copy deleted; within `budget` bytes, the first part of a
/// step always. Whether every part was brought (else the message waits for
/// the next step).
fn bring_media(text: &mut Text, vault: &Vault, budget: &mut u64) -> bool {
    let (folder, key) = vault;
    for part in text.parts.iter_mut().filter(|p| p.state == "here" && p.hash.is_empty()) {
        if part.size > *budget && *budget < rules::MEDIA_BATCH {
            return false;
        }
        let incoming = private().join("incoming").join(format!("{}-{}", text.id, part.seq));
        let answer = crate::steps::java("texts-part", &json!({ "id": int(&part.fields, "_id"), "target": incoming.display().to_string() }).to_string());
        if answer["allowed"] != true {
            return false;
        }
        if answer["copied"].as_i64().unwrap_or(-1) < 0 {
            // Gone meanwhile, or never downloaded: said so.
            part.state = "not-downloaded".into();
            continue;
        }
        let put = sioul_sync::blobs::hash_file(&incoming).map_err(|e| e.to_string()).and_then(|(hash, size)| sioul_sync::blobs::put(folder, key, &incoming, &hash).map(|_| (hash, size)));
        let _ = std::fs::remove_file(&incoming);
        match put {
            Ok((hash, size)) => {
                part.hash = hash;
                part.size = size;
                *budget = budget.saturating_sub(size);
            }
            Err(e) => {
                eprintln!("sioul: texts: {e}");
                return false;
            }
        }
    }
    true
}

/// The conversations' people of a `texts-read` answer, by thread.
fn threads_of(answer: &Value) -> BTreeMap<String, Vec<String>> {
    answer["threads"]
        .as_object()
        .map(|o| o.iter().map(|(id, people)| (id.clone(), people.as_array().map(|a| a.iter().filter_map(|p| p.as_str().map(str::to_string)).collect()).unwrap_or_default())).collect())
        .unwrap_or_default()
}

/// What a step took of a `texts-read` answer: its texts as they travel, the
/// last plain and multimedia rows taken, the media bytes put in the sharing.
struct Taken {
    texts: Vec<Text>,
    sms: i64,
    mms: i64,
    media: u64,
    /// The rows the answer held: more than were taken, or a full batch, leaves the import behind.
    rows: usize,
}

/// The texts of a `texts-read` answer as they travel, `BATCH` at most, the
/// media of each multimedia message brought within `MEDIA_BATCH` bytes; a
/// message whose media do not fit ends the list there, for the next step.
fn take(answer: &Value, vault: &Vault, region: Option<&phones::Region>) -> Taken {
    let threads = threads_of(answer);
    let cap = cap_mb() << 20;
    let mut budget = rules::MEDIA_BATCH;
    let raws = raws(answer);
    let mut taken = Taken { texts: Vec::new(), sms: 0, mms: 0, media: 0, rows: raws.len() };
    for raw in raws {
        if taken.texts.len() >= rules::BATCH {
            break;
        }
        let id = int(&raw.fields, "_id");
        let mut text = text_of(&raw, &threads, region, cap);
        if raw.kind == "mms" {
            let before = budget;
            if !bring_media(&mut text, vault, &mut budget) {
                break;
            }
            taken.media += before - budget;
            taken.mms = taken.mms.max(id);
        } else {
            taken.sms = taken.sms.max(id);
        }
        taken.texts.push(text);
    }
    taken
}

/// Texts added to this phone's log, each line once; how many were written.
fn write_log(here: &str, seal: &Seal, texts: &[Text]) -> usize {
    let line = |t: &Text| t.line_id();
    rules::append(&rules::own_file(&root(), rules::LOG, here), texts, seal, Some(&line)).unwrap_or_else(|e| {
        eprintln!("sioul: texts: {e}");
        0
    })
}

/// One step of the import, the first and every one after: the texts after
/// the reader's marks, `BATCH` at most (the plain texts first, then the
/// multimedia ones), their media within `MEDIA_BATCH` bytes, written to this
/// phone's log. A multimedia message whose media do not fit waits, with every
/// one after it, for the next step. Whether anything was written.
fn import_step(here: &str, seal: &Seal, vault: &Vault, region: Option<&phones::Region>, now: i64) -> bool {
    let mut reader = Reader::load();
    if reader.started == 0 {
        reader.started = now;
    }
    let answer = crate::steps::java("texts-read", &json!({ "sms": reader.sms, "mms": reader.mms, "limit": rules::BATCH }).to_string());
    if answer["allowed"] != true {
        return false;
    }
    let taken = take(&answer, vault, region);
    let written = write_log(here, seal, &taken.texts);
    reader.sms = reader.sms.max(taken.sms);
    reader.mms = reader.mms.max(taken.mms);
    reader.done_texts += written as u64;
    reader.done_media += taken.media;
    reader.behind = taken.rows >= rules::BATCH || taken.texts.len() < taken.rows;
    if let Err(e) = reader.save() {
        eprintln!("sioul: texts: {e}");
    }
    if written > 0 {
        // Never what they say: how many, and how much.
        eprintln!("sioul: texts: {written} read for your computers ({} so far, {} bytes of media)", reader.done_texts, reader.done_media);
    }
    written > 0
}

/// The rows on the phone at or under the marks that its log lacks, and the
/// rows its log holds that the phone no longer has; by table.
#[derive(Debug, Default, PartialEq, Eq)]
struct Gaps {
    missing_sms: Vec<i64>,
    missing_mms: Vec<i64>,
    gone: Vec<String>,
}

fn gaps(log: &[Text], sms: &BTreeSet<i64>, mms: &BTreeSet<i64>, sms_mark: i64, mms_mark: i64) -> Gaps {
    let row = |id: &str| id.split_once('-').map(|(table, n)| (table.to_string(), n.parse::<i64>().unwrap_or(0)));
    let logged: BTreeSet<(String, i64)> = log.iter().filter(|t| t.deleted == 0).filter_map(|t| row(&t.id)).collect();
    let marked: BTreeSet<&str> = log.iter().filter(|t| t.deleted > 0).map(|t| t.id.as_str()).collect();
    let missing = |table: &str, there: &BTreeSet<i64>, mark: i64| -> Vec<i64> { there.iter().copied().filter(|n| *n <= mark && !logged.contains(&(table.to_string(), *n))).take(rules::BATCH).collect() };
    let gone = logged
        .iter()
        .filter(|(table, n)| match table.as_str() {
            "sms" => *n <= sms_mark && !sms.contains(n),
            "mms" => *n <= mms_mark && !mms.contains(n),
            _ => false,
        })
        .map(|(table, n)| format!("{table}-{n}"))
        .filter(|id| !marked.contains(id.as_str()))
        .collect();
    Gaps { missing_sms: missing("sms", sms, sms_mark), missing_mms: missing("mms", mms, mms_mark), gone }
}

/// Once a day: what the phone holds measured for the settings; the texts
/// deleted on it marked so in its log, a line each (the archive keeps the
/// text); and a text the reader passed while Android hid it (in the outbox
/// as a newer one came) read now. Whether a line was written.
fn look_again(here: &str, seal: &Seal, vault: &Vault, region: Option<&phones::Region>, now: i64) -> bool {
    let mut reader = Reader::load();
    if now - reader.looked < 86_400_000 {
        return false;
    }
    let answer = crate::steps::java("texts-ids", &json!({ "measure": true }).to_string());
    if answer["allowed"] != true {
        return false;
    }
    let ids = |name: &str| -> BTreeSet<i64> { answer[name].as_array().map(|a| a.iter().filter_map(Value::as_i64).collect()).unwrap_or_default() };
    reader.looked = now;
    reader.measure(&answer, now);
    if let Err(e) = reader.save() {
        eprintln!("sioul: texts: {e}");
    }
    let log: Vec<Text> = rules::read_file(&rules::own_file(&root(), rules::LOG, here), seal);
    let found = gaps(&log, &ids("sms"), &ids("mms"), reader.sms, reader.mms);
    let by_id: BTreeMap<&str, &Text> = log.iter().filter(|t| t.deleted == 0).map(|t| (t.id.as_str(), t)).collect();
    let marks: Vec<Text> = found
        .gone
        .iter()
        .filter_map(|id| by_id.get(id.as_str()))
        .map(|t| Text { id: t.id.clone(), at: t.at, direction: t.direction.clone(), with: t.with.clone(), deleted: now, ..Text::default() })
        .collect();
    let mut written = write_log(here, seal, &marks);
    if written > 0 {
        eprintln!("sioul: texts: {written} deleted on the phone, marked so");
    }
    if !found.missing_sms.is_empty() || !found.missing_mms.is_empty() {
        let answer = crate::steps::java("texts-read", &json!({ "sms_ids": found.missing_sms, "mms_ids": found.missing_mms, "limit": rules::BATCH }).to_string());
        if answer["allowed"] == true {
            let taken = take(&answer, vault, region);
            let caught = write_log(here, seal, &taken.texts);
            if caught > 0 {
                eprintln!("sioul: texts: {caught} passed earlier, read now");
                reader.done_texts += caught as u64;
                reader.done_media += taken.media;
                let _ = reader.save();
            }
            written += caught;
        }
    }
    written > 0
}

// ---------------------------------------------------------------- the phone: sending

/// One part's result, as Java wrote it (TextSend.line).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct PartResult {
    key: String,
    kind: String,
    part: i64,
    parts: i64,
    code: i64,
    uri: String,
    status: i64,
    at: i64,
}

/// Android's code for a part not sent, as the words say it (SmsManager's RESULT_ERROR_…).
fn reason_of(code: i64) -> &'static str {
    match code {
        2 => "radio-off",
        4 => "no-service",
        5 => "limit",
        18 | 30 | 31 => "no-sim",
        _ => "other",
    }
}

/// "content://sms/414" → "sms-414": the row as the log names it.
fn row_of(uri: &str) -> String {
    uri.trim().rsplit('/').next().filter(|id| !id.is_empty() && id.chars().all(|c| c.is_ascii_digit())).map(|id| format!("sms-{id}")).unwrap_or_default()
}

/// Each text's outcomes from its parts' results: sent when every part was;
/// not sent, and why, when one was not; delivered when the carrier said every
/// part was received (never "not delivered" for a report that did not come).
fn outcomes_from(results: &[PartResult]) -> Vec<Outcome> {
    let mut by: BTreeMap<&str, Vec<&PartResult>> = BTreeMap::new();
    for r in results.iter().filter(|r| !r.key.is_empty()) {
        by.entry(r.key.as_str()).or_default().push(r);
    }
    let mut out = Vec::new();
    for (key, parts) in by {
        let count = parts.iter().map(|p| p.parts.max(1)).max().unwrap_or(1);
        let sent: Vec<&&PartResult> = parts.iter().filter(|p| p.kind == "sent").collect();
        let at = |list: &[&&PartResult]| list.iter().map(|p| p.at).max().unwrap_or(0);
        if let Some(failed) = sent.iter().find(|p| p.code != -1) {
            out.push(Outcome { key: key.into(), state: "failed".into(), at: failed.at, reason: reason_of(failed.code).into(), ..Outcome::default() });
            continue;
        }
        let done: BTreeSet<i64> = sent.iter().map(|p| p.part).collect();
        if done.len() as i64 >= count {
            let row = sent.iter().map(|p| row_of(&p.uri)).find(|r| !r.is_empty()).unwrap_or_default();
            out.push(Outcome { key: key.into(), state: "sent".into(), at: at(&sent), row, ..Outcome::default() });
            let delivered: Vec<&&PartResult> = parts.iter().filter(|p| p.kind == "delivered" && (0..0x20).contains(&p.status)).collect();
            if delivered.iter().map(|p| p.part).collect::<BTreeSet<_>>().len() as i64 >= count {
                out.push(Outcome { key: key.into(), state: "delivered".into(), at: at(&delivered), ..Outcome::default() });
            }
        }
    }
    out
}

/// Outcomes added to this phone's own file, each (key, state) once.
fn say(here: &str, seal: &Seal, outcomes: &[Outcome]) -> bool {
    let id = |o: &Outcome| format!("{}/{}", o.key, o.state);
    match rules::append(&rules::own_file(&root(), rules::OUTCOME, here), outcomes, seal, Some(&id)) {
        Ok(n) => n > 0,
        Err(e) => {
            eprintln!("sioul: texts: {e}");
            false
        }
    }
}

/// Java's results copied into this phone's outcomes, and the ledger told.
fn copy_results(here: &str, seal: &Seal, now: i64) -> bool {
    let results: Vec<PartResult> = std::fs::read_to_string(private().join(rules::RESULTS)).unwrap_or_default().lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
    if results.is_empty() {
        return false;
    }
    let outcomes = outcomes_from(&results);
    let changed = say(here, seal, &outcomes);
    let ledger = rules::Ledger::open(&private(), now).ok();
    for o in &outcomes {
        if ledger.as_ref().is_some_and(|l| l.state(&o.key) != Some("done")) {
            let _ = rules::record(&private(), &Claim { key: o.key.clone(), at: now, state: "done".into(), ..Claim::default() });
        }
    }
    changed
}

/// The requests under each root (`<root>/send/<device>.jsonl`), each key
/// once, the first root's first: the ledger then decides each once.
fn requests_from(roots: &[PathBuf], seal: &Seal) -> Vec<Request> {
    let mut requests: Vec<Request> = Vec::new();
    for root in roots {
        for request in rules::read_requests(root, seal) {
            if !requests.iter().any(|known| known.key == request.key) {
                requests.push(request);
            }
        }
    }
    requests
}

/// Each request decided once, under the ledger's lock (both processes of the
/// phone may step): claimed and flushed before Android is asked; the others
/// said (expired, refused, clocks apart) and written in the ledger, so that
/// they are decided once too. Whether an outcome was written.
fn decide(here: &str, seal: &Seal, region: Option<&'static phones::Region>, now: i64) -> bool {
    // The records' copy, then the same requests in their own small files, the
    // sharing folder's and the copy fetched from its server: one reaches this
    // phone within its quarter of an hour, whatever the records' size
    // (docs/database.md, "Sent to the server too").
    let roots: Vec<PathBuf> = std::iter::once(root()).chain(crate::share::texts_copies()).collect();
    let requests = requests_from(&roots, seal);
    if requests.iter().all(|r| r.phone != here) {
        return false;
    }
    let ledger_path = private().join(rules::LEDGER);
    let mut said = Vec::new();
    let mut to_send = Vec::new();
    let claimed = sioul_core::filelock::with_lock(&ledger_path, || -> Result<(), String> {
        let ledger = rules::Ledger::open(&private(), now)?;
        for r in &requests {
            let verdict = rules::verdict(r, here, now, &ledger, region);
            let (state, reason) = match verdict {
                Verdict::NotMine | Verdict::Decided => continue,
                Verdict::Send => {
                    rules::record(&private(), &Claim { key: r.key.clone(), at: now, state: "claimed".into(), to: phones::key(&r.to, region), hash: rules::words_hash(&r.body) })?;
                    to_send.push(r.clone());
                    continue;
                }
                Verdict::Expired => ("expired", ""),
                Verdict::Clocks => ("clocks", ""),
                Verdict::BeforeMark => ("refused", "before-mark"),
                Verdict::Refused(why) => ("refused", why),
            };
            rules::record(&private(), &Claim { key: r.key.clone(), at: now, state: state.into(), ..Claim::default() })?;
            said.push(Outcome { key: r.key.clone(), state: state.into(), at: now, reason: reason.into(), ..Outcome::default() });
        }
        Ok(())
    });
    if let Err(e) = claimed {
        // A claim not flushed: nothing is handed to Android.
        eprintln!("sioul: texts: {e}");
        return say(here, seal, &said);
    }
    let texts = if to_send.is_empty() { Vec::new() } else { rules::read_logs(&root(), seal) };
    for r in to_send {
        // The SIM: as asked, else the conversation's last on this phone.
        let sub = if r.sub >= 0 { r.sub } else { rules::last_sim(&rules::conversation_id(&[phones::key(&r.to, region)]), here, &texts) };
        let answer = crate::steps::java("texts-send", &json!({ "key": r.key, "to": phones::key(&r.to, region), "body": r.body, "sub": sub }).to_string());
        if answer["handed"] == true {
            let _ = rules::record(&private(), &Claim { key: r.key.clone(), at: now_ms(), state: "handed".into(), ..Claim::default() });
            said.push(Outcome { key: r.key.clone(), state: "handed".into(), at: now_ms(), ..Outcome::default() });
            eprintln!("sioul: texts: one handed to Android ({} part(s))", answer["parts"].as_i64().unwrap_or(1));
        } else if answer.is_null() {
            // No answer from Java at all: whether Android took it is not known. Looked for later, never sent again.
            eprintln!("sioul: texts: Android's answer did not come; the text is looked for later");
        } else {
            let reason = answer["reason"].as_str().unwrap_or("other").to_string();
            let _ = rules::record(&private(), &Claim { key: r.key.clone(), at: now_ms(), state: "done".into(), ..Claim::default() });
            said.push(Outcome { key: r.key.clone(), state: "failed".into(), at: now_ms(), reason, ..Outcome::default() });
        }
    }
    say(here, seal, &said)
}

/// A claim left without Android's word (Sioul stopped between the claim and
/// the hand-off): looked for among the texts Sioul sent since, by its number
/// and its words' hash; found, sent; else in doubt. Never sent again.
fn look_for_lost(here: &str, seal: &Seal, region: Option<&phones::Region>, now: i64) -> bool {
    let Ok(ledger) = rules::Ledger::open(&private(), now) else { return false };
    let lost: Vec<&Claim> = ledger.claims.iter().filter(|c| c.state == "claimed" && ledger.state(&c.key) == Some("claimed") && now - c.at >= rules::LOOK_AFTER_MS).collect();
    if lost.is_empty() {
        return false;
    }
    let since = lost.iter().map(|c| c.at).min().unwrap_or(now) - 60_000;
    let answer = crate::steps::java("texts-find", &json!({ "since": since }).to_string());
    if answer["allowed"] != true {
        return false;
    }
    let sent: Vec<Raw> = raws(&answer);
    let mut said = Vec::new();
    for claim in lost {
        let found = sent
            .iter()
            .find(|r| int(&r.fields, "date") >= claim.at - 60_000 && rules::words_hash(&string(&r.fields, "body")) == claim.hash && phones::key(&string(&r.fields, "address"), region) == claim.to)
            .map(|r| format!("sms-{}", int(&r.fields, "_id")));
        if let Some(outcome) = rules::doubt_after(claim, found, now) {
            let _ = rules::record(&private(), &Claim { key: claim.key.clone(), at: now, state: "done".into(), ..Claim::default() });
            said.push(outcome);
        }
    }
    say(here, seal, &said)
}

/// The quick part of the phone's step, at every exchange: Android's words
/// copied, the requests decided and sent, a lost claim looked for. Under one
/// lock for the phone's processes. Whether something was written.
fn light_step(here: &str, seal: &Seal, region: Option<&'static phones::Region>) -> bool {
    sioul_core::filelock::with_lock(&private().join(".step"), || {
        let now = now_ms();
        let mut changed = copy_results(here, seal, now);
        changed |= decide(here, seal, region, now);
        changed |= look_for_lost(here, seal, region, now);
        changed
    })
}

/// The part that may take a while, never inside an exchange: the history's
/// next batch and its media when due (`import_now`), and the day's look for
/// deletions. Under its own lock: one import at a time on the phone.
fn heavy_step(here: &str, seal: &Seal, vault: &Vault, region: Option<&phones::Region>) -> bool {
    sioul_core::filelock::with_lock(&private().join(".import"), || {
        let now = now_ms();
        let (texts_changed, plugged) = changes(true);
        let mut changed = import_now(&Reader::load(), texts_changed, plugged, now) && import_step(here, seal, vault, region, now);
        changed |= look_again(here, seal, vault, region, now);
        changed
    })
}

/// Java's word on the phone (`Texts.changes`): whether its texts changed
/// since it was last asked with `clear` (a text came, was sent, deleted or
/// read: Android's provider watched by the background service; not watched
/// in this process, or no answer: yes, as at every step before), and whether
/// the phone is on its charger.
fn changes(clear: bool) -> (bool, bool) {
    let answer = crate::steps::java("texts-changes", &json!({ "clear": clear }).to_string());
    (answer["changed"] != false, answer["plugged"] == true)
}

/// Whether the import reads the phone's texts now, at `now` (ms;
/// docs/texts.md, "Reading, on the phone: paced"). A heavy import, the first
/// (the whole history) or one the last batch left behind (full, or media
/// that did not fit), only while the phone charges: it may write and send
/// hundreds of megabytes, on mobile data too. Caught up, when the phone's
/// texts changed since the last look, and with the day's look for deletions
/// all the same (a change Android did not say): most steps find nothing new,
/// and ask Android nothing.
fn import_now(reader: &Reader, changed: bool, plugged: bool, now: i64) -> bool {
    if reader.started == 0 || reader.behind { plugged } else { changed || now - reader.looked >= 86_400_000 }
}

/// A bulk import going on, on the charger or waiting for it (`import_now`):
/// the sharing's full rounds wait meanwhile (`steps`), so that it does not
/// restate the records again and again (8 October 2026: a first import made
/// six full rounds, each sent twice).
pub(crate) fn importing_in_bulk() -> bool {
    phone() && sealed().is_some() && Reader::load().behind
}

/// The background service's step, after its exchange: the quick part, then
/// the import's next batch. Whether something was written to share now.
pub(crate) fn phone_step() -> bool {
    if !phone() {
        return false;
    }
    let Some((here, seal, Some(vault))) = sealed() else { return false };
    let region = sioul_core::reach::region(&load_config());
    let quick = light_step(&here, &seal, region);
    heavy_step(&here, &seal, &vault, region) || quick
}

// ---------------------------------------------------------------- every device

/// When this device last took its old requests and outcomes out (ms): once a day.
static TRIMMED: Mutex<i64> = Mutex::new(0);
/// The import (a phone's window) or the media's fetching (a computer) running in a thread of this process.
static WORKING: AtomicBool = AtomicBool::new(false);

/// Before an exchange: on a phone, the quick part of its step; on a
/// computer, how far its clock may run ahead of the phone's. Then, in a
/// thread of its own that never holds the exchange: the import's next batch
/// (a phone's window; the background service runs it after its exchange) or
/// the media brought here (a computer). Once a day, old requests and
/// outcomes out (texts never).
pub(crate) fn before_exchange() {
    let Some((here, seal, vault)) = sealed() else { return };
    if phone() {
        light_step(&here, &seal, sioul_core::reach::region(&load_config()));
    } else {
        measure_clock();
    }
    if vault.is_some() && !crate::steps::in_service() && !WORKING.swap(true, Ordering::AcqRel) {
        std::thread::spawn(|| {
            if let Some((here, seal, Some(vault))) = sealed() {
                if phone() {
                    heavy_step(&here, &seal, &vault, sioul_core::reach::region(&load_config()));
                } else {
                    fetch_media(&seal, &vault);
                }
            }
            WORKING.store(false, Ordering::Release);
        });
    }
    let now = now_ms();
    let due = TRIMMED.lock().map(|mut last| {
        let due = now - *last > 86_400_000;
        if due {
            *last = now;
        }
        due
    });
    if due.unwrap_or(false) {
        let done = rules::trim_own(&root(), &here, &seal, now).and_then(|_| rules::trim_others(&root(), &here, &seal, now));
        if phone() {
            let _ = rules::trim_ledger(&private(), now);
        }
        if let Err(e) = done {
            eprintln!("sioul: texts: {e}");
        }
    }
}

// ---------------------------------------------------------------- the media kept here

/// A media file kept here, sealed: `texts/media/<hash>` in the data folder.
fn media_path(hash: &str) -> PathBuf {
    root().join(rules::MEDIA).join(hash.chars().filter(char::is_ascii_hexdigit).collect::<String>())
}

fn here_media(hash: &str) -> bool {
    !hash.is_empty() && media_path(hash).exists()
}

/// The phones' logs as last seen by `fetch_media` (each file's size and
/// time), and whether media were left for next time: nothing new, nothing done.
static FETCHED: Mutex<(Vec<(u64, u128)>, bool)> = Mutex::new((Vec::new(), true));

/// The media the texts name and this device has not yet: brought from the
/// sharing's blobs, sealed here, `MEDIA_BATCH` bytes at a time (the first
/// always); one the sync app has not brought yet waits for next time.
fn fetch_media(seal: &Seal, vault: &Vault) {
    let (folder, key) = vault;
    let mut logs: Vec<(u64, u128)> = std::fs::read_dir(root().join(rules::LOG))
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok()?.metadata().ok())
        .map(|m| (m.len(), m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos())))
        .collect();
    logs.sort_unstable();
    if FETCHED.lock().is_ok_and(|seen| seen.0 == logs && !seen.1) {
        return;
    }
    let mut wanted: Vec<(String, u64)> = rules::read_logs(&root(), seal).iter().flat_map(|t| t.parts.iter()).filter(|p| p.state == "here" && !p.hash.is_empty() && !here_media(&p.hash)).map(|p| (p.hash.clone(), p.size)).collect();
    wanted.sort();
    wanted.dedup();
    let mut budget = rules::MEDIA_BATCH;
    let (mut brought, mut left) = (0, false);
    for (hash, size) in wanted {
        if size > budget && budget < rules::MEDIA_BATCH {
            left = true;
            break;
        }
        let incoming = private().join("incoming").join(&hash);
        if let Some(dir) = incoming.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if sioul_sync::blobs::get(folder, key, &hash, &incoming).is_err() {
            left = true;
            continue;
        }
        let bytes = std::fs::read(&incoming);
        let _ = std::fs::remove_file(&incoming);
        let Ok(bytes) = bytes else {
            left = true;
            continue;
        };
        if let Err(e) = write_whole(&media_path(&hash), &seal.seal_bytes(&bytes)) {
            eprintln!("sioul: texts: {e}");
            left = true;
            break;
        }
        budget = budget.saturating_sub(bytes.len() as u64);
        brought += 1;
    }
    if let Ok(mut seen) = FETCHED.lock() {
        *seen = (logs, left);
    }
    if brought > 0 {
        eprintln!("sioul: texts: {brought} media file(s) brought");
    }
}

/// Where a media file is opened for the page: Sioul's cache (yours alone), emptied when the page closes.
fn opened_folder() -> PathBuf {
    sioul_core::config::cache_dir().join(rules::FOLDER)
}

/// A media file opened for the page or saved: its plain copy in `opened_folder`; none when it is not here.
fn open_media(hash: &str, ct: &str) -> Option<PathBuf> {
    let path = media_path(hash);
    let extension = ct.split('/').nth(1).and_then(|s| s.split(['+', ';', '.', '-']).next()).filter(|e| !e.is_empty() && e.chars().all(|c| c.is_ascii_alphanumeric())).unwrap_or("bin");
    let target = opened_folder().join(format!("{}.{extension}", path.file_name()?.to_string_lossy()));
    // Opened already for the page: not read and opened again each time a row shows it.
    if target.exists() {
        return Some(target);
    }
    let (_, seal, _) = sealed()?;
    let plain = seal.open_bytes(&std::fs::read(&path).ok()?)?;
    write_whole(&target, &plain).ok()?;
    Some(target)
}

// ---------------------------------------------------------------- a computer: the clock

/// How far this computer's clock may run ahead of a phone's (`texts/clock.json`
/// in the state folder): for each phone, the last time its entry said, and
/// the least gap seen in the last day.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Clock {
    #[serde(default)]
    phones: BTreeMap<String, Gap>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Gap {
    /// The phone's entry's last `exported` (seconds, the phone's clock).
    seen: i64,
    /// The least gap (ms) and when it was seen.
    least: i64,
    at: i64,
}

impl Clock {
    fn path() -> PathBuf {
        private().join("clock.json")
    }
}

/// At each exchange: the sending phone's entry's time, when it changed,
/// compared with this computer's clock; the least such gap of the last day
/// is how far this clock may run ahead of the phone's (transit included: an
/// upper bound, so that a request looks older, never younger).
fn measure_clock() {
    // Every half minute at most: the devices' entries are opened to read it.
    static LOOKED: Mutex<i64> = Mutex::new(0);
    let now = now_ms();
    if let Ok(mut looked) = LOOKED.lock() {
        if now - *looked < 30_000 {
            return;
        }
        *looked = now;
    }
    let Some(phone) = sending_phone() else { return };
    let mut clock: Clock = std::fs::read_to_string(Clock::path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    let gap = clock.phones.entry(phone.id.clone()).or_default();
    if phone.exported <= 0 || phone.exported == gap.seen {
        return;
    }
    let sample = now - phone.exported * 1000;
    gap.seen = phone.exported;
    if gap.at == 0 || now - gap.at > 86_400_000 || sample < gap.least {
        gap.least = sample;
        gap.at = now;
    }
    if let Ok(text) = serde_json::to_string(&clock) {
        let _ = write_whole(&Clock::path(), text.as_bytes());
    }
}

/// How far this clock may run ahead of a phone's (ms): the least gap of the last day, 0 unknown.
fn ahead_of(phone: &str) -> i64 {
    let clock: Clock = std::fs::read_to_string(Clock::path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    clock.phones.get(phone).filter(|g| now_ms() - g.at <= 86_400_000).map_or(0, |g| g.least.max(0))
}

// ---------------------------------------------------------------- a computer: the phone that sends

/// The phones in the sharing, the one chosen to send first, else the one that shared last.
fn phones() -> Vec<sioul_sync::devices::Entry> {
    let Some((_, Some((folder, key)))) = crate::share::vault() else { return Vec::new() };
    let mut all: Vec<sioul_sync::devices::Entry> = sioul_sync::devices::all(&folder, &key).0.into_iter().filter(|d| d.kind == sioul_sync::devices::PHONE).collect();
    let chosen = choice();
    all.sort_by_key(|d| (d.id != chosen.phone, -d.exported));
    all
}

fn sending_phone() -> Option<sioul_sync::devices::Entry> {
    if demo() && !phone() {
        // The demo's stand-in phone, which shared two minutes ago.
        let exported = jiff::Timestamp::now().as_second() - 120;
        return Some(sioul_sync::devices::Entry { id: "demo-phone".into(), name: String::new(), kind: sioul_sync::devices::PHONE.into(), exported, ..Default::default() });
    }
    phones().into_iter().next()
}

// ---------------------------------------------------------------- a computer: the page

fn names(senders: std::rc::Rc<Senders>) -> impl Fn(&str) -> Option<String> {
    move |key: &str| {
        let judged = senders.judge_number(key);
        (!judged.card.trim().is_empty()).then_some(judged.card)
    }
}

thread_local! {
    /// The settings and your lists and address books, as the page last read
    /// them: read again when the settings' file changed, or after a minute.
    static LOOKS: std::cell::RefCell<Option<(Option<(u64, std::time::SystemTime)>, std::time::Instant, std::rc::Rc<Config>, std::rc::Rc<Senders>)>> = const { std::cell::RefCell::new(None) };
}

/// The settings and your senders for the page's looks (`LOOKS`): each
/// opening of a conversation read them twice, on the window's thread.
fn looks() -> (std::rc::Rc<Config>, std::rc::Rc<Senders>) {
    let stamp = std::fs::metadata(config_dir().join("config.toml")).ok().map(|m| (m.len(), m.modified().unwrap_or(std::time::UNIX_EPOCH)));
    LOOKS.with(|cell| {
        let mut kept = cell.borrow_mut();
        if let Some((s, at, config, senders)) = kept.as_ref()
            && *s == stamp
            && at.elapsed() < std::time::Duration::from_secs(60)
        {
            return (std::rc::Rc::clone(config), std::rc::Rc::clone(senders));
        }
        let config = std::rc::Rc::new(load_config());
        let senders = std::rc::Rc::new(Senders::load(&config));
        *kept = Some((stamp, std::time::Instant::now(), std::rc::Rc::clone(&config), std::rc::Rc::clone(&senders)));
        (config, senders)
    })
}

/// The texts, the requests and their outcomes as last read, kept while their
/// files stay as they were (size and time) for this device: the whole
/// history is opened once, not at each look of the page (9,800 sealed lines
/// read twice per conversation opened froze the window for seconds).
struct Kept {
    here: String,
    stamp: Vec<(PathBuf, u64, std::time::SystemTime)>,
    texts: std::sync::Arc<Vec<Text>>,
    requests: std::sync::Arc<Vec<Request>>,
    outcomes: std::sync::Arc<Vec<Outcome>>,
}

static KEPT: Mutex<Option<Kept>> = Mutex::new(None);

/// Each record file of the three stores, its size and time.
fn stamps(root: &Path) -> Vec<(PathBuf, u64, std::time::SystemTime)> {
    let mut all = Vec::new();
    for store in [rules::LOG, rules::SEND, rules::OUTCOME] {
        for entry in std::fs::read_dir(root.join(store)).into_iter().flatten().flatten() {
            if let Ok(meta) = entry.metadata() {
                all.push((entry.path(), meta.len(), meta.modified().unwrap_or(std::time::UNIX_EPOCH)));
            }
        }
    }
    all.sort();
    all
}

/// The texts, the requests and their outcomes (`KEPT`), read again only when a file changed.
#[allow(clippy::type_complexity)]
fn records(here: &str, seal: &Seal) -> (std::sync::Arc<Vec<Text>>, std::sync::Arc<Vec<Request>>, std::sync::Arc<Vec<Outcome>>) {
    let root = root();
    let stamp = stamps(&root);
    let mut kept = KEPT.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(k) = kept.as_ref().filter(|k| k.here == here && k.stamp == stamp) {
        return (std::sync::Arc::clone(&k.texts), std::sync::Arc::clone(&k.requests), std::sync::Arc::clone(&k.outcomes));
    }
    let texts = std::sync::Arc::new(rules::read_logs(&root, seal));
    let requests = std::sync::Arc::new(rules::read_requests(&root, seal));
    let outcomes = std::sync::Arc::new(rules::read_outcomes(&root, seal));
    *kept = Some(Kept { here: here.to_string(), stamp, texts: std::sync::Arc::clone(&texts), requests: std::sync::Arc::clone(&requests), outcomes: std::sync::Arc::clone(&outcomes) });
    (texts, requests, outcomes)
}

/// The Texts page's list (TextsPage.qml): {can (the part on, a phone that
/// sends), phone: {id, name, shared, said}, conversations, said, never,
/// empty (the list's words when it has no row)}; without them, `can` false and
/// why. With `query`, the conversations holding a text with all its words (or
/// whose people's names or numbers do), each with the newest such text as its
/// last words.
fn view(query: &str) -> Value {
    let tr = tr();
    let Some((here, seal, _)) = sealed() else { return json!({ "can": false, "said": tr.text("texts-page-off", None), "conversations": [] }) };
    let Some(phone) = sending_phone() else { return json!({ "can": false, "said": tr.text("texts-page-no-phone", None), "conversations": [] }) };
    let (config, senders) = looks();
    let now = Zoned::now();
    let name_of = names(senders);
    let shared = (phone.exported > 0).then_some(phone.exported * 1000);
    let v = rules::Viewer { now: &now, tr, region: sioul_core::reach::region(&config), name_of: &name_of, phone_shared: shared, media_here: &here_media };
    let (texts, requests, _) = records(&here, &seal);
    let mut conversations = rules::conversations(&texts, &requests, &v);
    let query = query.trim();
    if !query.is_empty() {
        // The newest text found in each conversation: its words, under its name.
        let mut found: BTreeMap<String, String> = BTreeMap::new();
        for f in rules::search(query, &texts, &requests, &[], &v, usize::MAX) {
            found.entry(f.conversation).or_insert_with(|| f.message.body.chars().take(80).collect());
        }
        conversations.retain(|c| found.contains_key(&c.id));
        for c in &mut conversations {
            if let Some(words) = found.get(&c.id).filter(|w| !w.is_empty()) {
                c.last = words.clone();
            }
        }
    }
    let mut rows = with_drafts(conversations, &sioul_core::textdraft::all(&seal), &v);
    if !query.is_empty()
        && let Value::Array(list) = &mut rows
    {
        // A draft's conversation with no text yet: kept when its name or number holds the words.
        let words = sioul_core::rules::fold(query);
        list.retain(|row| row["at"].as_i64().unwrap_or(0) > 0 || words.split(' ').all(|w| sioul_core::rules::fold(&format!("{} {}", row["title"].as_str().unwrap_or(""), row["id"].as_str().unwrap_or(""))).contains(w)));
    }
    let shared_at = shared.map(|ms| rules::time_of_day(ms, &now)).unwrap_or_default();
    let mut args = sioul_core::i18n::args();
    args.set("time", shared_at.clone());
    json!({
        "can": true,
        "phone": { "id": phone.id, "name": phone.name, "shared": shared_at, "said": if shared_at.is_empty() { String::new() } else { tr.text("texts-page-shared", Some(&args)) } },
        "conversations": rows,
        "said": "",
        "never": tr.text("texts-page-never", None),
        "empty": tr.text(if query.is_empty() { "texts-page-empty" } else { "texts-search-none" }, None),
    })
}

/// The conversations with the drafts an AI agent wrote for them (`textdraft`,
/// docs/mcp.md, "Texts"): how many wait in each, said under its last words;
/// a draft to someone with no conversation yet makes one, first.
fn with_drafts(conversations: Vec<rules::Conversation>, drafts: &[sioul_core::textdraft::TextDraft], v: &rules::Viewer) -> Value {
    let waiting = |id: &str| drafts.iter().filter(|d| d.conversation == id).count();
    let mut out: Vec<Value> = Vec::new();
    let mut new: Vec<&str> = drafts.iter().map(|d| d.conversation.as_str()).filter(|id| !conversations.iter().any(|c| c.id == *id)).collect();
    new.sort_unstable();
    new.dedup();
    for id in new {
        let title = id.split(',').map(|k| (v.name_of)(k).unwrap_or_else(|| sioul_core::calls::shown_number(k, v.region))).collect::<Vec<_>>().join(", ");
        out.push(json!({ "id": id, "with": [id], "title": title, "last": "", "when": "", "at": 0, "group": false, "drafts": waiting(id) }));
    }
    for c in conversations {
        let mut row = serde_json::to_value(&c).unwrap_or_default();
        row["drafts"] = json!(waiting(&c.id));
        out.push(row);
    }
    Value::Array(out)
}

/// One conversation (TextsPage.qml): {id, title, messages, can_write, group}.
fn conversation(id: &str) -> Value {
    let Some((here, seal, _)) = sealed() else { return json!({ "messages": [] }) };
    let (config, senders) = looks();
    let now = Zoned::now();
    let name_of = names(senders);
    let phone = sending_phone();
    let shared = phone.as_ref().filter(|p| p.exported > 0).map(|p| p.exported * 1000);
    let region = sioul_core::reach::region(&config);
    let v = rules::Viewer { now: &now, tr: tr(), region, name_of: &name_of, phone_shared: shared, media_here: &here_media };
    let (texts, requests, outcomes) = records(&here, &seal);
    let messages = rules::messages(id, &texts, &requests, &outcomes, &v);
    let with: Vec<String> = id.split(',').map(str::to_string).collect();
    let title = with.iter().map(|k| name_of(k).unwrap_or_else(|| sioul_core::calls::shown_number(k, region))).collect::<Vec<_>>().join(", ");
    // What an AI agent drafted for it: waiting for you, never sent by itself.
    let drafts: Vec<Value> = sioul_core::textdraft::all(&seal).into_iter().filter(|d| d.conversation == id).map(|d| json!({ "id": d.id, "body": d.body, "when": rules::time_of_day(d.written, &now) })).collect();
    json!({
        "id": id,
        "title": title,
        "messages": messages,
        "drafts": drafts,
        "can_write": with.len() == 1,
        "group": with.len() > 1,
    })
}

/// A draft an AI agent wrote, used (its words, for the box: sending stays
/// yours) or discarded: taken out of the drafts either way. {body}, "" when gone.
fn draft_taken(id: &str) -> Value {
    let Some((_, seal, _)) = sealed() else { return json!({ "body": "" }) };
    let body = sioul_core::textdraft::all(&seal).into_iter().find(|d| d.id == id).map(|d| d.body).unwrap_or_default();
    let _ = sioul_core::textdraft::remove(id, &seal);
    json!({ "body": body })
}

/// A text written here for the phone to send: refused at once when the
/// phone would refuse it (said why), else a request with a key of its own in
/// this device's file, which the sharing carries. {said, shared, key}.
fn send(to: &str, body: &str, again: &str) -> (Value, bool) {
    let tr = tr();
    let Some((here, seal, _)) = sealed() else { return (json!({ "said": tr.text("texts-page-off", None), "shared": false }), false) };
    let Some(phone) = sending_phone() else { return (json!({ "said": tr.text("texts-page-no-phone", None), "shared": false }), false) };
    let region = sioul_core::reach::region(&load_config());
    if let Some(why) = rules::refusal(to, body, region) {
        return (json!({ "said": tr.text(&format!("texts-refused-{why}"), None), "shared": false }), false);
    }
    let request = Request { key: rules::new_key(), phone: phone.id.clone(), to: to.trim().to_string(), body: body.to_string(), sub: -1, written: now_ms(), ahead: ahead_of(&phone.id), again: again.to_string(), device: String::new() };
    match rules::append(&rules::own_file(&root(), rules::SEND, &here), std::slice::from_ref(&request), &seal, None) {
        Ok(_) => {
            // Its small file to the server now, not behind the records.
            crate::share::texts_now();
            (json!({ "said": "", "shared": true, "key": request.key }), true)
        }
        Err(e) => (json!({ "said": e, "shared": false }), false),
    }
}

/// Send again: a request of its own, a new key, the same number and words.
fn again(key: &str) -> (Value, bool) {
    let Some((_, seal, _)) = sealed() else { return (json!({ "said": tr().text("texts-page-off", None), "shared": false }), false) };
    match rules::read_requests(&root(), &seal).into_iter().find(|r| r.key == key) {
        Some(old) => send(&old.to, &old.body, key),
        None => (json!({ "said": "", "shared": false }), false),
    }
}

/// How many texts a draft takes: {parts, said}.
fn parts(body: &str) -> Value {
    let (count, gsm) = rules::parts(body);
    let mut args = sioul_core::i18n::args();
    args.set("count", i64::try_from(count).unwrap_or(0));
    args.set("characters", i64::try_from(body.chars().count()).unwrap_or(0));
    let said = if count == 0 { String::new() } else { tr().text(if gsm { "texts-parts" } else { "texts-parts-unicode" }, Some(&args)) };
    json!({ "parts": count, "said": said })
}

/// "1.2 MB".
fn size_words(bytes: u64) -> String {
    sioul_core::view::size(tr(), usize::try_from(bytes).unwrap_or(usize::MAX))
}

/// What this device keeps of the texts, for its part's line in the sharing
/// panel ("Keeps 4,830 texts and 210 MB of media here."); "" when none.
pub(crate) fn kept_words() -> String {
    let Some((here, seal, _)) = sealed() else { return String::new() };
    let (texts, _, _) = records(&here, &seal);
    if texts.is_empty() {
        return String::new();
    }
    let (count, bytes) = rules::kept(&texts, &here_media);
    let mut args = sioul_core::i18n::args();
    args.set("count", i64::try_from(count).unwrap_or(0));
    args.set("size", size_words(bytes));
    tr().text("texts-kept", Some(&args))
}

// ---------------------------------------------------------------- Settings ▸ This phone ▸ Texts

/// The phone's tab (TextsSetup.qml): {android, state (each permission, the
/// SIMs), on (the part here), sharing, progress (what the phone holds and how
/// much has gone), importing (more to go), cap (the setting row of the largest
/// media file brought)}.
fn setup() -> Value {
    let here = sioul_sync::share::Here::load(&state_dir());
    let android = cfg!(target_os = "android");
    let state = if android { crate::steps::java("texts-state", "{}") } else { Value::Null };
    let mut reader = Reader::load();
    // Measured before the import begins; then each day, with the look for deletions.
    if android && state["read"] == true && reader.measured == 0 {
        let answer = crate::steps::java("texts-ids", &json!({ "measure": true }).to_string());
        if answer["allowed"] == true {
            reader.measure(&answer, now_ms());
            let _ = reader.save();
        }
    }
    let tr = tr();
    let mut args = sioul_core::i18n::args();
    args.set("count", i64::try_from(reader.total_texts).unwrap_or(0));
    args.set("size", size_words(reader.total_media));
    args.set("done", i64::try_from(reader.done_texts).unwrap_or(0));
    args.set("done_size", size_words(reader.done_media));
    // A heavy import waits for the charger (`import_now`): said, calmly.
    let goes_now = !android || (reader.started > 0 && !reader.behind) || changes(false).1;
    let progress = match (reader.measured, reader.done_texts) {
        (0, _) => String::new(),
        (_, 0) if goes_now => tr.text("texts-setup-holds", Some(&args)),
        (_, 0) => format!("{} {}", tr.text("texts-setup-holds", Some(&args)), tr.text("texts-setup-charging", None)),
        (_, done) if done >= reader.total_texts => tr.text("texts-setup-done", Some(&args)),
        _ if goes_now => tr.text("texts-setup-progress", Some(&args)),
        _ => tr.text("texts-setup-progress-charging", Some(&args)),
    };
    let choices: Vec<Value> = CAPS_MB
        .iter()
        .map(|mb| {
            let mut args = sioul_core::i18n::args();
            args.set("size", size_words(mb << 20));
            json!({ "value": mb.to_string(), "label": tr.text("texts-setup-cap-value", Some(&args)) })
        })
        .collect();
    json!({
        "android": android,
        "state": state,
        "on": here.shares(rules::PART, &load_config()),
        "sharing": here.folder.is_some(),
        "progress": progress,
        "importing": reader.measured > 0 && reader.done_texts < reader.total_texts,
        "cap": { "key": "cap", "kind": "choice", "label": tr.text("texts-setup-cap", None), "help": tr.text("texts-setup-cap-help", None), "value": cap_mb().to_string(), "choices": choices },
    })
}

/// The largest media file brought to your computers, chosen on the phone (MB).
fn set_cap(value: &str) -> Result<(), String> {
    let mb: u64 = value.trim().parse().map_err(|_| format!("{value}?"))?;
    let path = choice_path();
    sioul_core::filelock::with_lock(&path, || {
        let mut chosen = choice();
        chosen.media_cap_mb = (mb != rules::MEDIA_CAP_MB).then_some(mb.clamp(1, sioul_sync::blobs::LARGEST >> 20));
        write_whole(&path, toml::to_string(&chosen).map_err(|e| e.to_string())?.as_bytes())
    })
}

/// What the window asks (`Sioul::texts`): "view" {query}, "conversation" {id},
/// "send" {to, body}, "again" {key}, "parts" {body}, "media" {hash, ct} (its
/// address, to show or open it), "save-media" {hash, ct, path}, "close" (the
/// page closed: the media opened for it deleted), "setup", "ask" (Android's
/// question for the permissions), "part" {on}, "cap" {value}. Its JSON, and
/// whether what travels changed (to share now).
pub(crate) fn call(verb: &str, json: &str) -> (String, bool) {
    let asked: Value = serde_json::from_str(json).unwrap_or_default();
    let text = |key: &str| asked[key].as_str().unwrap_or_default().to_string();
    let (answer, shared) = match verb {
        "view" => (view(&text("query")), false),
        "conversation" => (conversation(&text("id")), false),
        "send" => send(&text("to"), &text("body"), ""),
        "again" => again(&text("key")),
        "parts" => (parts(&text("body")), false),
        // An AI agent's draft: its words for the box, or gone; nothing is sent.
        "draft-use" | "draft-discard" => (draft_taken(&text("id")), false),
        "media" => {
            let url = open_media(&text("hash"), &text("ct")).map(|p| crate::backend::file_url(&p)).unwrap_or_default();
            (json!({ "url": url }), false)
        }
        "save-media" => {
            let target = crate::backend::local_path(&text("path"));
            let saved = !target.as_os_str().is_empty() && open_media(&text("hash"), &text("ct")).is_some_and(|p| std::fs::copy(p, &target).is_ok());
            (json!({ "saved": saved }), false)
        }
        "close" => {
            let _ = std::fs::remove_dir_all(opened_folder());
            (json!({}), false)
        }
        "setup" => (setup(), false),
        "ask" => {
            crate::steps::java("texts-ask", "{}");
            (setup(), false)
        }
        "part" => {
            let problem = crate::share::set_part(rules::PART, asked["on"].as_bool().unwrap_or(false));
            let mut tab = setup();
            tab["said"] = json!(problem);
            (tab, problem.is_empty())
        }
        "cap" => {
            let problem = set_cap(&text("value")).err().unwrap_or_default();
            let mut tab = setup();
            tab["said"] = json!(problem);
            (tab, false)
        }
        _ => (json!({}), false),
    };
    (answer.to_string(), shared)
}

/// Whether the Texts page has something to show here: the part on, and a phone that sends.
pub(crate) fn readable() -> bool {
    sealed().is_some() && sending_phone().is_some()
}

/// A person's conversation on the Texts page, from their numbers as written:
/// the first whole one, keyed; "" while texts are not read here.
pub(crate) fn conversation_of(numbers: &[String]) -> String {
    if sealed().is_none() {
        return String::new();
    }
    let region = sioul_core::reach::region(&load_config());
    numbers.iter().map(|n| phones::key(n.trim().trim_start_matches(sioul_core::porch::TEL), region)).find(|k| phones::is_whole(k)).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Thursday 8 October 2026, 10:00 in Paris.
    const NOW: i64 = 1_791_446_400_000;

    fn part(key: &str, kind: &str, part: i64, parts: i64, code: i64, uri: &str, status: i64, at: i64) -> PartResult {
        PartResult { key: key.into(), kind: kind.into(), part, parts, code, uri: uri.into(), status, at }
    }

    /// A request found in its small file (the sharing folder's, the server's
    /// copy) before the records bring it, read beside the records' copy:
    /// each key once, its device by its file's name.
    #[test]
    fn requests_are_read_from_their_small_files_too_each_once() {
        let base = std::env::temp_dir().join(format!("sioul-texts-copies-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let seal = Seal::Key(TextSeal::new(&[3u8; 32]));
        let desk = "0f8fad5b-d9cb-469f-a165-70867728950e";
        let request = |key: &str| Request { key: key.into(), phone: "phone".into(), to: "0199001234".into(), body: "On my way".into(), sub: -1, written: NOW, ahead: 0, again: String::new(), device: String::new() };
        let (records, folder, fetched) = (base.join("records"), base.join("folder"), base.join("fetched"));
        rules::append(&rules::own_file(&records, rules::SEND, desk), &[request("a")], &seal, None).unwrap();
        rules::append(&rules::own_file(&folder, rules::SEND, desk), &[request("a"), request("b")], &seal, None).unwrap();
        rules::append(&rules::own_file(&fetched, rules::SEND, desk), &[request("a"), request("b"), request("c")], &seal, None).unwrap();
        let read = requests_from(&[records.clone(), folder, fetched], &seal);
        assert_eq!(read.iter().map(|r| r.key.as_str()).collect::<Vec<_>>(), ["a", "b", "c"]);
        assert!(read.iter().all(|r| r.device == desk), "{read:?}");
        // Nothing but the records: as before.
        assert_eq!(requests_from(&[records, base.join("none")], &seal).len(), 1);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_text_s_parts_make_one_outcome_each_and_delivered_only_when_reported() {
        let results = [
            part("a", "sent", 0, 2, -1, "content://sms/414", -1, NOW),
            part("a", "sent", 1, 2, -1, "content://sms/414", -1, NOW + 1000),
            part("a", "delivered", 0, 2, -1, "", 0, NOW + 5000),
            part("b", "sent", 0, 1, -1, "content://sms/415", -1, NOW),
            part("b", "delivered", 0, 1, -1, "", 0, NOW + 3000),
            part("c", "sent", 0, 1, 4, "", -1, NOW),
            part("d", "sent", 0, 2, -1, "content://sms/416", -1, NOW),
        ];
        let said: Vec<(String, String, String)> = outcomes_from(&results).into_iter().map(|o| (o.key, o.state, if o.row.is_empty() { o.reason } else { o.row })).collect();
        assert_eq!(said, [
            ("a".to_string(), "sent".to_string(), "sms-414".to_string()),
            ("b".into(), "sent".into(), "sms-415".into()),
            ("b".into(), "delivered".into(), String::new()),
            ("c".into(), "failed".into(), "no-service".into()),
        ], "a: one part of two delivered, no word; d: one part of two sent, not yet");
    }

    /// The rows as Java writes them (android/jvm-checks/TextsCheck.java):
    /// every column kept; a text's people from its conversation; a received
    /// multimedia message's sender, words and picture, a big video left on
    /// the phone, one still to download said; a group read as a group.
    #[test]
    fn what_java_reads_is_kept_whole_and_read_here() {
        let json = r#"{"texts":[
            {"kind":"sms","fields":{"_id":412,"thread_id":7,"address":"01 99 00 12 34","date":1791446340000,"date_sent":1791446338000,"type":1,"read":1,"seen":1,"body":"Votre rendez-vous est déplacé.","sub_id":2,"service_center":"+33609001390","protocol":0,"status":-1}},
            {"kind":"mms","fields":{"_id":31,"thread_id":9,"date":1791446340,"msg_box":1,"m_type":132,"sub_id":1},
             "addrs":[{"address":"+33465710042","type":137,"charset":106},{"address":"insert-address-token","type":151,"charset":106}],
             "parts":[{"_id":70,"seq":-1,"ct":"application/smil","text":"<smil/>"},{"_id":71,"seq":0,"ct":"image/jpeg","name":"a.jpg","size":220000,"cid":"<a>"},{"_id":72,"seq":1,"ct":"video/mp4","cl":"b.mp4","size":30000000},{"_id":73,"seq":2,"ct":"text/plain","text":"Le colis est arrivé.","chset":106}]},
            {"kind":"mms","fields":{"_id":32,"thread_id":11,"date":1791446350,"msg_box":1,"m_type":130},"addrs":[{"address":"+33465710042","type":137}],"parts":[]},
            {"kind":"sms","fields":{"_id":413,"thread_id":12,"address":"+33465710042","date":1791446360000,"type":1,"body":"Pour le groupe."}}],
            "threads":{"7":["0199001234"],"9":["+33465710042"],"12":["+33465710042","+33199001234"]},"sms":413,"mms":32,"allowed":true}"#;
        let answer: Value = serde_json::from_str(json).unwrap();
        let region = phones::region_named("FR");
        let threads: BTreeMap<String, Vec<String>> = answer["threads"].as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.as_array().unwrap().iter().map(|p| p.as_str().unwrap().to_string()).collect())).collect();
        let texts: Vec<Text> = raws(&answer).iter().map(|r| text_of(r, &threads, region, 10 << 20)).collect();
        let sms = &texts[0];
        assert_eq!((sms.id.as_str(), sms.with.as_slice(), sms.sub, sms.direction.as_str(), sms.at), ("sms-412", &["+33199001234".to_string()][..], 2, "in", 1_791_446_340_000));
        assert_eq!(sms.fields.len(), 13, "every column kept");
        let mms = &texts[1];
        assert_eq!((mms.id.as_str(), mms.at, mms.with.as_slice(), mms.body.as_str(), mms.picture, mms.sub), ("mms-31", 1_791_446_340_000, &["+33465710042".to_string()][..], "Le colis est arrivé.", true, 1));
        assert_eq!(mms.parts.iter().map(|p| p.state.as_str()).collect::<Vec<_>>(), ["text", "here", "too-big", "text"]);
        assert_eq!((mms.parts[1].name.as_str(), mms.parts[2].name.as_str(), mms.parts[1].size), ("a.jpg", "b.mp4", 220_000));
        assert_eq!((mms.parts[1].fields.get("cid"), mms.parts[3].fields.get("chset"), mms.parts[3].fields.get("text")), (Some(&json!("<a>")), Some(&json!(106)), None), "the other columns kept, none twice");
        assert_eq!(mms.addrs.len(), 2);
        assert_eq!((texts[2].parts.len(), texts[2].with.as_slice()), (0, &["+33465710042".to_string()][..]), "nothing to download yet");
        assert_eq!(texts[3].with, ["+33465710042", "+33199001234"], "a group, from its conversation");
        assert_eq!(texts[3].from, "+33465710042");
        let waiting = Raw { kind: "mms".into(), parts: vec![[("ct".to_string(), json!("image/png")), ("size".to_string(), json!(-1))].into_iter().collect()], ..Raw::default() };
        assert_eq!(text_of(&waiting, &threads, region, 10 << 20).parts[0].state, "not-downloaded");
        assert_eq!(row_of("content://sms/414"), "sms-414");
        assert_eq!(row_of(""), "");
    }

    /// The daily look: a row the reader passed while Android hid it is read
    /// now (never one after the marks: the import's); a row gone from the
    /// phone is marked once.
    #[test]
    fn the_daily_look_finds_what_was_passed_and_what_went() {
        let row = |id: &str| Text { id: id.into(), at: NOW, ..Text::default() };
        let log = vec![row("sms-1"), row("sms-2"), row("sms-4"), row("mms-3"), Text { deleted: NOW, ..row("sms-2") }, row("sms-5")];
        let sms: BTreeSet<i64> = [1, 3, 4, 6, 9].into();
        let found = gaps(&log, &sms, &BTreeSet::new(), 6, 3);
        assert_eq!(found.missing_sms, [3, 6], "passed while hidden; 9 is after the mark");
        assert!(found.missing_mms.is_empty());
        assert_eq!(found.gone, ["mms-3", "sms-5"], "sms-2 is marked already");
    }

    /// The import asks Android only when the phone's texts changed; a heavy
    /// one (the first, of the whole history, or one a full batch left behind)
    /// only while the phone charges.
    #[test]
    fn a_heavy_import_waits_for_the_charger() {
        let first = Reader::default();
        assert!(!import_now(&first, true, false, NOW), "the whole history, on the battery: it waits");
        assert!(import_now(&first, false, true, NOW), "on the charger");
        let caught_up = Reader { started: NOW - 1_000, looked: NOW - 3_600_000, ..Reader::default() };
        assert!(import_now(&caught_up, true, false, NOW), "a new text, on the battery");
        assert!(!import_now(&caught_up, false, false, NOW) && !import_now(&caught_up, false, true, NOW), "nothing changed: Android not asked");
        assert!(import_now(&caught_up, false, false, NOW + 86_400_000), "the day's look: read all the same");
        let behind = Reader { started: NOW - 1_000, behind: true, ..caught_up.clone() };
        assert!(!import_now(&behind, true, false, NOW), "a full batch left more: on the charger only");
        assert!(import_now(&behind, false, true, NOW), "on the charger, batch after batch");
        // A reader written before this mark: caught up.
        let older: Reader = serde_json::from_str(r#"{"version":2,"sms":10,"mms":3,"started":1}"#).unwrap();
        assert!(!older.behind && import_now(&older, true, false, NOW));
    }

    #[test]
    fn nothing_runs_on_a_computer_without_the_part() {
        if phone() || demo() {
            return;
        }
        assert!(!phone_step());
        let (answer, shared) = call("send", r#"{"to":"01 99 00 12 34","body":"Bonjour"}"#);
        assert!(!shared, "{answer}");
    }
}

