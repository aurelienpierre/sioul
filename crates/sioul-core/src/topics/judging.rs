// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The topics' model on every device (docs/topics.md, "The model"): its
//! table where this device keeps it, a message judged by it, and what it
//! judged, kept per table.
//!
//! - **The table**, `$XDG_DATA_HOME/sioul/topics/table.bin`: this device's
//!   own training writes it (`write_table`; docs/spam-filter.md, "Every
//!   device trains its own"), the sharing carries it only while a device's
//!   Sioul does not train its own, and every device reads its own (`cached`). Its last eight bytes,
//!   its checksum, name it (`Judgments::table`).
//! - **A message judged** (`judge_card`): one line, its proven sender's
//!   words then the spam filter's words of its subject and its start
//!   (`topics::line`), with the tokenizer's words the table was made with
//!   (`Meta::lexicon`), so that a phone and the computer read it alike.
//! - **What was judged** stays on this device, never shared:
//!   `$XDG_CACHE_HOME/sioul/topics/judged/<table>.jsonl`, a line per message
//!   by its bare Message-ID (none: its account and file's digest), its topic
//!   by id and its band, or none. A new table starts a new file and the old
//!   ones go. Mail arriving is judged after each fetch (`judge_files`); the
//!   mail kept here, a bounded number at a time, when a table arrives
//!   (`judge_kept`).
//!
//! A topic the model judges is the last word on a message (`rules::with_model`):
//! your word, your rules, a conversation, the seeds and the presets come first.

use super::table::{Band, Table};
use crate::card::Card;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Bytes of a message read for its words: its headers and its text's start.
const START: u64 = 1 << 20;

/// Where this device keeps the table.
pub fn table_path() -> PathBuf {
    crate::config::data_dir().join("topics").join("table.bin")
}

/// Where this device keeps what each table judged.
pub fn judged_dir() -> PathBuf {
    crate::config::cache_dir().join("topics").join("judged")
}

/// A table written where every device reads it, whole or not at all; every
/// view of topics told (`changes`).
pub fn write_table(path: &Path, table: &Table) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    crate::filelock::replace(path, &table.to_bytes()).map_err(|e| format!("{}: {e}", path.display()))?;
    crate::changes::said(crate::changes::Kind::Topics);
    Ok(())
}

/// A table's name: its file's checksum (its last eight bytes).
pub fn table_id(bytes: &[u8]) -> u64 {
    bytes.len().checked_sub(8).and_then(|at| bytes.get(at..)).and_then(|b| b.try_into().ok()).map_or(0, u64::from_le_bytes)
}

type Loaded = (PathBuf, std::time::SystemTime, u64, Arc<Table>, u64);
static TABLE: Mutex<Option<Loaded>> = Mutex::new(None);

/// The table at `path` and its name, read again only when its file changed; none without one, or one this Sioul cannot read.
pub fn cached(path: &Path) -> Option<(Arc<Table>, u64)> {
    let meta = std::fs::metadata(path).ok()?;
    let (modified, len) = (meta.modified().ok()?, meta.len());
    let mut kept = TABLE.lock().ok()?;
    if let Some((p, m, l, table, id)) = kept.as_ref()
        && p == path
        && *m == modified
        && *l == len
    {
        return Some((Arc::clone(table), *id));
    }
    let bytes = std::fs::read(path).ok()?;
    let table = Arc::new(Table::from_bytes(&bytes).ok()?);
    let id = table_id(&bytes);
    *kept = Some((path.to_path_buf(), modified, len, Arc::clone(&table), id));
    Some((table, id))
}

/// A message's line as the table reads it: its sender's words when Sioul's
/// checks verify it, then the spam filter's words of its subject and its start.
pub fn line_of(lexicon: &crate::spam::tokenize::Lexicon, card: &Card, verified: bool) -> Vec<String> {
    let words: Vec<String> = crate::spam::tokenize::tokens_with(lexicon, &card.subject, &card.excerpt).words.into_iter().filter(|w| !w.is_empty() && !w.contains(char::is_whitespace)).collect();
    super::line(&words, &super::sender_words(card, verified))
}

/// What a table says of a message: its topic by id and its band, none for "no topic".
pub fn judge_card(table: &Table, card: &Card, verified: bool) -> Option<(String, Band)> {
    let lexicon = crate::spam::tokenize::Lexicon::cached(&table.meta.lexicon.clone().unwrap_or_default());
    let judged = table.judge(&line_of(&lexicon, card, verified));
    match (judged.topic, judged.band) {
        (Some(t), band @ (Band::Sure | Band::Maybe)) => table.meta.topics.get(t).map(|id| (id.clone(), band)),
        _ => None,
    }
}

/// One line of what a table judged.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Line {
    id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    topic: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    band: Option<Band>,
}

/// What the table in use judged of the mail kept here.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Judgments {
    /// The table's name (`table_id`); 0 without a table.
    pub table: u64,
    /// Each message with a topic, by its key: the topic's id and its band.
    pub topics: HashMap<String, (String, Band)>,
    /// Every message judged, a topic or none.
    pub judged: HashSet<String>,
}

impl Judgments {
    /// What the table at `path` judged, as this device keeps it.
    pub fn load(path: &Path) -> Judgments {
        let Some((_, table)) = cached(path) else { return Judgments::default() };
        Judgments::read(&judged_dir(), table)
    }

    fn read(dir: &Path, table: u64) -> Judgments {
        let mut out = Judgments { table, ..Judgments::default() };
        let text = std::fs::read_to_string(file_of(dir, table)).unwrap_or_default();
        for line in text.lines().filter_map(|l| serde_json::from_str::<Line>(l).ok()) {
            match (line.topic, line.band) {
                (Some(topic), Some(band)) => {
                    out.topics.insert(line.id.clone(), (topic, band));
                }
                _ => {
                    out.topics.remove(&line.id);
                }
            }
            out.judged.insert(line.id);
        }
        out
    }

    /// A message's topic and band, by its bare Message-ID.
    pub fn of(&self, id: &str) -> Option<&(String, Band)> {
        self.topics.get(id)
    }
}

fn file_of(dir: &Path, table: u64) -> PathBuf {
    dir.join(format!("{table:016x}.jsonl"))
}

/// What a table judged, appended to its file; another table's files removed.
fn keep(dir: &Path, table: u64, judged: &[(String, Option<(String, Band)>)]) -> Result<(), String> {
    use std::io::Write;
    if judged.is_empty() {
        return Ok(());
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let own = file_of(dir, table);
    for entry in std::fs::read_dir(dir).into_iter().flatten().filter_map(Result::ok) {
        if entry.path() != own && entry.path().extension().is_some_and(|e| e == "jsonl") {
            let _ = std::fs::remove_file(entry.path());
        }
    }
    let mut text = String::new();
    for (id, said) in judged {
        let line = Line { id: id.clone(), topic: said.as_ref().map(|(t, _)| t.clone()), band: said.as_ref().map(|(_, b)| *b) };
        text.push_str(&serde_json::to_string(&line).map_err(|e| e.to_string())?);
        text.push('\n');
    }
    crate::filelock::with_lock(&own, || {
        std::fs::OpenOptions::new().create(true).append(true).open(&own).and_then(|mut file| file.write_all(text.as_bytes())).map_err(|e| format!("{}: {e}", own.display()))
    })?;
    crate::changes::said(crate::changes::Kind::Topics);
    Ok(())
}

/// A message's key: its bare Message-ID, else its account and its file's digest.
fn key_of(account: &str, card: &Card, digest: &str) -> String {
    card.message_id.as_deref().map(crate::mailindex::bare_id).filter(|id| !id.is_empty()).unwrap_or_else(|| format!("{account}/{digest}"))
}

/// A message's start, its headers and the beginning of its text.
fn card_of(path: &Path) -> Option<Card> {
    use std::io::Read as _;
    let mut bytes = Vec::new();
    std::fs::File::open(path).ok()?.take(START).read_to_end(&mut bytes).ok()?;
    Card::from_bytes(&bytes)
}

/// Whether Sioul's checks verify a message's sender (the stamps its account trusts), and it is not your own.
fn verified(card: &Card, trusted: &[String], own: &HashSet<String>) -> bool {
    use crate::trust::{self, Trust};
    let results = trust::read_auth_results(&card.headers, trusted);
    let excused = card.is_list.then(|| trust::list_excused(results.as_ref(), &card.headers, &trust::sealers(trusted, &[]))).flatten();
    let (judged, _) = trust::judge_sender(results.as_ref(), excused.as_ref(), card.sender_domain());
    judged == Trust::Verified && !card.from_address.as_deref().is_some_and(|a| own.contains(&a.trim().to_lowercase()))
}

fn own_addresses(config: &crate::config::Config) -> HashSet<String> {
    crate::porch::own_addresses(&config.mail_sources()).into_iter().map(|a| a.trim().to_lowercase()).collect()
}

/// Mail of `account` that just arrived, judged by the table in use; how many judged.
pub fn judge_files(config: &crate::config::Config, account: &str, files: &[PathBuf]) -> usize {
    let Some((table, id)) = cached(&table_path()) else { return 0 };
    let dir = judged_dir();
    let done = Judgments::read(&dir, id);
    let trusted = config.mail_sources().into_iter().find(|s| s.account.as_deref() == Some(account)).map(|s| s.trusted_ids).unwrap_or_default();
    let own = own_addresses(config);
    let mut judged = Vec::new();
    for file in files {
        let Some(card) = card_of(file) else { continue };
        let digest = file.file_name().map(|n| n.to_string_lossy().split(':').next().unwrap_or("").to_string()).unwrap_or_default();
        let key = key_of(account, &card, &digest);
        if done.judged.contains(&key) || judged.iter().any(|(k, _)| *k == key) {
            continue;
        }
        let said = judge_card(&table, &card, verified(&card, &trusted, &own));
        judged.push((key, said));
    }
    let n = judged.len();
    let _ = keep(&dir, id, &judged);
    n
}

/// A folder's or a file's time, as `MailStamp` holds it.
type Stamped = (PathBuf, Option<std::time::SystemTime>, u64);

/// The mail kept here as a walk found it, cheaply: each address's folder,
/// and each of its folders with its `new` and `cur`, by their times (a
/// folder's time moves whenever a message comes into it, goes, or changes
/// its flags, which renames it), and the table's judgments file by its time
/// and length.
#[derive(Debug, Clone, PartialEq, Eq)]
struct MailStamp {
    table: u64,
    places: Vec<Stamped>,
}

fn stamped(path: PathBuf) -> Stamped {
    let meta = std::fs::metadata(&path).ok();
    let (time, len) = (meta.as_ref().and_then(|m| m.modified().ok()), meta.map_or(0, |m| m.len()));
    (path, time, len)
}

fn mail_stamp(addresses: &[(String, PathBuf)], dir: &Path, table: u64) -> MailStamp {
    let mut places = Vec::new();
    for (_, root) in addresses {
        places.push(stamped(root.clone()));
        for local in crate::mailcache::folders(root) {
            let folder = root.join(&local);
            places.extend([stamped(folder.join("new")), stamped(folder.join("cur"))]);
            if !local.is_empty() {
                places.push(stamped(folder));
            }
        }
    }
    places.push(stamped(file_of(dir, table)));
    MailStamp { table, places }
}

/// The mail kept here as the last walk of this process left it with nothing
/// more to judge (`judge_kept`): while it stays so, the next walk is passed
/// over. The window looks each minute, a phone's step at each of its own:
/// each walk lists every message of every folder, and a mailbox of tens of
/// thousands of messages costs a fraction of a second each time, for nothing
/// once all of it is judged.
static SETTLED: Mutex<Option<MailStamp>> = Mutex::new(None);

/// The mail kept here that the table in use has not judged yet, `limit`
/// messages at most (Junk, Trash and Drafts left out); how many judged, and
/// whether some wait still. Nothing is walked while the mail kept here and
/// what was judged stay as the last walk that left nothing to judge found
/// them (`SETTLED`).
pub fn judge_kept(config: &crate::config::Config, limit: usize) -> (usize, bool) {
    use crate::folders::Role;
    let Some((table, id)) = cached(&table_path()) else { return (0, false) };
    let dir = judged_dir();
    let addresses: Vec<(String, std::path::PathBuf)> = config.accounts.iter().filter(|a| a.kind == crate::config::AccountKind::Imap).map(|a| (a.id.clone(), a.maildir_path())).collect();
    let before = mail_stamp(&addresses, &dir, id);
    if SETTLED.lock().is_ok_and(|settled| settled.as_ref() == Some(&before)) {
        return (0, false);
    }
    let done = Judgments::read(&dir, id);
    let sources = config.mail_sources();
    let own = own_addresses(config);
    let mut judged: Vec<(String, Option<(String, Band)>)> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut more = false;
    let saved: Vec<_> = addresses.iter().map(|(id, _)| crate::folders::saved(id)).collect();
    'folders: for (a, local) in crate::mailpass::folders(&addresses) {
        let (account, root) = &addresses[a];
        let trusted = sources.iter().find(|s| s.account.as_deref() == Some(account.as_str())).map(|s| s.trusted_ids.clone()).unwrap_or_default();
        let role = if local.is_empty() { Role::Inbox } else { saved[a].iter().find(|f| f.local == local).map_or_else(|| crate::folders::role_of(local.trim_start_matches('.'), None), |f| f.role) };
        if matches!(role, Role::Junk | Role::Trash | Role::Drafts) {
            continue;
        }
        for found in crate::mailpass::folder(&crate::mailpass::Headers, account, root, &local, 0).found {
            let key = found.line.message_id.as_deref().map(crate::mailindex::bare_id).filter(|k| !k.is_empty()).unwrap_or_else(|| format!("{account}/{}", found.line.unique));
            if done.judged.contains(&key) || !seen.insert(key.clone()) {
                continue;
            }
            if judged.len() >= limit {
                more = true;
                break 'folders;
            }
            let Some(card) = card_of(&found.path) else { continue };
            let said = judge_card(&table, &card, verified(&card, &trusted, &own));
            judged.push((key, said));
        }
    }
    let n = judged.len();
    let kept = keep(&dir, id, &judged).is_ok();
    // Nothing more to judge: settled as found before the walk (mail come
    // meanwhile moved a folder's time since, and the next walk reads it),
    // the judgments file as this walk left it.
    if let Ok(mut settled) = SETTLED.lock() {
        *settled = (!more && kept).then(|| MailStamp { places: before.places.iter().take(before.places.len() - 1).cloned().chain([stamped(file_of(&dir, id))]).collect(), ..before });
    }
    (n, more)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What a table judged is kept by message, the newest word winning; a
    /// new table's file takes the place of the old one's.
    #[test]
    fn what_a_table_judged_is_kept_by_message() {
        let dir = std::env::temp_dir().join(format!("sioul-topics-judged-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ((), said) = crate::changes::heard(|| keep(&dir, 7, &[("a@example.org".into(), Some(("bank".into(), Band::Sure))), ("b@example.org".into(), None)]).unwrap());
        assert_eq!(said, [crate::changes::Kind::Topics]);
        keep(&dir, 7, &[("c@example.org".into(), Some(("bank".into(), Band::Maybe)))]).unwrap();
        let read = Judgments::read(&dir, 7);
        assert_eq!(read.of("a@example.org"), Some(&("bank".to_string(), Band::Sure)));
        assert_eq!(read.of("c@example.org").map(|(_, b)| *b), Some(Band::Maybe));
        assert!(read.of("b@example.org").is_none() && read.judged.contains("b@example.org"));
        keep(&dir, 8, &[("a@example.org".into(), None)]).unwrap();
        assert!(!file_of(&dir, 7).exists(), "the old table's file goes");
        assert!(Judgments::read(&dir, 8).of("a@example.org").is_none());
        assert_eq!(table_id(&[0, 0, 1, 0, 0, 0, 0, 0, 0, 0]), 1);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// The mail kept here, stamped cheaply for the walk of what is left to
    /// judge (`judge_kept`): the same while nothing moves; another as soon as
    /// a message comes, changes its flags or goes, a folder is made, or the
    /// judgments file changes. Folder times are set back between changes, as
    /// a coarse clock would leave them, so that only the change itself moves them.
    #[test]
    fn the_mail_s_stamp_moves_with_any_change_and_only_then() {
        let base = std::env::temp_dir().join(format!("sioul-topics-stamp-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (root, judged) = (base.join("mail"), base.join("judged"));
        for sub in ["new", "cur", "tmp", ".Archive/new", ".Archive/cur", ".Archive/tmp"] {
            std::fs::create_dir_all(root.join(sub)).unwrap();
        }
        let addresses = [("a".to_string(), root.clone())];
        let past = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        let settle = || {
            for dir in ["", "new", "cur", ".Archive", ".Archive/new", ".Archive/cur"] {
                std::fs::File::open(root.join(dir)).unwrap().set_modified(past).unwrap();
            }
        };
        settle();
        let first = mail_stamp(&addresses, &judged, 7);
        assert_eq!(first, mail_stamp(&addresses, &judged, 7), "nothing moved");
        assert_ne!(first, mail_stamp(&addresses, &judged, 8), "another table");
        let mut seen = vec![first];
        let mut changed = |change: &dyn Fn()| {
            settle();
            change();
            let now = mail_stamp(&addresses, &judged, 7);
            assert!(!seen.contains(&now), "{:?}", now.places);
            seen.push(now);
        };
        changed(&|| std::fs::write(root.join("new/1.a"), "Subject: one\r\n\r\n").unwrap());
        changed(&|| std::fs::rename(root.join("new/1.a"), root.join("cur/1.a:2,S")).unwrap());
        changed(&|| std::fs::write(root.join(".Archive/cur/2.a:2,"), "Subject: two\r\n\r\n").unwrap());
        changed(&|| std::fs::remove_file(root.join("cur/1.a:2,S")).unwrap());
        changed(&|| ["new", "cur", "tmp"].iter().for_each(|sub| std::fs::create_dir_all(root.join(".Later").join(sub)).unwrap()));
        changed(&|| keep(&judged, 7, &[("c@example.org".into(), None)]).unwrap());
        let _ = std::fs::remove_dir_all(base);
    }
}
