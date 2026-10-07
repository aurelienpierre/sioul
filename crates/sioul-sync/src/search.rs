// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Searching the mail a server holds and this device does not
//! (docs/client.md, "Searching").
//!
//! Sioul keeps recent mail here and brings older mail a round at a time, while
//! the disk has room; folders you left on the server are not kept at all. A
//! search looks here first (`sioul_core::mailsearch`), then asks the server
//! the same for those folders and days: one connection per account, each
//! folder examined (read-only), `UID SEARCH` with the search's keys (RFC 9051
//! §6.4.4; Gmail's own `X-GM-RAW` for attachments, where the server offers
//! it), what is kept here already left out by UID, then the headers, size,
//! flags and parts of the newest (`BODY.PEEK[HEADER]`, `BODYSTRUCTURE`): the
//! list shows them, and nothing is marked read. A message opened is brought
//! here whole ([`bring`]), into its folder, as a sync would have.
//!
//! Strings that are not ASCII go as literals (RFC 9051 §4.3), with `CHARSET
//! UTF-8`: non-synchronizing (`{n+}`, RFC 7888) where the server takes them,
//! else waiting for the server's go-ahead before each.

use crate::SyncError;
use crate::imap::{self, COMMAND, Imap, Server};
use crate::mailbox;
use async_imap::imap_proto::{BodyStructure, MailboxDatum, Response, Status};
use futures_util::StreamExt;
use sioul_core::card::ImapOrigin;
use sioul_core::config::Account;
use sioul_core::folders::Folder;
use sioul_core::maildir;
use sioul_core::mailsearch::{Found, Search, ServerRef, Token, imap_key, imap_key_without_words};
use sioul_core::porch::Senders;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

/// Messages fetched per FETCH of headers.
const BATCH: usize = 100;

/// What an account's server found.
#[derive(Debug, Default)]
pub struct Outcome {
    /// The newest found, `limit` at most, newest first.
    pub found: Vec<Found>,
    /// How many the server found that are not kept here, beyond those shown.
    pub more: usize,
}

/// Whether a string can go quoted (RFC 9051 §4.3: 7-bit, no line break).
fn quotable(text: &str) -> bool {
    text.bytes().all(|b| (0x20..0x7f).contains(&b))
}

fn quoted(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

/// A SEARCH command's pieces: the first sent tagged; each next one after the
/// server's go-ahead for the literal ending the one before (none with LITERAL+).
fn command(tokens: &[Token], literal_plus: bool) -> Vec<String> {
    let utf8 = tokens.iter().any(|t| matches!(t, Token::Text(s) if !quotable(s)));
    let mut pieces = vec![String::from(if utf8 { "UID SEARCH CHARSET UTF-8" } else { "UID SEARCH" })];
    for token in tokens {
        let current = pieces.last_mut().expect("one piece at least");
        let word = match token {
            Token::Atom(atom) => atom.clone(),
            Token::Text(text) if quotable(text) => quoted(text),
            Token::Text(text) => {
                // Line breaks have no place in a search; the rest goes byte for byte.
                let text = text.replace(['\r', '\n'], " ");
                if !(current.ends_with('(') || current.is_empty()) {
                    current.push(' ');
                }
                if literal_plus {
                    current.push_str(&format!("{{{}+}}\r\n{text}", text.len()));
                } else {
                    current.push_str(&format!("{{{}}}", text.len()));
                    pieces.push(text);
                }
                continue;
            }
        };
        if !(current.ends_with('(') || word == ")" || current.ends_with('\n')) {
            current.push(' ');
        }
        current.push_str(&word);
    }
    pieces
}

/// `UID SEARCH` with literals where needed; the UIDs found.
async fn uid_search(session: &mut Imap, tokens: &[Token], literal_plus: bool) -> Result<BTreeSet<u32>, SyncError> {
    let pieces = command(tokens, literal_plus);
    let tag = imap::within(COMMAND, session.run_command(&pieces[0])).await?.map_err(imap::server)?;
    let mut next = 1;
    let mut found = BTreeSet::new();
    loop {
        let response = imap::within(COMMAND, session.read_response()).await?.map_err(|e| SyncError::Network(e.to_string()))?;
        let Some(response) = response else { return Err(SyncError::Network("the server closed the connection".into())) };
        match response.parsed() {
            Response::Continue(_) if next < pieces.len() => {
                imap::within(COMMAND, session.run_command_untagged(&pieces[next])).await?.map_err(imap::server)?;
                next += 1;
            }
            Response::MailboxData(MailboxDatum::Search(uids)) => found.extend(uids.iter().copied()),
            Response::Done { tag: done, status, outcome } if *done == tag => {
                return match status {
                    Status::Ok => Ok(found),
                    _ => Err(SyncError::Server(outcome.information.as_deref().unwrap_or("SEARCH refused").trim().to_string())),
                };
            }
            // What the server says meanwhile (EXISTS, EXPUNGE…) changes nothing here.
            _ => {}
        }
    }
}

/// An attachment's name from its part's parameters: `filename` (RFC 2183),
/// else `name`, encoded words decoded.
fn part_name(disposition: Option<&[(std::borrow::Cow<'_, str>, std::borrow::Cow<'_, str>)]>, params: Option<&[(std::borrow::Cow<'_, str>, std::borrow::Cow<'_, str>)]>) -> Option<String> {
    let find = |list: Option<&[(std::borrow::Cow<'_, str>, std::borrow::Cow<'_, str>)]>, key: &str| list?.iter().find(|(k, _)| k.eq_ignore_ascii_case(key) || k.to_ascii_lowercase().starts_with(&format!("{key}*"))).map(|(_, v)| v.to_string());
    let raw = find(disposition, "filename").or_else(|| find(params, "name"))?;
    // "=?UTF-8?Q?relev=C3=A9.pdf?=" as mail_parser reads a header.
    let decoded = sioul_core::card::Card::from_bytes(format!("Subject: {raw}\r\n\r\n").as_bytes()).map(|c| c.subject).filter(|s| !s.is_empty());
    Some(decoded.unwrap_or(raw))
}

/// A message's attachments as its BODYSTRUCTURE describes them, their names
/// and types: the parts that have a name, OpenPGP's own left out (as `Card`
/// counts them).
fn parts_of(structure: &BodyStructure<'_>, out: &mut Vec<(String, String)>) {
    let (common, children): (&async_imap::imap_proto::BodyContentCommon<'_>, &[BodyStructure<'_>]) = match structure {
        BodyStructure::Multipart { common, bodies, .. } => (common, bodies.as_slice()),
        BodyStructure::Basic { common, .. } | BodyStructure::Text { common, .. } | BodyStructure::Message { common, .. } => (common, &[]),
    };
    for child in children {
        parts_of(child, out);
    }
    if !children.is_empty() {
        return;
    }
    let mime = format!("{}/{}", common.ty.ty, common.ty.subtype).to_ascii_lowercase();
    if matches!(mime.as_str(), "application/pgp-signature" | "application/pgp-encrypted" | "application/pgp-keys") {
        return;
    }
    let disposition = common.disposition.as_ref().and_then(|d| d.params.as_deref());
    let is_attachment = common.disposition.as_ref().is_some_and(|d| d.ty.eq_ignore_ascii_case("attachment"));
    if let Some(name) = part_name(disposition, common.ty.params.as_deref()) {
        if mime == "application/octet-stream" && name.eq_ignore_ascii_case("encrypted.asc") {
            return;
        }
        out.push((name, mime));
    } else if is_attachment {
        out.push((String::new(), mime));
    }
}

/// Searches the server of `account` for `search`, in `folders` (their server
/// names as listed), leaving out what is kept here (`here`: per folder, the
/// UIDVALIDITY and the UIDs of its copies). At most `limit` come back, the
/// newest first. `senders` judge who a sender is to you, when a condition asks it.
pub fn search(account: &Account, password: &str, search: &Search, folders: &[Folder], here: &BTreeMap<String, BTreeSet<(u32, u32)>>, limit: usize, senders: Option<&Senders>) -> Result<Outcome, SyncError> {
    let server = Server::of(account)?;
    crate::fetch::block_on(async {
        let mut session = imap::open(&server, password).await?;
        let result = search_in(&mut session, account, search, folders, here, limit, senders).await;
        let _ = session.logout().await;
        result
    })
}

#[allow(clippy::too_many_arguments)]
async fn search_in(session: &mut Imap, account: &Account, search: &Search, folders: &[Folder], here: &BTreeMap<String, BTreeSet<(u32, u32)>>, limit: usize, senders: Option<&Senders>) -> Result<Outcome, SyncError> {
    let capabilities = imap::within(COMMAND, session.capabilities()).await?.map_err(imap::server)?;
    let gmail = capabilities.has_str("X-GM-EXT-1");
    let literal_plus = capabilities.has_str("LITERAL+");
    let mut outcome = Outcome::default();
    for folder in folders {
        let Some(key) = imap_key(search, &account.id, folder, gmail) else { continue };
        let examined = imap::within(COMMAND, session.examine(&folder.name)).await?.map_err(imap::server)?;
        let validity = examined.uid_validity.unwrap_or(0);
        if examined.exists == 0 {
            continue;
        }
        let kept: BTreeSet<u32> = here.get(&folder.name).map(|s| s.iter().filter(|(v, _)| *v == validity).map(|(_, u)| *u).collect()).unwrap_or_default();
        let found = match uid_search(session, &key.tokens(), literal_plus).await {
            Ok(found) => found,
            // A server that refuses some of the words (GreenMail reads FROM as
            // a whole address and refuses a part of one): the folder asked
            // again without them, the newest checked here.
            Err(SyncError::Server(_)) => match imap_key_without_words(search, &account.id, folder) {
                Some(plain) => uid_search(session, &plain.tokens(), literal_plus).await?,
                None => continue,
            },
            Err(e) => return Err(e),
        };
        // The newest first (the highest UIDs), those kept here left out.
        let mut uids: Vec<u32> = found.into_iter().filter(|uid| !kept.contains(uid)).collect();
        uids.sort_unstable_by(|a, b| b.cmp(a));
        let beyond = uids.len().saturating_sub(limit);
        uids.truncate(limit);
        outcome.more += beyond;
        for batch in uids.chunks(BATCH) {
            let set = mailbox::ranges(&batch.iter().copied().collect::<BTreeSet<u32>>());
            let mut fetches = imap::within(COMMAND, session.uid_fetch(&set, "(UID FLAGS INTERNALDATE RFC822.SIZE BODY.PEEK[HEADER] BODYSTRUCTURE)")).await?.map_err(imap::server)?;
            while let Some(fetch) = fetches.next().await {
                let fetch = fetch.map_err(imap::server)?;
                let (Some(uid), Some(header)) = (fetch.uid, fetch.header()) else { continue };
                let parts = fetch.bodystructure().map(|structure| {
                    let mut parts = Vec::new();
                    parts_of(structure, &mut parts);
                    parts
                });
                let flags = crate::fetch::maildir_flags(fetch.flags());
                let received = fetch.internal_date().map(|d| d.timestamp());
                let Some(found) = Found::new(&account.id, folder, ImapOrigin { validity, uid }, flags, u64::from(fetch.size.unwrap_or(0)), received, header, parts) else { continue };
                // What the server cannot say exactly (a name whole, an attachment's
                // name or kind, who the sender is to you) is checked here; its text, the server matched.
                if found.matches(search, senders) {
                    outcome.found.push(found);
                }
            }
        }
    }
    // The newest first, across folders; what goes past the limit is counted.
    outcome.found.sort_by_key(|f| std::cmp::Reverse(f.date().unwrap_or(0)));
    if outcome.found.len() > limit {
        outcome.more += outcome.found.len() - limit;
        outcome.found.truncate(limit);
    }
    Ok(outcome)
}

/// Brings a message seen on its server only into its folder here, whole and
/// checked, as a sync would have (unread stays unread: BODY.PEEK). Its file.
pub fn bring(account: &Account, password: &str, place: &ServerRef) -> Result<PathBuf, SyncError> {
    let folder = mailbox::folders(&account.id).into_iter().find(|f| f.name == place.folder).ok_or_else(|| SyncError::Server(format!("{}: no such folder", place.folder)))?;
    let root = kept_root(account, &folder).ok_or_else(|| SyncError::Server(format!("{}: not a folder Sioul can keep", place.folder)))?;
    // Here already (a sync came first): that copy.
    if let Some(path) = here(&root, place.origin) {
        return Ok(path);
    }
    let server = Server::of(account)?;
    let written = crate::fetch::block_on(async {
        let mut session = imap::open(&server, password).await?;
        let result = bring_in(&mut session, &folder, place.origin, &root).await;
        let _ = session.logout().await;
        result
    })?;
    written.ok_or_else(|| SyncError::Server("this message is no longer on the server".into()))
}

async fn bring_in(session: &mut Imap, folder: &Folder, origin: ImapOrigin, root: &std::path::Path) -> Result<Option<PathBuf>, SyncError> {
    let examined = imap::within(COMMAND, session.examine(&folder.name)).await?.map_err(imap::server)?;
    if examined.uid_validity.unwrap_or(0) != origin.validity {
        return Err(SyncError::Server("the server renumbered this folder; fetch the mail again".into()));
    }
    let arrived = imap::within(COMMAND, crate::fetch::fetch_batch(session, &origin.uid.to_string())).await??;
    let verifier = crate::verify::Verifier::new();
    let written = crate::fetch::store_checked(arrived, origin.validity, root, verifier.as_ref()).await?;
    Ok(written.into_iter().find(|(uid, _)| *uid == origin.uid).map(|(_, path)| path))
}

/// The copy here of a message of a folder, if there is one.
fn here(root: &std::path::Path, origin: ImapOrigin) -> Option<PathBuf> {
    ["cur", "new"].iter().filter_map(|sub| std::fs::read_dir(root.join(sub)).ok()).flatten().filter_map(Result::ok).map(|e| e.path()).find(|p| maildir::origin_of(p) == Some(origin))
}

/// Where a folder's copies are kept here, as a sync keeps them: the inbox in
/// the account's Maildir itself, any other folder under one name of its own
/// (`mailbox::own_dir`, which never names the Maildir nor what is above it).
fn kept_root(account: &Account, folder: &Folder) -> Option<PathBuf> {
    if folder.role == sioul_core::folders::Role::Inbox && folder.local.is_empty() {
        return Some(account.maildir_path());
    }
    mailbox::own_dir(account, folder)
}

/// What is kept here of `folder`: its copies' UIDVALIDITY and UID.
pub fn kept_uids(account: &Account, folder: &Folder) -> BTreeSet<(u32, u32)> {
    let Some(root) = kept_root(account, folder) else { return BTreeSet::new() };
    ["cur", "new"]
        .iter()
        .filter_map(|sub| std::fs::read_dir(root.join(sub)).ok())
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|e| maildir::origin_of(&e.path()))
        .map(|o| (o.validity, o.uid))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_go_quoted_or_as_literals() {
        let tokens = vec![Token::Atom("FROM".into()), Token::Text("bank.example".into()), Token::Atom("SUBJECT".into()), Token::Text("relevé \"juin\"".into()), Token::Atom("UNSEEN".into())];
        // Waiting for the go-ahead: the command cut after the literal's size.
        assert_eq!(command(&tokens, false), vec!["UID SEARCH CHARSET UTF-8 FROM \"bank.example\" SUBJECT {14}".to_string(), "relevé \"juin\" UNSEEN".to_string()]);
        // LITERAL+: one piece.
        assert_eq!(command(&tokens, true), vec!["UID SEARCH CHARSET UTF-8 FROM \"bank.example\" SUBJECT {14+}\r\nrelevé \"juin\" UNSEEN".to_string()]);
        // ASCII only: no CHARSET, quotes escaped, groups without inner spaces.
        let ascii = vec![Token::Atom("OR".into()), Token::Atom("(".into()), Token::Atom("NOT".into()), Token::Atom("TO".into()), Token::Text("a\"b".into()), Token::Atom(")".into()), Token::Atom("ALL".into())];
        assert_eq!(command(&ascii, false), vec![r#"UID SEARCH OR (NOT TO "a\"b") ALL"#.to_string()]);
    }

    /// Against a GreenMail server (docs/building.md), with invented mail:
    /// `SIOUL_TEST_IMAP=localhost:3993 SIOUL_TEST_INSECURE_TLS=1 cargo test
    /// -p sioul-sync --features insecure-test-tls -- --ignored greenmail`.
    /// Puts three messages in a fresh mailbox of its own, searches, brings
    /// one here, then moves two at once into another folder.
    #[cfg(feature = "insecure-test-tls")]
    #[test]
    #[ignore]
    fn greenmail_search_bring_and_move() {
        greenmail::run();
    }

    #[cfg(feature = "insecure-test-tls")]
    mod greenmail {
        use super::*;
        use sioul_core::config::Security;
        use sioul_core::rules::{Condition, Field, Join, Test};

        fn message(from: &str, subject: &str, body: &str, attachment: bool) -> Vec<u8> {
            // A subject that is not ASCII goes encoded (RFC 2047), as mail
            // programs write it: GreenMail gives raw 8-bit headers back as Latin-1.
            let subject_line = if subject.is_ascii() {
                subject.to_string()
            } else {
                let q: String = subject
                    .bytes()
                    .map(|b| match b {
                        b' ' => "_".to_string(),
                        b if b.is_ascii_alphanumeric() => char::from(b).to_string(),
                        b => format!("={b:02X}"),
                    })
                    .collect();
                format!("=?UTF-8?Q?{q}?=")
            };
            let id: String = subject.chars().filter(char::is_ascii_alphanumeric).collect();
            let head = format!("From: {from}\r\nTo: tester@example.org\r\nSubject: {subject_line}\r\nDate: Wed, 03 Jun 2026 10:00:00 +0200\r\nMessage-ID: <{id}@example.org>\r\nMIME-Version: 1.0\r\n");
            if attachment {
                format!("{head}Content-Type: multipart/mixed; boundary=\"b\"\r\n\r\n--b\r\nContent-Type: text/plain; charset=UTF-8\r\nContent-Transfer-Encoding: 8bit\r\n\r\n{body}\r\n--b\r\nContent-Type: application/pdf; name=\"releve-juin.pdf\"\r\nContent-Disposition: attachment; filename=\"releve-juin.pdf\"\r\nContent-Transfer-Encoding: base64\r\n\r\nJVBERi0xLjQK\r\n--b--\r\n").into_bytes()
            } else {
                format!("{head}Content-Type: text/plain; charset=UTF-8\r\nContent-Transfer-Encoding: 8bit\r\n\r\n{body}\r\n").into_bytes()
            }
        }

        pub(super) fn run() {
            let address = std::env::var("SIOUL_TEST_IMAP").unwrap_or_else(|_| "localhost:3993".into());
            let (host, port) = address.rsplit_once(':').unwrap();
            let login = format!("search-{}-{}@example.org", std::process::id(), jiff::Timestamp::now().as_second());
            let root = std::env::temp_dir().join(format!("sioul-search-greenmail-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            // SAFETY: a test of its own, before any thread of it starts.
            unsafe {
                std::env::set_var("XDG_STATE_HOME", root.join("state"));
                std::env::set_var("XDG_DATA_HOME", root.join("data"));
                std::env::set_var("SIOUL_TEST_INSECURE_TLS", "1");
            }
            let mut account = Account::imap("tester", &login, host, port.parse().unwrap(), Security::Tls, None);
            account.maildir = Some(root.join("mail").display().to_string());
            let server = Server::of(&account).unwrap();
            let inbox = sioul_core::folders::folder("INBOX", None, None);
            // Three messages on the server only, and a folder to sort into.
            crate::fetch::block_on(async {
                let mut session = imap::open(&server, "x").await?;
                for raw in [message("Banque <no-reply@banque.example>", "Relevé de juin", "Votre relevé, réunion le 12.", true), message("Banque <no-reply@banque.example>", "Rappel", "Rien de joint.", false), message("Paul <paul@example.org>", "Lunch", "Midi ?", false)] {
                    imap::within(COMMAND, session.append("INBOX", None, None, &raw)).await?.map_err(imap::server)?;
                }
                let _ = imap::within(COMMAND, session.create("Banque")).await?;
                let listed = mailbox::list(&mut session).await?;
                mailbox::save(&account.id, &listed)?;
                let _ = session.logout().await;
                Ok(())
            })
            .unwrap();
            let condition = |field, test, value: &str| Condition::new(field, test, value);
            let find = |search: &Search| super::search(&account, "x", search, std::slice::from_ref(&inbox), &BTreeMap::new(), 50, None).unwrap();
            // GreenMail reads FROM, TO and CC as whole addresses, where RFC 9051
            // finds parts of them, and refuses a part of one ("BAD Search command
            // not supported"): the folder is then asked again without the words,
            // and the sender checked here.
            let bank = Search { conditions: vec![condition(Field::From, Test::Contains, "@banque.example")], join: Join::All };
            let found = find(&bank);
            assert_eq!(found.found.len(), 2, "{found:?}");
            assert_eq!(find(&Search { conditions: vec![condition(Field::From, Test::Is, "no-reply@banque.example")], join: Join::All }).found.len(), 2);
            // An attachment: the usual headers asked, its part checked here.
            let with_pdf = Search { conditions: vec![condition(Field::From, Test::Contains, "no-reply@banque.example"), condition(Field::AttachmentType, Test::Is, "pdf")], join: Join::All };
            let found = find(&with_pdf);
            assert_eq!(found.found.len(), 1, "{found:?}");
            assert_eq!(found.found[0].card.shape.parts, vec![("releve-juin.pdf".to_string(), "application/pdf".to_string())]);
            // Words with accents, as a literal: non-synchronizing (GreenMail offers LITERAL+)…
            let accents = Search { conditions: vec![condition(Field::Subject, Test::Contains, "relevé")], join: Join::All };
            assert_eq!(find(&accents).found.len(), 1);
            // …and waiting for the server's go-ahead, as where it does not.
            let tokens = imap_key(&accents, &account.id, &inbox, false).unwrap().tokens();
            let waited = crate::fetch::block_on(async {
                let mut session = imap::open(&server, "x").await?;
                imap::within(COMMAND, session.examine("INBOX")).await?.map_err(imap::server)?;
                let found = uid_search(&mut session, &tokens, false).await;
                let _ = session.logout().await;
                found
            })
            .unwrap();
            assert_eq!(waited.len(), 1, "{waited:?}");
            // Any of them.
            let either = Search { conditions: vec![condition(Field::From, Test::Contains, "paul@example.org"), condition(Field::Subject, Test::Contains, "Rappel")], join: Join::Any };
            assert_eq!(find(&either).found.len(), 2);
            // Brought here, then left out of the next search: it is kept here now.
            let first = &find(&with_pdf).found[0];
            let file = bring(&account, "x", &first.place()).unwrap();
            assert!(file.is_file() && maildir::origin_of(&file) == Some(first.origin), "{file:?}");
            let mut here = BTreeMap::new();
            here.insert("INBOX".to_string(), kept_uids(&account, &inbox));
            assert!(super::search(&account, "x", &with_pdf, std::slice::from_ref(&inbox), &here, 50, None).unwrap().found.is_empty());
            // Two moved at once: the one kept here and one on the server only, in one connection.
            let rappel = find(&Search { conditions: vec![condition(Field::Subject, Test::Is, "Rappel")], join: Join::All }).found.remove(0);
            mailbox::act_many(&account, "x", std::slice::from_ref(&file), &[("INBOX".to_string(), rappel.origin)], &mailbox::Action::Move("Banque".into())).unwrap();
            assert!(!file.exists(), "the copy here leaves with it");
            let banque = sioul_core::folders::folder("Banque", None, None);
            let moved = super::search(&account, "x", &bank, std::slice::from_ref(&banque), &BTreeMap::new(), 50, None).unwrap();
            assert_eq!(moved.found.len(), 2, "{moved:?}");
            assert!(find(&bank).found.is_empty(), "out of the inbox");
            let _ = std::fs::remove_dir_all(&root);
        }
    }
}
