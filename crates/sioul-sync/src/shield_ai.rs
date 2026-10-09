// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The AI reading of a shielded address (docs/shield.md). Only when you allow
//! it for that address: the subject and text of each new message in its inbox
//! are sent once to Anthropic's API, with the question of `shield::ai_prompt`,
//! and the answer (tone, topic, one neutral line) is kept by Message-ID. The
//! key lives in the system keyring, under the service "sioul".

use crate::SyncError;
use sioul_core::config::Config;
use sioul_core::shield::{self, AiCache};
use std::time::Duration;

/// A small, fast model: the question is short, the answer one line.
pub const MODEL: &str = "claude-haiku-4-5-20251001";
const ENDPOINT: &str = "https://api.anthropic.com/v1/messages";
/// Messages read per address and round; a burst of mail is read over the next rounds.
const PER_ROUND: usize = 20;

fn key_entry() -> Result<keyring::Entry, SyncError> {
    keyring::Entry::new("sioul", "Anthropic API key").map_err(|e| SyncError::Keyring(e.to_string()))
}

/// The key kept for Anthropic's API, if any.
pub fn api_key() -> Option<String> {
    key_entry().ok()?.get_password().ok().filter(|k| !k.trim().is_empty())
}

/// Keeps the key, replacing any older one; an empty key forgets it.
pub fn save_api_key(key: &str) -> Result<(), SyncError> {
    let entry = key_entry()?;
    if key.trim().is_empty() {
        return match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(SyncError::Keyring(e.to_string())),
        };
    }
    entry.set_password(key.trim()).map_err(|e| SyncError::Keyring(e.to_string()))
}

/// What a round of reading did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Read {
    /// Messages read and kept.
    pub read: usize,
    /// Messages still to read, for the next rounds.
    pub left: usize,
}

/// Reads the new mail of every shielded address that allows the AI, up to
/// `PER_ROUND` messages each, newest first. Nothing is sent without a key.
pub fn read_new(config: &Config) -> Result<Read, SyncError> {
    if !config.accounts.iter().any(|a| a.shield && a.shield_ai) {
        return Ok(Read::default());
    }
    let Some(key) = api_key() else { return Ok(Read::default()) };
    read_with(config, ENDPOINT, &key)
}

/// `read_new`, its questions sent to `endpoint` with `key`.
fn read_with(config: &Config, endpoint: &str, key: &str) -> Result<Read, SyncError> {
    let accounts: Vec<_> = config.accounts.iter().filter(|a| a.shield && a.shield_ai).collect();
    // No redirect followed: the key is a header of its own, which would go along to any other address.
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(60))).http_status_as_error(false).max_redirects(0).build().into();
    let mut done = Read::default();
    for account in accounts {
        let mut cache = AiCache::load(&account.id);
        // Neither answered, nor sent twice for answers not in the form asked (`AiCache::wants`).
        let mut waiting: Vec<_> = sioul_core::maildir::read_messages(&account.maildir_path())
            .into_iter()
            .filter_map(|card| Some((shield::ai_key(&card)?, card)))
            .filter(|(id, _)| cache.wants(id))
            .collect();
        waiting.sort_by_key(|(_, card)| std::cmp::Reverse(card.date.unwrap_or(0)));
        done.left += waiting.len().saturating_sub(PER_ROUND);
        for (id, card) in waiting.into_iter().take(PER_ROUND) {
            match ask(&agent, endpoint, key, &shield::ai_prompt(&card.subject, &card.excerpt)) {
                Ok(Some(assessment)) => {
                    cache.malformed.remove(&id);
                    cache.messages.insert(id, assessment);
                    // Kept at once: an answer is never asked for twice.
                    cache.save(&account.id).map_err(SyncError::Disk)?;
                    done.read += 1;
                }
                // An answer not in the form asked: the word lists keep judging
                // that one. Marked, kept at once: asked once more at most.
                Ok(None) => {
                    *cache.malformed.entry(id).or_default() += 1;
                    cache.save(&account.id).map_err(SyncError::Disk)?;
                }
                Err(e) => return Err(e),
            }
        }
    }
    Ok(done)
}

/// One question; None when the answer is not the JSON asked for.
fn ask(agent: &ureq::Agent, endpoint: &str, key: &str, prompt: &str) -> Result<Option<shield::Assessment>, SyncError> {
    let body = serde_json::json!({
        "model": MODEL,
        "max_tokens": 200,
        "messages": [{ "role": "user", "content": prompt }],
    });
    let mut response = agent
        .post(endpoint)
        .header("x-api-key", key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .header("User-Agent", "Sioul")
        .send(body.to_string())
        .map_err(|e| SyncError::Network(e.to_string()))?;
    let status = response.status().as_u16();
    let text = response.body_mut().read_to_string().unwrap_or_default();
    match status {
        200 => {}
        401 | 403 => return Err(SyncError::Login(format!("Anthropic API: {status}"))),
        // Busy or rate-limited: the next round asks again.
        _ => return Err(SyncError::Server(format!("Anthropic API: {status}"))),
    }
    let answer: serde_json::Value = serde_json::from_str(&text).map_err(|e| SyncError::Server(e.to_string()))?;
    let said: String = answer["content"].as_array().into_iter().flatten().filter_map(|part| part["text"].as_str()).collect();
    Ok(shield::read_ai_answer(&said))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read as _, Write as _};

    /// A one-shot server that answers `status` and `body`, and hands back the request it got.
    fn stand_in(status: u16, body: &'static str) -> (String, std::thread::JoinHandle<String>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/v1/messages", listener.local_addr().unwrap());
        let served = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut request = Vec::new();
            let mut chunk = [0u8; 4096];
            // Headers, then as many bytes as Content-Length says.
            loop {
                let n = stream.read(&mut chunk).unwrap();
                request.extend_from_slice(&chunk[..n]);
                let text = String::from_utf8_lossy(&request).to_string();
                if let Some(end) = text.find("\r\n\r\n") {
                    let length = text.lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length: ").map(|v| v.trim().parse::<usize>().unwrap())).unwrap_or(0);
                    if request.len() >= end + 4 + length {
                        break;
                    }
                }
                if n == 0 {
                    break;
                }
            }
            write!(stream, "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            String::from_utf8_lossy(&request).to_string()
        });
        (url, served)
    }

    fn agent() -> ureq::Agent {
        ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(10))).http_status_as_error(false).build().into()
    }

    #[test]
    fn an_answer_is_read() {
        let (url, served) = stand_in(200, r#"{"id":"msg_1","type":"message","role":"assistant","content":[{"type":"text","text":"{\"tone\": \"calm\", \"topic\": \"work\", \"summary\": \"A three-month mission, asks your rate.\"}"}],"model":"claude-haiku-4-5-20251001","stop_reason":"end_turn"}"#);
        let assessment = ask(&agent(), &url, "test-key", "Subject: Mission").unwrap().unwrap();
        assert_eq!((assessment.tone, assessment.topic), (shield::Tone::Calm, shield::Topic::Work));
        assert_eq!(assessment.summary, "A three-month mission, asks your rate.");
        assert!(assessment.by_ai);
        let request = served.join().unwrap();
        assert!(request.contains("x-api-key: test-key"), "{request}");
        assert!(request.contains("anthropic-version: 2023-06-01"));
        assert!(request.contains(MODEL));
    }

    /// A stand-in that answers every request with `body`, `times` requests at
    /// most, and counts them.
    fn stand_in_counting(body: &'static str, times: usize) -> (String, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/v1/messages", listener.local_addr().unwrap());
        let count = std::sync::Arc::new(AtomicUsize::new(0));
        let counted = std::sync::Arc::clone(&count);
        std::thread::spawn(move || {
            for _ in 0..times {
                let Ok((mut stream, _)) = listener.accept() else { return };
                stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
                let mut request = Vec::new();
                let mut chunk = [0u8; 4096];
                loop {
                    let n = stream.read(&mut chunk).unwrap_or(0);
                    request.extend_from_slice(&chunk[..n]);
                    let text = String::from_utf8_lossy(&request).to_string();
                    let length = text.lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length: ").map(|v| v.trim().parse::<usize>().unwrap())).unwrap_or(0);
                    if n == 0 || text.find("\r\n\r\n").is_some_and(|end| request.len() >= end + 4 + length) {
                        break;
                    }
                }
                counted.fetch_add(1, Ordering::SeqCst);
                let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            }
        });
        (url, count)
    }

    /// An answer not in the form asked is marked: the message is sent once
    /// more at most, never at every round. A stand-in answers; nothing goes to Anthropic.
    #[test]
    fn a_malformed_answer_is_asked_once_more_at_most() {
        use std::sync::atomic::Ordering;
        let home = crate::dav::stand_in::home();
        let mut account = sioul_core::config::Account::imap("shield-ai-tester", "public@example.org", "localhost", 993, sioul_core::config::Security::Tls, None);
        account.shield = true;
        account.shield_ai = true;
        let mail = home.join(format!("mail-shield-ai-{}", std::process::id()));
        account.maildir = Some(mail.display().to_string());
        std::fs::create_dir_all(mail.join("cur")).unwrap();
        std::fs::write(mail.join("cur").join(format!("1759300001.U1-1.test{}2,S", sioul_core::maildir::INFO)), "From: Someone <someone@example.net>\r\nTo: public@example.org\r\nSubject: About your software\r\nMessage-ID: <malformed-1@example.net>\r\nDate: Thu, 01 Oct 2026 10:00:00 +0200\r\n\r\nA question about your software.\r\n").unwrap();
        let mut config = Config::default();
        config.accounts = vec![account.clone()];
        let (url, asked) = stand_in_counting(r#"{"content":[{"type":"text","text":"I cannot help with that."}]}"#, 3);
        for _ in 0..4 {
            assert_eq!(read_with(&config, &url, "test-key").unwrap(), Read { read: 0, left: 0 });
        }
        assert_eq!(asked.load(Ordering::SeqCst), AiCache::TIMES as usize, "sent twice, then never again");
        let cache = AiCache::load(&account.id);
        assert!(cache.messages.is_empty() && cache.malformed.values().eq([&AiCache::TIMES]), "{cache:?}");
        // An answer in the form asked replaces the mark.
        let mut kept = cache.clone();
        kept.malformed.clear();
        kept.save(&account.id).unwrap();
        let (url, asked) = stand_in_counting(r#"{"content":[{"type":"text","text":"{\"tone\": \"calm\", \"topic\": \"support\", \"summary\": \"Asks about the software.\"}"}]}"#, 2);
        assert_eq!(read_with(&config, &url, "test-key").unwrap().read, 1);
        let cache = AiCache::load(&account.id);
        assert!(cache.malformed.is_empty() && cache.messages.len() == 1 && asked.load(Ordering::SeqCst) == 1, "{cache:?}");
        let _ = std::fs::remove_dir_all(&mail);
    }

    #[test]
    fn a_refused_key_stops() {
        let (url, served) = stand_in(401, r#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}"#);
        assert!(matches!(ask(&agent(), &url, "bad", "Subject: x"), Err(SyncError::Login(_))));
        served.join().unwrap();
        // Another form of answer: left to the word lists.
        let (url, served) = stand_in(200, r#"{"content":[{"type":"text","text":"I cannot help with that."}]}"#);
        assert_eq!(ask(&agent(), &url, "k", "Subject: x").unwrap(), None);
        served.join().unwrap();
    }
}
