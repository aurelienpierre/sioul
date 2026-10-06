// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What other applications hand Sioul to write: files and text shared to it
//! (Android's share sheet, or one of your addresses straight from it), a
//! `mailto:` link (Android, or the computer's mail program). Each becomes a
//! draft, its window opened; nothing is sent until you press Send
//! (docs/client.md, "Writing from other applications").
//!
//! A request waits in `handed/incoming/` (in Sioul's data folder) until the
//! window takes it: `<id>.json`, the request ([`Handed`]), written once and
//! whole by whoever hands it (Android's ShareActivity.java, a second
//! `sioul-app mailto:…`); `<id>.done.json` ([`Arrived`]), the files copied for
//! it, written once they all are; `<id>.draft` ([`Mark`]), the draft made from
//! it while its files are still coming. The files are Sioul's own copies, in
//! `handed/files/<id>/`: they go with the draft when it is sent or discarded
//! (`Draft::discard`). Neither travels to your other devices: the sharing
//! carries the drafts, not this folder, and a request is this device's.

use crate::compose::{Draft, split_addresses};
use crate::config::data_dir;
use crate::config::{Account, Config};
use crate::mailto;
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, SystemTime};

/// A message another application asks Sioul to write.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Handed {
    /// Its own name, the name of its files' folder too: letters, digits and "-".
    pub id: String,
    /// The account to write from (one of your addresses chosen in Android's
    /// share sheet), by its id or its address; "" for Sioul's usual one.
    #[serde(default)]
    pub account: String,
    /// A `mailto:` address, as it came (RFC 6068, `mailto::parse`).
    #[serde(default)]
    pub mailto: String,
    #[serde(default)]
    pub to: Vec<String>,
    #[serde(default)]
    pub cc: Vec<String>,
    #[serde(default)]
    pub bcc: Vec<String>,
    #[serde(default)]
    pub subject: String,
    /// The text shared, put in the message's text.
    #[serde(default)]
    pub text: String,
    /// How many files are being copied for it; they come as `<id>.done.json`.
    #[serde(default)]
    pub files: u32,
    /// The process copying them: another one, and they will never all come.
    #[serde(default)]
    pub copier: u32,
    /// Files not taken at all, by name: one named by its path on the phone
    /// (any application could name Sioul's own private files that way), or
    /// one of Sioul's own.
    #[serde(default)]
    pub refused: Vec<String>,
}

impl Handed {
    /// A request of this process, named after the moment it came.
    pub fn new() -> Handed {
        Handed { id: new_id(), copier: std::process::id(), ..Handed::default() }
    }
}

/// The files copied for a request.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Arrived {
    #[serde(default)]
    pub files: Vec<ArrivedFile>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArrivedFile {
    /// Its name, as the application that shared it gave it.
    #[serde(default)]
    pub name: String,
    /// Where it was copied, in its request's folder; "" when it could not be.
    #[serde(default)]
    pub path: String,
    /// Why it could not be, in the system's words.
    #[serde(default)]
    pub error: String,
    /// Copied only in part: the copy stopped with Sioul.
    #[serde(default)]
    pub stopped: bool,
}

/// A request taken, its files still coming: the draft made from it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mark {
    /// The request's id.
    #[serde(default)]
    pub id: String,
    pub draft: String,
    #[serde(default)]
    pub copier: u32,
    #[serde(default)]
    pub files: u32,
}

/// A name for a request: the moment, and a little of the process, as drafts' names are.
fn new_id() -> String {
    let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default();
    format!("{:x}-{:x}-{:x}", now.as_secs(), now.subsec_nanos(), std::process::id())
}

/// Whether `id` can name a request: letters, digits and "-", nothing that
/// leaves its folder.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 80 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// The folders of requests and of their files.
#[derive(Debug, Clone)]
pub struct Incoming {
    root: PathBuf,
}

impl Incoming {
    /// Those of this profile: `handed/` in Sioul's data folder.
    pub fn here() -> Incoming {
        Incoming { root: data_dir().join("handed") }
    }

    /// Those in `root` (tests).
    pub fn at(root: &Path) -> Incoming {
        Incoming { root: root.to_path_buf() }
    }

    fn requests(&self) -> PathBuf {
        self.root.join("incoming")
    }

    /// Where the files of every request are copied, each in a folder of its own.
    pub fn files(&self) -> PathBuf {
        self.root.join("files")
    }

    fn named(&self, id: &str, suffix: &str) -> PathBuf {
        self.requests().join(format!("{id}{suffix}"))
    }

    /// Where the files of request `id` are copied.
    pub fn files_of(&self, id: &str) -> PathBuf {
        self.files().join(id)
    }

    /// A request handed for the window to take: written whole or not at all.
    pub fn hand(&self, handed: &Handed) -> Result<PathBuf, String> {
        if !valid_id(&handed.id) {
            return Err(format!("{}: not a request's name", handed.id));
        }
        let path = self.named(&handed.id, ".json");
        write_whole(&path, &serde_json::to_string(handed).map_err(|e| e.to_string())?)?;
        Ok(path)
    }

    /// The requests waiting, oldest first. One that does not read (cut, or
    /// from a newer Sioul) is set aside as `.unread`, read no more.
    pub fn waiting(&self) -> Vec<Handed> {
        let Ok(entries) = std::fs::read_dir(self.requests()) else { return Vec::new() };
        let mut found: Vec<Handed> = Vec::new();
        for path in entries.filter_map(Result::ok).map(|e| e.path()) {
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else { continue };
            let Some(id) = name.strip_suffix(".json").filter(|id| valid_id(id)) else { continue };
            match std::fs::read_to_string(&path).ok().and_then(|text| serde_json::from_str::<Handed>(&text).ok()).filter(|h| h.id == id) {
                Some(handed) => found.push(handed),
                None => {
                    let _ = std::fs::rename(&path, path.with_extension("unread"));
                }
            }
        }
        found.sort_by(|a, b| a.id.cmp(&b.id));
        found
    }

    /// The files copied for request `id`, once they all are; none before.
    pub fn arrived(&self, id: &str) -> Option<Arrived> {
        serde_json::from_str(&std::fs::read_to_string(self.named(id, ".done.json")).ok()?).ok()
    }

    /// Request `id` taken, made into draft `draft`, `files` still coming from
    /// process `copier`: kept, so that a Sioul started again finds them.
    pub fn mark(&self, id: &str, draft: &str, copier: u32, files: u32) -> Result<(), String> {
        let mark = Mark { id: id.to_string(), draft: draft.to_string(), copier, files };
        write_whole(&self.named(id, ".draft"), &serde_json::to_string(&mark).map_err(|e| e.to_string())?)?;
        let _ = std::fs::remove_file(self.named(id, ".json"));
        Ok(())
    }

    /// The requests taken whose files were still coming.
    pub fn marks(&self) -> Vec<Mark> {
        let Ok(entries) = std::fs::read_dir(self.requests()) else { return Vec::new() };
        let mut found: Vec<Mark> = entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "draft"))
            .filter_map(|p| serde_json::from_str::<Mark>(&std::fs::read_to_string(&p).ok()?).ok())
            .filter(|m| valid_id(&m.id))
            .collect();
        found.sort_by(|a, b| a.id.cmp(&b.id));
        found
    }

    /// Request `id` done with: its request, its mark, its arrival.
    pub fn done(&self, id: &str) {
        for suffix in [".json", ".draft", ".done.json"] {
            let _ = std::fs::remove_file(self.named(id, suffix));
        }
    }

    /// Request `id` dropped, the files copied for it too (no draft to take them).
    pub fn drop_request(&self, id: &str) {
        self.done(id);
        if valid_id(id) {
            let _ = std::fs::remove_dir_all(self.files_of(id));
        }
    }

    /// What arrived of request `id` when its copy stopped before the end
    /// (Sioul stopped meanwhile): the files copied whole; those cut short
    /// removed and said, as those never begun.
    pub fn salvage(&self, id: &str, expected: u32) -> Arrived {
        let mut arrived = Arrived::default();
        if let Ok(entries) = std::fs::read_dir(self.files_of(id)) {
            let mut paths: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
            paths.sort();
            for path in paths {
                let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                if let Some(cut) = name.strip_suffix(".part") {
                    let _ = std::fs::remove_file(&path);
                    arrived.files.push(ArrivedFile { name: cut.to_string(), stopped: true, ..ArrivedFile::default() });
                } else if path.is_file() {
                    arrived.files.push(ArrivedFile { name, path: path.to_string_lossy().to_string(), ..ArrivedFile::default() });
                }
            }
        }
        let missing = (expected as usize).saturating_sub(arrived.files.len());
        arrived.files.extend((0..missing).map(|_| ArrivedFile { stopped: true, ..ArrivedFile::default() }));
        arrived
    }

    /// Folders of copies nobody holds any more (a draft gone in a way that
    /// left them, a Sioul stopped while copying), removed once a day old.
    /// `held` are the files the drafts hold.
    pub fn tidy(&self, held: &[PathBuf], now: SystemTime) {
        let Ok(entries) = std::fs::read_dir(self.files()) else { return };
        for folder in entries.filter_map(Result::ok).map(|e| e.path()) {
            let Some(id) = folder.file_name().and_then(|n| n.to_str()).filter(|id| valid_id(id)).map(str::to_string) else { continue };
            let waiting = [".json", ".draft", ".done.json"].iter().any(|suffix| self.named(&id, suffix).exists());
            let old = std::fs::metadata(&folder).and_then(|m| m.modified()).is_ok_and(|at| now.duration_since(at).unwrap_or_default() > Duration::from_secs(24 * 3600));
            if !waiting && old && !held.iter().any(|path| path.starts_with(&folder)) {
                let _ = std::fs::remove_dir_all(&folder);
            }
        }
    }
}

/// A text written whole: under another name, then renamed.
fn write_whole(path: &Path, text: &str) -> Result<(), String> {
    let fail = |e: std::io::Error| format!("{}: {e}", path.display());
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(fail)?;
    }
    let temporary = path.with_extension("new");
    std::fs::write(&temporary, text).map_err(fail)?;
    std::fs::rename(&temporary, path).map_err(fail)
}

/// The account a message handed to Sioul goes out from: the one asked (by
/// its id or its address) when it sends, else Sioul's usual one, the first
/// by priority among those that send, as for "Write". None when no account
/// sends.
pub fn sender<'a>(config: &'a Config, asked: &str) -> Option<&'a Account> {
    let asked = asked.trim();
    let sends = || config.accounts.iter().filter(|a| a.syncs());
    sends()
        .find(|a| !asked.is_empty() && (a.id == asked || a.address.as_deref().is_some_and(|address| address.eq_ignore_ascii_case(asked))))
        .or_else(|| sends().min_by_key(|a| a.priority))
}

/// The draft a request asks for, not saved yet: its recipients (the
/// `mailto:` address's, then those handed beside it, each once), its subject,
/// its text (the address's body, then the text shared), the message it
/// answers, and the account's signature below. None when no account sends.
pub fn draft(handed: &Handed, config: &Config) -> Option<Draft> {
    let account = sender(config, &handed.account)?;
    let asked = mailto::parse(&handed.mailto).unwrap_or_default();
    let mut draft = Draft::new(&account.id);
    let list = |mut from_address: Vec<String>, beside: &[String]| {
        from_address.extend(beside.iter().flat_map(|entry| split_addresses(entry)));
        mailto::once_each(&mut from_address);
        from_address
    };
    draft.to = list(asked.to, &handed.to);
    draft.cc = list(asked.cc, &handed.cc);
    draft.bcc = list(asked.bcc, &handed.bcc);
    draft.subject = [asked.subject.as_str(), handed.subject.as_str()]
        .iter()
        .map(|s| s.replace(['\r', '\n'], " ").trim().to_string())
        .find(|s| !s.is_empty())
        .unwrap_or_default();
    let text = handed.text.replace("\r\n", "\n");
    draft.body = [asked.body.trim_end(), text.trim_end()].iter().filter(|t| !t.trim().is_empty()).copied().collect::<Vec<&str>>().join("\n\n");
    if let Some(id) = asked.in_reply_to {
        draft.references = vec![id.trim_matches(['<', '>']).to_string()];
        draft.in_reply_to = Some(id.trim_matches(['<', '>']).to_string());
    }
    draft.add_signature(account.signature.as_deref());
    Some(draft)
}

/// The files of `arrived` attached to `draft`, those copied into `folder`
/// (their request's) only; returns those that could not be, by name, with why.
pub fn attach(draft: &mut Draft, folder: &Path, arrived: &Arrived) -> Vec<ArrivedFile> {
    let mut missing = Vec::new();
    for file in &arrived.files {
        let path = PathBuf::from(&file.path);
        let inside = !file.path.is_empty() && path.starts_with(folder) && path.components().all(|c| !matches!(c, Component::ParentDir | Component::CurDir));
        if inside && path.is_file() {
            if !draft.attachments.contains(&path) {
                draft.attachments.push(path);
            }
        } else {
            missing.push(file.clone());
        }
    }
    missing
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Priority, Security};

    fn config() -> Config {
        let mut config = Config::default();
        let mut work = Account::imap("work", "me@work.example", "imap.work.example", 993, Security::Tls, None);
        work.signature = Some("Me, at work".into());
        let mut home = Account::imap("home", "me@home.example", "imap.home.example", 993, Security::Tls, None);
        home.priority = Priority::Above;
        // A calendar account sends nothing.
        let calendars = Account::dav("calendars", "me@dav.example", "dav.example", None, None);
        config.accounts = vec![work, home, calendars];
        config
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sioul-handed-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_sender_is_the_one_chosen_else_the_usual_one() {
        let config = config();
        // An address chosen in the share sheet: its account, by id or by address.
        assert_eq!(sender(&config, "work").unwrap().id, "work");
        assert_eq!(sender(&config, "ME@WORK.example").unwrap().id, "work");
        // Nothing chosen, an account gone, one that sends nothing: the usual one, first by priority.
        assert_eq!(sender(&config, "").unwrap().id, "home");
        assert_eq!(sender(&config, "gone").unwrap().id, "home");
        assert_eq!(sender(&config, "calendars").unwrap().id, "home");
        // No account that sends: none.
        let mut none = config.clone();
        none.accounts.retain(|a| !a.syncs());
        assert!(sender(&none, "work").is_none());
        let handed = Handed { id: "x".into(), text: "hello".into(), ..Handed::default() };
        assert!(draft(&handed, &none).is_none());
    }

    #[test]
    fn a_share_becomes_a_draft() {
        let config = config();
        // A photo shared from the gallery to one address, with a line of text.
        let handed = Handed {
            id: "1a-2b".into(),
            account: "work".into(),
            subject: "The\nflat".into(),
            text: "Here it is.\r\nSee you".into(),
            to: vec!["jane@example.org, Bob <bob@example.org>".into()],
            cc: vec!["JANE@example.org".into(), "carl@example.org".into()],
            files: 1,
            ..Handed::default()
        };
        let made = draft(&handed, &config).unwrap();
        assert_eq!(made.account, "work");
        assert_eq!(made.to, ["jane@example.org", "Bob <bob@example.org>"]);
        // Once per list; the same address may be in To and Cc, as given.
        assert_eq!(made.cc, ["JANE@example.org", "carl@example.org"]);
        assert_eq!(made.subject, "The flat");
        assert_eq!(made.body, "Here it is.\nSee you\n\n-- \nMe, at work");
        assert!(made.attachments.is_empty() && made.kind == crate::compose::DraftKind::New);
    }

    #[test]
    fn a_mailto_link_becomes_a_draft() {
        let config = config();
        let handed = Handed {
            id: "x".into(),
            mailto: "mailto:list@example.org?cc=a@example.org&subject=Re%3A%20Minutes&body=I%20agree.&In-Reply-To=%3Cm1@example.org%3E".into(),
            to: vec!["list@example.org".into(), "b@example.org".into()],
            subject: "ignored: the address's subject comes first".into(),
            text: "Shared beside it.".into(),
            ..Handed::default()
        };
        let made = draft(&handed, &config).unwrap();
        // Nothing chosen: the usual sender, without a signature here.
        assert_eq!(made.account, "home");
        assert_eq!((made.to.as_slice(), made.cc.as_slice()), (&["list@example.org".to_string(), "b@example.org".to_string()][..], &["a@example.org".to_string()][..]));
        assert_eq!(made.subject, "Re: Minutes");
        assert_eq!(made.body, "I agree.\n\nShared beside it.");
        assert_eq!((made.in_reply_to.as_deref(), made.references.as_slice()), (Some("m1@example.org"), &["m1@example.org".to_string()][..]));
        // Not a mailto: address: the rest only.
        let odd = Handed { id: "y".into(), mailto: "https://example.org".into(), subject: "S".into(), ..Handed::default() };
        let made = draft(&odd, &config).unwrap();
        assert_eq!((made.to.len(), made.subject.as_str(), made.body.as_str()), (0, "S", ""));
    }

    #[test]
    fn requests_and_their_files_go_through_the_folder() {
        let dir = scratch("folder");
        let incoming = Incoming::at(&dir);
        assert!(incoming.waiting().is_empty() && incoming.marks().is_empty());
        let mut handed = Handed::new();
        handed.text = "A PDF".into();
        handed.files = 2;
        handed.refused = vec!["id_rsa".into()];
        incoming.hand(&handed).unwrap();
        // Unreadable ones are set aside, a bad name never read.
        std::fs::write(dir.join("incoming").join("0-cut.json"), "{\"id\": ").unwrap();
        std::fs::write(dir.join("incoming").join("..json"), "{}").unwrap();
        assert_eq!(incoming.waiting(), vec![handed.clone()]);
        assert!(dir.join("incoming").join("0-cut.unread").exists());
        assert!(Incoming::at(&dir).hand(&Handed { id: "../x".into(), ..Handed::default() }).is_err());
        // Taken: marked with its draft while the files come.
        incoming.mark(&handed.id, "d1", 42, 2).unwrap();
        assert!(incoming.waiting().is_empty());
        assert_eq!(incoming.marks(), vec![Mark { id: handed.id.clone(), draft: "d1".into(), copier: 42, files: 2 }]);
        assert!(incoming.arrived(&handed.id).is_none());
        // They come: one copied, one refused by its application.
        let folder = incoming.files_of(&handed.id);
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("Lease.pdf"), b"%PDF").unwrap();
        let arrived = Arrived {
            files: vec![
                ArrivedFile { name: "Lease.pdf".into(), path: folder.join("Lease.pdf").to_string_lossy().into(), ..ArrivedFile::default() },
                ArrivedFile { name: "Photo.jpg".into(), error: "Permission denied".into(), ..ArrivedFile::default() },
                // Never a file outside the request's own folder.
                ArrivedFile { name: "x".into(), path: folder.join("..").join("..").join("secret").to_string_lossy().into(), ..ArrivedFile::default() },
            ],
        };
        write_whole(&dir.join("incoming").join(format!("{}.done.json", handed.id)), &serde_json::to_string(&arrived).unwrap()).unwrap();
        let mut made = Draft::new("work");
        let missing = attach(&mut made, &folder, &incoming.arrived(&handed.id).unwrap());
        assert_eq!(made.attachments, vec![folder.join("Lease.pdf")]);
        assert_eq!(missing.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["Photo.jpg", "x"]);
        // Attached twice, once.
        attach(&mut made, &folder, &arrived);
        assert_eq!(made.attachments.len(), 1);
        incoming.done(&handed.id);
        assert!(incoming.marks().is_empty() && incoming.arrived(&handed.id).is_none());
        // Folders nobody holds go after a day; a held one, or a young one, stays.
        let later = SystemTime::now() + Duration::from_secs(25 * 3600);
        incoming.tidy(&made.attachments, later);
        assert!(folder.exists());
        incoming.tidy(&[], SystemTime::now());
        assert!(folder.exists());
        incoming.tidy(&[], later);
        assert!(!folder.exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_copy_stopped_halfway_keeps_what_came_whole() {
        let dir = scratch("salvage");
        let incoming = Incoming::at(&dir);
        let folder = incoming.files_of("r1");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("a.jpg"), b"whole").unwrap();
        std::fs::write(folder.join("b.mp4.part"), b"half").unwrap();
        let arrived = incoming.salvage("r1", 3);
        assert!(!folder.join("b.mp4.part").exists());
        let mut made = Draft::new("work");
        let missing = attach(&mut made, &folder, &arrived);
        assert_eq!(made.attachments, vec![folder.join("a.jpg")]);
        // The one cut short is named; the one never begun has no name to say.
        assert_eq!(missing.iter().map(|f| (f.name.as_str(), f.stopped)).collect::<Vec<_>>(), [("b.mp4", true), ("", true)]);
        // A request dropped takes its copies with it.
        incoming.drop_request("r1");
        assert!(!folder.exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
