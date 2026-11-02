// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The window's one opener for a message (the unified mailbox's §3.3, part
//! 2): `window.openMail(ref)` asks [`find`] off the window's thread
//! (`Sioul::find_message`), by the message's address and the hints of where
//! this device last saw it; the answer comes back by `message_found`.
//!
//! This device first (`mailfind::find`, the folders' indexes, never a whole
//! message); no copy here and a Message-ID: the window is told that the
//! servers are asked, then each address's server is asked, read only, the
//! way a search asks, and the copy that ranks first brought here
//! (`mailfind::ask_servers`). Never from the demo profile (`mailfind` answers
//! "unreachable" there), never for a digest, and never from the phone's
//! background step, which does not reach this module (a window's invokable).
//!
//! An event's invitation is asked for by the event's UID: its message comes
//! from the index (`MailIndex::invitations`), its organizer's first.

use crate::backend::{load_config, tr};
use serde::{Deserialize, Serialize};
use sioul_core::mailfind::{self, Address, Found, MailRef, Servers};
use sioul_core::mailindex::MailIndex;
use std::path::PathBuf;

/// A message as the window asks for it (`window.openMail`'s reference):
/// its address, the only part ever written anywhere, and hints (its file,
/// its account, folder and UID); or an event's UID, for its invitation.
#[derive(Debug, Default, Deserialize)]
pub(crate) struct Asked {
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub account: Option<String>,
    #[serde(default)]
    pub folder: Option<String>,
    #[serde(default)]
    pub uid: Option<u32>,
    #[serde(default)]
    pub file: String,
    /// An event's UID: the message that invited you to it.
    #[serde(default)]
    pub invitation: String,
    /// That event's organizer's address: their invitation first.
    #[serde(default)]
    pub organizer: String,
}

/// What the window is told (`message_found`): {state, file, address, place, reused, said}.
/// `state`: "asking" (no copy here: the servers are being asked), "here"
/// (`file` opens), "not-here" (no copy here and none asked: a message without
/// a Message-ID), "on-its-way" (Sioul moved it without learning where the
/// server put it: `said` names the folder, the copy comes with its next
/// sync, `mailmoved`), "gone" (no server holds it), "unreachable" (a server
/// could not be reached, or the demo profile), "no-invitation" (no message
/// here invited you to that event).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct Answer {
    pub state: &'static str,
    pub file: String,
    pub address: String,
    /// "In Archive · work@example.org": where it was found, when not an inbox.
    pub place: String,
    /// Another message, from another sender, carries the same Message-ID.
    pub reused: bool,
    /// "On its way to Archive · work@example.org. It opens as soon as it is here."
    pub said: String,
}

impl Answer {
    /// A state alone, no file nor words.
    pub(crate) fn only(state: &'static str) -> Answer {
        Answer { state, file: String::new(), address: String::new(), place: String::new(), reused: false, said: String::new() }
    }
}

/// The message the window asks for, found: here, else on the servers
/// (through `servers`, after `asking` is told), as the module says.
pub(crate) fn find(asked: &Asked, servers: &dyn Servers, asking: &dyn Fn()) -> Answer {
    find_in(&load_config(), asked, servers, asking)
}

fn find_in(config: &sioul_core::config::Config, asked: &Asked, servers: &dyn Servers, asking: &dyn Fn()) -> Answer {
    let roots = MailIndex::roots_of(config);
    let wanted = if asked.invitation.is_empty() {
        MailRef {
            address: asked.address.trim().to_string(),
            account: asked.account.clone().filter(|a| !a.is_empty()),
            folder: asked.folder.clone().filter(|f| !f.is_empty()),
            uid: asked.uid,
            file: (!asked.file.is_empty()).then(|| PathBuf::from(&asked.file)),
        }
    } else {
        match invitation(config, &roots, &asked.invitation, &asked.organizer, &asked.file) {
            Some(wanted) => wanted,
            None => return Answer::only("no-invitation"),
        }
    };
    let found = match mailfind::find(&roots, &wanted) {
        Found::NotHere if matches!(Address::parse(&wanted.address), Some(Address::Id(_))) => {
            asking();
            mailfind::ask_servers(&roots, &config.accounts, &wanted, servers)
        }
        found => found,
    };
    match found {
        Found::Here(here) => {
            let place = if here.role == sioul_core::folders::Role::Inbox { String::new() } else { place(config, &here) };
            Answer { state: "here", file: here.path.display().to_string(), address: here.address, place, reused: here.reused, said: String::new() }
        }
        Found::OnItsWay { account, folder } => {
            let (folder, address) = named(config, &account, &folder, None);
            Answer { said: crate::backend::say("reader-on-its-way", &[("folder", folder), ("address", address)]), ..Answer::only("on-its-way") }
        }
        Found::Gone => Answer::only("gone"),
        Found::Unreachable => Answer::only("unreachable"),
        Found::NotHere | Found::Kept => Answer::only("not-here"),
    }
}

/// The message that invited you to the event of `uid`, as the index knows
/// it: the one the card named (`hint`, `pim::invitation_of`'s file), else its
/// organizer's, else the copy that ranks first; never a file the index does
/// not give for that UID.
fn invitation(config: &sioul_core::config::Config, roots: &[sioul_core::mailindex::Root], uid: &str, organizer: &str, hint: &str) -> Option<MailRef> {
    let index = MailIndex::build_ranked(roots);
    let found = index.invitations(uid);
    let organizer = organizer.trim().trim_start_matches("mailto:").to_ascii_lowercase();
    let named = (!hint.is_empty()).then(|| found.iter().find(|m| m.path == std::path::Path::new(hint))).flatten();
    let mail = named.or_else(|| found.iter().find(|m| !organizer.is_empty() && m.address == organizer)).or_else(|| found.first())?;
    let account = config.account_of(&mail.path)?.id.clone();
    let address = mailfind::address_of_file(&account, &mail.path)?;
    Some(MailRef { address, account: Some(account), file: Some(mail.path.clone()), ..MailRef::default() })
}

/// "In Archive · work@example.org": the folder as you read its name, and the address.
fn place(config: &sioul_core::config::Config, here: &mailfind::Here) -> String {
    let (folder, address) = named(config, &here.account, &here.folder, Some(here.role));
    crate::backend::say("mail-found-in", &[("folder", folder), ("address", address)])
}

/// A folder of `account` (its server name) as you read it, its purpose's
/// word or its own name (`role`: its purpose when known, else the saved
/// list's), and the account's address.
fn named(config: &sioul_core::config::Config, account: &str, folder: &str, role: Option<sioul_core::folders::Role>) -> (String, String) {
    use sioul_core::folders::Role;
    let saved = sioul_core::folders::saved(account).into_iter().find(|f| f.name == folder);
    let words = match role.or(saved.as_ref().map(|f| f.role)) {
        Some(Role::Other) | None => saved.map_or_else(|| folder.to_string(), |f| f.display),
        Some(role) => tr().text(role.message_id(), None),
    };
    let address = config.accounts.iter().find(|a| a.id == account).and_then(|a| a.address.clone()).unwrap_or_else(|| account.to_string());
    (words, address)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sioul_core::mailfind::ServerCopy;
    use std::cell::RefCell;

    /// Stand-in servers: what each holds, what was asked, whether one is down.
    #[derive(Default)]
    struct Stand {
        held: Vec<(String, ServerCopy, PathBuf)>,
        down: bool,
        asked: RefCell<Vec<String>>,
    }

    impl Servers for Stand {
        fn copies(&self, account: &sioul_core::config::Account, id: &str) -> Result<Vec<ServerCopy>, String> {
            self.asked.borrow_mut().push(format!("{}:{id}", account.id));
            if self.down {
                return Err("down".into());
            }
            Ok(self.held.iter().filter(|(a, c, _)| *a == account.id && c.message_id.trim_matches(['<', '>']) == id).map(|(_, c, _)| c.clone()).collect())
        }

        fn bring(&self, account: &sioul_core::config::Account, copy: &ServerCopy) -> Result<PathBuf, String> {
            let (_, _, text) = self.held.iter().find(|(a, c, _)| *a == account.id && c == copy).ok_or("none")?;
            let folder = account.maildir_path().join(".Archive").join("cur");
            std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
            let path = folder.join(format!("1777000100.U{}-{}.sioul:2,", copy.origin.validity, copy.origin.uid));
            std::fs::copy(text, &path).map_err(|e| e.to_string())?;
            Ok(path)
        }
    }

    fn message(id: Option<&str>, from: &str, subject: &str) -> String {
        let id = id.map(|id| format!("Message-ID: <{id}>\r\n")).unwrap_or_default();
        format!("{id}From: {from}\r\nTo: me@example.org\r\nDate: Mon, 2 Nov 2026 09:00:00 +0100\r\nSubject: {subject}\r\n\r\nHello.\r\n")
    }

    /// A stand-in address with an inbox and an archive, under the tests' own
    /// data folder, its folders and numbering saved as a sync saves them; a
    /// configuration holding it alone.
    fn stand_in(name: &str) -> (sioul_core::config::Config, sioul_core::config::Account, PathBuf) {
        use sioul_core::folders::{Role, folder};
        sioul_core::config::stand_ins_only();
        let id = format!("open-{name}-{}", std::process::id());
        let root = sioul_core::config::data_dir().join(format!("openmail-{id}"));
        let _ = std::fs::remove_dir_all(&root);
        let folders = [folder("INBOX", Some("/"), Some(Role::Inbox)), folder("Archive", Some("/"), Some(Role::Archive))];
        for f in &folders {
            for sub in ["tmp", "new", "cur"] {
                std::fs::create_dir_all(root.join(&f.local).join(sub)).unwrap();
            }
        }
        let sync = sioul_core::config::state_dir().join("sync");
        std::fs::create_dir_all(&sync).unwrap();
        std::fs::write(sync.join(format!("{id}.toml")), folders.iter().map(|f| format!("[folders.\"{}\"]\nuidvalidity = 7\nlast_uid = 99\n", f.name)).collect::<String>()).unwrap();
        let saved: String = folders.iter().map(|f| format!("[[folder]]\nname = \"{}\"\ndisplay = \"{}\"\nrole = \"{}\"\nlocal = \"{}\"\n", f.name, f.display, serde_json::to_value(f.role).unwrap().as_str().unwrap(), f.local)).collect();
        std::fs::write(sioul_core::folders::saved_path(&id), saved).unwrap();
        let mut account = sioul_core::config::Account::imap(&id, &format!("{id}@example.org"), "imap.example.org", 993, sioul_core::config::Security::Tls, None);
        account.maildir = Some(root.display().to_string());
        let mut config = sioul_core::config::Config::default();
        config.accounts.push(account.clone());
        (config, account, root)
    }

    /// The opener's answers (the unified mailbox's part 2): a copy here opens,
    /// its folder said when it is not the inbox, no server asked; no copy
    /// here, the window told the servers are asked, one asked and its copy
    /// brought and opened; none holds it: "gone"; a server down:
    /// "unreachable"; a message without a Message-ID: never asked.
    #[test]
    fn the_opener_finds_here_then_asks_the_servers() {
        let (config, account, root) = stand_in("answers");
        let archived = root.join(".Archive/cur/1777000000.U7-3.sioul:2,S");
        std::fs::write(&archived, message(Some("archived@example.org"), "Ana <ana@example.org>", "The plans")).unwrap();
        let server_text = root.join("server-copy.eml");
        std::fs::write(&server_text, message(Some("away@example.org"), "Ben <ben@example.org>", "Kept on the server")).unwrap();
        let away = ServerCopy { folder: sioul_core::folders::folder("Archive", Some("/"), Some(sioul_core::folders::Role::Archive)), origin: sioul_core::card::ImapOrigin { validity: 7, uid: 9 }, message_id: "<away@example.org>".into(), sender: "ben@example.org".into(), arrived: 1_777_000_000, date: None, subject: "Kept on the server".into() };
        let stand = Stand { held: vec![(account.id.clone(), away, server_text)], ..Stand::default() };
        let told = RefCell::new(0);
        let asking = || *told.borrow_mut() += 1;
        let ask = |address: &str| Asked { address: address.into(), ..Asked::default() };
        let here = find_in(&config, &ask("mid:archived@example.org"), &stand, &asking);
        assert_eq!((here.state, here.file.as_str()), ("here", archived.to_str().unwrap()));
        assert!(here.place.contains(account.address.as_deref().unwrap()), "where: {}", here.place);
        assert!(stand.asked.borrow().is_empty() && *told.borrow() == 0, "no server asked for a copy here");
        let brought = find_in(&config, &ask("mid:away@example.org"), &stand, &asking);
        assert_eq!(brought.state, "here", "{brought:?}");
        assert!(brought.file.contains(".Archive") && std::path::Path::new(&brought.file).is_file(), "brought here: {brought:?}");
        assert_eq!(*told.borrow(), 1, "the window told the servers are asked");
        assert_eq!(find_in(&config, &ask("mid:nowhere@example.org"), &stand, &asking).state, "gone");
        let down = Stand { down: true, ..Stand::default() };
        assert_eq!(find_in(&config, &ask("mid:nowhere@example.org"), &down, &asking).state, "unreachable");
        let asked = stand.asked.borrow().len();
        assert_eq!(find_in(&config, &ask(&format!("sioul:mail/{}/0123456789abcdef", account.id)), &stand, &asking).state, "not-here", "a digest: no header names it");
        assert_eq!(stand.asked.borrow().len(), asked, "no server asked for a digest");
        // A hinted file: opened as it is, no server asked.
        let hinted = Asked { address: "mid:archived@example.org".into(), file: archived.display().to_string(), ..Asked::default() };
        assert_eq!(find_in(&config, &hinted, &stand, &asking).file, archived.display().to_string());
        // Moved by Sioul without the server saying where (the unified mailbox's
        // part 3): on its way to its folder, named as you read it, never asked;
        // its copy here, it opens.
        let moving = format!("mid:moving-{}@example.org", std::process::id());
        sioul_core::mailmoved::note(&account.id, std::slice::from_ref(&moving), "Archive", jiff::Timestamp::now().as_second()).unwrap();
        let asked = stand.asked.borrow().len();
        let on_its_way = find_in(&config, &ask(&moving), &stand, &asking);
        assert_eq!(on_its_way.state, "on-its-way", "{on_its_way:?}");
        assert!(on_its_way.said.contains(&tr().text("folder-archive", None)) && on_its_way.said.contains(account.address.as_deref().unwrap()), "{}", on_its_way.said);
        assert_eq!(stand.asked.borrow().len(), asked, "no server asked while it is on its way");
        let arrived = root.join(".Archive/cur/1777000000.U7-5.sioul:2,S");
        std::fs::write(&arrived, message(Some(moving.trim_start_matches("mid:")), "Ana <ana@example.org>", "Moved")).unwrap();
        assert_eq!(find_in(&config, &ask(&moving), &stand, &asking).file, arrived.display().to_string());
        let _ = sioul_core::mailmoved::forget(&[moving]);
        let _ = std::fs::remove_dir_all(&root);
        if let Some(dir) = sioul_core::mailcache::account_dir(&account.id) {
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    /// The event card's Its invitation: the message that invited you, found by
    /// the event's UID, its organizer's before a forwarded copy; none: said.
    #[test]
    fn an_event_s_invitation_opens_by_its_uid() {
        let (config, account, root) = stand_in("invitation");
        let invite = |id: &str, from: &str| format!("Message-ID: <{id}>\r\nFrom: {from}\r\nTo: me@example.org\r\nDate: Mon, 2 Nov 2026 09:00:00 +0100\r\nSubject: Invitation: Review\r\nContent-Type: text/calendar; method=REQUEST; charset=utf-8\r\n\r\nBEGIN:VCALENDAR\r\nMETHOD:REQUEST\r\nBEGIN:VEVENT\r\nUID:open-event-1\r\nDTSTART:20261110T090000Z\r\nSUMMARY:Review\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n");
        let forwarded = root.join("cur/1777000001.U7-1.sioul:2,S");
        let organizer = root.join("cur/1777000002.U7-2.sioul:2,S");
        std::fs::write(&forwarded, invite("forwarded@example.org", "Cat <cat@example.org>")).unwrap();
        std::fs::write(&organizer, invite("organizer@example.org", "Ana <ana@example.org>")).unwrap();
        let none = Stand::default();
        let asking = || {};
        let theirs = find_in(&config, &Asked { invitation: "open-event-1".into(), organizer: "Ana@example.org".into(), ..Asked::default() }, &none, &asking);
        assert_eq!((theirs.state, theirs.file.as_str(), theirs.address.as_str()), ("here", organizer.to_str().unwrap(), "mid:organizer@example.org"));
        let any = find_in(&config, &Asked { invitation: "open-event-1".into(), ..Asked::default() }, &none, &asking);
        assert_eq!(any.state, "here");
        // The one the card named (its file), when the index gives it for that UID; a file it does not give, never.
        let named = find_in(&config, &Asked { invitation: "open-event-1".into(), organizer: "ana@example.org".into(), file: forwarded.display().to_string(), ..Asked::default() }, &none, &asking);
        assert_eq!(named.file, forwarded.display().to_string());
        let elsewhere = find_in(&config, &Asked { invitation: "open-event-1".into(), organizer: "ana@example.org".into(), file: "/etc/passwd".into(), ..Asked::default() }, &none, &asking);
        assert_eq!(elsewhere.file, organizer.display().to_string(), "the organizer's");
        assert_eq!(find_in(&config, &Asked { invitation: "open-event-2".into(), ..Asked::default() }, &none, &asking).state, "no-invitation");
        assert!(none.asked.borrow().is_empty());
        let _ = std::fs::remove_dir_all(&root);
        if let Some(dir) = sioul_core::mailcache::account_dir(&account.id) {
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    /// The opener leaves the dose's path as it was (the brief's rule): in
    /// `openThing`, the first line's kinds, and the dose, need and stopped
    /// branches answering a notification, byte for byte as at 180bd26; the
    /// opener's own branches beside them.
    #[test]
    fn the_dose_and_need_branches_stay_as_they_were() {
        let main = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("qml/main.qml")).unwrap();
        let kept = [
            "        if ([\"task\", \"event\", \"dose\", \"need\", \"stopped\"].indexOf(item.kind) < 0)\n            cardHost.leave()\n",
            "        else if (item.kind === \"dose\")\n            window.askDose(item.key)\n        else if (item.kind === \"need\")\n            window.askNeed(item.key)\n        else if (item.kind === \"stopped\")\n            window.askNeed(\"\")\n",
        ];
        for text in kept {
            assert_eq!(main.matches(text).count(), 1, "kept as it was: {text}");
        }
        assert!(main.contains("window.openMail(mailOpener.refOf(item))"), "the mail branch through the opener");
    }
}
