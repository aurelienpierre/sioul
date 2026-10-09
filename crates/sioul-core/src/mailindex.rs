// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Messages by their Message-ID, to follow `mid:` links (RFC 2392) from a
//! task, a note or a budget line to the message itself.
//!
//! Only headers are read, and what was read is kept in a cache file
//! (`$XDG_CACHE_HOME/sioul/mail-index.tsv`), by the part of a Maildir name
//! that never changes: a new look reads only the messages that came since.

use crate::maildir;
use mail_parser::MessageParser;
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

/// One message, as a link shows it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MailRef {
    pub path: PathBuf,
    pub subject: String,
    /// The sender's name, else address.
    pub from: String,
    /// The sender's address, lowercase.
    pub address: String,
    /// The Date header, Unix seconds.
    pub date: i64,
    /// The messages it answers or cites (In-Reply-To, References), bare ids.
    pub refs: Vec<String>,
}

/// Messages by Message-ID (without angle brackets).
#[derive(Debug, Clone, Default)]
pub struct MailIndex {
    by_id: BTreeMap<String, MailRef>,
}

/// "<abc@example.org>", "mid:abc@example.org" → "abc@example.org".
pub fn bare_id(id: &str) -> String {
    let id = id.trim();
    let id = id.strip_prefix("mid:").unwrap_or(id);
    crate::notes::decode(id.trim_start_matches('<').trim_end_matches('>'))
}

/// The headers of a message file: what comes before the first empty line, at most 256 KB.
fn headers_of(path: &Path) -> Option<Vec<u8>> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut bytes = Vec::with_capacity(8192);
    file.by_ref().take(256 * 1024).read_to_end(&mut bytes).ok()?;
    let end = bytes.windows(4).position(|w| w == b"\r\n\r\n").map(|p| p + 4).or_else(|| bytes.windows(2).position(|w| w == b"\n\n").map(|p| p + 2)).unwrap_or(bytes.len());
    bytes.truncate(end);
    Some(bytes)
}

fn read_ref(path: &Path) -> Option<(String, MailRef)> {
    let raw = headers_of(path)?;
    let message = MessageParser::default().parse_headers(&raw[..])?;
    let id = bare_id(message.message_id()?);
    let sender = message.from().and_then(|a| a.first());
    let from = sender.map(|a| a.name().or(a.address()).unwrap_or("").to_string()).unwrap_or_default();
    let address = sender.and_then(|a| a.address()).unwrap_or("").to_lowercase();
    let subject = message.subject().unwrap_or("").to_string();
    let date = message.date().map_or(0, |d| d.to_timestamp());
    let mut refs: Vec<String> = Vec::new();
    for list in [message.in_reply_to().as_text_list(), message.references().as_text_list()].into_iter().flatten() {
        for id in list.iter().map(|r| bare_id(r)).filter(|r| !r.is_empty()) {
            if !refs.contains(&id) {
                refs.push(id);
            }
        }
    }
    Some((id, MailRef { path: path.to_path_buf(), subject, from, address, date, refs }))
}

/// Every message file under a Maildir root: its folders' `cur` and `new`.
pub fn message_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if !entry.file_type().is_ok_and(|t| t.is_dir()) {
                continue;
            }
            if name == "cur" || name == "new" {
                out.extend(std::fs::read_dir(&path).into_iter().flatten().filter_map(Result::ok).map(|e| e.path()).filter(|p| p.is_file() && !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.'))));
            } else if name != "tmp" {
                stack.push(path);
            }
        }
    }
    out
}

impl MailIndex {
    /// Indexes the Maildirs under `roots`, reading only what `cache` does not know yet;
    /// the cache is written back.
    pub fn build(roots: &[PathBuf], cache: &Path) -> MailIndex {
        // unique part → (id, subject, from, address, date, refs); a line of an
        // older form, without the address or the refs, is read from its file again.
        let mut known: BTreeMap<String, (String, String, String, String, i64, Vec<String>)> = BTreeMap::new();
        for line in std::fs::read_to_string(cache).unwrap_or_default().lines() {
            let fields: Vec<&str> = line.split('\t').collect();
            if let [unique, id, subject, from, address, date, refs] = fields[..] {
                let refs = refs.split(' ').filter(|r| !r.is_empty()).map(str::to_string).collect();
                known.insert(unique.to_string(), (id.to_string(), subject.to_string(), from.to_string(), address.to_string(), date.parse().unwrap_or(0), refs));
            }
        }
        let mut index = MailIndex::default();
        let mut fresh: Vec<String> = Vec::new();
        for path in roots.iter().flat_map(|r| message_files(r)) {
            let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let unique = maildir::unique_part(&name).to_string();
            let (id, mail) = match known.get(&unique) {
                Some((id, subject, from, address, date, refs)) => (id.clone(), MailRef { path: path.clone(), subject: subject.clone(), from: from.clone(), address: address.clone(), date: *date, refs: refs.clone() }),
                None => match read_ref(&path) {
                    Some(found) => found,
                    None => continue,
                },
            };
            let clean = |t: &str| t.replace(['\t', '\n', '\r'], " ");
            fresh.push(format!("{unique}\t{}\t{}\t{}\t{}\t{}\t{}", clean(&id), clean(&mail.subject), clean(&mail.from), clean(&mail.address), mail.date, clean(&mail.refs.join(" "))));
            // The same message in two folders (Gmail's labels): the first found stays.
            index.by_id.entry(id).or_insert(mail);
        }
        if let Some(parent) = cache.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(cache, fresh.join("\n"));
        index
    }

    /// The message with this Message-ID ("abc@x", "<abc@x>" or "mid:abc@x").
    pub fn get(&self, id: &str) -> Option<&MailRef> {
        self.by_id.get(&bare_id(id))
    }

    /// Adds a message known otherwise (a test, a message just sent).
    pub fn insert(&mut self, id: &str, mail: MailRef) {
        self.by_id.insert(bare_id(id), mail);
    }

    /// Every message, by its Message-ID.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &MailRef)> {
        self.by_id.iter()
    }

    /// The messages of the conversations `seeds` belong to: tied by the ids
    /// they answer or cite, through any number of others (RFC 5322 §3.6.4).
    pub fn conversations(&self, seeds: &std::collections::BTreeSet<String>) -> std::collections::BTreeSet<String> {
        self.conversations_through(seeds, |_, _| true)
    }

    /// The messages of the conversations `seeds` belong to, as
    /// `conversations` finds them, through the messages `keep` lets in only
    /// (`Threads::through`).
    pub fn conversations_through(&self, seeds: &std::collections::BTreeSet<String>, keep: impl FnMut(&str, &MailRef) -> bool) -> std::collections::BTreeSet<String> {
        self.threads().through(seeds, keep)
    }

    /// Who cites whom, read once, to follow several projects' conversations.
    pub fn threads(&self) -> Threads<'_> {
        let mut cited_by: std::collections::HashMap<&str, Vec<&str>> = std::collections::HashMap::new();
        for (id, mail) in &self.by_id {
            for cited in &mail.refs {
                cited_by.entry(cited.as_str()).or_default().push(id.as_str());
            }
        }
        Threads { index: self, cited_by }
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}

/// The conversations of an index: who cites each id, known or only cited (`MailIndex::threads`).
pub struct Threads<'a> {
    index: &'a MailIndex,
    cited_by: std::collections::HashMap<&'a str, Vec<&'a str>>,
}

impl<'a> Threads<'a> {
    /// The messages of the conversations `seeds` belong to, through the
    /// messages `keep` lets in only: one it leaves out is not among them and
    /// ties nothing together (a forged message citing two conversations does
    /// not join them). An id cited but not kept here still ties the messages
    /// that cite it. The seeds are kept as given; `keep` is asked once at
    /// most per message, of those reached.
    pub fn through(&self, seeds: &std::collections::BTreeSet<String>, mut keep: impl FnMut(&str, &MailRef) -> bool) -> std::collections::BTreeSet<String> {
        let index: &'a MailIndex = self.index;
        let by_id = &index.by_id;
        let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
        let mut walk: Vec<&str> = Vec::new();
        for seed in seeds {
            if let Some((id, _)) = by_id.get_key_value(seed)
                && seen.insert(id.as_str())
            {
                walk.push(id.as_str());
            }
        }
        let mut out = std::collections::BTreeSet::new();
        while let Some(at) = walk.pop() {
            let cites = by_id.get(at).map_or(&[][..], |mail| mail.refs.as_slice());
            if by_id.contains_key(at) {
                out.insert(at.to_string());
            }
            let next: Vec<&str> = cites.iter().map(String::as_str).chain(self.cited_by.get(at).into_iter().flatten().copied()).collect();
            for id in next {
                if !seen.insert(id) {
                    continue;
                }
                match by_id.get(id) {
                    Some(mail) if !keep(id, mail) => {}
                    _ => walk.push(id),
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_messages_by_id_and_remembers_them() {
        let root = std::env::temp_dir().join(format!("sioul-mailindex-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let inbox = root.join("cur");
        std::fs::create_dir_all(&inbox).unwrap();
        std::fs::create_dir_all(root.join(".Sent").join("new")).unwrap();
        // ":" before the flags, "!" on Windows, where ":" cannot be in a file name.
        let named = |flags: &str| format!("1.U1-1.sioul{}2,{flags}", maildir::INFO);
        std::fs::write(inbox.join(named("S")), "Message-ID: <abc@caf.example>\r\nFrom: CAF <ne-pas-repondre@caf.example>\r\nSubject: Votre dossier\r\nDate: Thu, 01 Oct 2026 10:00:00 +0200\r\n\r\nBody\r\n").unwrap();
        std::fs::write(root.join(".Sent").join("new").join("2.U1-2.sioul"), "Message-ID: <sent@example.net>\nSubject: Re: Votre dossier\n\nHi\n").unwrap();
        let cache = root.join("cache.tsv");
        let index = MailIndex::build(std::slice::from_ref(&root), &cache);
        assert_eq!(index.len(), 2);
        let found = index.get("mid:abc@caf.example").unwrap();
        assert_eq!((found.subject.as_str(), found.from.as_str()), ("Votre dossier", "CAF"));
        assert_eq!(index.get("<sent@example.net>").unwrap().subject, "Re: Votre dossier");
        // From the cache, the file renamed by a flag still found.
        std::fs::rename(inbox.join(named("S")), inbox.join(named("FS"))).unwrap();
        let again = MailIndex::build(&[root.clone()], &cache);
        assert!(again.get("abc@caf.example").unwrap().path.ends_with(named("FS")));
        std::fs::remove_dir_all(&root).unwrap();
    }
}
