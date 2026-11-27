// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Each draft's copy in its account's Drafts folder, kept at the account's
//! rounds (`sioul_core::draftserver`; docs/client.md, "Drafts on the
//! server"), on a connection of its own, opened only when this device has
//! something to place or remove there, after the account's mail is fetched:
//! a slow or refused upload never holds back new mail, the rules or the
//! other folders.
//!
//! A draft placed: its copy appended (`\Draft \Seen`, within a time that
//! grows with its size), then every other copy of the same draft removed (an
//! older version, or the same placed twice by two devices, the highest UID
//! of the version kept). Copies are found by a listing of the Drafts
//! folder's heads, `UID FETCH 1:* (BODY.PEEK[HEADER.FIELDS (MESSAGE-ID
//! X-SIOUL-DRAFT FROM)])`, which every server answers (a Drafts folder is
//! small), each counted only with both marks of one version, from the
//! account's own address (`draftserver::own_copy`): never a SEARCH, which
//! some servers (Gmail among them) answer with nothing for a header Sioul
//! invented or a part of a Message-ID. Each version has a Message-ID of its
//! own: Gmail drops a second message under a Message-ID it holds. A copy of
//! the version there already (another device placed it) is kept, nothing
//! appended. A draft gone: its every copy removed. Removed is marked
//! `\Deleted` and, where the server has UIDPLUS (RFC 4315), expunged by its
//! UID alone; without it the mark stays, as for every message Sioul deletes.
//!
//! A server that refuses a copy (its NO: over quota, a copy past its limit):
//! that version is tried again after a growing wait, the other drafts going
//! on, and the refusal is said once on the account's card. A copy larger
//! than the server's APPENDLIMIT (RFC 7889) goes without its attachments. A
//! connection lost or too slow: the round ends there, its connection
//! dropped, and that version waits its turn too.

#[allow(unused_imports)]
use crate::files::Probe as _;
use crate::SyncError;
use crate::imap::{self, COMMAND};
use async_imap::Session;
use async_imap::imap_proto::rfc4315::UidSetMember;
use async_imap::imap_proto::{Response, ResponseCode, Status};
use futures_util::StreamExt;
use sioul_core::config::Account;
use sioul_core::draftserver::{self, Placed, Done, Mirror};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};

/// What the server says it can take.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Limits {
    /// UIDPLUS: UIDs expunged one by one, APPENDUID given.
    pub uidplus: bool,
    /// APPENDLIMIT (RFC 7889), in bytes, when said.
    pub append_limit: Option<u64>,
}

impl Limits {
    pub(crate) fn of(capabilities: &async_imap::types::Capabilities) -> Limits {
        let mut append_limit = None;
        for capability in capabilities.iter() {
            if let async_imap::types::Capability::Atom(atom) = capability
                && let Some(limit) = atom.strip_prefix("APPENDLIMIT=").and_then(|n| n.parse::<u64>().ok())
            {
                append_limit = Some(limit);
            }
        }
        Limits { uidplus: crate::mailbox::uidplus(capabilities), append_limit }
    }
}

/// What this device's state gives `account`'s next round to do; none when
/// nothing: no connection is then opened. Without `attachments` (a phone's
/// background step, bounded by its wake lock), a draft with files attached
/// is left to a round of the window.
pub(crate) fn work_for(account: &Account, attachments: bool) -> Option<draftserver::Work> {
    let now = jiff::Timestamp::now().as_second();
    let mut work = Mirror::load(&Mirror::default_path()).work(&account.id, &draftserver::drafts_here(), now);
    if !attachments {
        work.place.retain(|(draft, _, _)| !draft.attachments.iter().any(|path| path.present_file()));
    }
    Some(work).filter(|w| !w.is_empty())
}

/// Whether `account`'s drafts round has anything to do on this device: no
/// password read, no connection opened otherwise.
pub fn has_work(account: &Account, attachments: bool) -> bool {
    work_for(account, attachments).is_some()
}

/// The account's drafts round's own lock, across processes (the window and
/// a phone's background step, `sioul sync`): one round at a time, the others
/// skip theirs, so that two never place versions over each other.
pub(crate) fn lock(account: &Account) -> Option<crate::files::File> {
    let path = sioul_core::config::state_dir().join("sync").join(format!("{}.drafts.lock", account.id));
    crate::fetch::lock_within(&path, Duration::ZERO)
}

/// A round done: the state written. Returns the refusals to say, once each
/// (`say`: they are said on the account's card, and count as said).
pub(crate) fn finish(account: &Account, done: &Done, say: bool) -> Result<Vec<String>, SyncError> {
    let now = jiff::Timestamp::now().as_second();
    Mirror::update(&Mirror::default_path(), |m| m.done(&account.id, done, now, say)).map_err(SyncError::Disk)
}

/// How long an APPEND of `bytes` may take: a minute, and a second for each
/// 50 kB (an uplink of 400 kbit/s still gets there).
fn append_time(bytes: usize) -> Duration {
    Duration::from_secs(60 + (bytes / 50_000) as u64)
}

/// Why a step stopped.
enum Stop {
    /// The server answered NO or BAD: its words.
    Refused(String),
    /// The connection is lost, or too slow: the session is not to be used again.
    Broken(SyncError),
}

/// A server's answer read as a refusal, anything else as a broken connection.
fn stop_of(e: async_imap::error::Error) -> Stop {
    match e {
        async_imap::error::Error::No(words) | async_imap::error::Error::Bad(words) => Stop::Refused(words),
        other => Stop::Broken(imap::server(other)),
    }
}

/// The work done in the Drafts folder `name`, on `session`: what was done,
/// and the error that broke the connection, if one did (the session is then
/// not to be used again: the caller drops it).
pub(crate) async fn act<T>(session: &mut Session<T>, account: &Account, name: &str, work: &draftserver::Work, limits: Limits) -> (Done, Option<SyncError>)
where
    T: AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    let mut done = Done::default();
    let (validity, exists) = match imap::within(COMMAND, session.select(name)).await {
        Ok(Ok(mailbox)) => (mailbox.uid_validity.unwrap_or(0), mailbox.exists),
        Ok(Err(e)) => return (done, Some(imap::server(e))),
        Err(e) => return (done, Some(e)),
    };
    let keys = sioul_core::pgp::Keys::load();
    let address = account.address.clone().unwrap_or_default();
    let from = (account.name.clone().unwrap_or_default(), address.clone());
    let domain = address.rsplit_once('@').map_or("localhost", |(_, d)| d).to_string();
    // Every copy in the folder, listed once (and again after an APPEND whose UID the server did not say).
    let mut listed = if exists == 0 {
        Vec::new()
    } else {
        match listing(session, &address).await {
            Ok(listed) => listed,
            Err(e) => return (done, Some(e)),
        }
    };
    let copies_of = |listed: &[(u32, String, String)], id: &str| listed.iter().filter(|(_, i, _)| i == id).map(|(uid, _, rev)| (*uid, rev.clone())).collect::<Vec<_>>();
    for (id, _) in &work.remove {
        let found = copies_of(&listed, id);
        if let Err(e) = delete(session, &found.iter().map(|(uid, _)| *uid).collect::<Vec<_>>(), limits.uidplus).await {
            return (done, Some(e));
        }
        listed.retain(|(uid, _, _)| !found.iter().any(|(gone, _)| gone == uid));
        done.removed.push(id.clone());
    }
    for (draft, rev, _) in &work.place {
        let encrypt = draftserver::wants_encryption(draft);
        let own: Vec<_> = if encrypt { keys.self_cert_for(&address).into_iter().collect() } else { Vec::new() };
        let found = copies_of(&listed, &draft.id);
        // The newest copy of the version is kept, whoever placed it: every device and round agrees.
        let mut kept: Option<u32> = found.iter().filter(|(_, r)| r == rev).map(|(uid, _)| *uid).max();
        let mut placed = kept.map(|uid| Placed { validity, uid });
        if kept.is_none() {
            let date = jiff::Timestamp::now().as_second();
            let (mark, message_id) = (draftserver::mark(&draft.id, rev), draftserver::message_id(&draft.id, rev, &domain));
            let build = |attached: bool| sioul_core::compose::server_copy(draft, (&from.0, &from.1), date, &mark, &message_id, encrypt, attached, &own);
            let raw = match build(true) {
                // Past the server's limit: without its attachments.
                Ok(Some(raw)) if limits.append_limit.is_some_and(|limit| raw.len() as u64 > limit) => build(false),
                other => other,
            };
            let raw = match raw {
                Ok(Some(raw)) => raw,
                Ok(None) => {
                    // To be encrypted, and no certificate of yours here: no copy of this version,
                    // and none of an older one left in its place.
                    if let Err(e) = delete(session, &found.iter().map(|(uid, _)| *uid).collect::<Vec<_>>(), limits.uidplus).await {
                        return (done, Some(e));
                    }
                    done.withheld.push(draft.id.clone());
                    done.placed.push((draft.id.clone(), rev.clone(), None));
                    continue;
                }
                Err(words) => {
                    // A file it names could not be read: tried again later, like a refusal, never said.
                    eprintln!("{}: a draft's copy: {words}", account.id);
                    done.failed.push((draft.id.clone(), rev.clone(), String::new()));
                    continue;
                }
            };
            match append(session, name, &raw).await {
                Ok(appended) => {
                    placed = appended.filter(|c| c.validity == validity);
                    kept = placed.map(|c| c.uid);
                }
                Err(Stop::Refused(words)) => {
                    done.failed.push((draft.id.clone(), rev.clone(), words));
                    continue;
                }
                Err(Stop::Broken(e)) => {
                    done.failed.push((draft.id.clone(), rev.clone(), String::new()));
                    return (done, Some(e));
                }
            }
            // Its UID, when the server did not say it: the newest copy of the version listed again.
            if kept.is_none() {
                match listing(session, &address).await {
                    Ok(again) => {
                        kept = copies_of(&again, &draft.id).iter().filter(|(_, r)| r == rev).map(|(uid, _)| *uid).max();
                        placed = kept.map(|uid| Placed { validity, uid });
                        listed = again;
                    }
                    Err(e) => return (done, Some(e)),
                }
            }
        }
        // Only what was read back with both marks goes: never a message that is not one of Sioul's copies.
        let others: Vec<u32> = found.iter().map(|(uid, _)| *uid).filter(|uid| Some(*uid) != kept).collect();
        if let Err(e) = delete(session, &others, limits.uidplus).await {
            return (done, Some(e));
        }
        listed.retain(|(uid, _, _)| !others.contains(uid));
        done.placed.push((draft.id.clone(), rev.clone(), placed));
    }
    (done, None)
}

/// Every copy in the folder selected, by UID, with its draft and version: a
/// listing of every message's Message-ID, `X-Sioul-Draft` and From (one
/// FETCH, which every server answers; never a SEARCH), each counted only
/// with both marks of one version, from `own` (the account's address), and
/// not marked deleted.
async fn listing<T>(session: &mut Session<T>, own: &str) -> Result<Vec<(u32, String, String)>, SyncError>
where
    T: AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    let query = format!("(UID FLAGS BODY.PEEK[HEADER.FIELDS (MESSAGE-ID {} FROM)])", draftserver::HEADER.to_ascii_uppercase());
    let mut out = Vec::new();
    let fetched = imap::within(COMMAND, session.uid_fetch("1:*", query)).await?.map_err(imap::server)?;
    let mut fetched = std::pin::pin!(fetched);
    while let Some(answer) = fetched.next().await {
        let fetch = answer.map_err(imap::server)?;
        let deleted = fetch.flags().any(|f| matches!(f, async_imap::types::Flag::Deleted));
        let (Some(uid), Some(head)) = (fetch.uid, fetch.header()) else { continue };
        if deleted {
            continue;
        }
        if let Some((id, rev)) = draftserver::own_copy(head, own) {
            out.push((uid, id, rev));
        }
    }
    Ok(out)
}

/// `raw` appended to `folder` as a draft read: a synchronizing literal (RFC
/// 9051 §4.3), its bytes written once the server says "+", within a time
/// that grows with its size. Where the server put it, when it said
/// (APPENDUID); its NO or BAD, a refusal; anything else, a broken connection.
async fn append<T>(session: &mut Session<T>, folder: &str, raw: &[u8]) -> Result<Option<Placed>, Stop>
where
    T: AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    if folder.contains(['\r', '\n']) {
        return Err(Stop::Refused(format!("no folder {folder}")));
    }
    let quoted = format!("\"{}\"", folder.replace('\\', "\\\\").replace('"', "\\\""));
    let limit = append_time(raw.len());
    let id = match imap::within(COMMAND, session.run_command(format!("APPEND {quoted} (\\Draft \\Seen) {{{}}}", raw.len()))).await {
        Ok(Ok(id)) => id,
        Ok(Err(e)) => return Err(stop_of(e)),
        Err(e) => return Err(Stop::Broken(e)),
    };
    let broken = |e: String| Stop::Broken(SyncError::Network(e));
    loop {
        let response = match imap::within(COMMAND, session.read_response()).await {
            Ok(Ok(Some(response))) => response,
            Ok(Ok(None)) => return Err(broken("the connection was closed".into())),
            Ok(Err(e)) => return Err(broken(e.to_string())),
            Err(e) => return Err(Stop::Broken(e)),
        };
        match response.parsed() {
            Response::Continue { .. } => break,
            Response::Done { tag, outcome, .. } if tag == &id => return Err(Stop::Refused(format!("{:?} {}", outcome.code, outcome.information.as_deref().unwrap_or("")))),
            _ => {}
        }
    }
    let stream = session.get_mut();
    let written = imap::within(limit, async {
        stream.write_all(raw).await?;
        stream.write_all(b"\r\n").await?;
        stream.flush().await
    })
    .await;
    match written {
        Ok(Ok(())) => {}
        Ok(Err(e)) => return Err(broken(e.to_string())),
        Err(e) => return Err(Stop::Broken(e)),
    }
    loop {
        let response = match imap::within(limit, session.read_response()).await {
            Ok(Ok(Some(response))) => response,
            Ok(Ok(None)) => return Err(broken("the connection was closed".into())),
            Ok(Err(e)) => return Err(broken(e.to_string())),
            Err(e) => return Err(Stop::Broken(e)),
        };
        if let Response::Done { tag, status, outcome } = response.parsed()
            && tag == &id
        {
            let words = format!("{} {}", outcome.code.as_ref().map(|c| format!("{c:?}")).unwrap_or_default(), outcome.information.as_deref().unwrap_or("")).trim().to_string();
            return match status {
                Status::Ok => Ok(match &outcome.code {
                    Some(ResponseCode::AppendUid(validity, set)) => match set.as_slice() {
                        [UidSetMember::Uid(uid)] if *validity != 0 && *uid != 0 => Some(Placed { validity: *validity, uid: *uid }),
                        _ => None,
                    },
                    _ => None,
                }),
                _ => Err(Stop::Refused(words)),
            };
        }
    }
}

/// Copies marked `\Deleted`, and expunged by their UIDs where the server can.
async fn delete<T>(session: &mut Session<T>, uids: &[u32], uidplus: bool) -> Result<(), SyncError>
where
    T: AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    if uids.is_empty() {
        return Ok(());
    }
    let set = uids.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
    {
        let stored = imap::within(COMMAND, session.uid_store(&set, "+FLAGS.SILENT (\\Deleted)")).await?.map_err(imap::server)?;
        let mut stored = std::pin::pin!(stored);
        while let Some(answer) = stored.next().await {
            answer.map_err(imap::server)?;
        }
    }
    if !uidplus {
        return Ok(());
    }
    let gone = imap::within(COMMAND, session.uid_expunge(&set)).await?.map_err(imap::server)?;
    let mut gone = std::pin::pin!(gone);
    while let Some(answer) = gone.next().await {
        answer.map_err(imap::server)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sioul_core::compose::Draft;
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
    use tokio::net::TcpListener;

    /// One message of the stand-in's Drafts folder.
    #[derive(Debug, Clone)]
    struct Kept {
        uid: u32,
        flags: String,
        raw: Vec<u8>,
    }

    /// The stand-in's Drafts folder, and how it behaves. It searches
    /// nothing at all (every SEARCH answers BAD): a round never needs one.
    #[derive(Debug, Default)]
    struct Server {
        kept: Vec<Kept>,
        /// As Gmail: a message under a Message-ID the folder holds already is
        /// not added; APPEND answers with the UID of the one there.
        gmail: bool,
        /// APPEND answers NO, over quota.
        full: bool,
        /// No APPENDUID in APPEND's answer.
        no_appenduid: bool,
        /// APPEND never answered: a link too slow, a server stuck.
        stuck: bool,
    }

    type Folder = Arc<Mutex<Server>>;

    /// An IMAP server standing in for a mail provider's, without TLS, with
    /// one folder, "Drafts": LOGIN, CAPABILITY, SELECT, UID FETCH of the
    /// flags and of header fields, APPEND (APPENDUID, or NO when full, or as
    /// Gmail with a Message-ID it holds), UID STORE, UID EXPUNGE (UIDPLUS)
    /// and LOGOUT, as far as a round asks them; never a SEARCH. What the
    /// client sent is kept.
    async fn stand_in(listener: TcpListener, folder: Folder, heard: Arc<Mutex<Vec<String>>>) {
        let (stream, _) = listener.accept().await.unwrap();
        let (read, mut write) = stream.into_split();
        let mut reader = BufReader::new(read);
        write.write_all(b"* OK [CAPABILITY IMAP4rev1 UIDPLUS] Stand-in ready\r\n").await.unwrap();
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).await.unwrap() == 0 {
                return;
            }
            let line = line.trim_end().to_string();
            heard.lock().unwrap().push(line.clone());
            let (tag, command) = line.split_once(' ').unwrap();
            let upper = command.to_ascii_uppercase();
            let uids = |at: usize| -> Vec<u32> { command.split_whitespace().nth(at).unwrap().split(',').map(|n| n.parse().unwrap()).collect() };
            let answer = if upper.starts_with("LOGIN") {
                format!("{tag} OK Logged in\r\n")
            } else if upper.starts_with("CAPABILITY") {
                format!("* CAPABILITY IMAP4rev1 UIDPLUS\r\n{tag} OK Capability\r\n")
            } else if upper.starts_with("SELECT") {
                let n = folder.lock().unwrap().kept.len();
                format!("* {n} EXISTS\r\n* OK [UIDVALIDITY 7] UIDs valid\r\n* OK [UIDNEXT 100] next\r\n{tag} OK [READ-WRITE] Selected\r\n")
            } else if upper.contains("SEARCH") {
                format!("{tag} BAD No search in this stand-in\r\n")
            } else if upper.starts_with("UID FETCH") {
                let wanted = command.split_whitespace().nth(2).unwrap().to_string();
                let set: Vec<u32> = if wanted == "1:*" { Vec::new() } else { uids(2) };
                let fields: Vec<String> = command.split_once("HEADER.FIELDS (").map(|(_, f)| f.split(')').next().unwrap().split_whitespace().map(str::to_string).collect()).unwrap_or_default();
                let kept = folder.lock().unwrap().kept.clone();
                let mut out = String::new();
                for (seq, m) in kept.iter().enumerate().filter(|(_, m)| wanted == "1:*" || set.contains(&m.uid)) {
                    let head: String = fields.iter().filter_map(|f| draftserver::field(&m.raw, f).map(|v| format!("{f}: {v}\r\n"))).collect::<String>() + "\r\n";
                    out.push_str(&format!("* {} FETCH (UID {} FLAGS ({}) BODY[HEADER.FIELDS ({})] {{{}}}\r\n{head})\r\n", seq + 1, m.uid, m.flags.trim(), fields.join(" "), head.len()));
                }
                format!("{out}{tag} OK Fetch done\r\n")
            } else if upper.starts_with("APPEND") {
                if folder.lock().unwrap().stuck {
                    // Never "+": the client waits until it gives up and drops the connection.
                    let mut rest = String::new();
                    let _ = reader.read_line(&mut rest).await;
                    return;
                }
                let length: usize = command.rsplit_once('{').unwrap().1.trim_end_matches('}').parse().unwrap();
                let flags = command.split_once('(').map(|(_, f)| f.split_once(')').unwrap().0.to_string()).unwrap_or_default();
                write.write_all(b"+ Ready\r\n").await.unwrap();
                let mut raw = vec![0u8; length];
                reader.read_exact(&mut raw).await.unwrap();
                let mut end = String::new();
                reader.read_line(&mut end).await.unwrap();
                let mut server = folder.lock().unwrap();
                let named = draftserver::field(&raw, "Message-ID");
                let held = server.kept.iter().find(|m| !m.flags.contains("\\Deleted") && named.is_some() && draftserver::field(&m.raw, "Message-ID") == named).map(|m| m.uid);
                if server.full {
                    format!("{tag} NO [OVERQUOTA] Quota exceeded\r\n")
                } else if let Some(uid) = held.filter(|_| server.gmail) {
                    // Gmail: one message per Message-ID; the second is the first.
                    format!("{tag} OK [APPENDUID 7 {uid}] Appended\r\n")
                } else {
                    let uid = server.kept.iter().map(|m| m.uid).max().unwrap_or(0) + 1;
                    server.kept.push(Kept { uid, flags, raw });
                    if server.no_appenduid { format!("{tag} OK Appended\r\n") } else { format!("{tag} OK [APPENDUID 7 {uid}] Appended\r\n") }
                }
            } else if upper.starts_with("UID STORE") {
                let set = uids(2);
                for m in folder.lock().unwrap().kept.iter_mut().filter(|m| set.contains(&m.uid)) {
                    m.flags.push_str(" \\Deleted");
                }
                format!("{tag} OK Stored\r\n")
            } else if upper.starts_with("UID EXPUNGE") {
                let set = uids(2);
                let mut server = folder.lock().unwrap();
                let mut out = String::new();
                while let Some(at) = server.kept.iter().position(|m| set.contains(&m.uid) && m.flags.contains("\\Deleted")) {
                    server.kept.remove(at);
                    out.push_str(&format!("* {} EXPUNGE\r\n", at + 1));
                }
                format!("{out}{tag} OK Expunged\r\n")
            } else if upper.starts_with("LOGOUT") {
                write.write_all(format!("* BYE\r\n{tag} OK Logged out\r\n").as_bytes()).await.unwrap();
                return;
            } else {
                format!("{tag} BAD Not in this stand-in\r\n")
            };
            write.write_all(answer.as_bytes()).await.unwrap();
        }
    }

    /// One round as an account's round makes it (`fetch::drafts_round`), its
    /// Drafts folder "Drafts", on the stand-in, on a connection of its own:
    /// what it did, what it said, what the client sent.
    fn round_on(folder: &Folder, account: &Account) -> (Done, Vec<String>, Vec<String>) {
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        runtime.block_on(async {
            let Some(work) = work_for(account, true) else { return (Done::default(), Vec::new(), Vec::new()) };
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let port = listener.local_addr().unwrap().port();
            let heard = Arc::new(Mutex::new(Vec::new()));
            let server = tokio::spawn(stand_in(listener, Arc::clone(folder), Arc::clone(&heard)));
            let tcp = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.unwrap();
            let mut client = async_imap::Client::new(tcp);
            client.read_response().await.unwrap();
            let mut session = client.login("me@example.org", "stand-in").await.map_err(|(e, _)| e).unwrap();
            let limits = Limits::of(&session.capabilities().await.unwrap());
            let (done, broken) = act(&mut session, account, "Drafts", &work, limits).await;
            assert!(broken.is_none(), "{broken:?}");
            let said = finish(account, &done, true).unwrap();
            session.logout().await.unwrap();
            server.await.unwrap();
            let heard = heard.lock().unwrap().clone();
            (done, said, heard)
        })
    }

    /// The Drafts folder as it stands: each of the account's own copies' draft id and version.
    fn marks(folder: &Folder) -> Vec<(String, String)> {
        folder.lock().unwrap().kept.iter().filter(|m| !m.flags.contains("\\Deleted")).filter_map(|m| draftserver::own_copy(&m.raw, "me@example.org")).collect()
    }

    fn account(name: &str) -> Account {
        crate::dav::stand_in::home();
        let mut a = Account::imap(&format!("drafts-{name}-{}", std::process::id()), "me@example.org", "localhost", 993, sioul_core::config::Security::Tls, None);
        a.name = Some("Me".into());
        a
    }

    /// Ready for its next try: the wait after a failure taken as over.
    fn wait_over(id: &str) {
        Mirror::update(&Mirror::default_path(), |m| {
            if let Some(retry) = m.drafts.get_mut(id).and_then(|e| e.retry.as_mut()) {
                retry.at = 0;
            }
        })
        .unwrap();
    }

    /// A draft saved here goes to its account's Drafts folder at the next
    /// round, once, marked as Sioul's and as a draft; a new version takes the
    /// old one's place; a copy another device placed of the same version is
    /// kept and nothing appended; two devices' copies of one version end as
    /// one; a draft sent here, or taken away by the sharing, leaves the
    /// server; a copy another program deleted is not put back; no connection
    /// when nothing changed; an encrypted draft goes there encrypted, or not
    /// at all. A stand-in server, never a real account.
    #[test]
    fn drafts_reach_the_servers_drafts_folder_once() {
        let account = account("once");
        let folder: Folder = Arc::new(Mutex::new(Server::default()));
        // Saved in the window: placed at the next round.
        let mut draft = Draft { to: vec!["Jane <jane@example.org>".into()], bcc: vec!["sam@example.net".into()], subject: "Lunch".into(), body: "Noon on **Friday**?".into(), ..Draft::new(&account.id) };
        draft.save().unwrap();
        let (done, _, _) = round_on(&folder, &account);
        let rev = draftserver::rev(&std::fs::read(draft.path()).unwrap());
        assert_eq!(done.placed, [(draft.id.clone(), rev.clone(), Some(Placed { validity: 7, uid: 1 }))]);
        assert_eq!(marks(&folder), [(draft.id.clone(), rev.clone())]);
        {
            let server = folder.lock().unwrap();
            let copy = String::from_utf8_lossy(&server.kept[0].raw).to_string();
            assert!(server.kept[0].flags.contains("\\Draft") && server.kept[0].flags.contains("\\Seen"), "{:?}", server.kept[0].flags);
            assert!(copy.contains("Subject: Lunch") && copy.contains("Noon on **Friday**?") && copy.contains("<strong>Friday</strong>"), "{copy}");
            assert!(copy.contains("jane@example.org") && copy.contains("Bcc: ") && copy.contains("sam@example.net"), "a draft keeps its Bcc: {copy}");
        }
        // Nothing changed: no connection at all.
        let (done, _, heard) = round_on(&folder, &account);
        assert_eq!(done, Done::default());
        assert!(heard.is_empty(), "{heard:?}");
        // A new version, under a Message-ID of its own: it takes the old one's place.
        draft.body = "Noon on Friday, at the market?".into();
        draft.save().unwrap();
        let rev2 = draftserver::rev(&std::fs::read(draft.path()).unwrap());
        round_on(&folder, &account);
        assert_eq!(marks(&folder), [(draft.id.clone(), rev2.clone())]);
        // Another device placed the same version twice meanwhile: one copy stays, nothing appended.
        let twin = folder.lock().unwrap().kept[0].clone();
        folder.lock().unwrap().kept.push(Kept { uid: 50, ..twin });
        Mirror::update(&Mirror::default_path(), |m| m.drafts.get_mut(&draft.id).unwrap().rev = "another-version".into()).unwrap();
        let (_, _, heard) = round_on(&folder, &account);
        assert!(!heard.iter().any(|l| l.contains("APPEND")), "{heard:?}");
        assert_eq!(marks(&folder), [(draft.id.clone(), rev2.clone())]);
        // Deleted in another mail program: not put back while the draft is unchanged.
        folder.lock().unwrap().kept.clear();
        let (done, _, _) = round_on(&folder, &account);
        assert_eq!(done, Done::default());
        assert!(folder.lock().unwrap().kept.is_empty(), "never put back");
        // Changed again in Sioul: placed again.
        draft.subject = "Lunch on Friday".into();
        draft.save().unwrap();
        round_on(&folder, &account);
        assert_eq!(marks(&folder).len(), 1);
        // Sent (or discarded) here: its copies leave the server.
        draft.discard();
        let (done, _, _) = round_on(&folder, &account);
        assert_eq!(done.removed, [draft.id.clone()]);
        assert!(folder.lock().unwrap().kept.is_empty());
        // This draft's marks gone (other tests of the process keep their own in the same state).
        let mirror = Mirror::load(&Mirror::default_path());
        assert!(!mirror.gone.contains_key(&draft.id) && !mirror.drafts.contains_key(&draft.id), "{mirror:?}");
        // Taken away by the sharing (sent on another device): the copy placed from here goes.
        let other = Draft { subject: "Rent".into(), body: "Here is the receipt.".into(), ..Draft::new(&account.id) };
        other.save().unwrap();
        round_on(&folder, &account);
        assert_eq!(marks(&folder).len(), 1);
        std::fs::remove_file(other.path()).unwrap();
        let (done, _, _) = round_on(&folder, &account);
        assert_eq!(done.removed, [other.id.clone()]);
        assert!(folder.lock().unwrap().kept.is_empty());
        // Encrypted, with no certificate of yours here: never there in clear.
        let sealed = Draft { subject: "Medical results".into(), body: "The secret clause.".into(), encrypt: true, ..Draft::new(&account.id) };
        sealed.save().unwrap();
        let (done, _, _) = round_on(&folder, &account);
        assert_eq!(done.withheld, [sealed.id.clone()]);
        assert!(folder.lock().unwrap().kept.is_empty());
        sealed.discard();
        round_on(&folder, &account);
    }

    /// Copies are found by listing the folder's heads, never by a search
    /// (the stand-in answers every SEARCH with BAD), on a server with
    /// APPENDUID or without it, and on one that keeps one message per
    /// Message-ID as Gmail does: each version, its own Message-ID, takes the
    /// old one's place, one copy at a time; a version another device placed
    /// already is kept, nothing appended; a draft sent leaves none behind.
    #[test]
    fn copies_never_pile_up_and_gmail_keeps_each_version() {
        for (name, server) in [("plain", Server::default()), ("nouid", Server { no_appenduid: true, ..Server::default() }), ("gmail", Server { gmail: true, ..Server::default() })] {
            let account = account(name);
            let folder: Folder = Arc::new(Mutex::new(server));
            let mut draft = Draft { subject: "Lunch".into(), body: "Noon?".into(), ..Draft::new(&account.id) };
            for body in ["Noon?", "Noon on Friday?", "Noon on Friday, at the market?"] {
                draft.body = body.into();
                draft.save().unwrap();
                let (_, _, heard) = round_on(&folder, &account);
                assert!(!heard.iter().any(|l| l.to_ascii_uppercase().contains("SEARCH")), "never a search: {heard:?}");
                let rev = draftserver::rev(&std::fs::read(draft.path()).unwrap());
                assert_eq!(marks(&folder), [(draft.id.clone(), rev)], "{name}: {body}");
                let copy = String::from_utf8_lossy(&folder.lock().unwrap().kept[0].raw).to_string();
                assert!(copy.contains(body), "{name}: the copy holds its own version's words: {copy}");
            }
            // Another device placed this very version (the same Message-ID): kept, nothing appended.
            let twin = folder.lock().unwrap().kept[0].clone();
            folder.lock().unwrap().kept.push(Kept { uid: 90, ..twin });
            Mirror::update(&Mirror::default_path(), |m| m.drafts.get_mut(&draft.id).unwrap().rev = "another-version".into()).unwrap();
            let (_, _, heard) = round_on(&folder, &account);
            assert!(!heard.iter().any(|l| l.contains("APPEND")), "{name}: {heard:?}");
            assert_eq!(marks(&folder).len(), 1, "{name}");
            draft.discard();
            round_on(&folder, &account);
            assert!(folder.lock().unwrap().kept.is_empty(), "{name}");
        }
    }

    /// Only the account's own copies are touched: a message from anyone else
    /// carrying both marks, and a draft another program wrote, stay.
    #[test]
    fn only_the_accounts_own_copies_are_touched() {
        let account = account("own");
        let folder: Folder = Arc::new(Mutex::new(Server::default()));
        let draft = Draft { subject: "Lunch".into(), body: "Noon?".into(), ..Draft::new(&account.id) };
        draft.save().unwrap();
        let rev = draftserver::rev(&std::fs::read(draft.path()).unwrap());
        let stranger = format!("Message-ID: <{}>\r\n{}: {}\r\nFrom: Someone <someone@elsewhere.example>\r\nSubject: Lunch\r\n\r\nNot yours.\r\n", draftserver::message_id(&draft.id, "old", "example.org"), draftserver::HEADER, draftserver::mark(&draft.id, "old"));
        let theirs = "Message-ID: <thunderbird-1@example.org>\r\nFrom: Me <me@example.org>\r\nSubject: Written elsewhere\r\n\r\nText.\r\n".to_string();
        folder.lock().unwrap().kept.extend([Kept { uid: 10, flags: "\\Draft".into(), raw: stranger.into_bytes() }, Kept { uid: 11, flags: "\\Draft".into(), raw: theirs.into_bytes() }]);
        round_on(&folder, &account);
        let kept: Vec<u32> = folder.lock().unwrap().kept.iter().map(|m| m.uid).collect();
        assert!(kept.contains(&10) && kept.contains(&11), "{kept:?}");
        assert_eq!(marks(&folder).iter().filter(|(_, r)| *r == rev).count(), 1);
        draft.discard();
        round_on(&folder, &account);
        assert_eq!(folder.lock().unwrap().kept.iter().map(|m| m.uid).collect::<Vec<_>>(), [10, 11]);
    }

    /// A phone's background step bounds the round (`fetch::drafts`, within
    /// what its wake lock leaves): a server that never answers holds it no
    /// longer than that, and the draft is left for the next step, nothing
    /// recorded; a draft with files attached is left to the window's rounds.
    #[test]
    fn a_stuck_server_holds_a_step_no_longer_than_its_bound() {
        let account = account("stuck");
        let folder: Folder = Arc::new(Mutex::new(Server { stuck: true, ..Server::default() }));
        let draft = Draft { subject: "Lunch".into(), body: "Noon?".into(), ..Draft::new(&account.id) };
        draft.save().unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let started = std::time::Instant::now();
        let bounded = runtime.block_on(async {
            let work = work_for(&account, false).unwrap();
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let port = listener.local_addr().unwrap().port();
            tokio::spawn(stand_in(listener, Arc::clone(&folder), Arc::default()));
            let tcp = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.unwrap();
            let mut client = async_imap::Client::new(tcp);
            client.read_response().await.unwrap();
            let mut session = client.login("me@example.org", "stand-in").await.map_err(|(e, _)| e).unwrap();
            tokio::time::timeout(Duration::from_secs(2), async {
                let (done, _) = act(&mut session, &account, "Drafts", &work, Limits::default()).await;
                finish(&account, &done, false).unwrap();
            })
            .await
        });
        assert!(bounded.is_err() && started.elapsed() < Duration::from_secs(10), "bounded: {:?}", started.elapsed());
        assert!(work_for(&account, false).is_some(), "left for the next step, nothing recorded");
        // Files attached: left to the window's rounds, never to a phone's step.
        let attached = std::env::temp_dir().join(format!("sioul-drafts-attached-{}.txt", std::process::id()));
        std::fs::write(&attached, "A file.").unwrap();
        let with_file = Draft { subject: "Receipt".into(), attachments: vec![attached.clone()], ..Draft::new(&account.id) };
        with_file.save().unwrap();
        assert!(work_for(&account, false).unwrap().place.iter().all(|(d, _, _)| d.id != with_file.id));
        assert!(work_for(&account, true).unwrap().place.iter().any(|(d, _, _)| d.id == with_file.id));
        draft.discard();
        with_file.discard();
        let _ = std::fs::remove_file(&attached);
    }

    /// A server that refuses a copy (over quota): said once, the other
    /// drafts placed all the same, the refused version tried again after its
    /// wait, and placed once the server takes it.
    #[test]
    fn a_refused_copy_is_said_once_and_tried_again_later() {
        let account = account("refused");
        let folder: Folder = Arc::new(Mutex::new(Server { full: true, ..Server::default() }));
        let first = Draft { subject: "One".into(), body: "First.".into(), ..Draft::new(&account.id) };
        first.save().unwrap();
        let (done, said, _) = round_on(&folder, &account);
        assert_eq!(done.failed.len(), 1);
        assert_eq!(said.len(), 1, "said once: {said:?}");
        assert!(said[0].contains("Quota"), "{said:?}");
        // Its wait not over: no connection.
        let (_, _, heard) = round_on(&folder, &account);
        assert!(heard.is_empty(), "{heard:?}");
        // Over: tried again, refused again, said no more.
        wait_over(&first.id);
        let (done, said, _) = round_on(&folder, &account);
        assert_eq!((done.failed.len(), said.len()), (1, 0));
        // Room again: placed, and another draft with it.
        folder.lock().unwrap().full = false;
        wait_over(&first.id);
        let second = Draft { subject: "Two".into(), body: "Second.".into(), ..Draft::new(&account.id) };
        second.save().unwrap();
        let (done, _, _) = round_on(&folder, &account);
        assert_eq!(done.placed.len(), 2, "{done:?}");
        assert_eq!(marks(&folder).len(), 2);
        first.discard();
        second.discard();
        round_on(&folder, &account);
    }

    /// An answer to an encrypted message, its Encrypt unticked (a recipient's
    /// key missing), goes there encrypted to your own key, never in clear.
    #[test]
    fn an_answer_to_encrypted_mail_never_goes_in_clear() {
        crate::dav::stand_in::home();
        let cert = sioul_core::pgp::certificates_in(&sioul_core::pgp::stand_in_key("Me <me@example.org>").unwrap()).remove(0);
        let original = sioul_core::config::state_dir().join(format!("encrypted-original-{}.eml", std::process::id()));
        std::fs::create_dir_all(original.parent().unwrap()).unwrap();
        std::fs::write(&original, sioul_core::pgp::seal_message(b"From: Jane <jane@example.org>\r\nSubject: The lease\r\n\r\nThe clause.\r\n", &[&cert]).unwrap()).unwrap();
        let answer = Draft { id: "answer1".into(), account: "home".into(), to: vec!["jane@example.org".into()], subject: "Re: The lease".into(), body: "Agreed on the clause.".into(), encrypt: false, original: Some(original.clone()), ..Draft::default() };
        assert!(draftserver::wants_encryption(&answer));
        let raw = sioul_core::compose::server_copy(&answer, ("Me", "me@example.org"), 1, &draftserver::mark("answer1", "r1"), &draftserver::message_id("answer1", "r1", "example.org"), true, true, &[&cert]).unwrap().unwrap();
        let text = String::from_utf8_lossy(&raw);
        assert!(!text.contains("Agreed") && !text.contains("The lease") && text.contains("multipart/encrypted"), "{text}");
        assert_eq!(sioul_core::compose::server_copy(&answer, ("Me", "me@example.org"), 1, "m", "i@example.org", true, true, &[]).unwrap(), None, "no key of yours: no copy");
        let _ = std::fs::remove_file(&original);
    }

    /// An encrypted draft's copy is encrypted to your own certificate, its
    /// subject and Bcc inside it: neither its words, its subject nor its Bcc
    /// lie on the server in clear, and your key opens them.
    #[test]
    fn an_encrypted_drafts_copy_is_encrypted_to_you() {
        let cert = sioul_core::pgp::certificates_in(&sioul_core::pgp::stand_in_key("Me <me@example.org>").unwrap()).remove(0);
        let draft = Draft { id: "sealed1".into(), account: "home".into(), to: vec!["jane@example.org".into()], bcc: vec!["sam@example.net".into()], subject: "Medical results".into(), body: "The secret clause.".into(), encrypt: true, ..Draft::default() };
        let raw = sioul_core::compose::server_copy(&draft, ("Me", "me@example.org"), 1_790_000_000, &draftserver::mark("sealed1", "r1"), &draftserver::message_id("sealed1", "r1", "example.org"), true, true, &[&cert]).unwrap().unwrap();
        let text = String::from_utf8_lossy(&raw);
        assert!(!text.contains("secret clause") && !text.contains("Medical") && !text.contains("sam@example.net"), "{text}");
        assert!(text.contains("multipart/encrypted") && text.contains("Subject: ...") && text.contains("jane@example.org"), "{text}");
        let keys = sioul_core::pgp::Keys { own: vec![cert.clone()], ..sioul_core::pgp::Keys::default() };
        let sessions = sioul_core::pgp::SessionKeys::new();
        let (opened, _) = sioul_core::pgp::open(&raw, &keys, &sioul_core::pgp::Unlock::new(&|_| None, &sessions)).expect("encrypted");
        let opened = String::from_utf8_lossy(&opened.expect("your key opens it")).to_string();
        assert!(opened.contains("The secret clause.") && opened.contains("Medical results") && opened.contains("sam@example.net"), "{opened}");
    }
}
