// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Drafts in the server's Drafts folder (decided 27 November 2026; docs/client.md,
//! "Drafts on the server"), so that other mail programs see what you are
//! writing, as every mail program does.
//!
//! **One source of truth**: a draft is Sioul's file (`compose::Draft`,
//! `drafts/<id>.toml`), carried between your devices by the sharing as
//! before. The server holds a copy of it made for other mail programs
//! (`compose::server_copy`), named by a Message-ID of its own for each version
//! ([`message_id`]: `sioul-draft.<id>.<rev>@<domain>`: Gmail drops a second
//! message under a Message-ID it holds already, so one Message-ID across
//! versions would keep the old one) and marked with the version it carries
//! ([`HEADER`]: `id=<id>; rev=<rev>`, the version a digest of the draft's
//! file, the same on every device that holds the same draft).
//!
//! **Never mail**: a message in the account's Drafts folder (or Gmail's All
//! Mail), from the account's own address, that carries both marks for the
//! same version of a draft ([`own_copy`]) is never stored by a fetch, so no
//! reader of the mail kept here meets it (agents, the money history, ties,
//! rules); the mail index and the agents' `read_message` leave out besides a
//! Drafts folder's message with both marks. The same marks in the inbox, or
//! from anyone else, are mail like any other.
//!
//! **Which device writes**: the device on which a draft was saved (the
//! writing window, an agent's draft: [`saved`]) puts its copy there at its
//! account's next round; a device that put a copy there before puts the
//! draft's new version there when the sharing brings one (its version is no
//! longer the one it placed). Placing a version removes the other copies of
//! the same draft (an older version, or the same placed twice by two devices
//! at once, the highest UID kept): two devices that placed different versions
//! meanwhile end with the one the sharing keeps, one copy. Copies are found
//! by a listing of the Drafts folder's heads (the Message-ID, [`HEADER`] and
//! From of each message), which every server answers, never by a search.
//!
//! **Sent or deleted**: a draft sent or discarded on a device ([`discarded`])
//! has its copies removed by that device; a device that placed a copy of a
//! draft the sharing then took away removes it too. A copy sent or deleted
//! in another mail program is never put back: Sioul keeps its draft (your
//! text is never lost), and places it again only once you change it.
//!
//! **Encrypted drafts**: a draft to be encrypted, or one that answers or
//! forwards an encrypted message whatever its Encrypt says
//! ([`wants_encryption`]), goes there encrypted to your own certificate, its
//! subject and Bcc inside it ("..." as the outer subject, as protected headers
//! write it), or not at all when no certificate of yours is here: nothing of
//! an encrypted conversation lies on a server in clear.
//!
//! **A server that refuses** (over quota, a copy past its limit), or a
//! connection lost on the way: that version is tried again after a growing
//! wait ([`Retry`]), the other drafts going on; a refusal is said once.
//!
//! What each device placed is its own state (`$XDG_STATE_HOME/sioul/drafts-on-server.toml`),
//! never shared: an older Sioul does none of this, and lists Sioul's copies in
//! the account's Drafts folder as it lists any other program's drafts.

use crate::compose::Draft;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The header that marks a copy: `id=<draft id>; rev=<version>`.
pub const HEADER: &str = "X-Sioul-Draft";

/// The left part of a copy's Message-ID: `sioul-draft.<id>.<rev>@<domain>`.
const ID_PREFIX: &str = "sioul-draft.";

/// Above this, a draft's attachments stay out of its copy (the copy is placed
/// again at each change of the draft): its text and recipients still go.
pub const ATTACHED_AT_MOST: u64 = 25 * 1024 * 1024;

/// The first wait before a version the server refused, or could not take, is
/// tried again; each try after doubles it, up to [`RETRY_AT_MOST`].
pub const RETRY_FIRST: i64 = 5 * 60;
pub const RETRY_AT_MOST: i64 = 24 * 3600;

/// A draft's version: a digest of its file as saved (FNV-1a, 64 bits), the
/// same on every device that holds the same draft, since the sharing carries
/// the file as it is.
pub fn rev(file: &[u8]) -> String {
    let mut digest: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in file {
        digest ^= u64::from(*byte);
        digest = digest.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{digest:016x}")
}

/// The value of [`HEADER`] for one version of a draft.
pub fn mark(id: &str, rev: &str) -> String {
    format!("id={id}; rev={rev}")
}

/// The Message-ID of the copy of one version of a draft (without its angle brackets).
pub fn message_id(id: &str, rev: &str, domain: &str) -> String {
    format!("{ID_PREFIX}{id}.{rev}@{domain}")
}

/// The draft and the version a copy's Message-ID names, if it names one.
pub fn id_of_copy(message_id: &str) -> Option<(String, String)> {
    let bare = message_id.trim().trim_start_matches('<').trim_end_matches('>');
    let (left, _) = bare.split_once('@')?;
    let (id, rev) = left.strip_prefix(ID_PREFIX)?.split_once('.')?;
    let word = |w: &str| !w.is_empty() && w.chars().all(|c| c.is_ascii_alphanumeric());
    (word(id) && word(rev)).then(|| (id.to_string(), rev.to_string()))
}

/// Whether a Message-ID is named as one of Sioul's copies of a draft.
pub fn is_copy(message_id: &str) -> bool {
    id_of_copy(message_id).is_some()
}

/// The id and the version a copy's [`HEADER`] says.
pub fn read_mark(value: &str) -> Option<(String, String)> {
    let mut id = None;
    let mut rev = None;
    for part in value.split(';') {
        let (name, said) = part.trim().split_once('=')?;
        match name.trim() {
            "id" => id = Some(said.trim().to_string()),
            "rev" => rev = Some(said.trim().to_string()),
            _ => {}
        }
    }
    Some((id.filter(|i| !i.is_empty())?, rev.filter(|r| !r.is_empty())?))
}

/// The value of the field `name` in a message's head, unfolded; none when absent.
pub fn field(raw: &[u8], name: &str) -> Option<String> {
    let end = raw.windows(4).position(|w| w == b"\r\n\r\n").or_else(|| raw.windows(2).position(|w| w == b"\n\n")).unwrap_or(raw.len());
    let text = String::from_utf8_lossy(&raw[..end]).replace("\r\n", "\n");
    let mut value: Option<String> = None;
    for line in text.lines() {
        match &mut value {
            Some(v) if line.starts_with([' ', '\t']) => v.push_str(line),
            Some(_) => break,
            None => {
                if let Some((n, v)) = line.split_once(':')
                    && n.trim().eq_ignore_ascii_case(name)
                {
                    value = Some(v.to_string());
                }
            }
        }
    }
    value.map(|v| v.split_whitespace().collect::<Vec<_>>().join(" "))
}

/// Whether a message is one of Sioul's copies of a draft, by both its marks:
/// its Message-ID names a version of a draft and its [`HEADER`] names the
/// same; then the draft's id and that version. A copy another program saved
/// again without the mark is that program's draft now, and stays mail.
pub fn copy_of(raw: &[u8]) -> Option<(String, String)> {
    let named = id_of_copy(&field(raw, "Message-ID")?)?;
    let marked = read_mark(&field(raw, HEADER)?)?;
    (marked == named).then_some(marked)
}

/// [`copy_of`], and from `own` (the account's own address): what a fetch of
/// the account's Drafts folder (or Gmail's All Mail) leaves out. The marks
/// from anyone else are only words in a message.
pub fn own_copy(raw: &[u8], own: &str) -> Option<(String, String)> {
    let from = field(raw, "From")?;
    let address = crate::compose::address_of(&from).or_else(|| crate::compose::address_of(from.trim()))?;
    (!own.is_empty() && address.eq_ignore_ascii_case(own.trim())).then(|| copy_of(raw)).flatten()
}

/// Whether a draft's copy is to be encrypted: Encrypt ticked, or the message
/// it answers or forwards came encrypted (an answer whose recipients' keys
/// are missing has Encrypt unticked, its words about an encrypted thread).
pub fn wants_encryption(draft: &Draft) -> bool {
    draft.encrypt || draft.original_here().and_then(|path| crate::filelock::read_whole(&path).ok()).is_some_and(|raw| crate::pgp::is_encrypted(&raw))
}

/// A copy written from this device: where the server put it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Placed {
    pub validity: u32,
    pub uid: u32,
}

/// A version not placed yet: refused by the server, or the connection lost
/// on the way; tried again once the wait is over.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Retry {
    pub rev: String,
    /// When it was last tried (Unix seconds).
    pub at: i64,
    pub tries: u32,
    /// The server's words when it refused.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub refused: String,
    /// The refusal was said on the account's card: never again for this version.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub said: bool,
}

impl Retry {
    /// When it may be tried again: [`RETRY_FIRST`], doubled at each try, [`RETRY_AT_MOST`] at most.
    pub fn due(&self) -> i64 {
        let doubled = RETRY_FIRST.saturating_mul(1_i64 << self.tries.saturating_sub(1).min(20));
        self.at.saturating_add(doubled.min(RETRY_AT_MOST))
    }
}

/// What this device placed and owes the server, by draft id; its own, never shared.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mirror {
    #[serde(default)]
    pub format: u32,
    /// Drafts saved here or placed from here.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub drafts: BTreeMap<String, Entry>,
    /// Drafts sent or discarded here (or moved to another account), with the
    /// accounts whose Drafts folder may hold a copy and the copies written
    /// there from here: removed there at their next round.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub gone: BTreeMap<String, Vec<Left>>,
}

/// An account whose Drafts folder may still hold copies of a draft gone.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Left {
    pub account: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub copies: Vec<Placed>,
}

/// One draft this device answers for.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// The account whose Drafts folder holds the copy placed from here; "" none yet.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub account: String,
    /// The version placed from here; "" none yet.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub rev: String,
    /// The copies written from here, where the server put them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub copies: Vec<Placed>,
    /// A version waiting to be tried again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry: Option<Retry>,
}

/// The state's format: 1.
pub const FORMAT: u32 = 1;

/// What one round of an account does.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Work {
    /// Drafts to place there: each draft, the version to place, the copies written from here.
    pub place: Vec<(Draft, String, Vec<Placed>)>,
    /// Drafts whose every copy is to go from there, with the copies written from here.
    pub remove: Vec<(String, Vec<Placed>)>,
}

impl Work {
    pub fn is_empty(&self) -> bool {
        self.place.is_empty() && self.remove.is_empty()
    }
}

/// What a round did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Done {
    /// Each draft placed with the version now there and where its copy is
    /// (none when it is not known); withheld drafts too, at their version,
    /// with no copy.
    pub placed: Vec<(String, String, Option<Placed>)>,
    /// The drafts whose copies are gone from there.
    pub removed: Vec<String>,
    /// Drafts not placed: to be encrypted, and no certificate of yours here.
    pub withheld: Vec<String>,
    /// Versions not placed, to try again: the draft, the version, the
    /// server's words when it refused ("" when the connection was lost).
    pub failed: Vec<(String, String, String)>,
}

impl Mirror {
    pub fn default_path() -> PathBuf {
        crate::config::state_dir().join("drafts-on-server.toml")
    }

    /// As kept; empty when missing or unreadable (nothing then is removed
    /// that this device placed before: its copies stay, never a draft lost).
    pub fn load(path: &Path) -> Mirror {
        crate::files::read_to_string(path).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            crate::files::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        let text = toml::to_string(&Mirror { format: FORMAT, ..self.clone() }).map_err(|e| e.to_string())?;
        crate::filelock::replace(path, text.as_bytes()).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Read, changed by `change`, and written back when it changed, under the file's lock.
    pub fn update<T>(path: &Path, change: impl FnOnce(&mut Mirror) -> T) -> Result<T, String> {
        crate::filelock::with_lock(path, || {
            let mut mirror = Mirror::load(path);
            let before = mirror.clone();
            let out = change(&mut mirror);
            if mirror != before {
                mirror.save(path)?;
            }
            Ok(out)
        })
    }

    /// What the next round of `account` has to do at `now`, with the drafts
    /// here now (`drafts`, each with the version of the very bytes it was read from).
    pub fn work(&self, account: &str, drafts: &[(Draft, String)], now: i64) -> Work {
        let mut work = Work::default();
        for (draft, rev) in drafts.iter().filter(|(d, _)| d.account == account && !d.outbox) {
            let Some(entry) = self.drafts.get(&draft.id) else { continue };
            // Saved here and not placed yet, a version the sharing brought, or moved from another account.
            let wanted = entry.rev != *rev || (!entry.account.is_empty() && entry.account != account);
            // A version that failed waits its turn; a new version goes at once.
            let waiting = entry.retry.as_ref().is_some_and(|r| r.rev == *rev && now < r.due());
            if wanted && !waiting {
                let copies = if entry.account == account { entry.copies.clone() } else { Vec::new() };
                work.place.push((draft.clone(), rev.clone(), copies));
            }
        }
        for (id, entry) in &self.drafts {
            if entry.account != account || entry.rev.is_empty() {
                continue;
            }
            // Taken away by the sharing (sent or discarded elsewhere), or moved to another account.
            let here = drafts.iter().find(|(d, _)| d.id == *id);
            if here.is_none_or(|(d, _)| d.account != account || d.outbox) {
                work.remove.push((id.clone(), entry.copies.clone()));
            }
        }
        for (id, lefts) in &self.gone {
            for left in lefts.iter().filter(|l| l.account == account) {
                if !work.remove.iter().any(|(r, _)| r == id) {
                    work.remove.push((id.clone(), left.copies.clone()));
                }
            }
        }
        work
    }

    /// A round of `account` done at `now`. Returns the refusals to say: each
    /// version's refusal not said yet, with the server's words; with `say`
    /// (a window that shows them on the account's card), they count as said.
    pub fn done(&mut self, account: &str, done: &Done, now: i64, say: bool) -> Vec<String> {
        let mut said = Vec::new();
        for (id, rev, copy) in &done.placed {
            let entry = self.drafts.entry(id.clone()).or_default();
            // Placed before in another account (the draft moved): its copies there go at that account's round.
            if !entry.account.is_empty() && entry.account != account {
                let left = Left { account: entry.account.clone(), copies: std::mem::take(&mut entry.copies) };
                let lefts = self.gone.entry(id.clone()).or_default();
                if !lefts.iter().any(|l| l.account == left.account) {
                    lefts.push(left);
                }
            }
            entry.account = account.to_string();
            entry.rev = rev.clone();
            entry.copies = copy.iter().copied().collect();
            entry.retry = None;
        }
        for (id, rev, refused) in &done.failed {
            let entry = self.drafts.entry(id.clone()).or_default();
            let before = entry.retry.as_ref().filter(|r| r.rev == *rev);
            let (tries, was_said) = before.map_or((0, false), |r| (r.tries, r.said));
            let to_say = !refused.is_empty() && !was_said;
            if to_say {
                said.push(refused.clone());
            }
            entry.retry = Some(Retry { rev: rev.clone(), at: now, tries: tries + 1, refused: refused.clone(), said: was_said || (to_say && say) });
        }
        for id in &done.removed {
            if self.drafts.get(id).is_some_and(|e| e.account == account) {
                // A draft still here whose account changed is placed again in its new one.
                let still_here = Draft::by_id(id).is_some_and(|d| !d.outbox);
                if still_here {
                    if let Some(entry) = self.drafts.get_mut(id) {
                        entry.account.clear();
                        entry.rev.clear();
                        entry.copies.clear();
                    }
                } else {
                    self.drafts.remove(id);
                }
            }
            if let Some(lefts) = self.gone.get_mut(id) {
                lefts.retain(|l| l.account != account);
                if lefts.is_empty() {
                    self.gone.remove(id);
                }
            }
        }
        said
    }
}

/// A draft saved on this device (`Draft::save`): placed on its account's
/// server at its next round. The device's outbox never goes there.
pub fn saved(draft: &Draft) {
    if draft.outbox || draft.account.is_empty() || draft.id.is_empty() {
        return;
    }
    let path = Mirror::default_path();
    // Known already: its round compares versions; nothing to write on each keystroke.
    if Mirror::load(&path).drafts.contains_key(&draft.id) {
        return;
    }
    if let Err(e) = Mirror::update(&path, |m| {
        m.drafts.entry(draft.id.clone()).or_default();
        // Saved again after it was sent or discarded (an Undo): its copies stay where they are.
        m.gone.remove(&draft.id);
    }) {
        eprintln!("{e}");
    }
}

/// A draft sent or discarded on this device (`Draft::discard`): its copies
/// go from its account's Drafts folder, and from the one this device placed
/// it in, at their next round.
pub fn discarded(draft: &Draft) {
    if draft.outbox || draft.id.is_empty() {
        return;
    }
    let path = Mirror::default_path();
    if let Err(e) = Mirror::update(&path, |m| {
        let mut lefts: Vec<Left> = Vec::new();
        let entry = m.drafts.remove(&draft.id).unwrap_or_default();
        if !entry.account.is_empty() {
            lefts.push(Left { account: entry.account.clone(), copies: entry.copies });
        }
        if !draft.account.is_empty() && draft.account != entry.account {
            lefts.push(Left { account: draft.account.clone(), copies: Vec::new() });
        }
        if !lefts.is_empty() {
            m.gone.insert(draft.id.clone(), lefts);
        }
    }) {
        eprintln!("{e}");
    }
}

/// Every draft of this device with the version of the very bytes it was
/// read from: a draft saved meanwhile is read whole next time, never its
/// words under another version's mark.
pub fn drafts_here() -> Vec<(Draft, String)> {
    let Ok(entries) = crate::files::read_dir(Draft::folder()) else { return Vec::new() };
    entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "toml"))
        .filter_map(|path| {
            let bytes = crate::files::read(&path).ok()?;
            let draft: Draft = toml::from_str(std::str::from_utf8(&bytes).ok()?).ok()?;
            Some((draft, rev(&bytes)))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft(id: &str, account: &str) -> Draft {
        Draft { id: id.into(), account: account.into(), ..Draft::default() }
    }

    /// The marks: one Message-ID for every version, the version in the
    /// header; a copy known by both marks, never by one alone.
    #[test]
    fn a_copy_says_which_draft_and_which_version() {
        assert_eq!(rev(b"body = \"Hello\""), rev(b"body = \"Hello\""));
        assert_ne!(rev(b"body = \"Hello\""), rev(b"body = \"Hello!\""));
        let value = mark("19a2b3c4d5e6f708", "0123456789abcdef");
        assert_eq!(read_mark(&value), Some(("19a2b3c4d5e6f708".into(), "0123456789abcdef".into())));
        assert_eq!(read_mark("id=; rev=1"), None);
        // A Message-ID of its own for each version: Gmail keeps one message per Message-ID.
        let id = message_id("19a2b3c4d5e6f708", "0123456789abcdef", "example.org");
        assert_ne!(id, message_id("19a2b3c4d5e6f708", "fedcba9876543210", "example.org"));
        assert_eq!(id_of_copy(&format!("<{id}>")), Some(("19a2b3c4d5e6f708".into(), "0123456789abcdef".into())));
        assert!(!is_copy("<1234@example.org>") && !is_copy("<sioul-draft.@example.org>") && !is_copy("<sioul-draft.abc@example.org>"));
        let copy = format!("Message-ID: <{id}>\r\n{HEADER}: {value}\r\nFrom: Me <me@example.org>\r\nSubject: Lunch\r\n\r\nText.\r\n");
        assert_eq!(copy_of(copy.as_bytes()), Some(("19a2b3c4d5e6f708".into(), "0123456789abcdef".into())));
        // Saved again by another program without the mark, marked for another draft or version: mail.
        assert_eq!(copy_of(format!("Message-ID: <{id}>\r\nSubject: Lunch\r\n\r\nText.\r\n").as_bytes()), None);
        assert_eq!(copy_of(format!("Message-ID: <{id}>\r\n{HEADER}: {}\r\n\r\nText.\r\n", mark("other1", "r")).as_bytes()), None);
        assert_eq!(copy_of(format!("Message-ID: <{id}>\r\n{HEADER}: {}\r\n\r\nText.\r\n", mark("19a2b3c4d5e6f708", "fedcba9876543210")).as_bytes()), None);
        assert_eq!(copy_of(format!("Message-ID: <x@example.org>\r\n{HEADER}: {value}\r\n\r\nText.\r\n").as_bytes()), None);
        // From the account's own address only: anyone else writing both marks sends mail.
        assert!(own_copy(copy.as_bytes(), "ME@example.org").is_some());
        assert_eq!(own_copy(copy.as_bytes(), "other@example.org"), None);
        assert_eq!(own_copy(copy.replace("me@example.org", "stranger@example.net").as_bytes(), "me@example.org"), None);
    }

    /// What a round does: a draft saved here is placed; a version the
    /// sharing brought over the one placed from here is placed again; a
    /// draft known only from another device is left to it; one taken away,
    /// sent or discarded here, or moved to another account, has its copies
    /// removed; the outbox never goes there; a version that failed waits a
    /// growing time, a new version goes at once; a refusal is said once.
    #[test]
    fn a_round_places_what_changed_and_removes_what_went() {
        let mut mirror = Mirror::default();
        let here = vec![(draft("saved", "home"), "r1".to_string()), (draft("other", "home"), "r1".to_string()), (Draft { outbox: true, ..draft("outbox", "home") }, "r1".to_string())];
        mirror.drafts.insert("saved".into(), Entry::default());
        mirror.drafts.insert("outbox".into(), Entry::default());
        let work = mirror.work("home", &here, 0);
        assert_eq!(work.place.iter().map(|(d, r, _)| (d.id.as_str(), r.as_str())).collect::<Vec<_>>(), [("saved", "r1")], "another device's draft is its own");
        assert!(work.remove.is_empty() && mirror.work("work", &here, 0).is_empty());
        let copy = Placed { validity: 7, uid: 3 };
        mirror.done("home", &Done { placed: vec![("saved".into(), "r1".into(), Some(copy))], ..Done::default() }, 0, true);
        assert!(mirror.work("home", &here, 0).is_empty(), "placed: nothing until it changes");
        // The sharing brings another version: placed again, its copy known.
        let changed = vec![(draft("saved", "home"), "r2".to_string())];
        assert_eq!(mirror.work("home", &changed, 0).place[0].2, [copy]);
        // Refused: said once, tried again after 5 minutes, then 10; a new version at once.
        let refused = Done { failed: vec![("saved".into(), "r2".into(), "NO [OVERQUOTA] Quota exceeded".into())], ..Done::default() };
        // Met first by a phone's background step, which shows no card: still to say.
        let mut quiet = mirror.clone();
        assert_eq!(quiet.done("home", &refused, 1000, false), ["NO [OVERQUOTA] Quota exceeded"]);
        assert_eq!(quiet.done("home", &refused, 1300, true).len(), 1, "said by the window next");
        assert_eq!(mirror.done("home", &refused, 1000, true), ["NO [OVERQUOTA] Quota exceeded"]);
        assert!(mirror.work("home", &changed, 1000 + 299).is_empty());
        assert_eq!(mirror.work("home", &changed, 1000 + 300).place.len(), 1);
        assert!(mirror.done("home", &refused, 1300, true).is_empty(), "said once");
        assert!(mirror.work("home", &changed, 1300 + 599).is_empty() && mirror.work("home", &changed, 1300 + 600).place.len() == 1);
        assert_eq!(mirror.work("home", &[(draft("saved", "home"), "r3".to_string())], 1301).place.len(), 1, "a new version at once");
        // Taken away by the sharing: removed, its copy known.
        assert_eq!(mirror.work("home", &[], 0).remove, [("saved".to_string(), vec![copy])]);
        // Moved to another account: removed from the first, placed in the second, in either order.
        let moved = vec![(draft("saved", "work"), "r1".to_string())];
        assert_eq!(mirror.work("home", &moved, 0).remove.len(), 1);
        assert_eq!(mirror.work("work", &moved, 0).place.len(), 1);
        let mut first_work = mirror.clone();
        first_work.done("work", &Done { placed: vec![("saved".into(), "r1".into(), None)], ..Done::default() }, 0, true);
        assert_eq!(first_work.work("home", &moved, 0).remove, [("saved".to_string(), vec![copy])], "the first account's copy still goes");
        // Sent here: removed from its account.
        mirror.gone.insert("sent".into(), vec![Left { account: "home".into(), copies: vec![] }]);
        assert!(mirror.work("home", &here, 0).remove.iter().any(|(id, _)| id == "sent"));
        mirror.done("home", &Done { removed: vec!["sent".into()], ..Done::default() }, 0, true);
        assert!(mirror.gone.is_empty());
    }
}
