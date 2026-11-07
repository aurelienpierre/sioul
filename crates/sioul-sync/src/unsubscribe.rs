// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Leaving a mailing list in one click, on the network: the HTTPS POST of
//! RFC 8058 (docs/client.md, "Unsubscribing"). When it may be sent is the
//! core's to say (`sioul_core::unsubscribe`); here it is sent as the RFC
//! asks: `List-Unsubscribe=One-Click` as a form, no cookie, no authorisation,
//! nothing of yours but what the list wrote in its own address. A redirect to
//! another site is never followed; the wait is short.
//!
//! Here too, what the core's rule for the lists of the mail you called spam
//! needs from this device (`spam_lists_here`), each message judged as the
//! Reader judges it (`offer_of`), and leaving one of them (`leave`). No agent
//! ever reaches these: the command line and the window only.

use crate::{SyncError, mailbox};
use sioul_core::config::{Config, Source};
use sioul_core::folders::Role;
use sioul_core::i18n::{self, Translator};
use sioul_core::porch::{self, Reason, Triaged};
use sioul_core::spam::labels::{self, Entry, Label};
use sioul_core::trust::Trust;
use sioul_core::unsubscribe::{self as core, Offer, Record, SaidCopy, SpamList, SpamLists};
use sioul_core::{mailcache, maildir, mailindex};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The longest the list's server may take to answer, all told.
pub const WAIT: Duration = Duration::from_secs(15);
/// Redirects followed within the same site (RFC 8058 asks senders for none).
const HOPS: usize = 3;

/// What the list's server made of the request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// It took it (2xx).
    Done,
    /// It answered with this status.
    Refused(u16),
    /// It sent Sioul to another site (its host), not followed.
    Elsewhere(String),
    /// No answer within `WAIT`.
    TimedOut,
}

/// Posts `List-Unsubscribe=One-Click` to `url`, an HTTPS address of a name on
/// the Internet (`sioul_core::unsubscribe::host_of`).
pub fn one_click(url: &str) -> Result<Outcome, SyncError> {
    let host = sioul_core::unsubscribe::host_of(url).filter(|_| url.get(..8).is_some_and(|s| s.eq_ignore_ascii_case("https://"))).ok_or_else(|| SyncError::Message(url.to_string()))?;
    // A fresh agent each time: no cookie kept from anything (ureq is built without them anyway).
    let agent: ureq::Agent = ureq::Agent::config_builder().https_only(true).max_redirects(0).http_status_as_error(false).timeout_global(Some(WAIT)).user_agent("Sioul").build().into();
    post(&agent, url, &host)
}

/// The request, and the redirects within `host` (a browser's way: 303 and the
/// older 301 and 302 continue as a GET, 307 and 308 post again).
fn post(agent: &ureq::Agent, url: &str, host: &str) -> Result<Outcome, SyncError> {
    let mut url = url.to_string();
    let mut posting = true;
    let mut last = 0;
    for _ in 0..=HOPS {
        let answer = if posting {
            agent.post(&url).header("Content-Type", "application/x-www-form-urlencoded").send(sioul_core::unsubscribe::ONE_CLICK)
        } else {
            agent.get(&url).call()
        };
        let response = match answer {
            Ok(response) => response,
            Err(ureq::Error::Timeout(_)) => return Ok(Outcome::TimedOut),
            Err(ureq::Error::Tls(e)) => return Err(SyncError::Tls(e.to_string())),
            Err(e) => return Err(SyncError::Network(e.to_string())),
        };
        last = response.status().as_u16();
        if (200..300).contains(&last) {
            return Ok(Outcome::Done);
        }
        if !matches!(last, 301 | 302 | 303 | 307 | 308) {
            return Ok(Outcome::Refused(last));
        }
        let location = response.headers().get("location").and_then(|v| v.to_str().ok()).unwrap_or("").trim().to_string();
        let next = resolve(&url, &location);
        let same = next.as_deref().and_then(authority_host).is_some_and(|h| h == host) && next.as_deref().is_some_and(|n| scheme(n) == scheme(&url));
        let Some(next) = next.filter(|_| same) else {
            return Ok(Outcome::Elsewhere(resolve(&url, &location).as_deref().and_then(authority_host).unwrap_or(location)));
        };
        posting = posting && matches!(last, 307 | 308);
        url = next;
    }
    Ok(Outcome::Refused(last))
}

/// What the status line or the terminal says of a POST: done, or why not.
pub fn sentence(list: &str, outcome: &Result<Outcome, SyncError>, tr: &Translator) -> String {
    let say = |id: &str, pairs: &[(&str, String)]| {
        let mut args = i18n::args();
        for (key, value) in pairs {
            // A status and a number of seconds go as numbers, as every count does.
            match value.parse::<i32>() {
                Ok(n) if n.to_string() == *value => args.set(key.to_string(), n),
                _ => args.set(key.to_string(), value.clone()),
            }
        }
        tr.text(id, Some(&args))
    };
    let failed = |why: String| say("unsubscribe-failed", &[("list", list.to_string()), ("why", why)]);
    match outcome {
        Ok(Outcome::Done) => say("unsubscribe-done-one-click", &[("list", list.to_string())]),
        Ok(Outcome::Refused(code)) => failed(say("unsubscribe-why-status", &[("code", code.to_string())])),
        Ok(Outcome::Elsewhere(host)) => failed(say("unsubscribe-why-redirect", &[("host", host.clone())])),
        Ok(Outcome::TimedOut) => failed(say("unsubscribe-why-timeout", &[("seconds", WAIT.as_secs().to_string())])),
        Err(SyncError::Tls(detail)) => failed(say("unsubscribe-why-tls", &[("detail", detail.clone())])),
        Err(e) => failed(say("unsubscribe-why-network", &[("detail", e.detail().to_string())])),
    }
}

/// Leaves one of the lists of the mail you called spam: one POST. Only a
/// server that took it writes the list in the record at `record`; anything
/// else leaves it to be tried again another time.
pub fn leave(list: &SpamList, record: &Path) -> Result<Outcome, SyncError> {
    leave_by(list, record, one_click)
}

fn leave_by(list: &SpamList, record: &Path, post: impl FnOnce(&str) -> Result<Outcome, SyncError>) -> Result<Outcome, SyncError> {
    let outcome = post(&list.url)?;
    if outcome == Outcome::Done {
        Record::add(record, list.left(jiff::Timestamp::now().as_second())).map_err(SyncError::Disk)?;
    }
    Ok(outcome)
}

/// What a message offers to leave its list (docs/client.md, "Unsubscribing"),
/// from the Porch's verdict on it: the one place it is decided, so that the
/// Reader's button, the batch and the command line agree. `senders` says who
/// you blocked: the Reader's verdict is made without them, so that their
/// message can still be read, and their list is refused here.
pub fn offer_of(config: &Config, senders: &porch::Senders, triaged: &Triaged) -> Option<Offer> {
    offer_among(config, &config.mail_sources(), senders, triaged)
}

fn offer_among(config: &Config, sources: &[Source], senders: &porch::Senders, triaged: &Triaged) -> Option<Offer> {
    let account = triaged.card.account.as_deref();
    let trusted = sources.iter().find(|s| s.account.as_deref() == account).map_or(&[][..], |s| s.trusted_ids.as_slice());
    let mut reasons = triaged.reasons.clone();
    if senders.who_of(&triaged.card) == sioul_core::reach::Who::Blocked {
        reasons.push(Reason::Blocked);
    }
    let mut offer = core::offer(&triaged.card, trusted, triaged.trust, &triaged.lane, &reasons)?;
    // In the junk: an unverified sender's is spam, answering it tells its sender your
    // address is read; a verified sender's way out stays, the message where it is.
    let file = triaged.card.path.as_deref();
    if file.and_then(|f| mailbox::folder_of(config.account_of(f)?, f)).is_some_and(|f| f.role == Role::Junk) {
        core::in_junk(&mut offer, triaged.trust);
    }
    Some(offer)
}

/// What judging a message reads beside it, once for many.
struct Judge<'a> {
    config: &'a Config,
    sources: Vec<Source>,
    store: Option<sioul_core::projects::ProjectStore>,
    known: porch::KnownSenders,
    senders: porch::Senders,
}

impl<'a> Judge<'a> {
    fn load(config: &'a Config) -> Judge<'a> {
        // The folders as the system resolves them, as the files judged are (a link, macOS's /var).
        let sources = config
            .mail_sources()
            .into_iter()
            .map(|mut source| {
                source.folder = source.folder.canonicalize().unwrap_or(source.folder);
                source
            })
            .collect();
        let ties = || sioul_core::links::LocalLinks::load(&sioul_core::links::LocalLinks::default_path());
        Judge {
            config,
            sources,
            store: config.notes_root_path().and_then(|root| sioul_core::projects::ProjectStore::load(&root).ok()).map(|store| store.with_ties(&ties())),
            known: porch::KnownSenders::load(&config.known_senders_path()),
            senders: porch::Senders::load(config),
        }
    }

    /// The copies at `files`, each judged as the Reader judges it.
    fn copies(&self, files: &[PathBuf]) -> Vec<SaidCopy> {
        let resolved: Vec<PathBuf> = files.iter().map(|file| file.canonicalize().unwrap_or_else(|_| file.clone())).collect();
        let now = jiff::Timestamp::now().as_second();
        porch::judge(&resolved, &self.sources, self.store.as_ref(), &self.known, &porch::Senders::default(), now)
            .into_iter()
            .map(|judged| {
                let way = offer_among(self.config, &self.sources, &self.senders, &judged).map(|offer| offer.way);
                let mut copy = copy_of(&judged.card);
                copy.way = way;
                copy.trust = judged.trust;
                copy
            })
            .collect()
    }
}

/// A message's card as a copy no verdict was asked of: its list, and nothing it offers.
fn copy_of(card: &sioul_core::card::Card) -> SaidCopy {
    let keywords = card.path.as_deref().map(maildir::keywords_of).unwrap_or_default();
    SaidCopy {
        id: card.message_id.as_deref().map(mailindex::bare_id).unwrap_or_default(),
        list: core::list_of(card),
        way: None,
        trust: Trust::Unverified,
        date: card.date.unwrap_or(0),
        address: card.from_address.clone().unwrap_or_default(),
        not_junk: keywords.not_junk && !keywords.junk,
    }
}

/// The Message-IDs of those of `files` (messages you just said are spam) that
/// offer one click from a verified sender. Most spam does not, and then
/// nothing else need be read.
pub fn one_click_among(config: &Config, files: &[PathBuf]) -> BTreeSet<String> {
    Judge::load(config).copies(files).into_iter().filter(|copy| !copy.id.is_empty() && copy.leaves_by().is_some()).map(|copy| copy.id).collect()
}

/// The lists to leave on this device (`unsubscribe::spam_lists`), from `said`
/// (every device's label log, oldest first) and what this device holds: every
/// copy of the messages it names, by the folders' indexes. Only what could be
/// left is judged; a message that names no list is known by its index line,
/// its file not read. With `only`, the messages you called spam that are
/// asked about (a Spam just said), your Not spam counting all the same.
pub fn spam_lists_here(config: &Config, said: &[Entry], record: &Record, only: Option<&BTreeSet<String>>) -> SpamLists {
    let newest = labels::newest_by_message_id(said);
    let words: BTreeMap<&str, Label> = newest.iter().filter(|(id, entry)| entry.label == Label::Ham || only.is_none_or(|o| o.contains(**id))).map(|(id, entry)| (*id, entry.label)).collect();
    if words.is_empty() {
        return SpamLists::default();
    }
    let said: Vec<Entry> = said.iter().filter(|entry| entry.message_id.as_deref().is_some_and(|id| words.contains_key(id))).cloned().collect();
    let _cards = maildir::CardsKept::begin();
    // Every copy of each: `said_spam` holds what a verdict may be asked of.
    let mut copies: Vec<SaidCopy> = Vec::new();
    let mut said_spam: Vec<(SaidCopy, PathBuf)> = Vec::new();
    for root in mailindex::MailIndex::roots_of(config) {
        for local in mailcache::folders(&root.path) {
            for (path, line) in mailcache::folder(&root.account, &root.path, &local) {
                let Some(word) = line.message_id.as_deref().map(mailindex::bare_id).and_then(|id| words.get(id.as_str()).copied()) else { continue };
                let card = if line.is_list { maildir::read_one(&path) } else { Some(line.card(&path)) };
                let Some(copy) = card.as_ref().map(copy_of) else { continue };
                if word == Label::Spam && line.is_list && !copy.not_junk {
                    said_spam.push((copy, path));
                } else {
                    copies.push(copy);
                }
            }
        }
    }
    // A list out of the question (left, or a Not spam of yours among its mail): its mail is not judged.
    let out: BTreeSet<String> = copies.iter().filter(|copy| copy.not_junk || words.get(copy.id.as_str()) == Some(&Label::Ham)).map(|copy| copy.list.key.clone()).collect();
    let (out, judged): (Vec<_>, Vec<_>) = said_spam.into_iter().partition(|(copy, _)| out.contains(&copy.list.key) || record.find(&copy.list.key).is_some());
    copies.extend(out.into_iter().map(|(copy, _)| copy));
    if !judged.is_empty() {
        let files: Vec<PathBuf> = judged.iter().map(|(_, path)| path.clone()).collect();
        let verdicts = Judge::load(config).copies(&files);
        // A copy with no verdict (it went meanwhile) stays as it was, offering nothing.
        if verdicts.len() == judged.len() {
            copies.extend(verdicts);
        } else {
            copies.extend(judged.into_iter().map(|(copy, _)| copy));
        }
    }
    core::spam_lists(&said, &copies, record)
}

/// "https", "http": the scheme, lower case.
fn scheme(url: &str) -> String {
    url.split_once("://").map(|(s, _)| s.to_ascii_lowercase()).unwrap_or_default()
}

/// The host of an absolute address, lower case, without its port.
fn authority_host(url: &str) -> Option<String> {
    let rest = url.split_once("://")?.1;
    let authority = rest.split(['/', '?', '#']).next()?;
    let authority = authority.rsplit_once('@').map_or(authority, |(_, host)| host);
    let host = authority.rsplit_once(':').filter(|(_, port)| port.chars().all(|c| c.is_ascii_digit())).map_or(authority, |(host, _)| host);
    (!host.is_empty()).then(|| host.trim_end_matches('.').to_ascii_lowercase())
}

/// Where a `Location` points, from `base`: an address of its own, one
/// without its scheme ("//host/x"), or a path on the same site.
fn resolve(base: &str, location: &str) -> Option<String> {
    if location.is_empty() {
        return None;
    }
    if location.contains("://") {
        return Some(location.to_string());
    }
    let (scheme, rest) = base.split_once("://")?;
    if let Some(other) = location.strip_prefix("//") {
        return Some(format!("{scheme}://{other}"));
    }
    let authority = rest.split(['/', '?', '#']).next()?;
    if location.starts_with('/') {
        return Some(format!("{scheme}://{authority}{location}"));
    }
    let path = rest[authority.len()..].split(['?', '#']).next().unwrap_or("");
    let folder = path.rsplit_once('/').map_or("", |(folder, _)| folder);
    Some(format!("{scheme}://{authority}{folder}/{location}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;

    /// What a stand-in server received: the request line, the headers (lower case), the body.
    #[derive(Debug, Default, Clone)]
    struct Seen {
        line: String,
        headers: Vec<(String, String)>,
        body: String,
    }

    /// A server on this computer that answers each request with the next of `answers`, and says what it got.
    fn stand_in(answers: Vec<String>) -> (String, std::sync::mpsc::Receiver<Seen>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        let (tell, heard) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for answer in answers {
                let Ok((stream, _)) = listener.accept() else { return };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut seen = Seen::default();
                reader.read_line(&mut seen.line).unwrap();
                let mut length = 0;
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    let line = line.trim_end().to_string();
                    if line.is_empty() {
                        break;
                    }
                    if let Some((name, value)) = line.split_once(':') {
                        let (name, value) = (name.trim().to_ascii_lowercase(), value.trim().to_string());
                        if name == "content-length" {
                            length = value.parse().unwrap_or(0);
                        }
                        seen.headers.push((name, value));
                    }
                }
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                seen.body = String::from_utf8_lossy(&body).into_owned();
                let _ = tell.send(seen);
                let mut stream = stream;
                let _ = stream.write_all(answer.as_bytes());
            }
        });
        (address, heard)
    }

    fn agent() -> ureq::Agent {
        ureq::Agent::config_builder().max_redirects(0).http_status_as_error(false).timeout_global(Some(Duration::from_secs(5))).user_agent("Sioul").build().into()
    }

    const OK: &str = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok";

    #[test]
    fn posts_the_one_click_form_and_nothing_else() {
        let (base, heard) = stand_in(vec![OK.to_string()]);
        let outcome = post(&agent(), &format!("{base}/u/a1b2?l=112"), "127.0.0.1").unwrap();
        assert_eq!(outcome, Outcome::Done);
        let seen = heard.recv().unwrap();
        assert_eq!(seen.line.trim_end(), "POST /u/a1b2?l=112 HTTP/1.1");
        assert_eq!(seen.body, "List-Unsubscribe=One-Click");
        let header = |name: &str| seen.headers.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str());
        assert_eq!(header("content-type"), Some("application/x-www-form-urlencoded"));
        // No cookie, no authorisation, nothing about you (RFC 8058 §4).
        assert_eq!((header("cookie"), header("authorization"), header("referer")), (None, None, None));
    }

    #[test]
    fn a_redirect_to_another_site_is_not_followed() {
        let elsewhere = "HTTP/1.1 302 Found\r\nLocation: https://tracker.example/landing\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string();
        let (base, heard) = stand_in(vec![elsewhere]);
        assert_eq!(post(&agent(), &format!("{base}/u"), "127.0.0.1").unwrap(), Outcome::Elsewhere("tracker.example".into()));
        assert_eq!(heard.recv().unwrap().body, "List-Unsubscribe=One-Click");
        // Within the same site: followed, a 307 posting again.
        let again = "HTTP/1.1 307 Temporary Redirect\r\nLocation: /u/confirm\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string();
        let (base, heard) = stand_in(vec![again, OK.to_string()]);
        assert_eq!(post(&agent(), &format!("{base}/u"), "127.0.0.1").unwrap(), Outcome::Done);
        let (first, second) = (heard.recv().unwrap(), heard.recv().unwrap());
        assert!(first.line.starts_with("POST /u ") && second.line.starts_with("POST /u/confirm "), "{first:?} {second:?}");
        // A refusal is said with its status.
        let gone = "HTTP/1.1 410 Gone\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string();
        let (base, _heard) = stand_in(vec![gone]);
        assert_eq!(post(&agent(), &format!("{base}/u"), "127.0.0.1").unwrap(), Outcome::Refused(410));
    }

    fn a_list() -> SpamList {
        SpamList { list: sioul_core::unsubscribe::List { key: "list-id:issues.letters.example".into(), name: "Type & Pixels".into() }, url: "https://letters.example/u/a1".into(), address: "letter@letters.example".into(), message_id: "m1@letters.example".into() }
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sioul-leave-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Only a server that took the request writes the list in the record: its
    /// 404, another site, no answer, a network that fails, all stay to be
    /// tried again; each said with the Reader's own sentence.
    #[test]
    fn only_done_is_recorded() {
        let dir = scratch("record");
        let record = dir.join("unsubscribed.toml");
        let list = a_list();
        let tr = Translator::new("en");
        let by = |base: String, heard: std::sync::mpsc::Receiver<Seen>, agent: ureq::Agent| {
            let outcome = leave_by(&list, &record, |_| post(&agent, &format!("{base}/u/a1"), "127.0.0.1"));
            (outcome, heard)
        };
        // Its server answers 404.
        let (base, heard) = stand_in(vec!["HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string()]);
        let (outcome, heard) = by(base, heard, agent());
        assert_eq!(*outcome.as_ref().unwrap(), Outcome::Refused(404));
        assert_eq!(heard.recv().unwrap().body, "List-Unsubscribe=One-Click");
        assert!(sentence(&list.list.name, &outcome, &tr).contains("404"));
        assert!(Record::load(&record).lists.is_empty());
        // It sends Sioul to another site.
        let (base, heard) = stand_in(vec!["HTTP/1.1 302 Found\r\nLocation: https://tracker.example/landing\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string()]);
        let (outcome, _heard) = by(base, heard, agent());
        assert_eq!(*outcome.as_ref().unwrap(), Outcome::Elsewhere("tracker.example".into()));
        assert!(sentence(&list.list.name, &outcome, &tr).contains("tracker.example"));
        assert!(Record::load(&record).lists.is_empty());
        // It takes the request and never answers.
        let silent = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", silent.local_addr().unwrap());
        let brief: ureq::Agent = ureq::Agent::config_builder().max_redirects(0).http_status_as_error(false).timeout_global(Some(Duration::from_millis(300))).user_agent("Sioul").build().into();
        let outcome = leave_by(&list, &record, |_| post(&brief, &format!("{base}/u/a1"), "127.0.0.1"));
        assert_eq!(*outcome.as_ref().unwrap(), Outcome::TimedOut);
        assert!(sentence(&list.list.name, &outcome, &tr).contains("15"));
        assert!(Record::load(&record).lists.is_empty());
        drop(silent);
        // Nobody there.
        let outcome = leave_by(&list, &record, |_| Err(SyncError::Network("refused".into())));
        assert!(sentence(&list.list.name, &outcome, &tr).contains("refused"));
        assert!(Record::load(&record).lists.is_empty());
        // It takes it: written, with its way.
        let (base, heard) = stand_in(vec![OK.to_string()]);
        let (outcome, _heard) = by(base, heard, agent());
        assert_eq!(outcome.unwrap(), Outcome::Done);
        let kept = Record::load(&record);
        assert_eq!(kept.lists.iter().map(|l| (l.key.as_str(), l.name.as_str(), l.address.as_str(), l.way.as_str())).collect::<Vec<_>>(), [("list-id:issues.letters.example", "Type & Pixels", "letter@letters.example", "one-click")]);
        assert_eq!(sentence("Type & Pixels", &Ok(Outcome::Done), &tr), "Unsubscribed from Type & Pixels.");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A stand-in address with an inbox and a Junk folder, in the test's own folders.
    struct Stand {
        config: Config,
        root: PathBuf,
        id: String,
    }

    impl Stand {
        fn new(name: &str) -> Stand {
            sioul_core::config::stand_ins_only();
            let id = format!("leave-{name}-{}", std::process::id());
            let root = sioul_core::config::data_dir().join(&id);
            let _ = std::fs::remove_dir_all(&root);
            let config: Config = toml::from_str(&format!("[[account]]\nid = \"{id}\"\nkind = \"imap\"\naddress = \"noa@home.example\"\nhost = \"imap.example.org\"\nmaildir = \"{}\"\ntrusted_authserv_ids = [\"mx.example.org\"]\n", root.display())).unwrap();
            let folders = vec![sioul_core::folders::folder("INBOX", Some("/"), Some(Role::Inbox)), sioul_core::folders::folder("Junk", Some("/"), Some(Role::Junk))];
            let saved: String = folders.iter().map(|f| format!("[[folder]]\nname = \"{}\"\ndisplay = \"{}\"\nrole = \"{}\"\nlocal = \"{}\"\n", f.name, f.display, serde_json::to_value(f.role).unwrap().as_str().unwrap(), f.local)).collect();
            std::fs::create_dir_all(sioul_core::folders::saved_path(&id).parent().unwrap()).unwrap();
            std::fs::write(sioul_core::folders::saved_path(&id), saved).unwrap();
            for f in &folders {
                for sub in ["tmp", "new", "cur"] {
                    std::fs::create_dir_all(root.join(&f.local).join(sub)).unwrap();
                }
            }
            Stand { config, root, id }
        }

        /// A message stored in `local` ("" for the inbox).
        fn put(&self, local: &str, uid: u32, flags: &str, raw: &str) -> PathBuf {
            let file = self.root.join(local).join("cur").join(format!("17770000{uid:02}.U7-{uid}.sioul{}2,{flags}", maildir::INFO));
            std::fs::write(&file, raw.replace('\n', "\r\n")).unwrap();
            file
        }

        fn word(&self, id: &str, at: i64, source: labels::Source) -> Entry {
            Entry { at, account: self.id.clone(), folder: "INBOX".into(), uidvalidity: 7, uid: 1, message_id: Some(id.to_string()), label: source.label(), source }
        }
    }

    impl Drop for Stand {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    /// A newsletter from `domain`, its results written by the account's provider,
    /// with `dmarc` as its result, both headers signed by `signer`.
    fn letter(id: &str, domain: &str, signer: &str, dmarc: &str, date: &str, url: &str) -> String {
        format!(
            "Authentication-Results: mx.example.org; dkim=pass header.d={signer} header.s=s1 header.b=QUJD; spf=pass smtp.mailfrom={signer}; dmarc={dmarc} header.from={domain}\n\
             DKIM-Signature: v=1; a=rsa-sha256; d={signer}; s=s1; h=from:to:subject:date:list-unsubscribe:list-unsubscribe-post; bh=Zm9v; b=QUJDREVG\n\
             Message-ID: <{id}>\nFrom: Letters <news@{domain}>\nTo: noa@home.example\nDate: {date}\nSubject: Issue\nList-Id: Letters <issues.{domain}>\n\
             List-Unsubscribe: <{url}>\nList-Unsubscribe-Post: List-Unsubscribe=One-Click\n\nThis week.\n"
        )
    }

    /// The rule on messages stored here, each judged as the Reader judges it:
    /// verified in the junk or the inbox, one POST to the newest; your word
    /// alone counts, as it stands now; an unverified sender whose host alone
    /// is aligned, a copy gone, a blocked sender and a list said not spam are not left.
    #[test]
    fn what_this_device_holds_answers_the_rule() {
        let stand = Stand::new("rule");
        const OLD: &str = "Mon, 26 Oct 2026 09:00:00 +0100";
        const NEW: &str = "Tue, 27 Oct 2026 09:00:00 +0100";
        // A list with two messages, in the junk and the inbox.
        stand.put(".Junk", 1, "S", &letter("one@letters.example", "letters.example", "letters.example", "pass", OLD, "https://letters.example/u/one"));
        stand.put("", 2, "S", &letter("two@letters.example", "letters.example", "letters.example", "pass", NEW, "https://letters.example/u/two"));
        // In the junk with no word of yours.
        stand.put(".Junk", 3, "S", &letter("silent@quiet.example", "quiet.example", "quiet.example", "pass", NEW, "https://quiet.example/u"));
        // Nothing proves its sender; only the address's host signed.
        stand.put("", 4, "S", &letter("host@borrowed.example", "borrowed.example", "mailer.example", "none", NEW, "https://unsub.mailer.example/u"));
        // A list with a message you said is not spam, and one you said is.
        stand.put("", 5, "S", &letter("kept@kept.example", "kept.example", "kept.example", "pass", NEW, "https://kept.example/u/1"));
        stand.put("", 6, "S", &letter("spam@kept.example", "kept.example", "kept.example", "pass", NEW, "https://kept.example/u/2"));
        // No way out at all: known by its index line.
        stand.put("", 7, "S", &format!("Message-ID: <plain@person.example>\nFrom: Paul <paul@person.example>\nTo: noa@home.example\nDate: {NEW}\nSubject: Hello\n\nHi.\n"));
        use labels::Source::{Block, Junk, NotSpam};
        let said = vec![
            stand.word("one@letters.example", 100, Junk),
            stand.word("two@letters.example", 110, Junk),
            stand.word("host@borrowed.example", 120, Junk),
            stand.word("kept@kept.example", 130, NotSpam),
            stand.word("spam@kept.example", 140, Junk),
            stand.word("plain@person.example", 150, Block),
            stand.word("gone@elsewhere.example", 160, Junk),
        ];
        let found = spam_lists_here(&stand.config, &said, &Record::default(), None);
        assert_eq!(found.lists.iter().map(|l| (l.list.key.as_str(), l.url.as_str(), l.domain())).collect::<Vec<_>>(), [("list-id:issues.letters.example", "https://letters.example/u/two", "letters.example")]);
        assert_eq!(found.not, core::NotLeft { not_one_click: 1, unverified: 1, refused: 0, gone: 1, left: 0, said_not_spam: 1 });
        // Not spam said since, on another device: the list stays.
        let mut changed = said.clone();
        changed.push(stand.word("two@letters.example", 200, NotSpam));
        let found = spam_lists_here(&stand.config, &changed, &Record::default(), None);
        assert!(found.lists.is_empty() && found.not.said_not_spam == 2, "{found:?}");
        // Left already.
        let record = Record { lists: vec![core::Left { key: "list-id:issues.letters.example".into(), name: "Letters".into(), address: String::new(), way: "one-click".into(), at: 1 }] };
        let found = spam_lists_here(&stand.config, &said, &record, None);
        assert!(found.lists.is_empty() && found.not.left == 1, "{found:?}");
        // Only the message you just said: its list and its address alone.
        let only: BTreeSet<String> = ["one@letters.example".to_string()].into();
        let found = spam_lists_here(&stand.config, &said, &Record::default(), Some(&only));
        assert_eq!(found.lists.iter().map(|l| l.url.as_str()).collect::<Vec<_>>(), ["https://letters.example/u/one"]);
        // Asked of the files themselves: proven and one click, the rest not.
        let files = [stand.root.join("cur").join(format!("1777000002.U7-2.sioul{}2,S", maildir::INFO)), stand.root.join("cur").join(format!("1777000004.U7-4.sioul{}2,S", maildir::INFO)), stand.root.join("cur").join(format!("1777000007.U7-7.sioul{}2,S", maildir::INFO))];
        assert_eq!(one_click_among(&stand.config, &files), BTreeSet::from(["two@letters.example".to_string()]));
    }

    /// A blocked sender's mail, proven and with one click, stays out, by the
    /// one reading the Reader's button uses (`offer_of`), though the verdict is
    /// made without who you blocked; `$NotJunk` on a copy, said in another
    /// mail client, takes your Spam back.
    #[test]
    fn blocked_and_said_elsewhere_stay_out() {
        let mut stand = Stand::new("blocked");
        const DATE: &str = "Tue, 27 Oct 2026 09:00:00 +0100";
        stand.put("", 1, "S", &letter("a@blocked.example", "blocked.example", "blocked.example", "pass", DATE, "https://blocked.example/u"));
        stand.put("", 2, format!("S{}", maildir::NOT_JUNK).as_str(), &letter("b@changed.example", "changed.example", "changed.example", "pass", DATE, "https://changed.example/u"));
        let blocked = sioul_core::config::config_dir().join(format!("blocked-{}.txt", stand.id));
        std::fs::create_dir_all(blocked.parent().unwrap()).unwrap();
        std::fs::write(&blocked, "*@blocked.example\n").unwrap();
        stand.config.blocked_senders = Some(blocked.display().to_string());
        let said = vec![stand.word("a@blocked.example", 100, labels::Source::Block), stand.word("b@changed.example", 110, labels::Source::Junk)];
        let found = spam_lists_here(&stand.config, &said, &Record::default(), None);
        assert!(found.lists.is_empty(), "{found:?}");
        assert_eq!(found.not, core::NotLeft { refused: 1, ..Default::default() }, "the second, your Spam taken back, is no mail said spam");
        // The Reader's offer on the same message, its verdict as the Reader makes it: refused, by the same path.
        let judge = Judge::load(&stand.config);
        let file = stand.root.join("cur").join(format!("1777000001.U7-1.sioul{}2,S", maildir::INFO)).canonicalize().unwrap();
        let verdict = porch::judge(std::slice::from_ref(&file), &judge.sources, None, &judge.known, &porch::Senders::default(), 1_790_000_000).into_iter().next().unwrap();
        assert!(!verdict.reasons.contains(&Reason::Blocked), "the verdict knows nobody blocked");
        assert_eq!(offer_of(&stand.config, &judge.senders, &verdict).unwrap().way, Err(core::Refusal::Blocked));
        // Off the list: one click again, for the Reader and for the batch alike.
        std::fs::write(&blocked, "").unwrap();
        let senders = porch::Senders::load(&stand.config);
        assert_eq!(offer_of(&stand.config, &senders, &verdict).unwrap().way, Ok(core::Way::OneClick { url: "https://blocked.example/u".into() }));
        assert_eq!(spam_lists_here(&stand.config, &said, &Record::default(), None).lists.len(), 1);
        let _ = std::fs::remove_file(blocked);
    }

    #[test]
    fn only_https_to_a_name_on_the_internet() {
        for url in ["http://letters.example/u", "https://127.0.0.1/u", "https://router.lan/u", "https://letters.example@evil.example/u"] {
            assert!(matches!(one_click(url), Err(SyncError::Message(_))), "{url}");
        }
        assert_eq!(resolve("https://a.example/u/x?y=1", "done"), Some("https://a.example/u/done".into()));
        assert_eq!(resolve("https://a.example/u", "//b.example/z"), Some("https://b.example/z".into()));
        assert_eq!(authority_host("https://user@A.Example:8443/x"), Some("a.example".into()));
    }
}
