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

use crate::backend::{QtThread, load_config, tr};
use cxx_qt_lib::QString;
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
    /// On a computer, the most a multimedia message from here may weigh,
    /// below the carrier's (kB: 300, for older phones and some networks
    /// abroad); none, the carrier's own (Settings ▸ Texts).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    mms_kb: Option<u64>,
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

// ---------------------------------------------------------------- the phone: what Android is asked

/// What the import asks Android (Java's `Texts.call`, through
/// `steps::java`): the phone's own provider of texts; a stand-in in tests.
trait Android {
    fn ask(&self, verb: &str, asked: &Value) -> Value;
}

/// This phone's Android.
struct Phone;

impl Android for Phone {
    fn ask(&self, verb: &str, asked: &Value) -> Value {
        crate::steps::java(verb, &asked.to_string())
    }
}

/// Where the import writes: this phone's log, its reader, the media copied from Android for a moment.
struct Files {
    log: PathBuf,
    reader: PathBuf,
    incoming: PathBuf,
}

impl Files {
    /// This phone's: its log in the data folder, the rest in the private texts folder.
    fn of(here: &str) -> Files {
        Files { log: rules::own_file(&root(), rules::LOG, here), reader: Reader::path(), incoming: private().join("incoming") }
    }
}

// ---------------------------------------------------------------- the phone: the whole history

/// Rows of the provider's two tables (`texts::Row`), as `reader.json` keeps
/// them: `[id, date]` each, in order.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
struct Rows {
    sms: Vec<[i64; 2]>,
    mms: Vec<[i64; 2]>,
}

impl Rows {
    fn of(rows: impl IntoIterator<Item = rules::Row>) -> Rows {
        let mut out = Rows::default();
        for row in rows {
            if row.mms {
                out.mms.push([row.id, row.date]);
            } else {
                out.sms.push([row.id, row.date]);
            }
        }
        out.sms.sort_unstable();
        out.sms.dedup();
        out.mms.sort_unstable();
        out.mms.dedup();
        out
    }

    fn set(&self) -> BTreeSet<rules::Row> {
        let rows = |mms: bool, list: &[[i64; 2]]| list.iter().map(|[id, date]| rules::Row { mms, id: *id, date: *date }).collect::<Vec<_>>();
        rows(false, &self.sms).into_iter().chain(rows(true, &self.mms)).collect()
    }

    fn len(&self) -> usize {
        self.sms.len() + self.mms.len()
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// The reader's state and the import's measures, private to the phone
/// (`texts/reader.json` in the state folder): the rows its log holds, those
/// still to read, when the import began, what the phone holds and how much
/// has gone.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
struct Reader {
    /// `READER_VERSION`; 0, the first version's (30 days, before the
    /// archive); 2, marks of the ids read, oldest first; 3, ranges of the
    /// ids read, newest first. Both knew a row by its id alone, which the
    /// plain texts' table gives again to the next text once the newest one
    /// was deleted: such a text was never read. Version 4 knows each row by
    /// its id and its date (`texts::Row`).
    version: u32,
    /// The rows this phone's log holds (texts deleted on the phone among
    /// them), made from the log itself when `indexed` is false (a reader of
    /// an older version, or one lost), and again at each day's look.
    logged: Rows,
    indexed: bool,
    /// Rows that appeared since a step and are not read yet: read at the
    /// next steps, on the battery too.
    arrived: Rows,
    /// The history still to read (a first import, or a block of old texts
    /// put at once): the newest first by date, all of it while the phone
    /// charges, its newest meanwhile.
    backlog: Rows,
    /// The newest text read (ms): on the battery, the history is read only above it.
    newest_at: i64,
    /// When the history was found (ms): a block of old texts, or what the
    /// log of a reader of an older version lacked; for a first import,
    /// `started`. On the battery, nothing older than a day before it is read.
    since: i64,
    started: i64,
    /// What the phone holds, as last measured (texts; media bytes), and when.
    total_texts: u64,
    total_media: u64,
    measured: i64,
    /// What has gone to your computers: the texts the log holds, the media bytes put in the sharing.
    done_texts: u64,
    done_media: u64,
    /// When the deletions were last looked for.
    looked: i64,
    /// A history waits (`backlog`): all of it only while the phone charges
    /// (`import_now`), its newest meanwhile (`recent_now`).
    behind: bool,
    /// Rows that came wait still (`arrived`: their media over a step's
    /// share): the next step reads on, on the battery too.
    left: bool,
}

/// The reader of the archive. The first version read 30 days: its marks
/// would pass the history, so its reader starts again from the first row.
/// The second and third knew rows by their ids alone: the log tells which
/// rows it holds (`Reader::index`), and what it lacks is read as a history.
const READER_VERSION: u32 = 4;
/// How many multimedia messages, at most, a step reads beside the texts.
const MMS_SHARE: usize = 100;
/// A day, in milliseconds.
const DAY_MS: i64 = 86_400_000;

impl Reader {
    fn path() -> PathBuf {
        private().join(rules::READER)
    }

    fn load() -> Reader {
        Reader::load_at(&Reader::path())
    }

    fn load_at(path: &Path) -> Reader {
        let fresh = || Reader { version: READER_VERSION, ..Reader::default() };
        match std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str::<Reader>(&t).ok()) {
            Some(reader) if reader.version == READER_VERSION => reader,
            // Its rows known by their ids alone: at the next step the log
            // tells which it holds, and what it lacks is read as a history.
            Some(reader) if reader.version == 2 || reader.version == 3 => Reader { version: READER_VERSION, indexed: false, logged: Rows::default(), arrived: Rows::default(), backlog: Rows::default(), ..reader },
            Some(_) => {
                // The first version's stores, in the state folder then: sealed copies of 30 days, read again whole now.
                if let Some(dir) = path.parent() {
                    for old in ["log", "send", "outcome", "ask", "answer"] {
                        let _ = std::fs::remove_dir_all(dir.join(old));
                    }
                }
                eprintln!("sioul: texts: the reader starts again from the first text, for the archive");
                fresh()
            }
            None => fresh(),
        }
    }

    fn save(&self) -> Result<(), String> {
        self.save_at(&Reader::path())
    }

    fn save_at(&self, path: &Path) -> Result<(), String> {
        write_whole(path, serde_json::to_string(self).map_err(|e| e.to_string())?.as_bytes())
    }

    /// What `texts-ids` measured: how many texts, how many media bytes.
    fn measure(&mut self, answer: &Value, now: i64) {
        let count = |name: &str| answer[name].as_array().map_or(0, Vec::len) as u64;
        self.total_texts = count("sms") + count("mms");
        self.total_media = answer["media"].as_u64().unwrap_or(0);
        self.measured = now;
    }

    /// What the log holds, read from the log itself: its rows, its newest
    /// text, how many texts and media bytes have gone (never a count a step
    /// cut short left behind).
    fn index(&mut self, log: &[Text]) {
        let texts: Vec<&Text> = log.iter().filter(|t| t.deleted == 0).collect();
        self.logged = Rows::of(texts.iter().filter_map(|t| t.row()));
        self.newest_at = texts.iter().map(|t| t.at).max().unwrap_or(0);
        self.done_texts = self.logged.len() as u64;
        self.done_media = texts.iter().map(|t| t.media_bytes()).sum();
        self.indexed = true;
    }
}

/// Every row the phone holds, from a `texts-ids` answer: each id with its
/// own date (`sms_at`, `mms_at`). None when the dates did not come with the
/// ids: then nothing is read for this step, never every row taken as new.
fn listed(answer: &Value) -> Option<BTreeSet<rules::Row>> {
    let mut out = BTreeSet::new();
    for (mms, ids, dates) in [(false, "sms", "sms_at"), (true, "mms", "mms_at")] {
        let (ids, dates) = (answer[ids].as_array()?, answer[dates].as_array()?);
        if ids.len() != dates.len() {
            return None;
        }
        for (id, date) in ids.iter().zip(dates) {
            out.insert(rules::Row { mms, id: id.as_i64()?, date: date.as_i64()? });
        }
    }
    Some(out)
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
    let text = if raw.kind == "mms" {
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
            id: rules::Row { mms: true, id: int(f, "_id"), date: int(f, "date") }.name(),
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
            id: rules::Row { mms: false, id: int(f, "_id"), date: int(f, "date") }.name(),
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
    };
    // Placed in time as every line read is: its sending time from its columns (`Text::placed`).
    text.placed()
}

fn raws(answer: &Value) -> Vec<Raw> {
    answer["texts"].as_array().map(|a| a.iter().filter_map(|v| serde_json::from_value(v.clone()).ok()).collect()).unwrap_or_default()
}

/// Each part of a message that travels, copied from Android into the
/// private folder, hashed, put in the sharing as a sealed blob (never inside
/// a line), the copy deleted; within `budget` bytes, the first part of a
/// step always. Whether every part was brought (else the message waits for
/// the next step).
fn bring_media(android: &dyn Android, incoming: &Path, text: &mut Text, vault: &Vault, budget: &mut u64) -> bool {
    let (folder, key) = vault;
    for part in text.parts.iter_mut().filter(|p| p.state == "here" && p.hash.is_empty()) {
        if part.size > *budget && *budget < rules::MEDIA_BATCH {
            return false;
        }
        let copy = incoming.join(format!("{}-{}", text.id, part.seq));
        let answer = android.ask("texts-part", &json!({ "id": int(&part.fields, "_id"), "target": copy.display().to_string() }));
        if answer["allowed"] != true {
            return false;
        }
        if answer["copied"].as_i64().unwrap_or(-1) < 0 {
            // Gone meanwhile, or never downloaded: said so.
            part.state = "not-downloaded".into();
            continue;
        }
        let put = sioul_sync::blobs::hash_file(&copy).map_err(|e| e.to_string()).and_then(|(hash, size)| sioul_sync::blobs::put(folder, key, &copy, &hash).map(|_| (hash, size)));
        let _ = std::fs::remove_file(&copy);
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

/// What a step read: its texts as they travel, the media bytes put in the sharing.
#[derive(Default)]
struct Read {
    texts: Vec<Text>,
    media: u64,
}

/// The rows `wanted` read from Android in one question, taken in that order,
/// each as it travels, its media put in the sharing within `budget` (the
/// first medium of a step always). A multimedia message whose media do not
/// fit stops the multimedia messages there for this step; the plain texts
/// go on. A row gone meanwhile, or whose id another row took meanwhile, is
/// left to the next listing. None when Android did not answer.
fn read_rows(android: &dyn Android, files: &Files, wanted: &[rules::Row], vault: &Vault, region: Option<&phones::Region>, budget: &mut u64) -> Option<Read> {
    if wanted.is_empty() {
        return Some(Read::default());
    }
    let ids = |mms: bool| wanted.iter().filter(|r| r.mms == mms).map(|r| r.id).collect::<Vec<_>>();
    let answer = android.ask("texts-read", &json!({ "sms_ids": ids(false), "mms_ids": ids(true), "limit": wanted.len() }));
    if answer["allowed"] != true {
        return None;
    }
    let threads = threads_of(&answer);
    let cap = cap_mb() << 20;
    let mut rows: BTreeMap<(bool, i64), Raw> = raws(&answer).into_iter().map(|raw| ((raw.kind == "mms", int(&raw.fields, "_id")), raw)).collect();
    let mut read = Read::default();
    let mut mms_on = true;
    for row in wanted {
        if row.mms && !mms_on {
            continue;
        }
        let Some(raw) = rows.remove(&(row.mms, row.id)) else { continue };
        if int(&raw.fields, "date") != row.date {
            continue;
        }
        let mut text = text_of(&raw, &threads, region, cap);
        if row.mms {
            let before = *budget;
            if !bring_media(android, &files.incoming, &mut text, vault, budget) {
                mms_on = false;
                continue;
            }
            read.media += before - *budget;
        }
        read.texts.push(text);
    }
    Some(read)
}

/// Texts added to this phone's log, each line once: how many were written, or why none could be.
fn write_log(log: &Path, seal: &Seal, texts: &[Text]) -> Result<usize, String> {
    let line = |t: &Text| t.line_id();
    rules::append(log, texts, seal, Some(&line))
}

/// One step of the import (docs/texts.md, "Reading, on the phone: paced").
/// Every row the phone holds is listed with its date (`texts-ids`), and
/// those its log lacks are the ones to read, wherever their ids are (the
/// plain texts' table gives a deleted newest text's id to the next one; a
/// provider emptied starts again at 1). Rows that appeared since the last
/// step are read at once, on the battery too, oldest first; 500 or more at
/// once are no texts that came but a block of old ones (SMS Backup &
/// Restore restoring a history), read as a first import is: the newest
/// first by date, with `all` (on the charger) `BATCH` at a time, else only
/// those newer than the newest text read before and than a day before the
/// history was found, so that a text that comes meanwhile reaches your
/// computers within a minute or two. Media within `MEDIA_BATCH` bytes, the
/// first always. A step with nothing to read asks Android one listing.
/// Whether anything was written.
#[allow(clippy::too_many_arguments)]
fn import_step(android: &dyn Android, files: &Files, seal: &Seal, vault: &Vault, region: Option<&phones::Region>, now: i64, all: bool) -> bool {
    let mut reader = Reader::load_at(&files.reader);
    let loaded = reader.clone();
    let first = reader.started == 0;
    if first {
        reader.started = now;
    }
    // A reader of an older version, or one lost: the log tells which rows it holds.
    let older = !reader.indexed;
    if older {
        reader.index(&rules::read_file::<Text>(&files.log, seal));
    }
    let answer = android.ask("texts-ids", &json!({ "measure": false }));
    let Some(listed) = (answer["allowed"] == true).then(|| listed(&answer)).flatten() else {
        if first || older {
            let _ = reader.save_at(&files.reader);
        }
        return false;
    };
    let logged = reader.logged.set();
    let (was_backlog, was_arrived) = (reader.backlog.set(), reader.arrived.set());
    let unread: Vec<rules::Row> = listed.iter().filter(|r| !logged.contains(r)).copied().collect();
    let fresh: Vec<rules::Row> = unread.iter().filter(|r| !was_backlog.contains(r) && !was_arrived.contains(r)).copied().collect();
    let mut backlog: BTreeSet<rules::Row> = unread.iter().filter(|r| was_backlog.contains(r)).copied().collect();
    let mut arrived: BTreeSet<rules::Row> = unread.iter().filter(|r| was_arrived.contains(r)).copied().collect();
    if first || older || fresh.len() >= rules::BATCH {
        // A history: a first import, what the log of an older reader lacks,
        // or a block of old texts put at once.
        if !fresh.is_empty() {
            reader.since = now;
        }
        backlog.extend(fresh);
    } else {
        arrived.extend(fresh);
    }
    // What came first, oldest first; then the history, newest first by date:
    // on the battery only what is dated within a day before the history was
    // found (its last day: a few texts of a restore, and every row that came
    // with it, a picture dated in seconds under a text read in that second
    // included: review of 16 October 2026, LOW 2).
    let floor = reader.since.max(reader.started) - DAY_MS;
    let mut came: Vec<rules::Row> = arrived.iter().copied().collect();
    came.sort_by_key(|r| (r.at(), r.id));
    let mut history: Vec<rules::Row> = backlog.iter().copied().filter(|r| all || r.at() >= floor).collect();
    history.sort_by(|a, b| (b.at(), b.id).cmp(&(a.at(), a.id)));
    let mut wanted: Vec<rules::Row> = Vec::new();
    let mut pictures = 0;
    for row in came.into_iter().chain(history) {
        if wanted.len() >= rules::BATCH {
            break;
        }
        if row.mms {
            if pictures >= MMS_SHARE {
                continue;
            }
            pictures += 1;
        }
        wanted.push(row);
    }
    let mut budget = rules::MEDIA_BATCH;
    let read = read_rows(android, files, &wanted, vault, region, &mut budget).unwrap_or_default();
    // Rows logged before and no longer listed: deleted, or re-dated by their provider.
    let gone: BTreeSet<(bool, i64)> = logged.difference(&listed).map(|r| (r.mms, r.id)).collect();
    // A conversation deleted in Sioul: its texts are read (counted as logged, never read again) and never written.
    let marks = rules::Marks::read_all(files.log.parent().and_then(Path::parent).unwrap_or(Path::new("")), seal);
    let shown: Vec<Text> = read.texts.iter().filter(|t| !marks.hides(t)).cloned().collect();
    let written = match write_log(&files.log, seal, &shown) {
        Ok(written) => {
            // In the log now (a row it held already, read again after a stop, too).
            let rows: Vec<rules::Row> = read.texts.iter().filter_map(Text::row).collect();
            // A row read under a new date where its old line is logged: that line marked moved now, never "deleted" later.
            if rows.iter().any(|r| gone.contains(&(r.mms, r.id))) {
                let marks = rules::moved_marks(&rules::read_file::<Text>(&files.log, seal), &listed, now);
                if let Err(e) = write_log(&files.log, seal, &marks) {
                    eprintln!("sioul: texts: {e}");
                }
            }
            for row in &rows {
                backlog.remove(row);
                arrived.remove(row);
            }
            reader.logged = Rows::of(logged.into_iter().chain(rows));
            reader.newest_at = read.texts.iter().map(|t| t.at).fold(reader.newest_at, i64::max);
            reader.done_media += read.media;
            written
        }
        Err(e) => {
            // Not in the log: read again at the next step.
            eprintln!("sioul: texts: {e}");
            0
        }
    };
    reader.backlog = Rows::of(backlog);
    reader.arrived = Rows::of(arrived);
    reader.behind = !reader.backlog.is_empty();
    reader.left = !reader.arrived.is_empty();
    reader.done_texts = reader.logged.len() as u64;
    // Written only when it changed: most steps find nothing new, and it holds every row of the log.
    if reader != loaded
        && let Err(e) = reader.save_at(&files.reader)
    {
        eprintln!("sioul: texts: {e}");
    }
    if written > 0 {
        // Never what they say: how many, and how much.
        eprintln!("sioul: texts: {written} read for your computers ({} so far, {} bytes of media)", reader.done_texts, reader.done_media);
    }
    written > 0
}

/// Once a day: what the phone holds measured for the settings; the log's
/// own counts taken again (`Reader::index`); and each text gone from the
/// phone marked so in the log, a line each, once (the archive keeps the
/// text): a row the phone no longer lists, by its id and its date, so that
/// a text deleted and the next one taking its id are told apart. The rows
/// to read are the steps' own (`import_step`). Whether a line was written.
fn look_again(android: &dyn Android, files: &Files, seal: &Seal, now: i64) -> bool {
    let mut reader = Reader::load_at(&files.reader);
    if now - reader.looked < DAY_MS {
        return false;
    }
    let answer = android.ask("texts-ids", &json!({ "measure": true }));
    if answer["allowed"] != true {
        return false;
    }
    let Some(listed) = listed(&answer) else {
        // A table Android could not list whole: the look waits an hour, never
        // listing (and measuring) again at every step meanwhile (review of
        // 16 October 2026, LOW 4); nothing is said deleted.
        reader.looked = now - DAY_MS + LOOK_AGAIN_MS;
        if let Err(e) = reader.save_at(&files.reader) {
            eprintln!("sioul: texts: {e}");
        }
        return false;
    };
    reader.looked = now;
    reader.measure(&answer, now);
    let log: Vec<Text> = rules::read_file(&files.log, seal);
    reader.index(&log);
    if let Err(e) = reader.save_at(&files.reader) {
        eprintln!("sioul: texts: {e}");
    }
    // A row whose date its provider rewrote: its old line marked moved, under its new name (review of
    // 16 October 2026, LOW 1: it showed twice, once "deleted on your phone"); the rest gone, marked deleted.
    let mut marks = rules::moved_marks(&log, &listed, now);
    let moved: BTreeSet<String> = marks.iter().map(|t| t.id.clone()).collect();
    let marked: BTreeSet<&str> = log.iter().filter(|t| t.deleted > 0).map(|t| t.id.as_str()).collect();
    marks.extend(
        log.iter()
            .filter(|t| t.deleted == 0 && !marked.contains(t.id.as_str()) && !moved.contains(&t.id) && t.row().is_some_and(|r| !listed.contains(&r)))
            .map(|t| Text { id: t.id.clone(), at: t.at, direction: t.direction.clone(), with: t.with.clone(), deleted: now, ..Text::default() }),
    );
    let written = write_log(&files.log, seal, &marks).unwrap_or_else(|e| {
        eprintln!("sioul: texts: {e}");
        0
    });
    if written > 0 {
        eprintln!("sioul: texts: {written} deleted on the phone, marked so");
    }
    written > 0
}

// ---------------------------------------------------------------- the phone: sending

/// A day's look that could not list every row is tried again this long after.
const LOOK_AGAIN_MS: i64 = 3_600_000;

/// One result, as Java wrote it (TextSend.line): a text's part sent or
/// delivered; or a multimedia message's word ("mms"): Android's code, the
/// row it filed, the HTTP status, the carrier's answer (M-Send.conf, hex).
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
    http: i64,
    conf: String,
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

/// "content://sms/414" → "sms-414", "content://mms/sent/31" → "mms-31": the
/// row as the log names it, its date read after (`name_rows`).
fn row_of(uri: &str) -> String {
    let uri = uri.trim();
    let table = if uri.starts_with("content://mms/") { "mms" } else { "sms" };
    uri.rsplit('/').next().filter(|id| !id.is_empty() && id.chars().all(|c| c.is_ascii_digit())).map(|id| format!("{table}-{id}")).unwrap_or_default()
}

/// Each text's outcomes from its parts' results: sent when every part was;
/// not sent, and why, when one was not; delivered when the carrier said every
/// part was received (never "not delivered" for a report that did not come).
fn outcomes_from(results: &[PartResult]) -> Vec<Outcome> {
    let mut by: BTreeMap<&str, Vec<&PartResult>> = BTreeMap::new();
    for r in results.iter().filter(|r| !r.key.is_empty() && r.kind != "mms") {
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

/// Hex's bytes; none for what is not hex.
fn unhex(hex: &str) -> Option<Vec<u8>> {
    let hex = hex.trim();
    if hex.len() % 2 != 0 {
        return None;
    }
    (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok()).collect()
}

/// Each multimedia message's outcome from its result (`texts::took`, with
/// the HTTP status Java read): sent (Android's Ok and the carrier's), not
/// sent and why, in doubt when each of Android's tries ended on the network
/// ("network": the carrier may have it); and the keys whose result is
/// Android's Ok without the carrier's answer, for the sent box to say
/// (`Step::copy_results`). Never "delivered": no report is asked.
fn mms_outcomes(results: &[PartResult]) -> (Vec<Outcome>, Vec<(String, i64)>) {
    let (mut out, mut look) = (Vec::new(), Vec::new());
    for r in results.iter().filter(|r| r.kind == "mms" && !r.key.is_empty()) {
        let conf = unhex(&r.conf).and_then(|bytes| sioul_core::mms::conf(&bytes));
        match rules::took(r.code, conf.as_ref(), r.http) {
            rules::Took::Sent => out.push(Outcome { key: r.key.clone(), state: "sent".into(), at: r.at, row: row_of(&r.uri), ..Outcome::default() }),
            rules::Took::Failed(why) => out.push(Outcome { key: r.key.clone(), state: "failed".into(), at: r.at, reason: why.into(), ..Outcome::default() }),
            rules::Took::Doubt(why) => out.push(Outcome { key: r.key.clone(), state: "doubt".into(), at: r.at, reason: why.into(), ..Outcome::default() }),
            rules::Took::Look => look.push((r.key.clone(), r.at)),
        }
    }
    (out, look)
}

/// Whether Java could look in the provider (`Texts.findMms`): the permission
/// given and the query answered. A look not made is never read as "not
/// there": made again at the next step.
fn looked(answer: &Value) -> bool {
    answer["allowed"] == true && answer["error"] != true
}

/// The row Sioul's multimedia message was filed as in the sent box, by its
/// key (`tr_id`), as Java found it (`Texts.findMms`: Sioul's rows alone):
/// its name in the log; none in the sent box (a failed one is out of sight).
fn sent_row(answer: &Value) -> Option<String> {
    answer["found"].as_array()?.iter().find(|row| row["box"] == 2).map(|row| rules::Row { mms: true, id: row["id"].as_i64().unwrap_or(0), date: row["date"].as_i64().unwrap_or(0) }.name())
}

/// The carrier's refusal Android read for a message it filed among the
/// failed (box 5), its status (`resp_st`, as `Texts.findMms` gives it), when
/// it read one: none when Android filed it there with no answer it could
/// read (`SendRequest.persistIfRequired`).
fn failed_status(answer: &Value) -> Option<u8> {
    answer["found"].as_array()?.iter().filter(|row| row["box"] == 5).filter_map(|row| u8::try_from(row["status"].as_i64()?).ok()).find(|status| *status > sioul_core::mms::OK)
}

/// Java's results set aside at the last trim (`trim_results`), read beside
/// its file until the next.
const RESULTS_ASIDE: &str = "results-aside.jsonl";

/// Java's results (`TextSend.Result`'s lines): its file, and the one set
/// aside at the last trim; only those of the outcomes' month
/// (`rules::KEPT_DAYS`), so that a result whose outcome was trimmed is
/// never said, written and shared again (the review of 18 October 2026, I-f).
fn results_of(private: &Path, now: i64) -> Vec<PartResult> {
    [rules::RESULTS, RESULTS_ASIDE]
        .iter()
        .flat_map(|name| std::fs::read_to_string(private.join(name)).unwrap_or_default().lines().filter_map(|l| serde_json::from_str::<PartResult>(l).ok()).collect::<Vec<_>>())
        .filter(|r| now - r.at <= rules::KEPT_DAYS * DAY_MS)
        .collect()
}

/// Java's results file kept to the outcomes' month, once a day, in the
/// phone's step under its lock (the review of 18 October 2026, I-f). Java
/// appends to it by its name at any moment, from either of the phone's
/// processes: it is never written over. It is set aside by a rename, and
/// the lines kept are appended to the new one Java's next line makes; the
/// file set aside is read too until the next trim, which first brings back
/// the line Java may have appended to it as it was renamed.
fn trim_results(private: &Path, now: i64) -> std::io::Result<()> {
    let current = private.join(rules::RESULTS);
    let aside = private.join(RESULTS_ASIDE);
    let kept = |text: &str| -> Vec<String> { text.lines().filter(|l| serde_json::from_str::<PartResult>(l).is_ok_and(|r| now - r.at <= rules::KEPT_DAYS * DAY_MS)).map(str::to_string).collect() };
    let append = |lines: &[String]| -> std::io::Result<()> {
        if lines.is_empty() {
            return Ok(());
        }
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new().create(true).append(true).open(&current)?;
        file.write_all(lines.iter().map(|l| format!("{l}\n")).collect::<String>().as_bytes())?;
        file.sync_all()
    };
    if let Ok(text) = std::fs::read_to_string(&aside) {
        let there: BTreeSet<String> = std::fs::read_to_string(&current).unwrap_or_default().lines().map(str::to_string).collect();
        append(&kept(&text).into_iter().filter(|l| !there.contains(l)).collect::<Vec<_>>())?;
        std::fs::remove_file(&aside)?;
    }
    match std::fs::rename(&current, &aside) {
        Ok(()) => append(&kept(&std::fs::read_to_string(&aside)?)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// What each SIM's carrier lets the phone send, as Java read it (`Texts.limits`).
fn limits_from(answer: &Value) -> Vec<rules::Limits> {
    answer["limits"].as_array().map(|all| all.iter().filter_map(|l| serde_json::from_value::<rules::Limits>(l.clone()).ok()).filter(|l| l.max > 0).collect()).unwrap_or_default()
}

/// Android's own maximum when the carrier's file says nothing (bytes): 300 kB.
const ANDROID_MAX: u64 = 300 * 1024;

/// Whether a multimedia message's media are all in the sharing (the folder,
/// or the copy fetched from its server beside it), not opened.
fn sealed_here(vault: &Vault, hash: &str) -> bool {
    let (folder, key) = vault;
    let there = |dir: &Path| std::fs::metadata(dir.join("blobs").join(sioul_sync::blobs::name(key, hash))).is_ok_and(|m| m.len() > 0);
    there(folder) || sioul_sync::remote::overlay(folder).is_some_and(|copy| there(&copy))
}

/// A multimedia message made ready on the phone, before its claim.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Prepared {
    /// Its message written, to go from that SIM.
    Ready { sub: i32 },
    /// A medium not here yet: no claim; asked for, looked at again at the next step.
    Waiting,
    /// Refused, said: larger than its SIM's carrier takes, or that carrier takes none.
    Refused(&'static str),
}

/// What the phone's quick step works with: where it reads and writes (the
/// data folder's texts, the private texts folder, the other copies of the
/// requests, the sharing's memory), the sharing's folder and key, Android
/// (a stand-in in tests).
struct Step<'a> {
    here: &'a str,
    seal: &'a Seal,
    region: Option<&'static phones::Region>,
    root: PathBuf,
    private: PathBuf,
    copies: Vec<PathBuf>,
    memory: PathBuf,
    vault: Option<&'a Vault>,
    android: &'a dyn Android,
    /// Where a device that asks stands in the sharing now (`standing_of`):
    /// a request is sent only for one counted, refused for one gone, left
    /// to wait for one not known yet (`texts::verdict`).
    standing: &'a dyn Fn(&str) -> rules::Standing,
}

impl Step<'_> {
    /// This phone's.
    fn here<'a>(here: &'a str, seal: &'a Seal, region: Option<&'static phones::Region>, vault: Option<&'a Vault>, standing: &'a dyn Fn(&str) -> rules::Standing) -> Step<'a> {
        Step { here, seal, region, root: root(), private: private(), copies: crate::share::texts_copies(), memory: crate::share::memory_path(), vault, android: &Phone, standing }
    }

    /// Outcomes added to this phone's own file, each (key, state) once.
    fn say(&self, outcomes: &[Outcome]) -> bool {
        let id = |o: &Outcome| format!("{}/{}", o.key, o.state);
        match rules::append(&rules::own_file(&self.root, rules::OUTCOME, self.here), outcomes, self.seal, Some(&id)) {
            Ok(n) => n > 0,
            Err(e) => {
                eprintln!("sioul: texts: {e}");
                false
            }
        }
    }

    fn record(&self, claim: &Claim) {
        if let Err(e) = rules::record(&self.private, claim) {
            eprintln!("sioul: texts: {e}");
        }
    }

    /// Where a multimedia message waits for Android, written before its
    /// claim: Android reads it by its address (Sioul's file provider), and
    /// it goes once its outcome is written.
    fn pdu_path(&self, key: &str) -> PathBuf {
        self.private.join(OUTGOING).join(format!("{}.pdu", key.chars().filter(char::is_ascii_hexdigit).collect::<String>()))
    }

    /// Java's results copied into this phone's outcomes, each row named
    /// (`name_rows`: those not said yet), and the ledger told. A multimedia
    /// message Android took with no answer of the carrier's is looked for in
    /// the sent box: there, sent; else not sent (Android filed it among the
    /// failed, which Sioul cannot see). A multimedia message's file goes once
    /// its outcome is written.
    fn copy_results(&self, now: i64) -> bool {
        let results = results_of(&self.private, now);
        if results.is_empty() {
            return false;
        }
        let said: BTreeSet<String> = rules::read_file::<Outcome>(&rules::own_file(&self.root, rules::OUTCOME, self.here), self.seal).iter().map(|o| format!("{}/{}", o.key, o.state)).collect();
        let mut outcomes = outcomes_from(&results);
        let (mms, look) = mms_outcomes(&results);
        outcomes.extend(mms);
        for (key, at) in look {
            if ["sent", "failed", "doubt"].iter().any(|state| said.contains(&format!("{key}/{state}"))) {
                continue;
            }
            let answer = self.android.ask("texts-find-mms", &json!({ "key": key }));
            // The sent box not readable now (the permission withdrawn, the query failed): looked at again at the
            // next step, and by the look for lost claims until Android's last word; never said refused for it.
            if !looked(&answer) {
                continue;
            }
            // In the sent box: sent. Among the failed with the carrier's refusal Android read: not sent, and why.
            // Else Android filed it among the failed with an answer nobody could read, after the carrier's HTTP
            // 2xx (Android's Ok comes only after one): the carrier may have it (the review's re-check, I-g).
            outcomes.push(match (sent_row(&answer), failed_status(&answer)) {
                (Some(row), _) => Outcome { key, state: "sent".into(), at, row, ..Outcome::default() },
                (None, Some(status)) => Outcome { key, state: "failed".into(), at, reason: rules::mms_failure(-1, Some(status)).into(), ..Outcome::default() },
                (None, None) => Outcome { key, state: "doubt".into(), at, reason: "answer".into(), ..Outcome::default() },
            });
        }
        let mut new: Vec<Outcome> = outcomes.iter().filter(|o| !said.contains(&format!("{}/{}", o.key, o.state))).cloned().collect();
        name_rows(self.android, &mut new);
        let changed = self.say(&new);
        let ledger = rules::Ledger::open(&self.private, now).ok();
        for o in &outcomes {
            if ledger.as_ref().is_some_and(|l| l.state(&o.key) != Some("done")) {
                self.record(&Claim { key: o.key.clone(), at: now, state: "done".into(), ..Claim::default() });
            }
            let _ = std::fs::remove_file(self.pdu_path(&o.key));
        }
        changed
    }

    /// A multimedia message made ready before its claim (decision 12 and
    /// 13): its media looked for first, by their files alone (one not here
    /// yet asked for by its name, nothing else done, nothing claimed); then
    /// opened from the sharing, each checked against its hash; its
    /// message built (`sioul_core::mms::encode`) and measured against what its
    /// SIM's carrier takes now (Android's live value, never one raised:
    /// larger, refused in words, never left to Android's code 5); written and
    /// flushed where Android will read it. The SIM: as asked, else the
    /// conversation's last (`sim_for`).
    fn prepare(&self, r: &Request, log: &std::cell::OnceCell<Vec<Text>>, now: i64) -> Prepared {
        let Some(vault) = self.vault else { return Prepared::Waiting };
        if r.key.len() != 32 || !r.key.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Prepared::Refused("other");
        }
        // Its media first, by a look at their files alone: one not here yet is asked for by its name, and nothing
        // else is done (neither the log nor Android asked, no medium opened), so that every step while a message
        // waits for its media, a dose's alarm's included, stays as quick as one with nothing to send (the review
        // of 18 October 2026, L-a). The log is read in the step that sends.
        let absent: Vec<rules::Medium> = r.media.iter().filter(|m| !sealed_here(vault, &m.hash)).cloned().collect();
        if !absent.is_empty() {
            self.want(&r.media, &absent);
            return Prepared::Waiting;
        }
        let to = phones::key(&r.to, self.region);
        let sub = self.sim_for(r, log);
        let answer = self.android.ask("texts-limits", &json!({ "sub": sub }));
        let limits = limits_from(&answer);
        let carrier = limits.iter().find(|l| l.sub == sub).or_else(|| limits.first());
        if carrier.is_some_and(|l| !l.mms) {
            return Prepared::Refused("no-mms");
        }
        let max = carrier.map_or(ANDROID_MAX, |l| l.max);
        // Never more brought than could go.
        if r.media.iter().map(|m| m.size).sum::<u64>() > max {
            return Prepared::Refused("too-large");
        }
        let (folder, key) = vault;
        let _ = std::fs::create_dir_all(self.private.join(OUTGOING));
        let opened = self.private.join(OUTGOING).join(format!("{}.media", self.pdu_path(&r.key).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()));
        let mut bytes = Vec::new();
        let mut missing = Vec::new();
        for medium in &r.media {
            let ok = sioul_sync::blobs::get(folder, key, &medium.hash, &opened).is_ok();
            let read = if ok { std::fs::read(&opened).ok() } else { None };
            let _ = std::fs::remove_file(&opened);
            match read {
                Some(read) if read.len() as u64 == medium.size => bytes.push(read),
                Some(_) => return Prepared::Refused("media-kind"),
                None => missing.push(medium.clone()),
            }
        }
        self.want(&r.media, &missing);
        if !missing.is_empty() {
            return Prepared::Waiting;
        }
        let media: Vec<sioul_core::mms::Medium> = r.media.iter().zip(&bytes).map(|(m, b)| sioul_core::mms::Medium { ct: &m.ct, bytes: b }).collect();
        let message = sioul_core::mms::Message { key: &r.key, to: &[to.as_str()], date: now / 1000, words: &r.words, media: &media };
        let Ok(pdu) = sioul_core::mms::encode(&message) else { return Prepared::Refused("other") };
        if pdu.len() as u64 > max {
            return Prepared::Refused("too-large");
        }
        match write_flushed(&self.pdu_path(&r.key), &pdu) {
            Ok(()) => Prepared::Ready { sub },
            // A disk too full, a moment: no claim, tried again at the next step.
            Err(e) => {
                eprintln!("sioul: texts: {e}");
                Prepared::Waiting
            }
        }
    }

    /// The SIM a request about to go leaves from: as asked, else the
    /// conversation's last on this phone, its log read then and once a step
    /// at most (`log`): never for requests decided already, which a step
    /// meets at every exchange and inside a dose's alarm (the review of 18
    /// October 2026, M-1).
    fn sim_for(&self, r: &Request, log: &std::cell::OnceCell<Vec<Text>>) -> i32 {
        if r.sub >= 0 {
            return r.sub;
        }
        let texts = log.get_or_init(|| {
            #[cfg(test)]
            LOG_READS.with(|reads| reads.set(reads.get() + 1));
            rules::read_logs(&self.root, self.seal)
        });
        rules::last_sim(&rules::conversation_id(&[phones::key(&r.to, self.region)]), self.here, texts)
    }

    /// A multimedia message's media asked for by their names (`blobs::Wanted::media`,
    /// which the pull's fetch brings from the server, and Sioul keeping the
    /// folder itself too): those missing; those here no longer.
    fn want(&self, media: &[rules::Medium], missing: &[rules::Medium]) {
        let Some((_, key)) = self.vault else { return };
        let name = |m: &rules::Medium| sioul_sync::blobs::name(key, &m.hash);
        let wanted = sioul_sync::blobs::Wanted::load(&self.memory);
        let asked = missing.iter().any(|m| !wanted.media.contains_key(&name(m)));
        let done = media.iter().filter(|m| !missing.contains(m)).any(|m| wanted.media.contains_key(&name(m)));
        if !asked && !done {
            return;
        }
        let now = jiff::Timestamp::now().as_second();
        let _ = sioul_sync::blobs::Wanted::change(&self.memory, |wanted| {
            for m in media.iter().filter(|m| !missing.contains(m)) {
                wanted.media.remove(&name(m));
            }
            for m in missing {
                wanted.media.entry(name(m)).or_insert_with(|| sioul_sync::blobs::Want { hash: m.hash.clone(), size: m.size, since: now, ..Default::default() });
            }
        });
    }

    /// Each request decided once, under the ledger's lock (both processes of the
    /// phone may step): claimed and flushed before Android is asked; the others
    /// said (expired, refused, clocks apart) and written in the ledger, so that
    /// they are decided once too. A multimedia message is made ready first
    /// (`prepare`), and claimed only once every medium is here and checked:
    /// until then nothing is claimed, and at its expiry it is said that its
    /// media did not come. Whether an outcome was written.
    fn decide(&self, now: i64) -> bool {
        // The records' copy, then the same requests in their own small files, the
        // sharing folder's and the copy fetched from its server: one reaches this
        // phone within its quarter of an hour, whatever the records' size
        // (docs/database.md, "Sent to the server too").
        let roots: Vec<PathBuf> = std::iter::once(self.root.clone()).chain(self.copies.iter().cloned()).collect();
        let requests = requests_from(&roots, self.seal);
        if requests.iter().all(|r| r.phone != self.here) {
            return false;
        }
        let mine = |r: &&Request| r.phone == self.here;
        // The phone's log, read only when a request about to go names no SIM (`sim_for`).
        let log = std::cell::OnceCell::new();
        // The multimedia messages that may go, made ready outside the ledger's
        // lock (the step's lock keeps the phone's other process out).
        let mut ready: BTreeMap<String, Prepared> = BTreeMap::new();
        if let Ok(ledger) = rules::Ledger::open(&self.private, now) {
            for r in requests.iter().filter(mine).filter(|r| r.multimedia()) {
                if rules::verdict(r, self.here, now, &ledger, self.region, self.standing) == Verdict::Send {
                    ready.insert(r.key.clone(), self.prepare(r, &log, now));
                }
            }
        }
        let ledger_path = self.private.join(rules::LEDGER);
        let mut said = Vec::new();
        let mut to_send = Vec::new();
        let mut to_send_mms: Vec<(Request, i32)> = Vec::new();
        let claimed = sioul_core::filelock::with_lock(&ledger_path, || -> Result<(), String> {
            let ledger = rules::Ledger::open(&self.private, now)?;
            for r in &requests {
                let verdict = rules::verdict(r, self.here, now, &ledger, self.region, self.standing);
                let (state, reason) = match verdict {
                    // Its device not known yet: judged again at the next step.
                    Verdict::NotMine | Verdict::Decided | Verdict::Wait => continue,
                    Verdict::Send if r.multimedia() => match ready.get(&r.key) {
                        Some(Prepared::Ready { sub }) => {
                            rules::record(&self.private, &Claim { key: r.key.clone(), at: now, state: "claimed".into(), to: phones::key(&r.to, self.region), hash: rules::words_hash(&r.words), mms: true, ..Claim::default() })?;
                            to_send_mms.push((r.clone(), *sub));
                            continue;
                        }
                        Some(Prepared::Refused(why)) => ("refused", *why),
                        // A medium not here yet: nothing claimed, nothing said.
                        _ => continue,
                    },
                    Verdict::Send => {
                        rules::record(&self.private, &Claim { key: r.key.clone(), at: now, state: "claimed".into(), to: phones::key(&r.to, self.region), hash: rules::words_hash(&r.body), ..Claim::default() })?;
                        to_send.push(r.clone());
                        continue;
                    }
                    Verdict::Expired if r.multimedia() && self.vault.is_some_and(|vault| r.media.iter().any(|m| !sealed_here(vault, &m.hash))) => ("expired", "media"),
                    Verdict::Expired => ("expired", ""),
                    Verdict::Clocks => ("clocks", ""),
                    Verdict::BeforeMark => ("refused", "before-mark"),
                    Verdict::Refused(why) => ("refused", why),
                };
                rules::record(&self.private, &Claim { key: r.key.clone(), at: now, state: state.into(), ..Claim::default() })?;
                said.push(Outcome { key: r.key.clone(), state: state.into(), at: now, reason: reason.into(), ..Outcome::default() });
                if r.multimedia() {
                    // Asked for no more.
                    self.want(&r.media, &[]);
                }
            }
            Ok(())
        });
        // Made ready and not claimed (refused, or decided meanwhile): its file goes.
        for (key, prepared) in &ready {
            if !to_send_mms.iter().any(|(r, _)| &r.key == key) && *prepared != Prepared::Waiting {
                let _ = std::fs::remove_file(self.pdu_path(key));
            }
        }
        if let Err(e) = claimed {
            // A claim not flushed: nothing is handed to Android.
            eprintln!("sioul: texts: {e}");
            for (r, _) in &to_send_mms {
                let _ = std::fs::remove_file(self.pdu_path(&r.key));
            }
            return self.say(&said);
        }
        for r in to_send {
            // The SIM: as asked, else the conversation's last on this phone.
            let sub = self.sim_for(&r, &log);
            let answer = self.android.ask("texts-send", &json!({ "key": r.key, "to": phones::key(&r.to, self.region), "body": r.body, "sub": sub }));
            if answer["handed"] == true {
                self.record(&Claim { key: r.key.clone(), at: now_ms(), state: "handed".into(), ..Claim::default() });
                // Their next texts come through at once for twenty minutes, as after a reply (`replies`).
                crate::replies::texted(&phones::key(&r.to, self.region));
                said.push(Outcome { key: r.key.clone(), state: "handed".into(), at: now_ms(), ..Outcome::default() });
                eprintln!("sioul: texts: one handed to Android ({} part(s))", answer["parts"].as_i64().unwrap_or(1));
            } else if answer.is_null() {
                // No answer from Java at all: whether Android took it is not known. Looked for later, never sent again.
                eprintln!("sioul: texts: Android's answer did not come; the text is looked for later");
            } else {
                let reason = answer["reason"].as_str().unwrap_or("other").to_string();
                self.record(&Claim { key: r.key.clone(), at: now_ms(), state: "done".into(), ..Claim::default() });
                said.push(Outcome { key: r.key.clone(), state: "failed".into(), at: now_ms(), reason, ..Outcome::default() });
            }
        }
        for (r, sub) in to_send_mms {
            let path = self.pdu_path(&r.key);
            let answer = self.android.ask("texts-send-mms", &json!({ "key": r.key, "sub": sub, "path": path }));
            if answer["handed"] == true {
                self.record(&Claim { key: r.key.clone(), at: now_ms(), state: "handed".into(), ..Claim::default() });
                crate::replies::texted(&phones::key(&r.to, self.region));
                said.push(Outcome { key: r.key.clone(), state: "handed".into(), at: now_ms(), ..Outcome::default() });
                eprintln!("sioul: texts: one multimedia message handed to Android");
            } else if answer.is_null() {
                // Looked for by its key 40 minutes on, never sent again.
                eprintln!("sioul: texts: Android's answer did not come; the multimedia message is looked for later");
            } else {
                let reason = answer["reason"].as_str().unwrap_or("other").to_string();
                self.record(&Claim { key: r.key.clone(), at: now_ms(), state: "done".into(), ..Claim::default() });
                said.push(Outcome { key: r.key.clone(), state: "failed".into(), at: now_ms(), reason, ..Outcome::default() });
                let _ = std::fs::remove_file(&path);
            }
        }
        self.say(&said)
    }

    /// A claim left without Android's word (Sioul stopped between the claim and
    /// the hand-off, or a phone restarted while Android waited): a text's
    /// looked for among the texts Sioul sent since, by its number and its
    /// words' hash, two minutes on; a multimedia message's by its key (the
    /// provider's `tr_id`), handed or not, at each step from
    /// `LOOK_AFTER_MMS_MS` on, until Android's last word (`MMS_LAST_WORD_MS`:
    /// it may still go until then). Found, sent; else in doubt. Never sent
    /// again.
    fn look_for_lost(&self, now: i64) -> bool {
        let Ok(ledger) = rules::Ledger::open(&self.private, now) else { return false };
        let lost: Vec<&Claim> = ledger.claims.iter().filter(|c| c.state == "claimed" && !c.mms && ledger.state(&c.key) == Some("claimed") && now - c.at >= rules::LOOK_AFTER_MS).collect();
        let lost_mms: Vec<&Claim> = ledger.claims.iter().filter(|c| c.state == "claimed" && c.mms && matches!(ledger.state(&c.key), Some("claimed" | "handed")) && now - c.at >= rules::LOOK_AFTER_MMS_MS).collect();
        let mut said = Vec::new();
        if !lost.is_empty() {
            let since = lost.iter().map(|c| c.at).min().unwrap_or(now) - 60_000;
            let answer = self.android.ask("texts-find", &json!({ "since": since }));
            if answer["allowed"] == true {
                let sent: Vec<Raw> = raws(&answer);
                for claim in lost {
                    let found = sent
                        .iter()
                        .find(|r| int(&r.fields, "date") >= claim.at - 60_000 && rules::words_hash(&string(&r.fields, "body")) == claim.hash && phones::key(&string(&r.fields, "address"), self.region) == claim.to)
                        .map(|r| rules::Row { mms: false, id: int(&r.fields, "_id"), date: int(&r.fields, "date") }.name());
                    if let Some(outcome) = rules::doubt_after(claim, found, now) {
                        self.record(&Claim { key: claim.key.clone(), at: now, state: "done".into(), ..Claim::default() });
                        said.push(outcome);
                    }
                }
            }
        }
        for claim in lost_mms {
            let answer = self.android.ask("texts-find-mms", &json!({ "key": claim.key }));
            // A look not made is made again at the next step; past Android's last word, the doubt all the same.
            if !looked(&answer) && now - claim.at < rules::MMS_LAST_WORD_MS {
                continue;
            }
            let found = if looked(&answer) { sent_row(&answer) } else { None };
            if let Some(outcome) = rules::doubt_after(claim, found, now) {
                self.record(&Claim { key: claim.key.clone(), at: now, state: "done".into(), ..Claim::default() });
                let _ = std::fs::remove_file(self.pdu_path(&claim.key));
                said.push(outcome);
            }
        }
        self.say(&said)
    }

    /// What each SIM's carrier lets this phone send, said in its outcomes
    /// (decision 5: a computer offers Attach… only for a phone that said it,
    /// and reduces to it): when it changed, else once a day. Numbers only,
    /// never a carrier's name.
    fn publish_limits(&self, now: i64) -> bool {
        let limits = limits_from(&self.android.ask("texts-limits", &json!({})));
        if limits.is_empty() {
            return false;
        }
        let own = rules::read_file::<Outcome>(&rules::own_file(&self.root, rules::OUTCOME, self.here), self.seal);
        let last = own.iter().filter(|o| o.key.is_empty() && o.state == "limits").max_by_key(|o| o.at);
        if last.is_some_and(|o| o.limits == limits && (0..DAY_MS).contains(&(now - o.at))) {
            return false;
        }
        let line = Outcome { state: "limits".into(), at: now, limits, ..Outcome::default() };
        let id = |o: &Outcome| format!("limits/{}", o.at);
        match rules::append(&rules::own_file(&self.root, rules::OUTCOME, self.here), std::slice::from_ref(&line), self.seal, Some(&id)) {
            Ok(n) => n > 0,
            Err(e) => {
                eprintln!("sioul: texts: {e}");
                false
            }
        }
    }

    /// The long texts still on their way to this phone (docs/texts.md,
    /// "Parts still on their way"): the parts Android holds of each until its
    /// last one comes, read by Java (`texts-arriving`: senders, counts and
    /// times, never words), grouped (`texts::arriving_from`), said in an
    /// "arriving" line of this phone's outcomes when they changed, and once
    /// more, with none, when the last parts came. Nothing when Android did
    /// not answer, nor while nothing was ever on its way.
    fn publish_arriving(&self, now: i64) -> bool {
        let answer = self.android.ask("texts-arriving", &json!({}));
        let Some(rows) = answer["raw"].as_array().filter(|_| answer["allowed"] == true) else { return false };
        let arriving = rules::arriving_from(rows, self.region);
        let file = rules::own_file(&self.root, rules::OUTCOME, self.here);
        let mut said = ARRIVING_SAID.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        // What this phone's outcomes said last, read from its file once in this process.
        let last = said.entry(file.clone()).or_insert_with(|| rules::read_file::<Outcome>(&file, self.seal).into_iter().filter(|o| o.key.is_empty() && o.state == "arriving").max_by_key(|o| o.at).map(|o| o.arriving).unwrap_or_default());
        if *last == arriving {
            return false;
        }
        let line = Outcome { state: "arriving".into(), at: now, arriving: arriving.clone(), ..Outcome::default() };
        let id = |o: &Outcome| format!("arriving/{}", o.at);
        match rules::append(&file, std::slice::from_ref(&line), self.seal, Some(&id)) {
            Ok(n) => {
                *last = arriving;
                n > 0
            }
            Err(e) => {
                eprintln!("sioul: texts: {e}");
                false
            }
        }
    }

    /// A multimedia message's file left behind (a crash before its claim, an
    /// outcome written by another way): gone once its key is done, or an
    /// hour old with no claim.
    fn sweep_outgoing(&self, now: i64) {
        let Ok(entries) = std::fs::read_dir(self.private.join(OUTGOING)) else { return };
        let ledger = rules::Ledger::open(&self.private, now).ok();
        for entry in entries.filter_map(Result::ok) {
            let name = entry.file_name().to_string_lossy().to_string();
            let key = name.trim_end_matches(".pdu").trim_end_matches(".media");
            let state = ledger.as_ref().and_then(|l| l.state(key).map(str::to_string));
            let old = entry.metadata().ok().and_then(|m| m.modified().ok()).and_then(|t| t.elapsed().ok()).is_some_and(|age| age.as_secs() > 3600);
            if state.as_deref() == Some("done") || (state.is_none() && old) || (state.as_deref().is_some_and(|s| !matches!(s, "claimed" | "handed")) && old) {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}

/// Where a multimedia message waits on the phone, in the private texts folder.
const OUTGOING: &str = "outgoing";

#[cfg(test)]
thread_local! {
    /// How many times this thread's steps read the phone's log (`Step::sim_for`): tests count them.
    static LOG_READS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// A private file written whole beside, flushed to the disk, then put in place.
fn write_flushed(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let fail = |e: std::io::Error| format!("{}: {e}", path.display());
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(fail)?;
    }
    let beside = path.with_extension("part");
    let mut options = std::fs::OpenOptions::new();
    options.create(true).write(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options.open(&beside).map_err(fail)?;
    file.write_all(bytes).map_err(fail)?;
    file.sync_all().map_err(fail)?;
    std::fs::rename(&beside, path).map_err(fail)
}

/// When the carriers' limits were last asked in this process (ms): once an hour at most.
static LIMITS_ASKED: Mutex<i64> = Mutex::new(0);
/// What each phone's outcomes last said of the long texts on their way to
/// it, by its file, in this process (`Step::publish_arriving`): its file read
/// once, never at each step.
static ARRIVING_SAID: Mutex<BTreeMap<PathBuf, Vec<rules::Arriving>>> = Mutex::new(BTreeMap::new());
/// When Java's results were last trimmed in this process (ms): once a day.
static RESULTS_TRIMMED: Mutex<i64> = Mutex::new(0);

/// Where a device stands, by the sharing's two sets (`share::asking`).
fn standing_in(counted: &BTreeSet<String>, gone: &BTreeSet<String>, device: &str) -> rules::Standing {
    if counted.contains(device) {
        rules::Standing::Counted
    } else if gone.contains(device) {
        rules::Standing::Gone
    } else {
        rules::Standing::Unknown
    }
}

/// Where each device that may ask this phone stands in the sharing now
/// (`share::asking`), for the replies' step (replies.rs).
pub(crate) fn standing_of(folder: &Path, key: &[u8; 32], here: &str, now: i64) -> impl Fn(&str) -> rules::Standing {
    let (counted, gone) = sioul_sync::share::asking(folder, key, &crate::share::memory_path(), here, now);
    move |device: &str| standing_in(&counted, &gone, device)
}

/// The quick part of the phone's step, at every exchange: Android's words
/// copied, the requests decided and sent, a lost claim looked for, what the
/// carriers let it send said (once an hour at most), the long texts still on
/// their way said when they changed, what a multimedia message left behind
/// swept. Under one lock for the phone's processes.
/// Whether something was written.
fn light_step(here: &str, seal: &Seal, region: Option<&'static phones::Region>, vault: Option<&Vault>) -> bool {
    // Where the devices stand in the sharing, read only when a request not decided yet asks (`texts::verdict`).
    let standings: std::cell::OnceCell<(BTreeSet<String>, BTreeSet<String>)> = std::cell::OnceCell::new();
    let standing = |device: &str| {
        let (counted, gone) = standings.get_or_init(|| vault.map(|(folder, key)| sioul_sync::share::asking(folder, key, &crate::share::memory_path(), here, now_ms())).unwrap_or_default());
        standing_in(counted, gone, device)
    };
    let step = Step::here(here, seal, region, vault, &standing);
    sioul_core::filelock::with_lock(&step.private.join(".step"), || {
        let now = now_ms();
        let mut changed = step.copy_results(now);
        let trim_due = RESULTS_TRIMMED.lock().is_ok_and(|mut last| {
            let due = now - *last >= DAY_MS;
            if due {
                *last = now;
            }
            due
        });
        if trim_due && let Err(e) = trim_results(&step.private, now) {
            eprintln!("sioul: texts: Java's results not trimmed: {e}");
        }
        changed |= step.decide(now);
        changed |= step.look_for_lost(now);
        let due = LIMITS_ASKED.lock().is_ok_and(|mut asked| {
            let due = now - *asked >= 3_600_000;
            if due {
                *asked = now;
            }
            due
        });
        if due {
            changed |= step.publish_limits(now);
        }
        changed |= step.publish_arriving(now);
        step.sweep_outgoing(now);
        changed
    })
}

/// Each outcome's row by its name in the log (`texts::Row::name`): Android
/// says a sent message's row by its id alone, which an older one may have
/// held (the plain texts' table gives a deleted newest text's id again); the
/// row's date, read from Android now, tells which it is (a multimedia
/// message's in seconds). A row Android no longer holds is left unnamed,
/// never named after an older message.
fn name_rows(android: &dyn Android, outcomes: &mut [Outcome]) {
    let bare = |row: &str, table: &str| row.strip_prefix(table).and_then(|rest| rest.strip_prefix('-')).filter(|id| !id.contains('@')).and_then(|id| id.parse::<i64>().ok());
    let sms_ids: Vec<i64> = outcomes.iter().filter_map(|o| bare(&o.row, "sms")).collect();
    let mms_ids: Vec<i64> = outcomes.iter().filter_map(|o| bare(&o.row, "mms")).collect();
    if sms_ids.is_empty() && mms_ids.is_empty() {
        return;
    }
    let answer = android.ask("texts-read", &json!({ "sms_ids": sms_ids, "mms_ids": mms_ids, "limit": sms_ids.len() + mms_ids.len() }));
    let dates: BTreeMap<(bool, i64), i64> = if answer["allowed"] == true { raws(&answer).iter().map(|r| ((r.kind == "mms", int(&r.fields, "_id")), int(&r.fields, "date"))).collect() } else { BTreeMap::new() };
    for o in outcomes.iter_mut() {
        for (mms, table) in [(false, "sms"), (true, "mms")] {
            if let Some(id) = bare(&o.row, table) {
                o.row = dates.get(&(mms, id)).map(|date| rules::Row { mms, id, date: *date }.name()).unwrap_or_default();
            }
        }
    }
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

/// The part that may take a while, never inside an exchange: the import's
/// next step when due (`import_now`: caught up, what came; a history, all of
/// it, on the charger), else the newest texts while a history waits for the
/// charger (`recent_now`); and the day's look for deletions. Under its own
/// lock: one import at a time on the phone.
fn heavy_step(here: &str, seal: &Seal, vault: &Vault, region: Option<&phones::Region>) -> bool {
    sioul_core::filelock::with_lock(&private().join(".import"), || {
        let now = now_ms();
        let (texts_changed, plugged) = changes(true);
        let reader = Reader::load();
        let files = Files::of(here);
        let mut changed = false;
        if import_now(&reader, texts_changed, plugged, now) {
            changed = import_step(&Phone, &files, seal, vault, region, now, plugged);
        } else if recent_now(&reader, texts_changed) {
            changed = import_step(&Phone, &files, seal, vault, region, now, false);
        }
        changed |= look_again(&Phone, &files, seal, now);
        // A conversation deleted in Sioul on a computer: its texts leave this log, every copy following.
        match rules::drop_hidden(&files.log, &rules::Marks::read_all(&root(), seal), seal) {
            Ok(gone) => changed |= gone > 0,
            Err(e) => eprintln!("sioul: texts: {e}"),
        }
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
/// docs/texts.md, "Reading, on the phone: paced"). A history (the first
/// import's, or a block of old texts put at once), all of it only while the
/// phone charges: it may write and send hundreds of megabytes, on mobile
/// data too. Caught up, when the phone's texts changed since the last look,
/// when the last step left texts that came (their media over its share), and
/// with the day's look for deletions all the same (a change Android did not
/// say): most steps find nothing new, and ask Android nothing.
fn import_now(reader: &Reader, changed: bool, plugged: bool, now: i64) -> bool {
    if reader.started == 0 || reader.behind { plugged } else { changed || reader.left || !reader.indexed || now - reader.looked >= DAY_MS }
}

/// Whether the newest texts are read now while a history waits for the
/// charger (`import_now`): at a first import, at the first step of a reader
/// of an older version (its log tells which rows it holds), then each time
/// the phone's texts changed, on the battery too; only the rows that came
/// and the history's newest (`import_step`), so that a text that comes
/// during a long import reaches your computers within a minute or two.
fn recent_now(reader: &Reader, changed: bool) -> bool {
    reader.started == 0 || !reader.indexed || (reader.behind && changed)
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
    let quick = light_step(&here, &seal, region, Some(&vault));
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
/// the media brought here (a computer); what came said to the window (`qt`)
/// as an exchange's news are (`shared_in`), so that the pages showing texts
/// read them again at once. Once a day, old requests and outcomes out
/// (texts never).
pub(crate) fn before_exchange(qt: Option<&QtThread>) {
    let Some((here, seal, vault)) = sealed() else { return };
    if phone() {
        light_step(&here, &seal, sioul_core::reach::region(&load_config()), vault.as_ref());
    } else {
        measure_clock();
    }
    if vault.is_some() && !crate::steps::in_service() && !WORKING.swap(true, Ordering::AcqRel) {
        let qt = qt.cloned();
        std::thread::spawn(move || {
            let mut came = false;
            if let Some((here, seal, Some(vault))) = sealed() {
                came = if phone() { heavy_step(&here, &seal, &vault, sioul_core::reach::region(&load_config())) } else { fetch_media(&seal, &vault) };
            }
            WORKING.store(false, Ordering::Release);
            if came && let Some(qt) = qt {
                let store = if phone() { "data/texts/log/" } else { "data/texts/media/" };
                let _ = qt.queue(move |mut sioul| sioul.as_mut().shared_in(QString::from(store)));
            }
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
/// Whether one was brought.
fn fetch_media(seal: &Seal, vault: &Vault) -> bool {
    let (folder, key) = vault;
    let mut logs: Vec<(u64, u128)> = std::fs::read_dir(root().join(rules::LOG))
        .into_iter()
        .chain(std::fs::read_dir(root().join(rules::SEND)))
        .flatten()
        .filter_map(|entry| entry.ok()?.metadata().ok())
        .map(|m| (m.len(), m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos())))
        .collect();
    logs.sort_unstable();
    if FETCHED.lock().is_ok_and(|seen| seen.0 == logs && !seen.1) {
        return false;
    }
    let mut wanted: Vec<(String, u64)> = rules::read_logs(&root(), seal).iter().flat_map(|t| t.parts.iter()).filter(|p| p.state == "here" && !p.hash.is_empty() && !here_media(&p.hash)).map(|p| (p.hash.clone(), p.size)).collect();
    // A multimedia message written on another computer, within the last day: its media, so that it
    // shows here too (the phone's log names them once it sent it, read back with its parts).
    let since = now_ms() - DAY_MS;
    wanted.extend(rules::read_requests(&root(), seal).iter().filter(|r| r.written >= since).flat_map(|r| r.media.iter()).filter(|m| !here_media(&m.hash)).map(|m| (m.hash.clone(), m.size)));
    wanted.sort();
    wanted.dedup();
    let mut budget = rules::MEDIA_BATCH;
    let (mut brought, mut left) = (0, false);
    // Sioul keeping the folder itself fetches a sealed file only by its
    // name (`blobs::Wanted::media`): the media not in its copy are asked
    // for, those brought are no longer (review of 17 October 2026, M4).
    let asking = crate::share::keeps_itself();
    let (mut ask, mut have): (Vec<(String, String, u64)>, Vec<String>) = (Vec::new(), Vec::new());
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
            if asking {
                ask.push((sioul_sync::blobs::name(key, &hash), hash.clone(), size));
            }
            left = true;
            continue;
        }
        if asking {
            have.push(sioul_sync::blobs::name(key, &hash));
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
    if asking && (!ask.is_empty() || !have.is_empty()) {
        let now = jiff::Timestamp::now().as_second();
        let _ = sioul_sync::blobs::Wanted::change(&crate::share::memory_path(), |wanted| {
            for name in &have {
                wanted.media.remove(name);
            }
            for (name, hash, size) in ask {
                wanted.media.entry(name).or_insert_with(|| sioul_sync::blobs::Want { hash, size, since: now, ..Default::default() });
            }
        });
    }
    if let Ok(mut seen) = FETCHED.lock() {
        *seen = (logs, left);
    }
    if brought > 0 {
        eprintln!("sioul: texts: {brought} media file(s) brought");
    }
    brought > 0
}

/// A medium the page shows that is not here: Sioul keeping the folder itself
/// asks for it again as if new, once it had stopped asking (three days in
/// vain, `sioul_sync::remote::sealed_step`; review of 18 October 2026, L-8).
fn ask_media_again(hash: &str) {
    if hash.is_empty() || !crate::share::keeps_itself() {
        return;
    }
    let Some((_, _, Some((_, key)))) = sealed() else { return };
    let name = sioul_sync::blobs::name(&key, hash);
    let now = jiff::Timestamp::now().as_second();
    let _ = sioul_sync::blobs::Wanted::change(&crate::share::memory_path(), |wanted| {
        if let Some(want) = wanted.media.get_mut(&name)
            && now - want.since >= 3 * 86_400
        {
            (want.since, want.again, want.tries) = (now, 0, 0);
        }
    });
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
pub(crate) fn ahead_of(phone: &str) -> i64 {
    let clock: Clock = std::fs::read_to_string(Clock::path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    clock.phones.get(phone).filter(|g| now_ms() - g.at <= 86_400_000).map_or(0, |g| g.least.max(0))
}

// ---------------------------------------------------------------- a computer: the phone that sends

/// The phones in the sharing, the one chosen to send first, else the one that shared last.
pub(crate) fn phones() -> Vec<sioul_sync::devices::Entry> {
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

/// The texts, the requests and their outcomes as last read for this device:
/// the whole history is opened once, not at each look of the page (9,800
/// sealed lines read twice per conversation opened froze the window for
/// seconds); then only the lines the phones' logs added (`rules::Logs`: the
/// page reads itself again at each exchange that brings a text), and the
/// requests and outcomes again when their files changed (size and time).
/// The texts each once across phones (`rules::one_each`), with the country
/// their numbers were keyed by, without the rows' raw columns (`fields`,
/// `addrs`), which only the fingerprints read.
struct Kept {
    here: String,
    logs: rules::Logs,
    stamp: Vec<(PathBuf, u64, std::time::SystemTime)>,
    region: Option<&'static str>,
    texts: std::sync::Arc<Vec<Text>>,
    requests: std::sync::Arc<Vec<Request>>,
    outcomes: std::sync::Arc<Vec<Outcome>>,
    /// Every device's marks (read, hidden), and the texts without those hidden (23 October 2026).
    marks: rules::Marks,
    visible: std::sync::Arc<Vec<Text>>,
}

static KEPT: Mutex<Option<Kept>> = Mutex::new(None);

/// Each file of the requests and the outcomes, its size and time.
fn stamps(root: &Path) -> Vec<(PathBuf, u64, std::time::SystemTime)> {
    let mut all = Vec::new();
    for store in [rules::SEND, rules::OUTCOME] {
        for entry in std::fs::read_dir(root.join(store)).into_iter().flatten().flatten() {
            if let Ok(meta) = entry.metadata() {
                all.push((entry.path(), meta.len(), meta.modified().unwrap_or(std::time::UNIX_EPOCH)));
            }
        }
    }
    all.sort();
    all
}

/// The texts, the requests and their outcomes (`KEPT`): the logs read on
/// where they grew, the texts made again only then (or for another
/// country); the requests and outcomes read again when a file changed. The
/// texts each once, a text found in two phones' logs (a restore onto a new
/// phone) shown once (`rules::one_each`).
#[allow(clippy::type_complexity)]
fn records(here: &str, seal: &Seal) -> (std::sync::Arc<Vec<Text>>, std::sync::Arc<Vec<Request>>, std::sync::Arc<Vec<Outcome>>) {
    let root = root();
    let stamp = stamps(&root);
    let region = sioul_core::reach::region(&looks().0);
    let mut kept = KEPT.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let k = match kept.take() {
        Some(k) if k.here == here => kept.insert(k),
        _ => kept.insert(Kept {
            here: here.to_string(),
            logs: rules::Logs::default(),
            stamp: Vec::new(),
            region: None,
            texts: std::sync::Arc::default(),
            requests: std::sync::Arc::default(),
            outcomes: std::sync::Arc::default(),
            marks: rules::Marks::default(),
            visible: std::sync::Arc::default(),
        }),
    };
    let first = k.stamp.is_empty() && k.texts.is_empty() && k.requests.is_empty();
    let mut texts_changed = first;
    if k.logs.refresh(&root, seal) || k.region != region.map(|r| r.code) {
        // Without their raw columns, which only the fingerprints read (`Logs`).
        k.texts = std::sync::Arc::new(k.logs.one_each(region));
        k.region = region.map(|r| r.code);
        texts_changed = true;
    }
    // A conversation deleted in Sioul, on any device: its texts until then are not shown here.
    let marks = rules::Marks::read_all(&root, seal);
    if texts_changed || marks.hidden != k.marks.hidden {
        k.visible = if marks.hidden.is_empty() { std::sync::Arc::clone(&k.texts) } else { std::sync::Arc::new(rules::visible(&k.texts, &marks)) };
    }
    k.marks = marks;
    if first || k.stamp != stamp {
        k.requests = std::sync::Arc::new(rules::read_requests(&root, seal));
        k.outcomes = std::sync::Arc::new(rules::read_outcomes(&root, seal));
        k.stamp = stamp;
    }
    let requests = if k.marks.hidden.is_empty() {
        std::sync::Arc::clone(&k.requests)
    } else {
        // The texts written from here to a conversation deleted since: gone with it.
        let hidden = &k.marks.hidden;
        std::sync::Arc::new(k.requests.iter().filter(|r| hidden.get(&phones::key(&r.to, region)).is_none_or(|upto| r.written > *upto)).cloned().collect())
    };
    (std::sync::Arc::clone(&k.visible), requests, std::sync::Arc::clone(&k.outcomes))
}

/// Every device's marks, and every text with those hidden: for the Porch's
/// folding (a notification of a text deleted since folds into nothing shown).
fn marked(here: &str, seal: &Seal) -> (rules::Marks, std::sync::Arc<Vec<Text>>) {
    let _ = records(here, seal);
    let kept = KEPT.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    kept.as_ref().map_or_else(Default::default, |k| (k.marks.clone(), std::sync::Arc::clone(&k.texts)))
}

/// The Porch's notices of texts not read yet (23 October 2026): one per
/// conversation, its sender and the newest's first words, when it came,
/// opening it in Texts; [] where texts are not read here. A notice comes on
/// arrival, as the list's new texts do; when the newest reached the phone
/// `rules::LATE_MS` or more after it was sent, it says when it was sent too
/// (`sent`, and `late`: how much earlier; "" otherwise).
pub(crate) fn notices() -> Value {
    if phone() || !readable() {
        return json!([]);
    }
    let Some((here, seal, _)) = sealed() else { return json!([]) };
    let (config, senders) = looks();
    let now = Zoned::now();
    let name_of = names(senders);
    let region = sioul_core::reach::region(&config);
    let (texts, _, _) = records(&here, &seal);
    let (marks, _) = marked(&here, &seal);
    let mut list: Vec<(i64, Value)> = rules::unread(&texts, &marks, now.timestamp().as_millisecond())
        .into_iter()
        .map(|(id, unread)| {
            let newest = unread[0];
            let who = newest.with.iter().map(|k| name_of(k).unwrap_or_else(|| sioul_core::calls::shown_number(k, region))).collect::<Vec<_>>().join(", ");
            let first: String = if newest.body.is_empty() { tr().text("phonemsgs-picture", None) } else { newest.body.chars().take(80).collect() };
            let late = newest.late();
            let (sent, late) = if late > 0 { (rules::time_of_day(newest.time(), &now), rules::late_span(late, tr())) } else { (String::new(), String::new()) };
            (newest.at, json!({ "conversation": id, "who": who, "first": first, "when": rules::time_of_day(newest.at, &now), "count": unread.len(), "sent": sent, "late": late }))
        })
        .collect();
    list.sort_by(|a, b| b.0.cmp(&a.0));
    Value::Array(list.into_iter().map(|(_, v)| v).collect())
}

/// Whether a phone's notification of a text (its sender's key, its time) is
/// a text the Texts page holds, or held before its conversation was
/// deleted: folded into it, never a second item on the Porch. None where
/// texts are not read here: every notification stays a line.
pub(crate) fn folding() -> Option<impl Fn(&str, i64) -> bool> {
    if phone() || !readable() {
        return None;
    }
    let (here, seal, _) = sealed()?;
    let (_, all) = marked(&here, &seal);
    Some(move |key: &str, at_ms: i64| rules::folded_into(key, at_ms, &all).is_some())
}

/// A conversation opened in Texts: read up to its newest text, on every
/// device, and the phone's notifications of it seen (gone from the Porch's notices).
fn mark_read(here: &str, seal: &Seal, id: &str, texts: &[Text]) {
    let newest = texts.iter().filter(|t| t.direction == "in" && rules::conversation_id(&t.with) == id).map(|t| t.at).max().unwrap_or(0);
    let read = KEPT.lock().unwrap_or_else(std::sync::PoisonError::into_inner).as_ref().and_then(|k| k.marks.read.get(id).copied()).unwrap_or(0);
    if newest > read {
        let mark = rules::TextMark { at: now_ms(), conversation: id.to_string(), read: newest, ..rules::TextMark::default() };
        if let Err(e) = rules::write_mark(&root(), here, &mark, seal) {
            eprintln!("sioul: texts: {e}");
        }
    }
    crate::phonemsgs::seen_for(&id.split(',').map(str::to_string).collect::<Vec<_>>());
}

/// A conversation deleted in Sioul (23 October 2026): Sioul is never the
/// phone's SMS app (it asks no SMS role, docs/texts.md), so Android lets it
/// delete nothing from the phone's provider. Its texts until now leave
/// Sioul on every device (a sealed mark; the phone writes its log again
/// without them, so every computer's copy follows); the phone's own
/// texting app keeps them. A text that comes later starts it again.
fn delete(id: &str) -> Value {
    let Some((here, seal, _)) = sealed() else { return json!({ "said": tr().text("texts-page-off", None) }) };
    let (texts, requests, _) = records(&here, &seal);
    let region = sioul_core::reach::region(&looks().0);
    let upto = texts
        .iter()
        .filter(|t| rules::conversation_id(&t.with) == id)
        .map(|t| t.at)
        .chain(requests.iter().filter(|r| phones::key(&r.to, region) == id).map(|r| r.written))
        .max()
        .unwrap_or(0)
        .max(now_ms());
    let mark = rules::TextMark { at: now_ms(), conversation: id.to_string(), read: upto, hidden: upto, ..rules::TextMark::default() };
    if let Err(e) = rules::write_mark(&root(), &here, &mark, &seal) {
        return json!({ "said": e });
    }
    crate::phonemsgs::seen_for(&id.split(',').map(str::to_string).collect::<Vec<_>>());
    forget_hidden_media(&here, &seal);
    json!({ "said": "" })
}

/// A conversation archived (out of the list, its texts kept, until a newer
/// text comes) or put back in the list, by a sealed mark every device reads.
fn archive(id: &str, archive: bool) -> Value {
    let Some((here, seal, _)) = sealed() else { return json!({ "said": tr().text("texts-page-off", None) }) };
    let (texts, requests, _) = records(&here, &seal);
    let region = sioul_core::reach::region(&looks().0);
    let newest = texts
        .iter()
        .filter(|t| rules::conversation_id(&t.with) == id)
        .map(|t| t.at)
        .chain(requests.iter().filter(|r| phones::key(&r.to, region) == id).map(|r| r.written))
        .max()
        .unwrap_or(0);
    let mark = rules::TextMark { at: now_ms(), conversation: id.to_string(), archived: if archive { newest.max(1) } else { 0 }, restored: !archive, ..rules::TextMark::default() };
    match rules::write_mark(&root(), &here, &mark, &seal) {
        Ok(_) => json!({ "said": tr().text(if archive { "texts-archived" } else { "texts-restored" }, None) }),
        Err(e) => json!({ "said": e }),
    }
}

/// The media of hidden texts gone from this device's copies, unless a text shown holds them too.
fn forget_hidden_media(here: &str, seal: &Seal) {
    let (marks, all) = marked(here, seal);
    let shown: BTreeSet<&str> = all.iter().filter(|t| !marks.hides(t)).flat_map(|t| t.parts.iter().map(|p| p.hash.as_str())).collect();
    for hash in all.iter().filter(|t| marks.hides(t)).flat_map(|t| t.parts.iter().map(|p| p.hash.as_str())).filter(|h| !h.is_empty() && !shown.contains(h)) {
        let _ = std::fs::remove_file(media_path(hash));
    }
}

/// The Texts page's list (TextsPage.qml): {can (the part on, a phone that
/// sends), phone: {id, name, shared, said}, conversations, said, never,
/// empty (the list's words when it has no row)}; without them, `can` false and
/// why. With `query`, the conversations holding a text with all its words (or
/// whose people's names or numbers do), each with the newest such text as its
/// last words.
fn view(query: &str, archived: bool) -> Value {
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
    // Unread until opened here (23 October 2026).
    let (marks, _) = marked(&here, &seal);
    let unread = rules::unread(&texts, &marks, now.timestamp().as_millisecond());
    if let Value::Array(list) = &mut rows {
        for row in list.iter_mut() {
            let id = row["id"].as_str().unwrap_or_default().to_string();
            row["unread"] = json!(unread.get(&id).map_or(0, Vec::len));
        }
        // Archived ones in their own view, out of the list until a newer text comes (25 October 2026).
        list.retain(|row| marks.archived(row["id"].as_str().unwrap_or_default(), row["at"].as_i64().unwrap_or(0)) == archived);
    }
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
        "empty": tr.text(if archived { "texts-archived-empty" } else if query.is_empty() { "texts-page-empty" } else { "texts-search-none" }, None),
        "archived": archived,
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

/// One conversation (TextsPage.qml): {id, title, messages, on_the_way (its
/// long texts still on their way to a phone, in words), drafts, can_write,
/// group, attach (Attach offered: the phone said what its carrier takes),
/// attach_said (else why not)}.
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
    // A long text still on its way to a phone, its last parts not come: said under the thread.
    let on_the_way = rules::on_the_way(id, &outcomes, &v);
    // Opened here: read, on every device, and its notices gone from the Porch.
    mark_read(&here, &seal, id, &texts);
    let with: Vec<String> = id.split(',').map(str::to_string).collect();
    let title = with.iter().map(|k| name_of(k).unwrap_or_else(|| sioul_core::calls::shown_number(k, region))).collect::<Vec<_>>().join(", ");
    // What an AI agent drafted for it: waiting for you, never sent by itself.
    let drafts: Vec<Value> = sioul_core::textdraft::all(&seal).into_iter().filter(|d| d.conversation == id).map(|d| json!({ "id": d.id, "body": d.body, "when": rules::time_of_day(d.written, &now) })).collect();
    let attach = attach_limit(phone.as_ref(), id);
    json!({
        "id": id,
        "title": title,
        "messages": messages,
        "on_the_way": on_the_way,
        "drafts": drafts,
        "can_write": with.len() == 1,
        "group": with.len() > 1,
        "attach": attach.is_ok(),
        "attach_said": attach.err().map_or(String::new(), |key| tr().text(key, None)),
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
fn send(to: &str, body: &str, again: &str, from: &str) -> (Value, bool) {
    let tr = tr();
    let Some((here, seal, _)) = sealed() else { return (json!({ "said": tr.text("texts-page-off", None), "shared": false }), false) };
    // From the phone asked (a reply on the Porch: the phone the text came to), else the one that sends.
    let phone = if from.is_empty() || (demo() && !phone()) { sending_phone() } else { phones().into_iter().find(|p| p.id == from) };
    let Some(phone) = phone else { return (json!({ "said": tr.text("texts-page-no-phone", None), "shared": false }), false) };
    let region = sioul_core::reach::region(&load_config());
    if let Some(why) = rules::refusal(to, body, region) {
        return (json!({ "said": tr.text(&format!("texts-refused-{why}"), None), "shared": false }), false);
    }
    let request = Request { key: rules::new_key(), phone: phone.id.clone(), to: to.trim().to_string(), body: body.to_string(), sub: -1, written: now_ms(), ahead: ahead_of(&phone.id), again: again.to_string(), ..Request::default() };
    match rules::write_request(&root(), &here, &request, &seal) {
        Ok(_) => {
            // Its small file to the server now, not behind the records.
            crate::share::texts_now();
            (json!({ "said": "", "shared": true, "key": request.key }), true)
        }
        Err(e) => (json!({ "said": e, "shared": false }), false),
    }
}

/// Send again: a request of its own, a new key, the same number and words;
/// a multimedia message's media again, sealed in the sharing already.
fn again(key: &str) -> (Value, bool) {
    let Some((_, seal, _)) = sealed() else { return (json!({ "said": tr().text("texts-page-off", None), "shared": false }), false) };
    match rules::read_requests(&root(), &seal).into_iter().find(|r| r.key == key) {
        Some(old) if old.multimedia() => request_mms(&old.to, &old.words, old.media, key, &old.phone),
        Some(old) => send(&old.to, &old.body, key, &old.phone),
        None => (json!({ "said": "", "shared": false }), false),
    }
}

/// A multimedia message written here for the phone to send (docs/texts.md,
/// "Multimedia messages sent from a computer"; the media reduced already,
/// each its content type and bytes): refused at once when the phone would
/// refuse it (said why); else each medium sealed into the sharing's blobs
/// (content-addressed: one sent twice travels once) and into this
/// computer's own media, so that the message shows it at once, then the
/// request, whose media go up before it (`media_to_send`). Never a file's
/// own name: a medium is its content alone. {said, shared, key}.
pub(crate) fn send_mms(to: &str, words: &str, media: &[(String, Vec<u8>)], from: &str) -> (Value, bool) {
    let tr = tr();
    let Some((_, seal, Some((folder, key)))) = sealed() else { return (json!({ "said": tr.text("texts-page-off", None), "shared": false }), false) };
    let staging = private().join("staging");
    let mut named: Vec<rules::Medium> = Vec::new();
    for (i, (ct, bytes)) in media.iter().enumerate() {
        let source = staging.join(i.to_string());
        let hashed = write_whole(&source, bytes).and_then(|()| sioul_sync::blobs::hash_file(&source).map_err(|e| format!("{}: {e}", source.display())));
        let _ = std::fs::remove_file(&source);
        match hashed {
            Ok((hash, size)) => named.push(rules::Medium { hash, ct: ct.clone(), size }),
            Err(e) => return (json!({ "said": e, "shared": false }), false),
        }
    }
    let region = sioul_core::reach::region(&load_config());
    if let Some(why) = rules::refusal_mms(to, words, &named, &[], region) {
        return (json!({ "said": tr.text(&format!("texts-refused-{why}"), None), "shared": false }), false);
    }
    for ((_, bytes), medium) in media.iter().zip(&named) {
        let source = staging.join(&medium.hash);
        let sealed_there = write_whole(&source, bytes).and_then(|()| sioul_sync::blobs::put(&folder, &key, &source, &medium.hash).map(|_| ()));
        let _ = std::fs::remove_file(&source);
        if let Err(e) = sealed_there {
            return (json!({ "said": e, "shared": false }), false);
        }
        if !here_media(&medium.hash)
            && let Err(e) = write_whole(&media_path(&medium.hash), &seal.seal_bytes(bytes))
        {
            eprintln!("sioul: texts: {e}");
        }
    }
    request_mms(to, words, named, "", from)
}

/// What one multimedia message from here may weigh (decision 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Weigh {
    /// The phone has not said what its carrier takes (its Sioul older, or not
    /// stepped since its update): no media offered yet.
    Unknown,
    /// Its carrier takes no multimedia messages from it.
    None,
    /// At most these bytes, whole; the carrier's largest picture's long side (0 none).
    Up { max: u64, side: u32 },
}

/// The smaller choice of Settings ▸ Texts (kB).
const LOW_KB: u64 = 300;

/// What one multimedia message from here to a conversation (`id`, its
/// number's key) may weigh, from a phone (decision 5): as the phone last
/// said its carriers take (`texts::limits_of`), for the SIM the conversation
/// last used (`weighed`).
fn weigh(phone: &str, id: &str) -> Weigh {
    let Some((here, seal, _)) = sealed() else { return Weigh::Unknown };
    let (texts, _, outcomes) = records(&here, &seal);
    weighed(rules::limits_of(&outcomes, phone, now_ms()).map(|(_, limits)| limits), rules::last_sim(id, phone, &texts), choice().mms_kb)
}

/// What a message may weigh by what the phone said (`limits`, none before
/// it said): its carrier's for `sim` (the conversation's last; when it is
/// unknown, -1, or the phone did not name it, the smallest of those that
/// take multimedia messages), and no more than chosen here (`chosen_kb`,
/// Settings ▸ Texts: 300 kB); with the carrier's largest picture's long side.
fn weighed(limits: Option<&[rules::Limits]>, sim: i32, chosen_kb: Option<u64>) -> Weigh {
    let Some(limits) = limits else { return Weigh::Unknown };
    let carrier = limits.iter().find(|l| sim >= 0 && l.sub == sim).or_else(|| limits.iter().filter(|l| l.mms).min_by_key(|l| l.max)).or_else(|| limits.first());
    match carrier {
        None => Weigh::Unknown,
        Some(l) if !l.mms => Weigh::None,
        // A line with no size is none the phone wrote (`limits_from` keeps sizes only).
        Some(l) if l.max == 0 => Weigh::Unknown,
        // Never above Android's own ceiling for a multimedia message, whatever a line says.
        Some(l) => Weigh::Up { max: chosen_kb.map_or(l.max, |kb| (kb * 1024).min(l.max)).min(rules::MMS_MAX_EVER), side: l.width.max(l.height) },
    }
}

/// Where the files attached on the page wait, made fit (Sioul's cache, yours alone).
fn attached_folder() -> PathBuf {
    sioul_core::config::cache_dir().join("texts-attached")
}

/// Whether `path` is a file in `folder` (the files attached, made fit),
/// its links and `..` resolved: none elsewhere is sent as a medium.
fn made_in(folder: &Path, path: &Path) -> bool {
    match (std::fs::canonicalize(folder), std::fs::canonicalize(path)) {
        (Ok(folder), Ok(path)) => path.starts_with(&folder) && path != folder && path.is_file(),
        _ => false,
    }
}

/// One attaching at a time: a second one waits, then makes its own anew.
static ATTACHING: Mutex<()> = Mutex::new(());
/// Each attaching's turn, and each letting go's: an attaching whose turn
/// passed (a newer one asked, or the files let go meanwhile) makes nothing,
/// or deletes what it made. The window never waits on `ATTACHING`.
static ATTACH_TURN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// At the window's start (lib.rs `run`, once this process is the profile's
/// Sioul): what an earlier run left attached on the Texts page, one that quit
/// or crashed with files attached, goes (the review of 18 October 2026, I-6).
#[cfg(not(target_os = "android"))]
pub(crate) fn at_start() {
    let_attached_go();
}

/// The files attached let go, and any attaching under way told so: the window's thread never waits.
fn let_attached_go() {
    ATTACH_TURN.fetch_add(1, Ordering::SeqCst);
    let _ = std::fs::remove_dir_all(attached_folder());
}

/// The files chosen on the page (`sources`, files or their addresses) made
/// fit for one multimedia message to a conversation (`attach::make`; what an
/// earlier attaching made goes first), off the window's thread:
/// {round (as asked: the page keeps its latest), media: [{source, kind, ct,
/// path, url, size, said}], said: [why each other did not go], limit (its
/// words), max (bytes)}. Nothing travels until Send.
pub(crate) fn attach(json: &str) -> String {
    let asked: Value = serde_json::from_str(json).unwrap_or_default();
    let tr = tr();
    let round = asked["round"].as_i64().unwrap_or(0);
    let id = asked["id"].as_str().unwrap_or_default().to_string();
    let from = asked["phone"].as_str().unwrap_or_default();
    let phone = if from.is_empty() || (demo() && !phone()) { sending_phone() } else { phones().into_iter().find(|p| p.id == from) };
    let sources: Vec<PathBuf> = asked["sources"].as_array().map(|all| all.iter().filter_map(|s| s.as_str()).map(crate::backend::local_path).filter(|p| !p.as_os_str().is_empty()).collect()).unwrap_or_default();
    let turn = ATTACH_TURN.fetch_add(1, Ordering::SeqCst) + 1;
    let (max, side) = match attach_limit(phone.as_ref(), &id) {
        Ok(limit) => limit,
        Err(key) => return json!({ "round": round, "media": [], "said": [tr.text(key, None)], "limit": "", "max": 0 }).to_string(),
    };
    let _one = ATTACHING.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let passed = || ATTACH_TURN.load(Ordering::SeqCst) != turn;
    if passed() {
        return json!({ "round": round, "media": [], "said": [], "limit": "", "max": 0 }).to_string();
    }
    let made = attach_made(&sources, max, side, round);
    // Let go while it was made: nothing of it stays.
    if passed() {
        let _ = std::fs::remove_dir_all(attached_folder());
    }
    made
}

/// What a multimedia message from here to a conversation may weigh (bytes)
/// and its pictures' long side at most (0 none), from the phone that sends
/// (`weigh`); else why nothing can be attached, its words' key: no phone, a
/// group (a later step), the phone not having said what its carrier takes
/// (Attach is offered only once it has, decision 11), its carrier taking none.
fn attach_limit(phone: Option<&sioul_sync::devices::Entry>, id: &str) -> Result<(u64, u32), &'static str> {
    let Some(phone) = phone else { return Err("texts-page-no-phone") };
    if id.contains(',') {
        return Err("texts-group-read-only");
    }
    match weigh(&phone.id, id) {
        Weigh::Unknown => Err("texts-attach-gate"),
        Weigh::None => Err("texts-sending-no-mms"),
        Weigh::Up { max, side } => Ok((max, side)),
    }
}

/// The files chosen made fit (`attach::make`), on a computer.
#[cfg(not(target_os = "android"))]
fn attach_made(sources: &[PathBuf], max: u64, side: u32, round: i64) -> String {
    let tr = tr();
    let folder = attached_folder();
    let _ = std::fs::remove_dir_all(&folder);
    let (made, left) = crate::attach::make(sources, max, side, &folder);
    let media: Vec<Value> = made
        .iter()
        .map(|m| {
            let mut args = sioul_core::i18n::args();
            args.set("size", size_words(m.size));
            // The file chosen, for the page to make them again (one removed); never sent.
            json!({ "source": m.source, "kind": m.kind, "ct": m.ct, "path": m.path, "url": crate::backend::file_url(Path::new(&m.path)), "size": m.size, "said": tr.text(&format!("texts-attached-{}", m.kind), Some(&args)) })
        })
        .collect();
    let said: Vec<String> = left
        .iter()
        .map(|l| {
            let mut args = sioul_core::i18n::args();
            args.set("name", Path::new(&l.source).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default());
            args.set("size", size_words(l.size));
            args.set("limit", size_words(max));
            args.set("kind", l.format);
            tr.text(&format!("texts-attach-left-{}", l.why), Some(&args))
        })
        .collect();
    json!({ "round": round, "media": media, "said": said, "limit": size_words(max), "max": max }).to_string()
}

/// A phone attaches nothing: it is the one that sends.
#[cfg(target_os = "android")]
fn attach_made(_: &[PathBuf], _: u64, _: u32, round: i64) -> String {
    json!({ "round": round, "media": [], "said": [tr().text("texts-refused-other", None)], "limit": "", "max": 0 }).to_string()
}

/// How much a multimedia message takes, as its words are typed: {parts,
/// said ("One multimedia message (412 kB of 600 kB)"), over, max (bytes)},
/// its media counted by their lengths alone (`mms::size_with_lengths`).
fn parts_mms(body: &str, media: &[Value], id: &str, from: &str) -> Value {
    let phone = if from.is_empty() || (demo() && !phone()) { sending_phone() } else { phones().into_iter().find(|p| p.id == from) };
    let max = match phone.map(|p| weigh(&p.id, id)) {
        Some(Weigh::Up { max, .. }) => max,
        _ => 0,
    };
    let cts: Vec<String> = media.iter().map(|m| m["ct"].as_str().unwrap_or("application/octet-stream").to_string()).collect();
    let lengths: Vec<usize> = media.iter().map(|m| m["size"].as_u64().unwrap_or(0) as usize).collect();
    let empty: Vec<sioul_core::mms::Medium> = cts.iter().map(|ct| sioul_core::mms::Medium { ct, bytes: &[] }).collect();
    let key = "0".repeat(32);
    // To the longest number, as the media were made (`attach::room_for`): never less than the phone's own.
    let message = sioul_core::mms::Message { key: &key, to: &[sioul_core::mms::LONGEST_NUMBER], date: jiff::Timestamp::now().as_second(), words: body, media: &empty };
    let size = sioul_core::mms::size_with_lengths(&message, &lengths).unwrap_or(0) as u64;
    let mut args = sioul_core::i18n::args();
    args.set("size", size_words(size));
    args.set("limit", size_words(max));
    json!({ "parts": 1, "said": tr().text("texts-parts-mms", Some(&args)), "over": max > 0 && size > max, "max": max })
}

/// Settings ▸ Texts, on a computer: how much a multimedia message from here
/// may weigh, the carrier's or less (300 kB), and what the phone said its
/// carrier takes, read only. {limit: a setting's row, phone: its lines}.
fn sending() -> Value {
    let tr = tr();
    let phone = sending_phone();
    let limits = sealed().map(|(here, seal, _)| records(&here, &seal).2).and_then(|outcomes| phone.as_ref().and_then(|p| rules::limits_of(&outcomes, &p.id, now_ms()).map(|(_, l)| l.to_vec()))).unwrap_or_default();
    // Never above what this computer prepares, whatever a line says (`rules::MMS_MAX_EVER`).
    let carrier = limits.iter().filter(|l| l.mms).map(|l| l.max).min().unwrap_or(614_400).min(rules::MMS_MAX_EVER);
    let size = |bytes: u64| {
        let mut args = sioul_core::i18n::args();
        args.set("size", size_words(bytes));
        args
    };
    let lines = limit_lines(&limits);
    let choices = vec![json!({ "value": "carrier", "label": tr.text("texts-sending-limit-carrier", Some(&size(carrier))) }), json!({ "value": "300", "label": tr.text("texts-sending-limit-low", Some(&size(LOW_KB * 1024))) })];
    json!({
        "limit": { "key": "limit", "kind": "choice", "label": tr.text("texts-sending-limit", None), "help": tr.text("texts-sending-limit-help", None), "value": if choice().mms_kb.is_some() { "300" } else { "carrier" }, "choices": choices },
        "phone": lines,
    })
}

/// What a phone's carriers take, in words, read only: one line, or one a SIM
/// in its order ("SIM 2: multimedia messages up to 600 kB. Pictures up to
/// 2592 × 1944 pixels."); none said yet.
fn limit_lines(limits: &[rules::Limits]) -> Vec<String> {
    let tr = tr();
    let size = |bytes: u64| {
        let mut args = sioul_core::i18n::args();
        args.set("size", size_words(bytes));
        args
    };
    // The picture size the carrier advises, which the pictures attached on a computer keep to.
    let pictures = |l: &rules::Limits| {
        if l.width == 0 || l.height == 0 {
            return String::new();
        }
        let mut args = sioul_core::i18n::args();
        args.set("width", i64::from(l.width));
        args.set("height", i64::from(l.height));
        format!(" {}", tr.text("texts-sending-pictures", Some(&args)))
    };
    match limits {
        [] => vec![tr.text("texts-sending-phone-none", None)],
        [one] if one.mms => vec![tr.text("texts-sending-phone", Some(&size(one.max))) + &pictures(one)],
        [_] => vec![tr.text("texts-sending-no-mms", None)],
        several => several
            .iter()
            .enumerate()
            .map(|(n, l)| {
                let mut args = size(l.max);
                args.set("sim", i64::try_from(n + 1).unwrap_or(0));
                if l.mms { tr.text("texts-sending-phone-sim", Some(&args)) + &pictures(l) } else { tr.text("texts-sending-no-mms-sim", Some(&args)) }
            })
            .collect(),
    }
}

/// The most a multimedia message from here may weigh: the carrier's ("carrier") or 300 kB ("300").
fn set_limit(value: &str) -> Result<(), String> {
    let path = choice_path();
    sioul_core::filelock::with_lock(&path, || {
        let mut chosen = choice();
        chosen.mms_kb = (value.trim() == "300").then_some(LOW_KB);
        write_whole(&path, toml::to_string(&chosen).map_err(|e| e.to_string())?.as_bytes())
    })
}

/// A multimedia message's request, its media sealed in the sharing already.
fn request_mms(to: &str, words: &str, media: Vec<rules::Medium>, again: &str, from: &str) -> (Value, bool) {
    let tr = tr();
    let Some((here, seal, _)) = sealed() else { return (json!({ "said": tr.text("texts-page-off", None), "shared": false }), false) };
    let phone = if from.is_empty() || (demo() && !phone()) { sending_phone() } else { phones().into_iter().find(|p| p.id == from) };
    let Some(phone) = phone else { return (json!({ "said": tr.text("texts-page-no-phone", None), "shared": false }), false) };
    let region = sioul_core::reach::region(&load_config());
    if let Some(why) = rules::refusal_mms(to, words, &media, &[], region) {
        return (json!({ "said": tr.text(&format!("texts-refused-{why}"), None), "shared": false }), false);
    }
    let request = Request { key: rules::new_key(), phone: phone.id.clone(), to: to.trim().to_string(), sub: -1, written: now_ms(), ahead: ahead_of(&phone.id), again: again.to_string(), words: words.to_string(), media, ..Request::default() };
    match rules::write_request(&root(), &here, &request, &seal) {
        Ok(_) => {
            // Its media, then its small file, to the server now, not behind the records.
            crate::share::texts_now();
            (json!({ "said": "", "shared": true, "key": request.key }), true)
        }
        Err(e) => (json!({ "said": e, "shared": false }), false),
    }
}

/// The media of this computer's multimedia messages that may still go (written
/// within their quarter of an hour), by their sealed files' names in the
/// sharing and the size each was sealed at: they go up first on the urgent
/// lane (`remote::send_urgent`), before the request that names them.
pub(crate) fn media_to_send() -> BTreeMap<String, u64> {
    let Some((here, seal, Some((folder, key)))) = sealed() else { return BTreeMap::new() };
    let now = now_ms();
    let mut out = BTreeMap::new();
    for r in rules::read_file::<Request>(&rules::own_file(&root(), rules::SEND, &here), &seal) {
        if !r.multimedia() || now - r.written > rules::EXPIRY_MS + rules::AHEAD_MS {
            continue;
        }
        for medium in &r.media {
            let name = sioul_sync::blobs::name(&key, &medium.hash);
            if let Ok(meta) = std::fs::metadata(folder.join("blobs").join(&name)) {
                out.insert(name, meta.len());
            }
        }
    }
    out
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
        // Asked and not given: on Android 15 and later the steps that hold whether
        // Android keeps restricted settings from Sioul or not ("guarded": no app can
        // read which); before, a question refused for good ("at-once").
        "guard": guard(&state),
        "cap": { "key": "cap", "kind": "choice", "label": tr.text("texts-setup-cap", None), "help": tr.text("texts-setup-cap-help", None), "value": cap_mb().to_string(), "choices": choices },
        // What its carriers take for a multimedia message, as Android says it now: read only (decision 5).
        "limits": if android { limit_lines(&limits_from(&crate::steps::java("texts-limits", "{}"))) } else { Vec::new() },
    })
}

/// What the tab says beside its question once the texts' permissions were
/// asked and are still not all given (`Texts.state`): "guarded" on Android 15
/// and later, where restricted settings may keep them from an app installed
/// from a file (13 and 14 keep notification access only); "at-once" when
/// Android answered without showing a question ("Don't allow" chosen twice);
/// else "".
fn guard(state: &Value) -> &'static str {
    let given = state["read"] == true && state["receive"] == true && state["send"] == true;
    if given || state["asked"] != true {
        ""
    } else if state["api"].as_i64().unwrap_or(0) >= 35 {
        "guarded"
    } else if state["at_once"] == true {
        "at-once"
    } else {
        ""
    }
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
/// "send" {to, body}, "send-mms" {to, words, media: [{ct, path}], phone} (a
/// multimedia message, its media made fit already by `attach`: a file
/// elsewhere is refused), "again" {key}, "parts"
/// {body, media: [{ct, size}], id, phone} (with media, how much the message
/// takes), "sending" and "limit" {value} (Settings ▸ Texts on a computer), "media" {hash, ct} (its
/// address, to show or open it), "save-media" {hash, ct, path}, "close" (the
/// page closed: the media opened for it and the files attached deleted),
/// "unattach" (the files attached let go), "setup", "ask" (Android's
/// question for the permissions), "info" (Sioul's app info), "part" {on},
/// "cap" {value}; and "access…", what Android withholds here (`access::call`).
/// Its JSON, and whether what travels changed (to share now).
pub(crate) fn call(verb: &str, json: &str) -> (String, bool) {
    let asked: Value = serde_json::from_str(json).unwrap_or_default();
    let text = |key: &str| asked[key].as_str().unwrap_or_default().to_string();
    let (answer, shared) = match verb {
        "view" => (view(&text("query"), asked["archived"].as_bool().unwrap_or(false)), false),
        "conversation" => (conversation(&text("id")), false),
        "send" => send(&text("to"), &text("body"), "", &text("phone")),
        // A multimedia message: its media reduced already, each {ct, path} (a file of this device's).
        "send-mms" => {
            let asked_media = asked["media"].as_array().cloned().unwrap_or_default();
            // Only files the page made fit (`attach`, its own folder): never one as it was chosen, with its own details.
            let folder = attached_folder();
            let media: Vec<(String, Vec<u8>)> = asked_media
                .iter()
                .filter_map(|m| {
                    let path = crate::backend::local_path(m["path"].as_str()?);
                    made_in(&folder, &path).then_some(())?;
                    Some((m["ct"].as_str()?.to_string(), std::fs::read(&path).ok()?))
                })
                .collect();
            if media.len() == asked_media.len() {
                let sent = send_mms(&text("to"), &text("words"), &media, &text("phone"));
                // Sealed into the sharing and here: the files made fit for it go.
                if sent.1 {
                    let_attached_go();
                }
                sent
            } else {
                (json!({ "said": tr().text("texts-refused-other", None), "shared": false }), false)
            }
        }
        "again" => again(&text("key")),
        // Archived, or put back in the list, on every device (25 October 2026).
        "archive" | "restore" => {
            let done = archive(&text("id"), verb == "archive");
            let mut tab = view("", verb == "restore");
            tab["said"] = done["said"].clone();
            (tab, true)
        }
        "delete" => {
            let done = delete(&text("id"));
            let mut tab = view("", false);
            tab["said"] = done["said"].clone();
            (tab, true)
        }
        "parts" => match asked["media"].as_array().filter(|media| !media.is_empty()) {
            Some(media) => (parts_mms(&text("body"), media, &text("id"), &text("phone")), false),
            None => (parts(&text("body")), false),
        },
        "sending" => (sending(), false),
        "limit" => {
            let problem = set_limit(&text("value")).err().unwrap_or_default();
            let mut tab = sending();
            tab["said"] = json!(problem);
            (tab, false)
        }
        // An AI agent's draft: its words for the box, or gone; nothing is sent.
        "draft-use" | "draft-discard" => (draft_taken(&text("id")), false),
        "media" => {
            let url = open_media(&text("hash"), &text("ct")).map(|p| crate::backend::file_url(&p)).unwrap_or_default();
            if url.is_empty() {
                ask_media_again(&text("hash"));
            }
            (json!({ "url": url }), false)
        }
        "save-media" => {
            let target = crate::backend::local_path(&text("path"));
            let saved = !target.as_os_str().is_empty() && open_media(&text("hash"), &text("ct")).is_some_and(|p| std::fs::copy(p, &target).is_ok());
            (json!({ "saved": saved }), false)
        }
        "close" => {
            let _ = std::fs::remove_dir_all(opened_folder());
            let_attached_go();
            (json!({}), false)
        }
        // The files attached let go (removed from the box, another conversation).
        "unattach" => {
            let_attached_go();
            (json!({}), false)
        }
        "setup" => (setup(), false),
        // Sioul's app info, where Android's restricted settings and the permissions are.
        "info" => {
            crate::steps::java("texts-info", "{}");
            (setup(), false)
        }
        // What Android withholds here, for the join's result and the Porch (access.rs).
        v if v.starts_with("access") => (crate::access::call(v, &asked), false),
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

/// Whether a phone sends texts for this computer: the part on here, and its
/// texts read here (its log): a text that came to it is answered from it.
pub(crate) fn sends_texts(phone_id: &str) -> bool {
    if demo() && !phone() {
        return phone_id == "demo-phone";
    }
    !phone_id.is_empty() && sealed().is_some() && rules::own_file(&root(), rules::LOG, phone_id).is_file()
}

/// The texts this computer wrote from a phone to a number (`dial`) since a
/// moment (ms), oldest first, for the Porch's reply under a line
/// (`replies::Mine`): the words and their state in the texts' own words ("Sent at 14:03.").
pub(crate) fn sent_to(phone_id: &str, dial: &str, since_ms: i64) -> Vec<sioul_core::replies::Mine> {
    let Some((here, seal, _)) = sealed() else { return Vec::new() };
    let (_, requests, outcomes) = records(&here, &seal);
    let region = sioul_core::reach::region(&load_config());
    let to = phones::key(dial, region);
    // When the phone the text goes from last shared, never another's.
    let shared = if demo() && !phone() { sending_phone() } else { phones().into_iter().find(|p| p.id == phone_id) }.filter(|p| p.exported > 0).map(|p| p.exported * 1000);
    let now = Zoned::now();
    let mut sent: Vec<&Request> = requests.iter().filter(|r| r.device == here && r.phone == phone_id && r.written >= since_ms && phones::key(&r.to, region) == to).collect();
    sent.sort_by_key(|r| r.written);
    sent.into_iter()
        .map(|r| {
            let said = rules::said(rules::state_of(&r.key, &outcomes), shared, tr(), &now, r.multimedia());
            sioul_core::replies::Mine { key: r.key.clone(), time: rules::time_of_day(r.written, &now), text: r.body.clone(), said: said.text, waiting: said.state == "waiting", again: said.again, texts: true }
        })
        .collect()
}

/// The numbers this computer texted (`phones::key`) whose phone took the text
/// since a moment (ms, the phone's clock): handed, sent or delivered then.
pub(crate) fn texted_since(since_ms: i64) -> Vec<String> {
    let Some((here, seal, _)) = sealed() else { return Vec::new() };
    let (_, requests, outcomes) = records(&here, &seal);
    let region = sioul_core::reach::region(&load_config());
    let took = |key: &str| outcomes.iter().any(|o| o.key == key && matches!(o.state.as_str(), "handed" | "sent" | "delivered") && o.at >= since_ms);
    requests.iter().filter(|r| r.device == here && took(&r.key)).map(|r| phones::key(&r.to, region)).collect()
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
        PartResult { key: key.into(), kind: kind.into(), part, parts, code, uri: uri.into(), status, at, ..PartResult::default() }
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
        let request = |key: &str| Request { key: key.into(), phone: "phone".into(), to: "0199001234".into(), body: "On my way".into(), sub: -1, written: NOW, ..Request::default() };
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
        assert_eq!((sms.id.as_str(), sms.with.as_slice(), sms.sub, sms.direction.as_str(), sms.at), ("sms-412@1791446340000", &["+33199001234".to_string()][..], 2, "in", 1_791_446_340_000), "named by its id and its own date");
        assert_eq!(sms.fields.len(), 13, "every column kept");
        let mms = &texts[1];
        assert_eq!((mms.id.as_str(), mms.at, mms.with.as_slice(), mms.body.as_str(), mms.picture, mms.sub), ("mms-31@1791446340", 1_791_446_340_000, &["+33465710042".to_string()][..], "Le colis est arrivé.", true, 1));
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

    /// The import asks Android only when the phone's texts changed; a heavy
    /// one (the first, of the whole history, or one a full batch left behind)
    /// only while the phone charges.
    #[test]
    fn a_heavy_import_waits_for_the_charger() {
        let first = Reader::default();
        assert!(!import_now(&first, true, false, NOW), "the whole history, on the battery: it waits");
        assert!(import_now(&first, false, true, NOW), "on the charger");
        let caught_up = Reader { started: NOW - 1_000, looked: NOW - 3_600_000, indexed: true, ..Reader::default() };
        assert!(import_now(&caught_up, true, false, NOW), "a new text, on the battery");
        assert!(!import_now(&caught_up, false, false, NOW) && !import_now(&caught_up, false, true, NOW), "nothing changed: Android not asked");
        assert!(import_now(&caught_up, false, false, NOW + 86_400_000), "the day's look: read all the same");
        let behind = Reader { started: NOW - 1_000, behind: true, ..caught_up.clone() };
        assert!(!import_now(&behind, true, false, NOW), "a full batch left more: on the charger only");
        assert!(import_now(&behind, false, true, NOW), "on the charger, batch after batch");
        // A reader written before this mark: caught up; its first step at once, on the battery too (its log indexed).
        let older: Reader = serde_json::from_str(r#"{"version":2,"sms":10,"mms":3,"started":1}"#).unwrap();
        assert!(!older.behind && import_now(&older, true, false, NOW));
        assert!(import_now(&older, false, false, NOW) && recent_now(&older, false));
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

    /// A stand-in phone's provider of texts, answering as Java does
    /// (Texts.read, Texts.ids, Texts.part; android/jvm-checks/TextsCheck.java):
    /// plain texts and multimedia messages by their ids, each with its own
    /// date, oldest first; a part's bytes; the rows Android hides (in the
    /// outbox) left out of all; a table it could not list whole left out of
    /// the listing (`unlisted`); and what it was asked, to count.
    #[derive(Default)]
    struct Provider {
        sms: std::cell::RefCell<BTreeMap<i64, Value>>,
        mms: std::cell::RefCell<BTreeMap<i64, Value>>,
        media: std::cell::RefCell<BTreeMap<i64, Vec<u8>>>,
        hidden: std::cell::RefCell<BTreeSet<i64>>,
        hidden_mms: std::cell::RefCell<BTreeSet<i64>>,
        unlisted: std::cell::Cell<Option<&'static str>>,
        asked: std::cell::RefCell<Vec<(String, usize)>>,
    }

    impl Android for Provider {
        fn ask(&self, verb: &str, asked: &Value) -> Value {
            let (hidden, hidden_mms) = (self.hidden.borrow(), self.hidden_mms.borrow());
            // Those named, else those after the mark.
            let chosen = |list: &Value, mark: &Value, id: i64| match list.as_array() {
                Some(ids) => ids.iter().any(|v| v.as_i64() == Some(id)),
                None => id > mark.as_i64().unwrap_or(0),
            };
            let answer = match verb {
                "texts-read" => {
                    let limit = asked["limit"].as_u64().unwrap_or(500) as usize;
                    let mut texts = Vec::new();
                    for (id, fields) in self.sms.borrow().iter().filter(|(id, _)| !hidden.contains(id) && chosen(&asked["sms_ids"], &asked["sms"], **id)) {
                        if texts.len() < limit {
                            texts.push(json!({ "kind": "sms", "fields": fields, "id": id }));
                        }
                    }
                    for (_, row) in self.mms.borrow().iter().filter(|(id, _)| !hidden_mms.contains(id) && chosen(&asked["mms_ids"], &asked["mms"], **id)) {
                        if texts.len() < limit {
                            let mut row = row.clone();
                            row["kind"] = json!("mms");
                            texts.push(row);
                        }
                    }
                    json!({ "texts": texts, "threads": {}, "allowed": true })
                }
                "texts-ids" => {
                    let sms: Vec<(i64, i64)> = self.sms.borrow().iter().filter(|(id, _)| !hidden.contains(id)).map(|(id, f)| (*id, f["date"].as_i64().unwrap())).collect();
                    let mms: Vec<(i64, i64)> = self.mms.borrow().iter().filter(|(id, _)| !hidden_mms.contains(id)).map(|(id, r)| (*id, r["fields"]["date"].as_i64().unwrap())).collect();
                    let mut out = json!({
                        "sms": sms.iter().map(|r| r.0).collect::<Vec<_>>(),
                        "sms_at": sms.iter().map(|r| r.1).collect::<Vec<_>>(),
                        "mms": mms.iter().map(|r| r.0).collect::<Vec<_>>(),
                        "mms_at": mms.iter().map(|r| r.1).collect::<Vec<_>>(),
                        "media": self.media.borrow().values().map(Vec::len).sum::<usize>(),
                        "allowed": true,
                    });
                    if let Some(table) = self.unlisted.get()
                        && let Some(listing) = out.as_object_mut()
                    {
                        listing.remove(table);
                        listing.remove(&format!("{table}_at"));
                    }
                    out
                }
                "texts-part" => {
                    let target = PathBuf::from(asked["target"].as_str().unwrap_or_default());
                    match self.media.borrow().get(&asked["id"].as_i64().unwrap_or(0)) {
                        Some(bytes) => {
                            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
                            std::fs::write(&target, bytes).unwrap();
                            json!({ "copied": bytes.len(), "allowed": true })
                        }
                        None => json!({ "copied": -1, "allowed": true }),
                    }
                }
                _ => Value::Null,
            };
            let rows = answer["texts"].as_array().map_or(0, Vec::len);
            self.asked.borrow_mut().push((verb.to_string(), rows));
            answer
        }
    }

    impl Provider {
        fn text(&self, id: i64, at: i64, body: &str) {
            self.sms.borrow_mut().insert(id, json!({ "_id": id, "thread_id": 3, "address": "04 65 71 00 42", "date": at, "date_sent": at, "type": 1, "read": 1, "body": body }));
        }
        fn picture(&self, id: i64, at: i64) {
            let part = 1000 + 10 * id;
            self.mms.borrow_mut().insert(id, json!({
                "fields": { "_id": id, "thread_id": 4, "date": at / 1000, "msg_box": 1, "m_type": 132, "sub_id": 1 },
                "addrs": [{ "address": "+33465710042", "type": 137, "charset": 106 }],
                "parts": [{ "_id": part, "mid": id, "seq": 0, "ct": "image/png", "name": "colis.png", "size": 4 }, { "_id": part + 1, "mid": id, "seq": 1, "ct": "text/plain", "text": "Le colis." }],
            }));
            self.media.borrow_mut().insert(part, vec![0x89, b'P', b'N', u8::try_from(id % 200).unwrap()]);
        }
        /// What was asked since the last look: each question and the rows it gave.
        fn questions(&self) -> Vec<(String, usize)> {
            std::mem::take(&mut *self.asked.borrow_mut())
        }
    }

    /// A phone of the tests: its log, reader and media in a folder of its own, the sharing's folder and key, the seal.
    struct Bench {
        base: PathBuf,
        files: Files,
        vault: Vault,
        seal: Seal,
    }

    impl Bench {
        fn new(name: &str) -> Bench {
            let base = std::env::temp_dir().join(format!("sioul-texts-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&base);
            let files = Files { log: base.join("data/texts/log/phone.jsonl"), reader: base.join("state/texts/reader.json"), incoming: base.join("state/texts/incoming") };
            let vault: Vault = (base.join("folder"), [5u8; 32]);
            std::fs::create_dir_all(&vault.0).unwrap();
            Bench { base, files, vault, seal: Seal::Key(TextSeal::new(&[6u8; 32])) }
        }
        fn step(&self, phone: &Provider, now: i64, charging: bool) -> bool {
            import_step(phone, &self.files, &self.seal, &self.vault, phones::region_named("FR"), now, charging)
        }
        fn look(&self, phone: &Provider, now: i64) -> bool {
            look_again(phone, &self.files, &self.seal, now)
        }
        fn log(&self) -> Vec<Text> {
            rules::read_file::<Text>(&self.files.log, &self.seal)
        }
        fn reader(&self) -> Reader {
            Reader::load_at(&self.files.reader)
        }
        /// The texts the log holds (deletion marks aside), by their rows.
        fn rows(&self) -> Vec<rules::Row> {
            self.log().iter().filter(|t| t.deleted == 0).filter_map(Text::row).collect()
        }
        /// Every row the phone holds is in the log, each once.
        fn each_once(&self, phone: &Provider) {
            let rows = self.rows();
            let once: BTreeSet<rules::Row> = rows.iter().copied().collect();
            assert_eq!(once.len(), rows.len(), "no row twice");
            let listed = listed(&phone.ask("texts-ids", &json!({}))).unwrap();
            assert!(listed.is_subset(&once), "every row read: {} missing", listed.difference(&once).count());
        }
    }

    impl Drop for Bench {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.base);
        }
    }

    /// A long history never holds back a text that comes meanwhile
    /// (docs/texts.md, "Reading, on the phone: paced"). A first import reads
    /// the newest first by date, on the battery only the last day's; a text
    /// that comes then is read at the next step; on the charger the rest,
    /// newest first, 500 at a time. Caught up, a block of old texts put at
    /// once (SMS Backup & Restore restoring a history, with a text that came
    /// after it) is read the same way; a text the outbox hid, once Android
    /// shows it. Each row once in the log, none passed.
    /// A conversation deleted in Sioul on a computer (23 October 2026): the
    /// phone writes its log again without its texts, never reads them back
    /// in, and a text that comes later is read as any.
    #[test]
    fn a_conversation_deleted_in_sioul_leaves_the_phones_log_for_good() {
        let bench = Bench::new("hidden");
        let phone = Provider::default();
        for id in 1..=3 {
            phone.text(id, NOW - 3_600_000 + id * 60_000, "Votre code : 123456");
        }
        assert!(bench.step(&phone, NOW, false));
        assert_eq!(bench.rows().len(), 3);
        let root = bench.base.join("data/texts");
        let mark = rules::TextMark { at: NOW, conversation: "+33465710042".into(), read: NOW, hidden: NOW, ..rules::TextMark::default() };
        rules::write_mark(&root, "computer", &mark, &bench.seal).unwrap();
        assert_eq!(rules::drop_hidden(&bench.files.log, &rules::Marks::read_all(&root, &bench.seal), &bench.seal).unwrap(), 3);
        assert!(bench.rows().is_empty(), "gone from the log, so from every computer's copy");
        // The phone's provider still holds them (its own app keeps them): never read back in.
        assert!(!bench.step(&phone, NOW + 60_000, false));
        assert!(bench.rows().is_empty());
        // One Android showed late, dated before the deletion: read, never written.
        phone.text(5, NOW - 1_000, "Votre code : 654321");
        bench.step(&phone, NOW + 90_000, false);
        assert!(bench.rows().is_empty(), "dated before it was deleted: hidden too");
        // A text that comes later shows again.
        phone.text(4, NOW + 120_000, "Votre colis arrive.");
        assert!(bench.step(&phone, NOW + 180_000, false));
        assert_eq!(bench.rows().iter().map(|r| r.id).collect::<Vec<_>>(), [4]);
    }

    #[test]
    fn a_text_that_comes_during_a_long_import_goes_first() {
        let bench = Bench::new("import");
        let day = 86_400_000;
        let phone = Provider::default();
        for id in 1..=1_200 {
            phone.text(id, NOW - 700 * day + id * 60_000, "Un ancien texte.");
        }
        for id in 1..=3 {
            phone.picture(id, NOW - 600 * day + id * 60_000);
        }
        for id in 1_201..=1_205 {
            phone.text(id, NOW - 3 * 3_600_000 + id, "Ce matin.");
        }
        let ids = |rows: &[rules::Row]| rows.iter().map(|r| r.id).collect::<Vec<_>>();
        // The first step, on the battery: this morning's alone, newest first; the history waits for the charger.
        assert!(recent_now(&bench.reader(), false) && !import_now(&bench.reader(), true, false, NOW));
        assert!(bench.step(&phone, NOW, false));
        assert_eq!(ids(&bench.rows()), [1_205, 1_204, 1_203, 1_202, 1_201]);
        let reader = bench.reader();
        assert!(reader.behind && !import_now(&reader, true, false, NOW + 60_000) && recent_now(&reader, true));
        assert!(!recent_now(&reader, false), "nothing new: Android not asked");
        // A text comes meanwhile: read at the next step, on the battery.
        phone.text(1_206, NOW + 60_000, "Et maintenant ?");
        assert!(bench.step(&phone, NOW + 70_000, false));
        assert_eq!(ids(&bench.rows()).last(), Some(&1_206));
        // On the charger: the rest, newest first by date (the pictures, then the texts), 500 rows a step.
        let mut steps = 0;
        while bench.reader().behind {
            assert!(bench.step(&phone, NOW + 100_000, true));
            steps += 1;
            assert!(steps <= 3, "1,203 rows: three steps");
        }
        let rows = bench.rows();
        assert_eq!(rows.len(), 1_209);
        assert_eq!((rows[6].mms, rows[6].id, rows[9].mms, rows[9].id), (true, 3, false, 1_200), "the history's newest first");
        assert!(bench.log().iter().filter(|t| t.id.starts_with("mms-")).all(|t| t.parts[0].state == "here" && !t.parts[0].hash.is_empty()), "the pictures put in the sharing");
        assert!(!bench.reader().behind, "caught up");
        // Caught up, a restore puts 900 old texts at once, one of them in the outbox for now, then a text comes.
        for id in 1_207..=2_106 {
            phone.text(id, NOW - 500 * day + id * 60_000, "Restauré.");
        }
        phone.hidden.borrow_mut().insert(1_500);
        phone.text(2_107, NOW + 3_600_000, "Après la restauration.");
        assert!(import_now(&bench.reader(), true, false, NOW + 3_700_000), "caught up: a new text, on the battery");
        assert!(bench.step(&phone, NOW + 3_700_000, false), "the text that came, not the block");
        assert_eq!(ids(&bench.rows()).last(), Some(&2_107));
        assert_eq!(bench.rows().len(), 1_210);
        assert!(bench.reader().behind);
        let mut steps = 0;
        while bench.reader().behind {
            assert!(bench.step(&phone, NOW + 3_800_000, true));
            steps += 1;
            assert!(steps <= 2, "899 rows: two steps");
        }
        assert_eq!(bench.rows()[1_210].id, 2_106, "the block's newest first");
        // Out of the outbox: read at the next step, a row that came.
        phone.hidden.borrow_mut().clear();
        assert!(bench.step(&phone, NOW + 3_900_000, false));
        assert_eq!(ids(&bench.rows()).last(), Some(&1_500));
        bench.each_once(&phone);
        assert!(!bench.look(&phone, NOW + 2 * day), "nothing gone");
        let reader = bench.reader();
        assert_eq!((reader.done_texts, reader.logged.len(), reader.behind, reader.left), (2_110, 2_110, false, false));
    }

    /// The plain texts' table gives a deleted newest text's id to the next
    /// text (no AUTOINCREMENT): that text is read at the next step, under a
    /// name of its own; the deleted one is marked so, once; and a text Sioul
    /// sent that took such an id has its state on its own row, never on the
    /// older text (the review of 16 October 2026, r1).
    #[test]
    fn a_text_that_takes_a_deleted_text_s_id_is_read() {
        let bench = Bench::new("reused");
        let phone = Provider::default();
        for id in 1..=10 {
            phone.text(id, NOW - 3_600_000 + id * 1_000, "Avant.");
        }
        assert!(bench.step(&phone, NOW - 3_000_000, true));
        assert!(!bench.reader().behind);
        // The newest deleted; the next text takes its id.
        phone.sms.borrow_mut().remove(&10);
        phone.text(10, NOW, "Un nouveau texte, sous le même numéro.");
        assert!(bench.step(&phone, NOW + 60_000, false), "read at the next step, on the battery");
        let log = bench.log();
        assert_eq!(log.len(), 11);
        assert!(log.iter().any(|t| t.id == rules::Row { mms: false, id: 10, date: NOW }.name() && t.body.starts_with("Un nouveau")));
        // The day's look: the older text of that id gone, said once.
        assert!(bench.look(&phone, NOW + 2 * 86_400_000));
        assert!(!bench.look(&phone, NOW + 4 * 86_400_000));
        let shown = rules::fold_logs(bench.log().into_iter().map(|t| Text { device: "phone".into(), ..t }).collect());
        assert_eq!(shown.iter().filter(|t| t.deleted > 0).map(|t| t.body.as_str()).collect::<Vec<_>>(), ["Avant."]);
        assert_eq!(shown.len(), 11);
        // A text sent from a computer whose row took that id again: named by its date, never the older text's.
        let mut sent = vec![Outcome { key: "k".into(), state: "sent".into(), at: NOW, row: "sms-10".into(), ..Outcome::default() }, Outcome { key: "g".into(), state: "sent".into(), at: NOW, row: "sms-77".into(), ..Outcome::default() }];
        name_rows(&phone, &mut sent);
        assert_eq!((sent[0].row.clone(), sent[1].row.clone()), (rules::Row { mms: false, id: 10, date: NOW }.name(), String::new()), "a row Android no longer holds: unnamed");
    }

    /// A provider emptied (every conversation deleted, its storage cleared)
    /// starts its ids at 1 again: the texts after it are read, and those
    /// before it marked deleted (r1c).
    #[test]
    fn a_provider_emptied_and_filled_again_is_read() {
        let bench = Bench::new("emptied");
        let phone = Provider::default();
        for id in 1..=100 {
            phone.text(id, NOW - 86_400_000 + id * 60_000, "Avant.");
        }
        assert!(bench.step(&phone, NOW - 3_600_000, true));
        phone.sms.borrow_mut().clear();
        for id in 1..=3 {
            phone.text(id, NOW + id * 1_000, "Après.");
        }
        assert!(bench.step(&phone, NOW + 60_000, false));
        assert_eq!(bench.log().iter().filter(|t| t.body == "Après.").count(), 3);
        assert!(bench.look(&phone, NOW + 2 * 86_400_000));
        assert_eq!(bench.log().iter().filter(|t| t.deleted > 0).count(), 100, "each of the old ones said deleted, once");
        bench.each_once(&phone);
    }

    /// A first import whose own texts sit under a restored block (by their
    /// ids), and a text that comes while the restore goes on (an id inside
    /// it): on the battery, by date, these come first, never the block (r2, r2b).
    #[test]
    fn the_newest_by_date_come_first_wherever_their_ids() {
        let day = 86_400_000;
        // r2: this phone's own 50 texts of this morning, then a restored block of 2,000 above them.
        let bench = Bench::new("own-under");
        let phone = Provider::default();
        for id in 1..=50 {
            phone.text(id, NOW - 5 * 3_600_000 + id * 60_000, "Ce matin, sur ce téléphone.");
        }
        for id in 51..=2_050 {
            phone.text(id, NOW - 400 * day + id * 60_000, "Restauré.");
        }
        assert!(bench.step(&phone, NOW, false));
        let rows = bench.rows();
        assert_eq!((rows.len(), rows.iter().all(|r| r.id <= 50)), (50, true), "this phone's own, not the block");
        // r2b: caught up, a restore of 3,000 rows with a text that came halfway through.
        let bench = Bench::new("during");
        let phone = Provider::default();
        for id in 1..=10 {
            phone.text(id, NOW - 3_600_000 + id, "Avant.");
        }
        assert!(bench.step(&phone, NOW - 3_000_000, true));
        for id in 11..=1_510 {
            phone.text(id, NOW - 300 * day + id * 60_000, "Restauré.");
        }
        phone.text(1_511, NOW, "Pendant la restauration.");
        for id in 1_512..=3_011 {
            phone.text(id, NOW - 300 * day + id * 60_000, "Restauré.");
        }
        assert!(bench.step(&phone, NOW + 60_000, false));
        assert_eq!(bench.rows().iter().map(|r| r.id).filter(|id| *id > 10).collect::<Vec<_>>(), [1_511], "the text that came, alone");
    }

    /// While a history waits: a picture whose date (in seconds) is under the
    /// newest text read, and one composed before a reply and sent after it,
    /// are read at the next step on the battery, as rows that came (r6, r6b).
    #[test]
    fn pictures_that_come_are_read_whatever_their_dates() {
        let bench = Bench::new("pictures");
        let day = 86_400_000;
        let phone = Provider::default();
        for id in 1..=600 {
            phone.text(id, NOW - 400 * day + id * 60_000, "Restauré.");
        }
        assert!(!bench.step(&phone, NOW - 2 * day, false), "a history: nothing of the last day");
        assert!(bench.reader().behind);
        // r6: a text read at 10:01:00.700, then a picture downloaded at 10:01:00.900 (its date: 10:01:00).
        phone.text(601, NOW + 60_700, "Une photo arrive.");
        assert!(bench.step(&phone, NOW + 60_800, false));
        phone.picture(1, NOW + 60_900);
        assert!(bench.step(&phone, NOW + 61_000, false), "the picture, on the battery");
        assert!(bench.rows().iter().any(|r| r.mms && r.id == 1));
        // r6b: a picture composed (in the outbox, hidden), a reply received and read, then the picture sent.
        phone.picture(2, NOW + 120_000);
        phone.hidden_mms.borrow_mut().insert(2);
        phone.picture(3, NOW + 180_000);
        assert!(bench.step(&phone, NOW + 190_000, false));
        phone.hidden_mms.borrow_mut().clear();
        assert!(bench.step(&phone, NOW + 240_000, false), "the picture sent, on the battery");
        assert!(bench.rows().iter().any(|r| r.mms && r.id == 2));
        assert!(bench.reader().behind, "the history waits still");
    }

    /// What a step asks Android (MEDIUM 3 of the review): caught up or with a
    /// history waiting, a change with nothing new costs one listing; a text
    /// that came, the listing and that row; on the charger, the listing and
    /// 500 rows.
    #[test]
    fn a_step_with_a_history_waiting_costs_what_a_caught_up_step_costs() {
        let bench = Bench::new("cost");
        let phone = Provider::default();
        for id in 1..=200 {
            phone.text(id, NOW - 3_600_000 + id, "Avant.");
        }
        assert!(bench.step(&phone, NOW - 3_000_000, true));
        phone.questions();
        assert!(!bench.step(&phone, NOW, false));
        assert_eq!(phone.questions(), [("texts-ids".to_string(), 0)], "caught up, nothing new");
        for id in 201..=3_500 {
            phone.text(id, NOW - 400 * 86_400_000 + id * 60_000, "Restauré.");
        }
        assert!(!bench.step(&phone, NOW + 1_000, false), "the block found: nothing of it on the battery");
        phone.questions();
        assert!(!bench.step(&phone, NOW + 2_000, false));
        assert_eq!(phone.questions(), [("texts-ids".to_string(), 0)], "a history waits, nothing new");
        phone.text(3_501, NOW + 3_000, "Un texte arrive.");
        assert!(bench.step(&phone, NOW + 4_000, false));
        assert_eq!(phone.questions(), [("texts-ids".to_string(), 0), ("texts-read".to_string(), 1)], "a text came: it alone");
        assert!(bench.step(&phone, NOW + 5_000, true));
        assert_eq!(phone.questions(), [("texts-ids".to_string(), 0), ("texts-read".to_string(), 500)], "on the charger");
    }

    /// A reader of version 2 or 3, which knew rows by their ids alone: the
    /// log tells which rows it holds, and what it lacks is read (a text that
    /// took a deleted text's id under version 2, lost then); the counts are
    /// the log's own, whatever a stop between the log and the reader left.
    #[test]
    fn a_reader_of_an_older_version_reads_what_its_log_lacks() {
        let bench = Bench::new("older");
        let phone = Provider::default();
        // The log as version 2 wrote it: rows 1 to 10 by their ids alone.
        let old: Vec<Text> = (1..=10).map(|id| Text { id: format!("sms-{id}"), at: NOW - 7_200_000 + id * 1_000, direction: "in".into(), with: vec!["+33465710042".into()], body: "Avant.".into(), ..Text::default() }).collect();
        rules::append(&bench.files.log, &old, &bench.seal, Some(&|t: &Text| t.line_id())).unwrap();
        for id in 1..=9 {
            phone.text(id, NOW - 7_200_000 + id * 1_000, "Avant.");
        }
        // Text 10 deleted, the next taking its id: version 2 never read it.
        phone.text(10, NOW - 600_000, "Perdu par la version 2.");
        std::fs::create_dir_all(bench.files.reader.parent().unwrap()).unwrap();
        for older in [r#"{"version":2,"sms":10,"mms":0,"started":1,"done_texts":7}"#, r#"{"version":3,"sms":10,"sms_read":[[0,10]],"started":1,"done_texts":7}"#] {
            std::fs::write(&bench.files.reader, older).unwrap();
            let reader = bench.reader();
            assert_eq!((reader.version, reader.indexed), (READER_VERSION, false), "{older}");
        }
        assert!(bench.step(&phone, NOW, false), "the text version 2 lost, read");
        assert!(bench.log().iter().any(|t| t.body.starts_with("Perdu")));
        assert_eq!(bench.reader().done_texts, 11, "the log's own count");
        assert!(bench.look(&phone, NOW + 2 * 86_400_000), "the older text of that id: deleted");
        bench.each_once(&phone);
        // A line written and the reader not saved (a stop between them): read again once, the count right.
        phone.text(11, NOW + 3 * 86_400_000, "Après un arrêt.");
        let line = Text { id: rules::Row { mms: false, id: 11, date: NOW + 3 * 86_400_000 }.name(), at: NOW + 3 * 86_400_000, direction: "in".into(), with: vec!["+33465710042".into()], body: "Après un arrêt.".into(), ..Text::default() };
        rules::append(&bench.files.log, &[line], &bench.seal, Some(&|t: &Text| t.line_id())).unwrap();
        assert!(!bench.step(&phone, NOW + 3 * 86_400_000 + 1_000, false), "in the log already: no line twice");
        assert_eq!(bench.reader().done_texts, 12);
        bench.each_once(&phone);
    }

    /// A table Android could not list whole is left out of its listing
    /// (`Texts.ids`): nothing is read at that step, and the day's look says
    /// nothing deleted, never "none there" taken from "not known"; listed
    /// again, every row comes.
    #[test]
    fn a_table_not_listed_is_never_taken_for_an_empty_one() {
        let bench = Bench::new("unlisted");
        let phone = Provider::default();
        for id in 1..=5 {
            phone.text(id, NOW - 3_600_000 + id, "Avant.");
        }
        phone.picture(1, NOW - 3_500_000);
        assert!(bench.step(&phone, NOW - 3_000_000, true));
        for table in ["sms", "mms"] {
            phone.unlisted.set(Some(table));
            phone.text(5 + bench.log().len() as i64, NOW, "Pendant.");
            assert!(!bench.step(&phone, NOW + 1_000, false), "{table} not listed: nothing read");
            assert!(!bench.look(&phone, NOW + 2 * 86_400_000), "{table} not listed: nothing said deleted");
            assert!(bench.log().iter().all(|t| t.deleted == 0));
        }
        phone.unlisted.set(None);
        assert!(bench.step(&phone, NOW + 3 * 86_400_000, false));
        assert!(!bench.look(&phone, NOW + 4 * 86_400_000), "nothing gone");
        bench.each_once(&phone);
    }

    /// A row that comes in the step that finds a history (a block of old
    /// texts put at once), dated under the newest text read (a picture's date
    /// is in seconds): read on the battery all the same, with the history's
    /// last day (review of 16 October 2026, LOW 2).
    #[test]
    fn a_row_that_comes_with_a_block_is_read_on_the_battery() {
        let bench = Bench::new("with-block");
        let phone = Provider::default();
        phone.text(1, NOW + 60_700, "Une photo arrive.");
        assert!(bench.step(&phone, NOW + 60_800, true));
        assert!(!bench.reader().behind);
        // The block (600 old texts) and, in the same step, a picture downloaded at 10:01:00.900 (dated 10:01:00).
        for id in 2..=601 {
            phone.text(id, NOW - 400 * DAY_MS + id * 60_000, "Restauré.");
        }
        phone.picture(1, NOW + 60_900);
        assert!(bench.step(&phone, NOW + 61_000, false));
        assert!(bench.rows().iter().any(|r| r.mms && r.id == 1), "the picture, on the battery");
        assert!(bench.reader().behind, "the history waits for the charger");
    }

    /// A row whose date its provider rewrote (an MMS downloaded or sent
    /// writes its date after its row; a text resent) is read again under its
    /// new date, and its old line marked moved, at once: computers show the
    /// message once, never once more "deleted on your phone"; a text that
    /// took a deleted text's id, its words other, leaves that one deleted
    /// (review of 16 October 2026, LOW 1), and so does one with the same
    /// words from the same person, hours later (review of 18 October 2026, L-3).
    #[test]
    fn a_row_its_provider_dated_again_is_one_text() {
        let bench = Bench::new("redated");
        let phone = Provider::default();
        phone.text(1, NOW - 600_000, "Bonjour.");
        phone.text(2, NOW - 500_000, "À demain.");
        phone.picture(3, NOW - 400_000);
        phone.text(4, NOW - 3 * 3_600_000, "Oui.");
        assert!(bench.step(&phone, NOW - 300_000, true));
        // The picture's row dated again a second later, the text's a millisecond: both read again.
        let picture = phone.mms.borrow_mut().remove(&3).unwrap();
        let mut moved = picture.clone();
        moved["fields"]["date"] = json!((NOW - 399_000) / 1000);
        phone.mms.borrow_mut().insert(3, moved);
        phone.text(1, NOW - 599_999, "Bonjour.");
        // Text 2 deleted, and its id taken by another text.
        phone.text(2, NOW - 100_000, "Autre chose.");
        // Her "Oui." deleted, and its id taken by her next "Oui.", hours later: another text.
        phone.text(4, NOW - 50_000, "Oui.");
        assert!(bench.step(&phone, NOW, false));
        let log = bench.log();
        let mut marks: Vec<(String, bool)> = log.iter().filter(|t| t.deleted > 0).map(|t| (t.id.split('@').next().unwrap_or_default().to_string(), !t.moved.is_empty())).collect();
        marks.sort();
        assert_eq!(marks, [("mms-3".to_string(), true), ("sms-1".to_string(), true)], "the re-dated rows' old lines marked moved, at once; text 2 not (other words), nor text 4 (the same words, hours later)");
        assert!(bench.look(&phone, NOW + DAY_MS));
        let shown = rules::fold_logs(bench.log().into_iter().map(|t| Text { device: "phone".into(), ..t }).collect());
        assert_eq!(shown.iter().filter(|t| t.body == "Bonjour.").count(), 1, "the text once, under its new date");
        assert_eq!(shown.iter().filter(|t| t.id.starts_with("mms-3@")).count(), 1, "the picture once");
        assert!(shown.iter().all(|t| t.deleted == 0 || t.body == "À demain." || t.body == "Oui."), "only the texts truly deleted said so");
        assert!(shown.iter().any(|t| t.body == "À demain." && t.deleted > 0));
        assert!(shown.iter().any(|t| t.body == "Autre chose." && t.deleted == 0));
        let mut oui: Vec<(i64, bool)> = shown.iter().filter(|t| t.body == "Oui.").map(|t| (t.at, t.deleted > 0)).collect();
        oui.sort_unstable();
        assert_eq!(oui, [(NOW - 3 * 3_600_000, true), (NOW - 50_000, false)], "her first \"Oui.\" deleted on the phone, kept here; her next one there");
    }

    /// A day's look whose listing is not whole waits an hour: never listing
    /// and measuring every medium again at every step meanwhile (review of
    /// 16 October 2026, LOW 4); listed whole, done for the day.
    #[test]
    fn a_look_that_cannot_list_waits_an_hour() {
        let bench = Bench::new("look-wait");
        let phone = Provider::default();
        phone.text(1, NOW - 3_600_000, "Avant.");
        assert!(bench.step(&phone, NOW - 3_000_000, true));
        phone.unlisted.set(Some("mms"));
        phone.questions();
        let day = NOW + DAY_MS;
        assert!(!bench.look(&phone, day));
        assert_eq!(phone.questions().iter().filter(|(verb, _)| verb == "texts-ids").count(), 1);
        assert_eq!(bench.reader().looked, day - DAY_MS + LOOK_AGAIN_MS, "tried again in an hour");
        assert!(!import_now(&bench.reader(), false, false, day + 60_000), "the day's look no longer due at every step");
        assert!(!bench.look(&phone, day + 30 * 60_000));
        assert!(phone.questions().is_empty(), "nothing asked within the hour");
        phone.unlisted.set(None);
        assert!(!bench.look(&phone, day + LOOK_AGAIN_MS + 1), "nothing gone");
        assert_eq!(bench.reader().looked, day + LOOK_AGAIN_MS + 1, "listed whole: done for the day");
    }

    /// A log that cannot be written (a full disk): its rows are not taken as
    /// read, and come at the next step that can write (LOW 5).
    #[test]
    fn rows_a_log_could_not_take_are_read_again() {
        let bench = Bench::new("unwritable");
        let phone = Provider::default();
        phone.text(1, NOW, "Bonjour.");
        std::fs::create_dir_all(&bench.files.log).unwrap();
        assert!(!bench.step(&phone, NOW + 1_000, true));
        assert_eq!(bench.reader().logged.len(), 0);
        std::fs::remove_dir_all(&bench.files.log).unwrap();
        assert!(bench.step(&phone, NOW + 2_000, false));
        bench.each_once(&phone);
    }

    /// A restore onto a phone that had read no text yet, its import begun a
    /// month before: on the battery, only the texts of the day before the
    /// block was found, never the month's history.
    #[test]
    fn a_block_found_late_reads_only_its_last_day_on_the_battery() {
        let bench = Bench::new("late");
        let day = 86_400_000;
        let phone = Provider::default();
        // A month ago: a first import, of nothing.
        assert!(!bench.step(&phone, NOW - 30 * day, false));
        assert!(!bench.reader().behind);
        // Now: 600 texts of the last twenty days, put at once.
        for id in 1..=600 {
            phone.text(id, NOW - 20 * day + id * 2_880_000, "Restauré.");
        }
        assert!(bench.step(&phone, NOW, false));
        let read = bench.log();
        assert!(!read.is_empty() && read.iter().all(|t| t.at >= NOW - day), "the last day's alone: {}", read.len());
        assert_eq!(read.len(), 31, "a text every 48 minutes: 31 from a day before to now");
    }

    // ---------------------------------------------------------------- multimedia messages, on the phone

    const MINUTE: i64 = 60_000;

    /// Android, as the phone's quick step asks it about multimedia messages:
    /// its carrier's values, what it was handed (and the message's bytes),
    /// the rows it filed by key, the dates of rows named.
    #[derive(Default)]
    struct Carrier {
        max: u64,
        mms: bool,
        /// Each multimedia message handed: its key, its SIM, its bytes.
        handed: std::cell::RefCell<Vec<(String, i64, Vec<u8>)>>,
        /// The rows filed, by key: (id, date in seconds, box).
        filed: std::cell::RefCell<BTreeMap<String, (i64, i64, i64)>>,
        /// What Android answers a hand-off: none for the default (taken).
        refuse: Option<&'static str>,
        /// The provider's query fails (`Texts.findMms` says so).
        look_fails: std::cell::Cell<bool>,
        /// The carrier's status Android read for a row, by key (`resp_st`).
        statuses: std::cell::RefCell<BTreeMap<String, i64>>,
    }

    impl Android for Carrier {
        fn ask(&self, verb: &str, asked: &Value) -> Value {
            match verb {
                "texts-limits" => json!({ "limits": [{ "sub": 1, "max": self.max, "width": 2592, "height": 1944, "recipients": 20, "group": true, "mms": self.mms }] }),
                "texts-send-mms" => {
                    if let Some(why) = self.refuse {
                        return json!({ "handed": false, "reason": why });
                    }
                    let bytes = std::fs::read(asked["path"].as_str().unwrap_or_default()).unwrap_or_default();
                    self.handed.borrow_mut().push((asked["key"].as_str().unwrap_or_default().to_string(), asked["sub"].as_i64().unwrap_or(-9), bytes));
                    json!({ "handed": true })
                }
                "texts-find-mms" if self.look_fails.get() => json!({ "allowed": true, "found": [], "error": true }),
                "texts-find-mms" => {
                    let key = asked["key"].as_str().unwrap_or_default();
                    let status = self.statuses.borrow().get(key).copied().unwrap_or(0);
                    let found: Vec<Value> = self.filed.borrow().get(key).map(|(id, date, mbox)| json!({ "id": id, "date": date, "box": mbox, "status": status })).into_iter().collect();
                    json!({ "allowed": true, "found": found })
                }
                "texts-read" => {
                    let rows: Vec<Value> = self.filed.borrow().values().map(|(id, date, _)| json!({ "kind": "mms", "fields": { "_id": id, "date": date } })).collect();
                    json!({ "allowed": true, "texts": rows })
                }
                "texts-find" => json!({ "allowed": true, "texts": [] }),
                _ => Value::Null,
            }
        }
    }

    /// A sharing that counts every device.
    fn every_device(_: &str) -> rules::Standing {
        rules::Standing::Counted
    }

    /// A phone's quick step on folders of its own: its data, its private
    /// texts folder, the sharing's memory, folder and key.
    struct Desk {
        base: PathBuf,
        vault: Vault,
        seal: Seal,
    }

    impl Desk {
        fn new(name: &str) -> Desk {
            let base = std::env::temp_dir().join(format!("sioul-mms-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&base);
            std::fs::create_dir_all(base.join("folder")).unwrap();
            std::fs::create_dir_all(base.join("share")).unwrap();
            // The ledger's mark, a day before: the requests come after it.
            rules::Ledger::open(&base.join("private"), NOW - DAY_MS).unwrap();
            Desk { vault: (base.join("folder"), [5u8; 32]), seal: Seal::Key(TextSeal::new(&[6u8; 32])), base }
        }

        fn step<'a>(&'a self, android: &'a dyn Android) -> Step<'a> {
            self.step_counting(android, &every_device)
        }

        /// The same, the devices standing as `standing` says.
        fn step_counting<'a>(&'a self, android: &'a dyn Android, standing: &'a dyn Fn(&str) -> rules::Standing) -> Step<'a> {
            Step { here: "phone", seal: &self.seal, region: phones::region_named("FR"), root: self.base.join("data"), private: self.base.join("private"), copies: Vec::new(), memory: self.base.join("share").join("memory.json"), vault: Some(&self.vault), android, standing }
        }

        /// A request written on a computer: its line in that computer's file, its writer sealed in it.
        fn asks(&self, request: &Request) {
            rules::write_request(&self.base.join("data"), "desk", request, &self.seal).unwrap();
        }

        /// A medium sealed into the sharing, as the computer that wrote the message seals it.
        fn seals(&self, bytes: &[u8]) -> rules::Medium {
            let source = self.base.join("medium");
            std::fs::write(&source, bytes).unwrap();
            let (hash, size) = sioul_sync::blobs::hash_file(&source).unwrap();
            sioul_sync::blobs::put(&self.vault.0, &self.vault.1, &source, &hash).unwrap();
            rules::Medium { hash, ct: "image/jpeg".into(), size }
        }

        /// The medium's name and description, not sealed (its blob not there yet).
        fn described(&self, bytes: &[u8]) -> rules::Medium {
            let source = self.base.join("medium");
            std::fs::write(&source, bytes).unwrap();
            let (hash, size) = sioul_sync::blobs::hash_file(&source).unwrap();
            rules::Medium { hash, ct: "image/jpeg".into(), size }
        }

        fn outcomes(&self) -> Vec<Outcome> {
            rules::read_file::<Outcome>(&rules::own_file(&self.base.join("data"), rules::OUTCOME, "phone"), &self.seal)
        }

        fn states(&self, key: &str) -> Vec<(String, String)> {
            self.outcomes().into_iter().filter(|o| o.key == key).map(|o| (o.state, o.reason)).collect()
        }

        fn ledger(&self) -> rules::Ledger {
            rules::Ledger::open(&self.base.join("private"), NOW).unwrap()
        }

        /// Java's word on a multimedia message, as TextSend.Result writes it.
        fn result(&self, key: &str, code: i64, uri: &str, conf: &[u8], at: i64) {
            self.result_http(key, code, uri, conf, 0, at);
        }

        /// The same, with the HTTP status Android adds to its code 4 (0 none).
        fn result_http(&self, key: &str, code: i64, uri: &str, conf: &[u8], http: i64, at: i64) {
            use std::io::Write as _;
            let hex: String = conf.iter().map(|b| format!("{b:02x}")).collect();
            let line = json!({ "key": key, "kind": "mms", "code": code, "uri": uri, "http": http, "conf": hex, "at": at });
            let path = self.base.join("private").join(rules::RESULTS);
            let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path).unwrap();
            writeln!(file, "{line}").unwrap();
        }

        fn wanted(&self) -> Vec<String> {
            sioul_sync::blobs::Wanted::load(&self.base.join("share").join("memory.json")).media.into_keys().collect()
        }
    }

    impl Drop for Desk {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.base);
        }
    }

    /// The carrier's answer: taken (Ok, its Message-ID), or not and why.
    fn conf(key: &str, status: u8) -> Vec<u8> {
        let mut out = vec![0x8C, 0x81, 0x98];
        out.extend_from_slice(key.as_bytes());
        out.extend_from_slice(&[0x00, 0x8D, 0x92, 0x92, status, 0x8B]);
        out.extend_from_slice(b"msg-1\0");
        out
    }

    fn mms(key: &str, media: Vec<rules::Medium>, written: i64) -> Request {
        Request { key: key.into(), phone: "phone".into(), to: "01 99 00 12 34".into(), words: "Voilà l’entrée.".into(), media, sub: 1, written, ..Request::default() }
    }

    const KEY_A: &str = "0f3c9a5e7b2d4f6a8c1e3b5d7f9a2c4e";
    const KEY_B: &str = "1a2b3c4d5e6f708192a3b4c5d6e7f809";

    /// A multimedia message is claimed only once each medium is here and
    /// checked (asked for by its name meanwhile, nothing claimed), built as
    /// Android sends it, handed once whatever is read again, and said sent
    /// on Android's Ok and the carrier's, its row named, its file gone
    /// (decisions 12 and 13).
    #[test]
    fn a_multimedia_message_goes_once_its_media_are_here() {
        let desk = Desk::new("goes");
        let carrier = Carrier { max: 614_400, mms: true, ..Carrier::default() };
        let picture: Vec<u8> = (0..3_000u32).map(|i| (i % 251) as u8).collect();
        let medium = desk.described(&picture);
        desk.asks(&mms(KEY_A, vec![medium.clone()], NOW - MINUTE));
        let step = desk.step(&carrier);
        // Its picture not in the sharing yet: nothing claimed, nothing said, asked for by its name.
        assert!(!step.decide(NOW));
        assert!(desk.ledger().state(KEY_A).is_none() && desk.outcomes().is_empty());
        assert_eq!(desk.wanted(), [sioul_sync::blobs::name(&desk.vault.1, &medium.hash)]);
        // Here: claimed, built, handed.
        desk.seals(&picture);
        assert!(step.decide(NOW + MINUTE));
        assert_eq!(desk.states(KEY_A), [("handed".to_string(), String::new())]);
        assert!(desk.ledger().claims.iter().any(|c| c.key == KEY_A && c.state == "claimed" && c.mms), "a multimedia message's claim");
        assert!(desk.wanted().is_empty(), "asked for no more");
        {
            let handed = carrier.handed.borrow();
            assert_eq!(handed.len(), 1);
            let (key, sub, pdu) = &handed[0];
            assert_eq!((key.as_str(), *sub), (KEY_A, 1));
            assert_eq!(&pdu[..3], [0x8C, 0x80, 0x98], "an M-Send.req");
            assert!(pdu.windows(KEY_A.len()).any(|w| w == KEY_A.as_bytes()), "its key as its transaction id");
            assert!(pdu.windows(b"+33199001234/TYPE=PLMN".len()).any(|w| w == b"+33199001234/TYPE=PLMN"));
            assert!(pdu.windows(picture.len()).any(|w| w == picture.as_slice()), "the picture whole");
            assert!(!pdu.windows(b".media".len()).any(|w| w == b".media") && pdu.windows(b"image1.jpg".len()).any(|w| w == b"image1.jpg"), "plain names");
        }
        // Read again, by the records and by its small file: never handed again.
        assert!(!step.decide(NOW + 2 * MINUTE));
        assert_eq!(carrier.handed.borrow().len(), 1, "never twice");
        // Android's Ok and the carrier's: sent, its row named, its file gone, the ledger done.
        carrier.filed.borrow_mut().insert(KEY_A.into(), (31, (NOW + 2 * MINUTE) / 1000, 2));
        desk.result(KEY_A, -1, "content://mms/sent/31", &conf(KEY_A, 0x80), NOW + 2 * MINUTE);
        assert!(step.copy_results(NOW + 3 * MINUTE));
        let sent = desk.outcomes().into_iter().find(|o| o.key == KEY_A && o.state == "sent").unwrap();
        assert_eq!(sent.row, format!("mms-31@{}", (NOW + 2 * MINUTE) / 1000));
        assert_eq!(desk.ledger().state(KEY_A), Some("done"));
        assert!(!step.pdu_path(KEY_A).exists(), "its file gone");
        // Looked for later: nothing more, it is done.
        assert!(!step.look_for_lost(NOW + 50 * MINUTE));
    }

    /// Larger than its SIM's carrier takes, or a carrier that takes none:
    /// refused, said, decided once; a medium of another size than its
    /// request says, refused; Android refusing it, not sent, said.
    #[test]
    fn a_multimedia_message_its_carrier_cannot_take_is_refused_in_words() {
        let desk = Desk::new("refused");
        let picture: Vec<u8> = (0..20_000u32).map(|i| (i % 253) as u8).collect();
        let medium = desk.seals(&picture);
        desk.asks(&mms(KEY_A, vec![medium.clone()], NOW - MINUTE));
        let small = Carrier { max: 10_000, mms: true, ..Carrier::default() };
        assert!(desk.step(&small).decide(NOW));
        assert_eq!(desk.states(KEY_A), [("refused".to_string(), "too-large".to_string())]);
        assert!(small.handed.borrow().is_empty() && !desk.step(&small).pdu_path(KEY_A).exists());
        desk.asks(&mms(KEY_B, vec![medium], NOW - MINUTE));
        let none = Carrier { max: 614_400, mms: false, ..Carrier::default() };
        assert!(desk.step(&none).decide(NOW));
        assert_eq!(desk.states(KEY_B), [("refused".to_string(), "no-mms".to_string())]);
        // Android refusing the hand-off: not sent, said; its file gone.
        let key_c = "2b3c4d5e6f708192a3b4c5d6e7f80910";
        desk.asks(&mms(key_c, vec![desk.seals(b"another picture")], NOW - MINUTE));
        let refusing = Carrier { max: 614_400, mms: true, refuse: Some("permission"), ..Carrier::default() };
        assert!(desk.step(&refusing).decide(NOW));
        assert_eq!(desk.states(key_c), [("failed".to_string(), "permission".to_string())]);
        assert!(!desk.step(&refusing).pdu_path(key_c).exists());
    }

    /// Its media never came within its quarter of an hour: expired, and said
    /// that they did not reach the phone; asked for no more.
    #[test]
    fn a_multimedia_message_whose_media_never_came_expires_said_so() {
        let desk = Desk::new("late");
        let carrier = Carrier { max: 614_400, mms: true, ..Carrier::default() };
        desk.asks(&mms(KEY_A, vec![desk.described(b"a picture that never came")], NOW - 10 * MINUTE));
        assert!(!desk.step(&carrier).decide(NOW));
        assert_eq!(desk.wanted().len(), 1);
        assert!(desk.step(&carrier).decide(NOW + 6 * MINUTE));
        assert_eq!(desk.states(KEY_A), [("expired".to_string(), "media".to_string())]);
        assert!(desk.wanted().is_empty(), "asked for no more");
        assert!(carrier.handed.borrow().is_empty());
    }

    /// The carrier's answer missing (Android's Ok alone): the sent box says,
    /// there, sent; else not sent (Android filed it among the failed). A
    /// carrier's refusal and Android's own codes, each in its words.
    #[test]
    fn a_multimedia_message_s_result_is_read_with_its_carrier_s_answer() {
        let desk = Desk::new("results");
        let carrier = Carrier::default();
        carrier.filed.borrow_mut().insert(KEY_A.into(), (40, NOW / 1000, 2));
        let (k3, k4, k5) = ("3c4d5e6f708192a3b4c5d6e7f8091011", "4d5e6f708192a3b4c5d6e7f809101112", "5e6f708192a3b4c5d6e7f80910111213");
        desk.result(KEY_A, -1, "content://mms/sent/40", &[], NOW);
        desk.result(KEY_B, -1, "", &[], NOW);
        desk.result(k3, -1, "content://mms/sent/41", &conf(k3, 0xE5), NOW);
        desk.result(k4, 11, "", &[], NOW);
        desk.result(k5, 4, "", &conf(k5, 0xC2), NOW);
        // Code 4: every try ended on the network (no HTTP status), or the carrier answered 503 (the review's M-2).
        let (k6, k7) = ("6f708192a3b4c5d6e7f8091011121314", "708192a3b4c5d6e7f809101112131415");
        desk.result_http(k6, 4, "", &[], 0, NOW);
        desk.result_http(k7, 4, "", &[], 503, NOW);
        // The sent box unreadable a first time (the review's L-2): nothing said of KEY_B then.
        carrier.look_fails.set(true);
        assert!(desk.step(&carrier).copy_results(NOW + MINUTE));
        assert!(desk.states(KEY_B).is_empty(), "a look not made says nothing: {:?}", desk.states(KEY_B));
        carrier.look_fails.set(false);
        assert!(desk.step(&carrier).copy_results(NOW + 2 * MINUTE));
        let said: BTreeMap<String, (String, String)> = desk.outcomes().into_iter().map(|o| (o.key.clone(), (o.state, if o.row.is_empty() { o.reason } else { o.row }))).collect();
        assert_eq!(said[KEY_A], ("sent".to_string(), format!("mms-40@{}", NOW / 1000)), "Android's Ok, the sent box holds it");
        assert_eq!(said[KEY_B], ("doubt".to_string(), "answer".to_string()), "Android's Ok, in no box Sioul sees: the carrier may have it (I-g)");
        assert_eq!(said[k3], ("failed".to_string(), "carrier-content".to_string()));
        assert_eq!(said[k4], ("failed".to_string(), "mms-data-off".to_string()));
        assert_eq!(said[k5], ("failed".to_string(), "carrier-busy".to_string()));
        assert_eq!(said[k6], ("doubt".to_string(), "network".to_string()), "the carrier may have it");
        assert_eq!(said[k7], ("failed".to_string(), "carrier-refused".to_string()));
        // Read again: each said once.
        let count = desk.outcomes().len();
        desk.step(&carrier).copy_results(NOW + 2 * MINUTE);
        assert_eq!(desk.outcomes().len(), count);
    }

    /// Android's Ok with the carrier's answer missing or unreadable: the sent
    /// box decides; filed among the failed with a refusal Android read, not
    /// sent and why; filed there with none, the carrier may have it (its
    /// HTTP 2xx came first), the doubt (the review's re-check, I-g).
    #[test]
    fn android_s_ok_with_an_answer_nobody_read_says_the_doubt() {
        let desk = Desk::new("ok-unread");
        let carrier = Carrier::default();
        let (k1, k2) = ("8192a3b4c5d6e7f80910111213141516", "92a3b4c5d6e7f8091011121314151617");
        carrier.filed.borrow_mut().insert(k1.into(), (60, NOW / 1000, 5));
        carrier.statuses.borrow_mut().insert(k1.into(), 0xE5);
        carrier.filed.borrow_mut().insert(k2.into(), (61, NOW / 1000, 5));
        desk.result(k1, -1, "", &[], NOW);
        desk.result(k2, -1, "", &[], NOW);
        assert!(desk.step(&carrier).copy_results(NOW + MINUTE));
        assert_eq!(desk.states(k1), [("failed".to_string(), "carrier-content".to_string())], "the refusal Android read");
        assert_eq!(desk.states(k2), [("doubt".to_string(), "answer".to_string())], "an answer nobody could read");
    }

    /// Java's results are read for the outcomes' month alone, and trimmed
    /// to it once a day without being written over: a line Java appends as
    /// the file is set aside is read, and kept at the next trim (the
    /// review's re-check, I-f).
    #[test]
    fn java_s_results_are_kept_a_month_and_never_said_again() {
        let desk = Desk::new("results-trim");
        let carrier = Carrier::default();
        let (old, new) = ("a3b4c5d6e7f8091011121314151617a8", "b4c5d6e7f8091011121314151617a8b9");
        desk.result_http(old, 11, "", &[], 0, NOW - 40 * DAY_MS);
        desk.result_http(new, 11, "", &[], 0, NOW - DAY_MS);
        assert!(desk.step(&carrier).copy_results(NOW));
        assert!(desk.states(old).is_empty(), "past the month: its outcome trimmed long ago, never said again");
        assert_eq!(desk.states(new), [("failed".to_string(), "mms-data-off".to_string())]);
        let private = desk.base.join("private");
        trim_results(&private, NOW).unwrap();
        let lines = |name: &str| std::fs::read_to_string(private.join(name)).unwrap_or_default().lines().count();
        assert_eq!((lines(rules::RESULTS), lines(RESULTS_ASIDE)), (1, 2), "the month's line kept; the old file set aside");
        // Java appends a line to the file as it is set aside: read now, kept at the next trim.
        let late = "c5d6e7f8091011121314151617a8b9c0";
        let mut aside = std::fs::read_to_string(private.join(RESULTS_ASIDE)).unwrap();
        aside.push_str(&format!("{}\n", json!({ "key": late, "kind": "mms", "code": 11, "http": 0, "at": NOW })));
        std::fs::write(private.join(RESULTS_ASIDE), aside).unwrap();
        assert!(desk.step(&carrier).copy_results(NOW + MINUTE));
        assert_eq!(desk.states(late), [("failed".to_string(), "mms-data-off".to_string())]);
        trim_results(&private, NOW + DAY_MS).unwrap();
        let kept: Vec<String> = results_of(&private, NOW + DAY_MS).into_iter().map(|r| r.key).collect();
        assert!(kept.contains(&new.to_string()) && kept.contains(&late.to_string()) && !kept.contains(&old.to_string()), "{kept:?}");
        assert_eq!(lines(rules::RESULTS), 2, "the late line brought back once, the month's kept");
    }

    /// A multimedia message's claim with no word from Android (a restart
    /// while it waited for a network, Sioul stopped after the hand-off):
    /// looked for by its key from 40 minutes on, handed or not; found, sent;
    /// not found, said in doubt only past Android's last word (100 minutes:
    /// the review's L-3), a look that fails made again; never sent again;
    /// its file gone.
    #[test]
    fn a_multimedia_message_with_no_word_is_looked_for_until_android_s_last_word() {
        let desk = Desk::new("lost");
        let carrier = Carrier { max: 614_400, mms: true, ..Carrier::default() };
        desk.asks(&mms(KEY_A, vec![desk.seals(b"picture a")], NOW - MINUTE));
        desk.asks(&mms(KEY_B, vec![desk.seals(b"picture b")], NOW - MINUTE));
        let step = desk.step(&carrier);
        assert!(step.decide(NOW));
        assert_eq!(carrier.handed.borrow().len(), 2);
        assert!(!step.look_for_lost(NOW + 39 * MINUTE), "Android may still wait for a network");
        carrier.filed.borrow_mut().insert(KEY_A.into(), (51, (NOW + 30 * MINUTE) / 1000, 2));
        assert!(step.look_for_lost(NOW + 41 * MINUTE));
        assert_eq!(desk.states(KEY_A).last().unwrap().0, "sent");
        assert_eq!(desk.states(KEY_B).last().unwrap().0, "handed", "it may still go");
        assert!(!step.pdu_path(KEY_A).exists() && step.pdu_path(KEY_B).exists());
        assert!(!step.look_for_lost(NOW + 70 * MINUTE));
        carrier.look_fails.set(true);
        assert!(!step.look_for_lost(NOW + 99 * MINUTE), "a look not made, before Android's last word: nothing said");
        carrier.look_fails.set(false);
        assert!(step.look_for_lost(NOW + 100 * MINUTE));
        assert_eq!(desk.states(KEY_B).last().unwrap().0, "doubt");
        assert!(!step.pdu_path(KEY_B).exists());
        assert!(!step.decide(NOW + 101 * MINUTE) && carrier.handed.borrow().len() == 2, "never sent again");
        assert!(!step.look_for_lost(NOW + 120 * MINUTE), "said once");
    }

    /// Only the files the page made fit are sent: none elsewhere, through a
    /// link or `..` either.
    #[test]
    fn only_files_made_fit_are_sent() {
        let root = std::env::temp_dir().join(format!("sioul-made-in-{}", std::process::id()));
        let folder = root.join("texts-attached");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("1-0.jpg"), b"made").unwrap();
        std::fs::write(root.join("IMG_0001.JPG"), b"chosen").unwrap();
        assert!(made_in(&folder, &folder.join("1-0.jpg")));
        assert!(!made_in(&folder, &root.join("IMG_0001.JPG")), "the file chosen itself");
        assert!(!made_in(&folder, &folder.join("..").join("IMG_0001.JPG")), "out by ..");
        assert!(!made_in(&folder, &folder), "the folder");
        assert!(!made_in(&folder, &folder.join("gone.jpg")), "none there");
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(root.join("IMG_0001.JPG"), folder.join("2-0.jpg")).unwrap();
            assert!(!made_in(&folder, &folder.join("2-0.jpg")), "a link out");
        }
        assert!(!made_in(&root.join("none"), &folder.join("1-0.jpg")), "no folder made");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// What the phone said its carriers take, read only: a line, or one a
    /// SIM, with the picture size its carrier advises when it said one.
    #[test]
    fn what_the_phone_said_is_shown_read_only() {
        let sim = |sub: i32, mms: bool, width: u32| rules::Limits { sub, max: 614_400, width, height: if width > 0 { 1944 } else { 0 }, recipients: 20, group: true, mms };
        assert_eq!(limit_lines(&[]).len(), 1, "none said yet");
        let one = limit_lines(&[sim(1, true, 2592)]);
        assert!(one.len() == 1 && one[0].contains("2592") && one[0].contains("1944"), "{one:?}");
        assert!(!limit_lines(&[sim(1, true, 0)])[0].contains('×'), "no picture size: none said");
        let two = limit_lines(&[sim(1, true, 2592), sim(2, false, 2592)]);
        assert!(two.len() == 2 && two[0].contains("2592") && !two[1].contains("2592"), "{two:?}");
    }

    /// What a multimedia message from a computer may weigh (decision 5):
    /// nothing before the phone said; the conversation's SIM, else the
    /// smallest that takes them; 300 kB when chosen, never over the carrier's.
    #[test]
    fn what_a_message_from_here_may_weigh() {
        let sim = |sub: i32, mms: bool, max: u64| rules::Limits { sub, max, width: 2592, height: 1944, recipients: 20, group: true, mms };
        assert_eq!(weighed(None, 1, None), Weigh::Unknown, "before the phone said: no Attach");
        let two = [sim(1, true, 614_400), sim(2, false, 307_200)];
        assert_eq!(weighed(Some(&two), 1, None), Weigh::Up { max: 614_400, side: 2592 }, "the conversation's SIM");
        assert_eq!(weighed(Some(&two), 2, None), Weigh::None, "its SIM takes no multimedia message");
        assert_eq!(weighed(Some(&two), -1, None), Weigh::Up { max: 614_400, side: 2592 }, "no SIM known: those that take them");
        assert_eq!(weighed(Some(&two), 7, None), Weigh::Up { max: 614_400, side: 2592 }, "a SIM the phone did not name");
        assert_eq!(weighed(Some(&two), 1, Some(300)), Weigh::Up { max: 307_200, side: 2592 }, "300 kB chosen here");
        assert_eq!(weighed(Some(&[sim(1, true, 102_400)]), 1, Some(300)), Weigh::Up { max: 102_400, side: 2592 }, "never more than the carrier's");
        assert_eq!(weighed(Some(&[sim(1, true, 614_400), sim(3, true, 307_200)]), -1, None), Weigh::Up { max: 307_200, side: 2592 }, "the smallest of two");
        assert_eq!(weighed(Some(&[sim(1, true, 0)]), 1, None), Weigh::Unknown, "a line with no size");
        assert_eq!(weighed(Some(&[]), 1, None), Weigh::Unknown);
    }

    /// The phone's log is read only when a request about to go names no SIM,
    /// once a step at most; never in a step whose requests are decided, as
    /// every step is but the one that sends, a dose's alarm's included (the
    /// review of 18 October 2026, M-1).
    #[test]
    fn a_quick_step_reads_the_log_only_for_a_text_about_to_go() {
        let desk = Desk::new("log-reads");
        a_log_of(&desk, 200);
        let carrier = Carrier { max: 614_400, mms: true, ..Carrier::default() };
        let reads = || LOG_READS.with(std::cell::Cell::get);
        let text = |key: &str, sub: i32| Request { key: key.into(), phone: "phone".into(), to: "01 99 00 12 34".into(), body: "J’arrive.".into(), sub, written: NOW - MINUTE, ..Request::default() };
        // Decided already, with no SIM: the log stays shut.
        desk.asks(&text(KEY_A, -1));
        rules::record(&desk.base.join("private"), &Claim { key: KEY_A.into(), at: NOW - MINUTE, state: "done".into(), ..Claim::default() }).unwrap();
        let step = desk.step(&carrier);
        let before = reads();
        step.copy_results(NOW);
        step.decide(NOW);
        step.look_for_lost(NOW);
        assert_eq!(reads(), before, "every request decided: the log never read");
        // About to go with its SIM named: shut still.
        desk.asks(&text(KEY_B, 1));
        step.decide(NOW);
        assert_eq!(reads(), before, "its SIM named");
        // A multimedia message waiting for a medium, no SIM named: the log stays shut, nothing opened (L-a).
        desk.asks(&Request { sub: -1, ..mms("4d5e6f708192a3b4c5d6e7f809101112", vec![desk.seals(b"here"), desk.described(b"on its way")], NOW - MINUTE) });
        let opened = std::fs::read_dir(desk.base.join("private")).map(|d| d.count()).unwrap_or(0);
        step.decide(NOW);
        assert_eq!(reads(), before, "a message waiting for its media");
        assert_eq!(std::fs::read_dir(desk.base.join("private")).map(|d| d.count()).unwrap_or(0), opened, "nothing opened or written for it");
        // Two about to go with none named: read once.
        desk.asks(&text("1b2c3d4e5f60718293a4b5c6d7e8f901", -1));
        desk.asks(&text("2c3d4e5f60718293a4b5c6d7e8f90112", -1));
        step.decide(NOW);
        assert_eq!(reads(), before + 1, "read once for both");
    }

    /// Only a device the sharing counts asks the phone to send; a request
    /// whose sealed writer is not its file's device is passed over (the
    /// review of 18 October 2026, I-4).
    #[test]
    fn only_a_device_the_sharing_counts_asks_the_phone_to_send() {
        let desk = Desk::new("counted");
        let carrier = Carrier { max: 614_400, mms: true, ..Carrier::default() };
        desk.asks(&Request { key: KEY_A.into(), phone: "phone".into(), to: "01 99 00 12 34".into(), body: "J’arrive.".into(), sub: 1, written: NOW - MINUTE, ..Request::default() });
        // Not known yet (a new computer's first text before its records): it waits, nothing decided (I-a)…
        let unknown = |_: &str| rules::Standing::Unknown;
        assert!(!desk.step_counting(&carrier, &unknown).decide(NOW));
        assert!(desk.states(KEY_A).is_empty() && desk.ledger().state(KEY_A).is_none(), "waiting, nothing said");
        // …and goes once its records came, within its quarter of an hour.
        desk.step(&carrier).decide(NOW + MINUTE);
        assert_eq!(desk.ledger().state(KEY_A), Some("claimed"));
        // Gone (you forgot it): refused, said so.
        desk.asks(&Request { key: KEY_B.into(), phone: "phone".into(), to: "01 99 00 12 34".into(), body: "J’arrive.".into(), sub: 1, written: NOW, ..Request::default() });
        let gone = |_: &str| rules::Standing::Gone;
        assert!(desk.step_counting(&carrier, &gone).decide(NOW + MINUTE));
        assert_eq!(desk.states(KEY_B), [("refused".to_string(), "not-counted".to_string())]);
        // Not known for its whole quarter of an hour: expired, never sent.
        desk.asks(&Request { key: "3c4d5e6f708192a3b4c5d6e7f8091011".into(), phone: "phone".into(), to: "01 99 00 12 34".into(), body: "J’arrive.".into(), sub: 1, written: NOW, ..Request::default() });
        assert!(desk.step_counting(&carrier, &unknown).decide(NOW + 16 * MINUTE));
        assert_eq!(desk.states("3c4d5e6f708192a3b4c5d6e7f8091011"), [("expired".to_string(), String::new())]);
        // Moved into another device's file (the server could): passed over, never decided.
        let line = std::fs::read_to_string(rules::own_file(&desk.base.join("data"), rules::SEND, "desk")).unwrap();
        std::fs::write(rules::own_file(&desk.base.join("data"), rules::SEND, "laptop"), line).unwrap();
        assert!(rules::read_requests(&desk.base.join("data"), &desk.seal).iter().all(|r| r.device == "desk"), "the moved line passed over");
    }

    /// A phone's log of `count` texts, sealed as the phone seals it, every
    /// column of a provider's row kept (made up: fiction numbers).
    fn a_log_of(desk: &Desk, count: usize) {
        let texts: Vec<Text> = (0..count)
            .map(|n| {
                let at = NOW - DAY_MS * 400 + n as i64 * 600_000;
                let number = format!("+3319900{:04}", n % 140);
                let body: String = (0..90).map(|i| char::from(b'a' + ((n * 7 + i) % 26) as u8)).collect();
                let mut fields = BTreeMap::new();
                for (k, v) in [("_id", json!(n)), ("thread_id", json!(n % 140)), ("address", json!(number)), ("person", Value::Null), ("date", json!(at)), ("date_sent", json!(at - 2_000)), ("protocol", json!(0)), ("read", json!(1)), ("status", json!(-1)), ("type", json!(1 + n % 2)), ("reply_path_present", json!(0)), ("subject", Value::Null), ("body", json!(body)), ("service_center", json!("+33199000000")), ("locked", json!(0)), ("sub_id", json!(1)), ("error_code", json!(0)), ("creator", json!("foundation.e.message")), ("seen", json!(1))] {
                    fields.insert(k.to_string(), v);
                }
                Text { id: format!("sms-{n}@{at}"), at, thread: (n % 140).to_string(), direction: if n % 2 == 0 { "in".into() } else { "out".into() }, with: vec![number], body, sub: 1, fields, ..Text::default() }
            })
            .collect();
        rules::append(&rules::own_file(&desk.base.join("data"), rules::LOG, "phone"), &texts, &desk.seal, None).unwrap();
    }

    /// The quick step's time with 20,000 texts in the phone's log and only
    /// decided requests (a dose's alarm runs it: `health::alarm_decide` →
    /// `share::exchange_here` → `before_exchange` → `light_step`), best of
    /// five: `cargo test --release -p sioul-app quick_step_s_time -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn a_quick_step_s_time_with_twenty_thousand_texts() {
        let desk = Desk::new("quick-time");
        a_log_of(&desk, 20_000);
        let request = Request { key: KEY_A.into(), phone: "phone".into(), to: "01 99 00 12 34".into(), body: "J’arrive.".into(), sub: -1, written: NOW - MINUTE, ..Request::default() };
        desk.asks(&request);
        rules::record(&desk.base.join("private"), &Claim { key: KEY_A.into(), at: NOW - MINUTE, state: "done".into(), ..Claim::default() }).unwrap();
        let carrier = Carrier { max: 614_400, mms: true, ..Carrier::default() };
        let step = desk.step(&carrier);
        let mut best = std::time::Duration::MAX;
        for _ in 0..5 {
            let t = std::time::Instant::now();
            step.copy_results(NOW);
            step.decide(NOW);
            step.look_for_lost(NOW);
            step.sweep_outgoing(NOW);
            best = best.min(t.elapsed());
        }
        println!("the quick step, 20,000 texts in the log, its one request decided: {best:?} (best of five)");
        // A multimedia message waiting for one of its two media, the other here (the review's L-a).
        let here = desk.seals(&[7u8; 120_000]);
        let absent = desk.described(b"not here yet");
        // As a computer writes it: no SIM named.
        desk.asks(&Request { sub: -1, ..mms(KEY_B, vec![here, absent], NOW - MINUTE) });
        let mut waiting = std::time::Duration::MAX;
        for _ in 0..5 {
            let t = std::time::Instant::now();
            step.copy_results(NOW);
            step.decide(NOW);
            step.look_for_lost(NOW);
            step.sweep_outgoing(NOW);
            waiting = waiting.min(t.elapsed());
        }
        assert!(desk.ledger().state(KEY_B).is_none(), "still waiting");
        println!("the same with a multimedia message waiting for one of its two media: {waiting:?} (best of five)");
    }

    /// What its carriers let the phone send, said once a day, and at once
    /// when it changes; never twice the same within the day.
    #[test]
    fn the_phone_says_what_its_carriers_let_it_send() {
        let desk = Desk::new("limits");
        let carrier = Carrier { max: 614_400, mms: true, ..Carrier::default() };
        let step = desk.step(&carrier);
        assert!(step.publish_limits(NOW));
        assert!(!step.publish_limits(NOW + 3_600_000), "the same, within the day");
        let lines = || desk.outcomes().into_iter().filter(|o| o.state == "limits").collect::<Vec<_>>();
        assert_eq!(lines().len(), 1);
        assert_eq!((lines()[0].key.as_str(), lines()[0].limits[0].max, lines()[0].limits[0].sub), ("", 614_400, 1));
        let changed = Carrier { max: 307_200, mms: true, ..Carrier::default() };
        assert!(desk.step(&changed).publish_limits(NOW + 2 * 3_600_000), "changed: said at once");
        assert!(desk.step(&changed).publish_limits(NOW + DAY_MS + 3 * 3_600_000), "a day on: said again");
        assert_eq!(lines().len(), 3);
        assert_eq!(rules::state_of("", &desk.outcomes()), None, "no request's state");
    }

    /// Android, as the phone's quick step asks it about the parts of long
    /// texts still on their way (`texts-arriving`): the rows Java would read,
    /// or none when Android refuses or gives nothing.
    struct Segments {
        rows: std::cell::RefCell<Value>,
        allowed: std::cell::Cell<bool>,
    }

    impl Android for Segments {
        fn ask(&self, verb: &str, _: &Value) -> Value {
            match verb {
                "texts-arriving" if !self.allowed.get() => json!({ "allowed": false }),
                "texts-arriving" => json!({ "allowed": true, "raw": self.rows.borrow().clone() }),
                _ => Value::Null,
            }
        }
    }

    /// The long texts still on their way to the phone: said in its outcomes
    /// when they change, once more with none when their last parts came,
    /// never the same twice; nothing while nothing came, nor when Android
    /// refuses or gives nothing; no request's state.
    #[test]
    fn a_long_text_on_its_way_is_said_when_it_changes() {
        let desk = Desk::new("arriving");
        let android = Segments { rows: std::cell::RefCell::new(json!([])), allowed: std::cell::Cell::new(true) };
        let step = desk.step(&android);
        let lines = || desk.outcomes().into_iter().filter(|o| o.state == "arriving").collect::<Vec<_>>();
        assert!(!step.publish_arriving(NOW), "nothing on its way, nothing ever said: nothing to say");
        let part = |sequence: i64, date: i64| json!({ "address": "05 36 49 00 42", "reference": 17, "count": 5, "sequence": sequence, "date": date, "sub": 1 });
        *android.rows.borrow_mut() = json!([part(1, NOW - 9 * MINUTE), part(2, NOW - 9 * MINUTE), part(3, NOW - 8 * MINUTE)]);
        assert!(step.publish_arriving(NOW));
        assert!(!step.publish_arriving(NOW + MINUTE), "the same: said once");
        assert_eq!(lines().len(), 1);
        assert_eq!((lines()[0].key.as_str(), &lines()[0].arriving), ("", &vec![rules::Arriving { from: "+33536490042".into(), parts: 3, count: 5, sent: NOW - 9 * MINUTE }]));
        // Android refuses, or gives nothing: nothing said, the word before stays.
        android.allowed.set(false);
        assert!(!step.publish_arriving(NOW + 2 * MINUTE));
        android.allowed.set(true);
        *android.rows.borrow_mut() = Value::Null;
        assert!(!step.publish_arriving(NOW + 2 * MINUTE), "not known is not none");
        // A fourth part came; then the last, and Android handed the text over: none on its way.
        *android.rows.borrow_mut() = json!([part(1, NOW - 9 * MINUTE), part(2, NOW - 9 * MINUTE), part(3, NOW - 8 * MINUTE), part(4, NOW - 8 * MINUTE)]);
        assert!(step.publish_arriving(NOW + 3 * MINUTE));
        *android.rows.borrow_mut() = json!([]);
        assert!(step.publish_arriving(NOW + 4 * MINUTE), "none any more: said once");
        assert!(!step.publish_arriving(NOW + 5 * MINUTE));
        let said: Vec<Vec<u32>> = lines().iter().map(|o| o.arriving.iter().map(|a| a.parts).collect()).collect();
        assert_eq!(said, [vec![3], vec![4], vec![]]);
        assert_eq!(rules::state_of("", &desk.outcomes()), None, "no request's state");
        assert!(rules::arriving_now(&desk.outcomes().into_iter().map(|o| Outcome { device: "phone".into(), ..o }).collect::<Vec<_>>(), NOW + 5 * MINUTE).is_empty(), "a computer reads none on its way");
    }
}
