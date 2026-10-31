// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Messages by their Message-ID, to follow `mid:` links (RFC 2392) from a
//! task, a note or a budget line to the message itself.
//!
//! Only headers are read, and what was read is kept in a cache file
//! (`$XDG_CACHE_HOME/sioul/mail-index.tsv`), by the part of a Maildir name
//! that never changes: a new look reads only the messages that came since.
//!
//! A message in several folders (Gmail's labels and "All Mail", Sent and the
//! inbox, the inbox and the trash) keeps every copy, the one that ranks
//! first in front (`folders::Role::rank`: the inbox, your own folders, the
//! archive, Sent, the junk and the trash, "All Mail"), so that `mid:` opens
//! that copy. The rank is found from each copy's folder at each build and is
//! never written: the cache's lines are those every version of Sioul reads
//! and writes.

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
    /// The copy that ranks first of each Message-ID.
    by_id: BTreeMap<String, MailRef>,
    /// Every file of each Message-ID, the copy that ranks first in front,
    /// then as found: a message copied into two folders, or another message
    /// given the same ID (anyone who knows one can), each kept.
    paths: BTreeMap<String, Vec<PathBuf>>,
}

/// A Maildir root and the folders its account's server last listed
/// (`folders::saved`), which say what each of its folders is for; without
/// them, a folder's purpose is read from its name.
#[derive(Debug, Clone, Default)]
pub struct Root {
    pub path: PathBuf,
    pub folders: Vec<crate::folders::Folder>,
}

/// What the folder holding `file` under `root` is for: the inbox for the
/// Maildir itself, else its saved folder's purpose, else its name's.
fn role_of(root: &Root, file: &Path) -> crate::folders::Role {
    use crate::folders::Role;
    let Some(dir) = file.parent().and_then(Path::parent) else { return Role::Other };
    if dir == root.path {
        return Role::Inbox;
    }
    let local = dir.strip_prefix(&root.path).ok().and_then(|p| p.to_str()).unwrap_or_default();
    match root.folders.iter().find(|f| f.local == local) {
        Some(folder) => folder.role,
        None => crate::folders::role_of(local.trim_start_matches('.'), None),
    }
}

/// "<abc@example.org>", "mid:abc@example.org", "mid:abc%40example.org",
/// "mid:%3Cabc%40example.org%3E" → "abc@example.org".
pub fn bare_id(id: &str) -> String {
    let id = id.trim();
    let id = id.strip_prefix("mid:").unwrap_or(id);
    let decoded = crate::notes::decode(id.trim_start_matches('<').trim_end_matches('>'));
    decoded.trim().trim_start_matches('<').trim_end_matches('>').to_string()
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
    /// Indexes the Maildirs under `roots`, reading only what `cache` does not
    /// know yet; the cache is written back. Each folder's purpose is read
    /// from its name (`build_ranked` takes the servers' word for it).
    pub fn build(roots: &[PathBuf], cache: &Path) -> MailIndex {
        let roots: Vec<Root> = roots.iter().map(|path| Root { path: path.clone(), folders: Vec::new() }).collect();
        MailIndex::build_ranked(&roots, cache)
    }

    /// The Maildirs of these accounts' addresses, each with its saved folders.
    pub fn roots_of(config: &crate::config::Config) -> Vec<Root> {
        config.accounts.iter().filter(|a| a.kind == crate::config::AccountKind::Imap).map(|a| Root { path: a.maildir_path(), folders: crate::folders::saved(&a.id) }).collect()
    }

    /// `build`, each copy ranked by its folder's purpose as `roots` say it.
    pub fn build_ranked(roots: &[Root], cache: &Path) -> MailIndex {
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
        // Every copy of each Message-ID, with its rank, in the order found.
        let mut copies: BTreeMap<String, Vec<(u8, MailRef)>> = BTreeMap::new();
        for (root, path) in roots.iter().flat_map(|r| message_files(&r.path).into_iter().map(move |p| (r, p))) {
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
            copies.entry(id).or_default().push((role_of(root, &path).rank(), mail));
        }
        // The copy that ranks first in front; copies of one rank as found.
        for (id, mut found) in copies {
            found.sort_by_key(|(rank, _)| *rank);
            index.paths.insert(id.clone(), found.iter().map(|(_, mail)| mail.path.clone()).collect());
            if let Some((_, first)) = found.into_iter().next() {
                index.by_id.insert(id, first);
            }
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

    /// Every file of this Message-ID found here, in the order found.
    pub fn paths(&self, id: &str) -> Vec<PathBuf> {
        self.paths.get(&bare_id(id)).cloned().unwrap_or_default()
    }

    /// Adds a message known otherwise (a test, a message just sent).
    pub fn insert(&mut self, id: &str, mail: MailRef) {
        self.paths.entry(bare_id(id)).or_default().push(mail.path.clone());
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

    /// One message in several folders opens from the copy that ranks first,
    /// whatever folder or address was read first: the inbox before the trash
    /// and Gmail's "All Mail", the archive before the trash; every copy kept.
    #[test]
    fn mid_opens_the_copy_that_ranks_first() {
        use crate::folders::{Folder, Role};
        let base = std::env::temp_dir().join(format!("sioul-mailindex-rank-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let put = |root: &Path, folder: &str, name: &str, id: &str| {
            let dir = root.join(folder).join("cur");
            std::fs::create_dir_all(&dir).unwrap();
            let file = dir.join(format!("{name}.U1-1.sioul{}2,S", maildir::INFO));
            std::fs::write(&file, format!("Message-ID: <{id}>\r\nFrom: A <a@example.org>\r\nSubject: Hello\r\n\r\nBody\r\n")).unwrap();
            file
        };
        let (first, second) = (base.join("first"), base.join("second"));
        // The first address read holds the trash's and "All Mail"'s copies; the second, the inbox's.
        let trash = put(&first, ".Trash", "1", "one@example.org");
        put(&first, ".[Gmail].All Mail", "2", "one@example.org");
        let inbox = put(&second, "", "3", "one@example.org");
        // A message in the trash and the archive only: the archive's, the trash read first.
        put(&first, ".Bin", "4", "two@example.org");
        let archive = put(&second, ".Archives", "5", "two@example.org");
        let folders = vec![
            Folder { name: "[Gmail]/All Mail".into(), display: "All Mail".into(), role: Role::All, local: ".[Gmail].All Mail".into(), special: true },
            Folder { name: "Bin".into(), display: "Bin".into(), role: Role::Trash, local: ".Bin".into(), special: true },
        ];
        let roots = [Root { path: first.clone(), folders }, Root { path: second.clone(), folders: vec![Folder { name: "Archives".into(), display: "Archives".into(), role: Role::Archive, local: ".Archives".into(), special: true }] }];
        let index = MailIndex::build_ranked(&roots, &base.join("cache.tsv"));
        assert_eq!(index.get("mid:one@example.org").map(|m| m.path.clone()), Some(inbox.clone()), "the inbox's copy");
        assert_eq!(index.paths("one@example.org").len(), 3, "every copy kept");
        assert_eq!(index.paths("one@example.org")[0], inbox);
        assert_eq!(index.get("two@example.org").map(|m| m.path.clone()), Some(archive), "the archive's copy, not the trash's");
        // By names alone, without the servers' word: the inbox before ".Trash".
        let plain = MailIndex::build(&[first.clone(), second.clone()], &base.join("cache.tsv"));
        assert_eq!(plain.get("one@example.org").map(|m| m.path.clone()), Some(inbox));
        assert!(plain.paths("one@example.org").contains(&trash));
        std::fs::remove_dir_all(&base).unwrap();
    }
}
