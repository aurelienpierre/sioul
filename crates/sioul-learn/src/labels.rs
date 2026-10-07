// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Spam or ham for each message of the corpus, from what is already there
//! (docs/spam-filter.md, "The labels": folders and tags as they are).
//!
//! - **Spam**: in a Junk-role folder, or carrying `$Junk` (or Thunderbird's
//!   older `Junk`).
//! - **Ham**: in any other folder kept (the corpus leaves out Trash, Drafts
//!   and Sent; of Gmail's All Mail it keeps what no other folder has, your own
//!   messages aside), or carrying `$NotJunk` (or `NonJunk`, `NotJunk`,
//!   `$NonJunk`, other mail apps' words for it).
//! - **Your actions** (every device's label log: junk, not junk, "Spam",
//!   "Not spam", block) beat folders and keywords; the newest one wins.
//! - **What your own filter moved** into a Junk folder (the matrix's "Move
//!   to spam") is not spam because it is there: until you say, a message
//!   whose only word is that Junk folder is left out.
//! - **One message, one label**: copies (the same Message-ID in several
//!   folders or accounts) become one; when they disagree, the log beats the
//!   keywords, which beat the Junk folder, which beats the other folders.
//!   Copies that disagree at the same level are left out.
//!
//! The filter's own verdicts are never labels: nothing here reads them.

use crate::corpus::{Place, Record};
use serde::{Deserialize, Serialize};
use sioul_core::folders::Role;
/// The label logs are phase 0's (`sioul_core::spam::labels`): one reader for their lines.
pub use sioul_core::spam::labels::{Entry, Label, Moved};
use std::collections::{BTreeMap, HashMap};

/// What decided a label, weakest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Evidence {
    /// A folder of yours other than Junk.
    Folder,
    /// The Junk folder: mostly your provider's own filing.
    JunkFolder,
    /// A junk or not-junk keyword, set by you or your other mail apps.
    Keyword,
    /// The label log: what you did in Sioul.
    Log,
}

/// What training needs of one record to decide its label (read in a first
/// pass, so the corpus is never held in memory whole).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Copy {
    pub place: Place,
    pub role: Role,
    pub date: i64,
    /// The bare Message-ID, else `header:<FNV-1a of the header block>`.
    pub key: String,
    pub junk: bool,
    pub not_junk: bool,
}

/// The keywords that say junk: `$Junk` (RFC 5788's registry), and `Junk`,
/// Thunderbird's older word, which other apps read too.
pub const JUNK: &[&str] = &["$Junk", "Junk"];
/// The keywords that say not junk: `$NotJunk`, Thunderbird's older
/// `NonJunk`, and `NotJunk` and `$NonJunk`, which other apps write.
pub const NOT_JUNK: &[&str] = &["$NotJunk", "NonJunk", "NotJunk", "$NonJunk"];

impl Copy {
    pub fn of(record: &Record) -> Copy {
        let header = record.header_bytes();
        let key = message_id(&header).unwrap_or_else(|| format!("header:{:016x}", fnv64(&header)));
        let junk = JUNK.iter().any(|k| record.has_flag(k));
        let not_junk = NOT_JUNK.iter().any(|k| record.has_flag(k));
        Copy { place: record.place(), role: record.role, date: record.date, key, junk, not_junk }
    }

    /// What this copy says by itself: a not-junk keyword (your correction)
    /// before a junk one, then the folder.
    fn says(&self) -> (Label, Evidence) {
        if self.not_junk {
            (Label::Ham, Evidence::Keyword)
        } else if self.junk {
            (Label::Spam, Evidence::Keyword)
        } else if self.role == Role::Junk {
            (Label::Spam, Evidence::JunkFolder)
        } else {
            (Label::Ham, Evidence::Folder)
        }
    }
}

/// A header block's Message-ID, bare, read as the label log reads it
/// (`spam::labels`), so that your actions find their message.
pub(crate) fn message_id(header: &[u8]) -> Option<String> {
    let headers = mail_parser::MessageParser::default().parse_headers(header)?;
    let id = sioul_core::mailindex::bare_id(headers.message_id()?);
    (!id.trim().is_empty()).then_some(id)
}

/// A header block's From address, lowercase.
pub(crate) fn from_address(header: &[u8]) -> Option<String> {
    let headers = mail_parser::MessageParser::default().parse_headers(header)?;
    let address = headers.from()?.first()?.address()?.trim().to_lowercase();
    (!address.is_empty()).then_some(address)
}

/// Your actions, as every device's label log keeps them (`$XDG_STATE_HOME/sioul/spam/labels/`), oldest first.
pub fn read_log(dirs: &crate::Dirs) -> Vec<Entry> {
    sioul_core::spam::labels::read_all_in(&dirs.state)
}

/// What your own filter moved, every device's log, oldest first.
pub fn read_moved(dirs: &crate::Dirs) -> Vec<Moved> {
    sioul_core::spam::labels::read_moved_in(&dirs.state)
}

/// One message to learn from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Labeled {
    pub key: String,
    /// The copy whose words and headers are learned from.
    pub place: Place,
    pub label: Label,
    pub evidence: Evidence,
    /// When it first arrived: the earliest INTERNALDATE of its copies.
    pub date: i64,
    /// The role of the folder the copy learned from is in (an inbox, a Junk folder…).
    pub role: Role,
}

/// What deciding the labels found, in numbers only.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Summary {
    /// Records read, copies included.
    pub records: u64,
    pub ham: u64,
    pub spam: u64,
    /// Messages whose copies disagreed at the same level: left out.
    pub ambiguous: u64,
    /// Messages your own filter moved into a Junk folder, about which you
    /// said nothing since: left out (the filter's verdicts are never labels).
    #[serde(default)]
    pub moved: u64,
    /// Labels decided by each kind of evidence.
    pub by_log: u64,
    pub by_keyword: u64,
    pub by_junk_folder: u64,
    pub by_folder: u64,
    /// Ham and spam per account (the copy learned from).
    pub accounts: BTreeMap<String, (u64, u64)>,
}

/// One label per message, from every copy read and the log; `moved`, what
/// your own filter moved: its Junk folder alone says nothing.
pub fn decide(copies: Vec<Copy>, log: &[Entry], moved: &[Moved]) -> (Vec<Labeled>, Summary) {
    let mut summary = Summary { records: copies.len() as u64, ..Summary::default() };
    // What your filter moved, by Message-ID.
    let filtered: std::collections::HashSet<&str> = moved.iter().filter_map(|m| m.message_id.as_deref()).collect();
    // The newest action for each Message-ID and each place.
    let mut by_id: HashMap<&str, &Entry> = HashMap::new();
    let mut by_place: HashMap<(&str, &str, u32, u32), &Entry> = HashMap::new();
    for action in log {
        if let Some(id) = action.message_id.as_deref().filter(|id| !id.is_empty()) {
            let newest = by_id.entry(id).or_insert(action);
            if action.at >= newest.at {
                *newest = action;
            }
        }
        let newest = by_place.entry((&action.account, &action.folder, action.uidvalidity, action.uid)).or_insert(action);
        if action.at >= newest.at {
            *newest = action;
        }
    }
    let mut groups: BTreeMap<String, Vec<Copy>> = BTreeMap::new();
    for copy in copies {
        groups.entry(copy.key.clone()).or_default().push(copy);
    }
    let mut labeled = Vec::with_capacity(groups.len());
    for (key, mut copies) in groups {
        let date = copies.iter().map(|c| c.date).filter(|d| *d > 0).min().unwrap_or(0);
        let action = std::iter::once(by_id.get(key.as_str()).copied())
            .chain(copies.iter().map(|c| by_place.get(&(c.place.account.as_str(), c.place.folder.as_str(), c.place.uidvalidity, c.place.uid)).copied()))
            .flatten()
            .max_by_key(|a| a.at);
        // Moved by your filter, and nothing said of it: its Junk folder is the filter's word, not yours.
        if action.is_none() && filtered.contains(key.as_str()) {
            copies.retain(|c| c.says().1 != Evidence::JunkFolder);
            if copies.is_empty() {
                summary.moved += 1;
                continue;
            }
        }
        let decided = match action {
            Some(action) => {
                // The copy where you acted, else the one whose own word agrees, else the first.
                let at = copies.iter().find(|c| c.place.account == action.account && c.place.folder == action.folder && c.place.uidvalidity == action.uidvalidity && c.place.uid == action.uid);
                let agreeing = copies.iter().filter(|c| c.says().0 == action.label).max_by_key(|c| (c.says().1, std::cmp::Reverse((c.date, c.place.clone()))));
                let copy = at.or(agreeing).unwrap_or(&copies[0]);
                Some((copy, action.label, Evidence::Log))
            }
            None => {
                let strongest = copies.iter().map(|c| c.says().1).max().expect("a group has copies");
                let top: Vec<&Copy> = copies.iter().filter(|c| c.says().1 == strongest).collect();
                if top.iter().any(|c| c.says().0 != top[0].says().0) {
                    summary.ambiguous += 1;
                    None
                } else {
                    let copy = top.iter().min_by_key(|c| (c.date, c.place.clone())).expect("not empty");
                    Some((*copy, copy.says().0, strongest))
                }
            }
        };
        let Some((copy, label, evidence)) = decided else { continue };
        match label {
            Label::Ham => summary.ham += 1,
            Label::Spam => summary.spam += 1,
        }
        match evidence {
            Evidence::Log => summary.by_log += 1,
            Evidence::Keyword => summary.by_keyword += 1,
            Evidence::JunkFolder => summary.by_junk_folder += 1,
            Evidence::Folder => summary.by_folder += 1,
        }
        let counts = summary.accounts.entry(copy.place.account.clone()).or_default();
        match label {
            Label::Ham => counts.0 += 1,
            Label::Spam => counts.1 += 1,
        }
        labeled.push(Labeled { key, place: copy.place.clone(), label, evidence, date, role: copy.role });
    }
    (labeled, summary)
}

/// 64-bit FNV-1a.
pub(crate) fn fnv64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, &b| (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn copy(account: &str, folder: &str, role: Role, uid: u32, key: &str) -> Copy {
        Copy { place: Place { account: account.into(), folder: folder.into(), uidvalidity: 1, uid }, role, date: i64::from(uid), key: key.into(), junk: false, not_junk: false }
    }

    fn action(at: i64, folder: &str, uid: u32, id: Option<&str>, label: Label) -> Entry {
        use sioul_core::spam::labels::Source;
        let source = if label == Label::Spam { Source::Junk } else { Source::NotSpam };
        Entry { at, account: "home".into(), folder: folder.into(), uidvalidity: 1, uid, message_id: id.map(str::to_string), label, source }
    }

    fn label_of<'a>(labeled: &'a [Labeled], key: &str) -> Option<&'a Labeled> {
        labeled.iter().find(|l| l.key == key)
    }

    #[test]
    fn folders_and_keywords() {
        let copies = vec![
            copy("home", "INBOX", Role::Inbox, 1, "a@example.org"),
            copy("home", "Junk", Role::Junk, 2, "b@example.org"),
            Copy { junk: true, ..copy("home", "INBOX", Role::Inbox, 3, "c@example.org") },
            Copy { not_junk: true, ..copy("home", "Junk", Role::Junk, 4, "d@example.org") },
            Copy { junk: true, not_junk: true, ..copy("home", "INBOX", Role::Inbox, 5, "e@example.org") },
        ];
        let (labeled, summary) = decide(copies, &[], &[]);
        let says = |key| label_of(&labeled, key).map(|l| (l.label, l.evidence));
        assert_eq!(says("a@example.org"), Some((Label::Ham, Evidence::Folder)));
        assert_eq!(says("b@example.org"), Some((Label::Spam, Evidence::JunkFolder)));
        assert_eq!(says("c@example.org"), Some((Label::Spam, Evidence::Keyword)));
        assert_eq!(says("d@example.org"), Some((Label::Ham, Evidence::Keyword)), "$NotJunk beats the Junk folder");
        assert_eq!(says("e@example.org"), Some((Label::Ham, Evidence::Keyword)), "your correction beats $Junk");
        assert_eq!((summary.ham, summary.spam, summary.records), (3, 2, 5));
        assert_eq!(summary.accounts["home"], (3, 2));
    }

    /// Thunderbird's older keywords, and other apps' words, count as `$Junk` and `$NotJunk` do.
    #[test]
    fn older_keywords_are_labels_too() {
        let record = |uid: u32, folder: &str, role: Role, flags: &[&str]| Record {
            account: "home".into(),
            folder: folder.into(),
            role,
            uidvalidity: 1,
            uid,
            date: i64::from(uid),
            flags: flags.iter().map(|f| f.to_string()).collect(),
            size: 0,
            header: format!("Message-ID: <{uid}@example.org>\r\n"),
            header_latin1: false,
            structure: None,
            plain: None,
            html: None,
            fetched: 0,
        };
        let copies = vec![
            Copy::of(&record(1, "INBOX", Role::Inbox, &["Junk"])),
            Copy::of(&record(2, "Junk", Role::Junk, &["NonJunk"])),
            Copy::of(&record(3, "Junk", Role::Junk, &["notjunk"])),
            Copy::of(&record(4, "Junk", Role::Junk, &["$NonJunk", "\\Seen"])),
            Copy::of(&record(5, "INBOX", Role::Inbox, &["Junk", "NonJunk"])),
            Copy::of(&record(6, "INBOX", Role::Inbox, &["JunkMail"])),
        ];
        assert_eq!(copies[0].key, "1@example.org");
        let (labeled, summary) = decide(copies, &[], &[]);
        let says = |n: u32| labeled.iter().find(|l| l.key == format!("{n}@example.org")).map(|l| (l.label, l.evidence));
        assert_eq!(says(1), Some((Label::Spam, Evidence::Keyword)));
        assert_eq!(says(2), Some((Label::Ham, Evidence::Keyword)), "NonJunk beats the Junk folder");
        assert_eq!(says(3), Some((Label::Ham, Evidence::Keyword)), "keywords' case aside");
        assert_eq!(says(4), Some((Label::Ham, Evidence::Keyword)));
        assert_eq!(says(5), Some((Label::Ham, Evidence::Keyword)), "Thunderbird left both: the correction wins");
        assert_eq!(says(6), Some((Label::Ham, Evidence::Folder)), "another keyword is no label");
        assert_eq!(summary.by_keyword, 5);
    }

    #[test]
    fn headers_read_as_the_porch_reads_them() {
        let header = b"From: =?utf-8?Q?Aur=C3=A9lien?= <Owner@Example.ORG>\r\nMessage-ID: <abc@example.org>\r\n";
        assert_eq!(from_address(header).as_deref(), Some("owner@example.org"));
        assert_eq!(message_id(header).as_deref(), Some("abc@example.org"));
        assert_eq!(message_id(b"Subject: none\r\n"), None);
    }

    /// Copies become one message; the stronger evidence wins, the same level disagreeing leaves it out.
    #[test]
    fn copies_and_their_disagreements() {
        let copies = vec![
            // In the inbox of one account, in Junk of another: Junk wins.
            copy("home", "INBOX", Role::Inbox, 10, "x@example.org"),
            copy("work", "Junk", Role::Junk, 11, "x@example.org"),
            // $Junk on one copy, $NotJunk on another: no way to tell.
            Copy { junk: true, ..copy("home", "INBOX", Role::Inbox, 12, "y@example.org") },
            Copy { not_junk: true, ..copy("work", "INBOX", Role::Inbox, 13, "y@example.org") },
            // The same message twice in ham folders: one message, its first arrival.
            copy("home", "Archive", Role::Archive, 21, "z@example.org"),
            copy("home", "INBOX", Role::Inbox, 20, "z@example.org"),
        ];
        let (labeled, summary) = decide(copies, &[], &[]);
        let x = label_of(&labeled, "x@example.org").unwrap();
        assert_eq!((x.label, x.evidence, x.place.account.as_str(), x.date), (Label::Spam, Evidence::JunkFolder, "work", 10));
        assert!(label_of(&labeled, "y@example.org").is_none());
        assert_eq!(summary.ambiguous, 1);
        let z = label_of(&labeled, "z@example.org").unwrap();
        assert_eq!((z.place.uid, z.date), (20, 20));
        assert_eq!(labeled.len(), 2);
    }

    /// Your actions beat everything, the newest first; by Message-ID, or by
    /// place for a message that has none.
    #[test]
    fn the_log_beats_folders_and_keywords() {
        let copies = vec![
            Copy { not_junk: true, ..copy("home", "Junk", Role::Junk, 1, "p@example.org") },
            copy("home", "INBOX", Role::Inbox, 2, "header:0000000000000001"),
            copy("home", "Junk", Role::Junk, 3, "q@example.org"),
        ];
        let log = vec![
            action(100, "INBOX", 9, Some("p@example.org"), Label::Ham),
            action(200, "INBOX", 9, Some("p@example.org"), Label::Spam),
            action(150, "INBOX", 2, None, Label::Spam),
            action(300, "Junk", 3, Some("q@example.org"), Label::Ham),
        ];
        let (labeled, summary) = decide(copies, &log, &[]);
        let says = |key| label_of(&labeled, key).map(|l| (l.label, l.evidence));
        assert_eq!(says("p@example.org"), Some((Label::Spam, Evidence::Log)), "the newest action");
        assert_eq!(says("header:0000000000000001"), Some((Label::Spam, Evidence::Log)), "by place");
        assert_eq!(says("q@example.org"), Some((Label::Ham, Evidence::Log)), "Not spam on a junk message");
        assert_eq!(summary.by_log, 3);
    }

    #[test]
    fn the_log_reads_phase_zeros_lines() {
        let dir = std::env::temp_dir().join(format!("sioul-learn-labels-{}", std::process::id()));
        let dirs = crate::Dirs::under(&dir);
        std::fs::create_dir_all(&dirs.state).unwrap();
        // The older single log, before the logs were shared; then another device's own.
        std::fs::write(
            dirs.state.join(sioul_core::spam::labels::FILE),
            "{\"at\":1791360000,\"account\":\"home\",\"folder\":\"INBOX\",\"uidvalidity\":1700000000,\"uid\":4521,\"message_id\":\"abc@example.org\",\"label\":\"ham\",\"source\":\"not-spam\"}\nnot json\n{\"at\":1,\"account\":\"home\",\"folder\":\"INBOX\",\"uidvalidity\":1,\"uid\":2,\"message_id\":null,\"label\":\"spam\",\"source\":\"block\"}\n",
        )
        .unwrap();
        std::fs::create_dir_all(dirs.labels()).unwrap();
        std::fs::write(dirs.labels().join("phone.jsonl"), "{\"at\":1791360500,\"account\":\"home\",\"folder\":\"Junk\",\"uidvalidity\":3,\"uid\":7,\"message_id\":\"def@example.org\",\"label\":\"spam\",\"source\":\"junk\"}\n").unwrap();
        let log = read_log(&dirs);
        assert_eq!(log.len(), 3, "every device's, oldest first");
        assert_eq!((log[0].label, log[0].message_id.as_deref()), (Label::Spam, None));
        assert_eq!((log[1].label, log[1].message_id.as_deref(), log[1].source), (Label::Ham, Some("abc@example.org"), sioul_core::spam::labels::Source::NotSpam));
        assert_eq!(log[2].message_id.as_deref(), Some("def@example.org"));
        let _ = std::fs::remove_dir_all(dir);
    }

    /// What your own filter moved into a Junk folder is no label: left out
    /// until you say; what you said wins; another copy of it still speaks.
    #[test]
    fn what_the_filter_moved_is_no_label() {
        let moved = |id: &str| Moved { at: 50, account: "home".into(), folder: "INBOX".into(), uidvalidity: 1, uid: 9, message_id: Some(id.into()), class: sioul_core::spam::Class::Spam };
        let copies = vec![
            copy("home", "Junk", Role::Junk, 1, "m@example.org"),
            copy("home", "Junk", Role::Junk, 2, "said@example.org"),
            copy("home", "Junk", Role::Junk, 3, "both@example.org"),
            copy("work", "INBOX", Role::Inbox, 4, "both@example.org"),
            copy("home", "Junk", Role::Junk, 5, "provider@example.org"),
        ];
        let log = vec![action(100, "Junk", 2, Some("said@example.org"), Label::Spam)];
        let moves = [moved("m@example.org"), moved("said@example.org"), moved("both@example.org")];
        let (labeled, summary) = decide(copies, &log, &moves);
        let says = |key| label_of(&labeled, key).map(|l| (l.label, l.evidence));
        assert_eq!(says("m@example.org"), None, "the filter's own word");
        assert_eq!(says("said@example.org"), Some((Label::Spam, Evidence::Log)), "then you said it");
        assert_eq!(says("both@example.org"), Some((Label::Ham, Evidence::Folder)), "its other copy");
        assert_eq!(says("provider@example.org"), Some((Label::Spam, Evidence::JunkFolder)), "your provider's filing, as before");
        assert_eq!((summary.moved, summary.spam, summary.ham), (1, 2, 1));
    }
}
