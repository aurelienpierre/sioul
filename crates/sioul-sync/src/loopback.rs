// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The door a browser comes back to after a sign-in on a provider's page, as
//! native apps should have it (OAuth 2.0 for native apps, RFC 8252 §7.3): a
//! port of this device's loopback address, chosen by the system, open for one
//! sign-in. Google's sign-in (`google.rs`) and Microsoft's (`microsoft.rs`)
//! share it; each provider says what an answer means (`Verdict`) and which
//! page the browser then shows, in your language.
//!
//! Nothing but the request line is read: `GET /?state=…&code=… HTTP/1.1`.
//! A request without a query (a favicon) gets a 404 and the wait goes on.
//! The state is compared in constant time. An answer that waits in the
//! system's queue is read even once the time is up: a phone freezes Sioul
//! while the browser is in front, and its clock then jumps.
//!
//! Also here, what both providers' requests need: PKCE's verifier and
//! challenge (RFC 7636), a random state, and the form encoding of a query.

use crate::SyncError;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Opens the door: `127.0.0.1` on a port the system chooses; else, when
/// `ipv6` allows it, `[::1]` (Google takes either; Microsoft takes no IPv6
/// loopback). The listener, and the redirect address the provider is given
/// (no path: `http://127.0.0.1:<port>`).
pub(crate) fn open(ipv6: bool) -> Result<(TcpListener, String), SyncError> {
    let bound = TcpListener::bind("127.0.0.1:0");
    let listener = match bound {
        Ok(listener) => listener,
        Err(_) if ipv6 => TcpListener::bind("[::1]:0").map_err(|e| SyncError::Network(e.to_string()))?,
        Err(e) => return Err(SyncError::Network(e.to_string())),
    };
    let address = listener.local_addr().map_err(|e| SyncError::Network(e.to_string()))?;
    let redirect = if address.is_ipv6() { format!("http://[::1]:{}", address.port()) } else { format!("http://127.0.0.1:{}", address.port()) };
    Ok((listener, redirect))
}

/// Random bytes (version 4 UUIDs' randomness), as URL-safe base64 without padding.
pub(crate) fn random(bytes: usize) -> String {
    let mut out = Vec::with_capacity(bytes);
    while out.len() < bytes {
        out.extend_from_slice(uuid::Uuid::new_v4().as_bytes());
    }
    out.truncate(bytes);
    URL_SAFE_NO_PAD.encode(out)
}

/// PKCE's challenge for a verifier: SHA-256, as URL-safe base64 without padding (S256).
pub(crate) fn challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

/// `key=value&…`, each value percent-encoded.
pub(crate) fn query(pairs: &[(&str, &str)]) -> String {
    pairs.iter().map(|(k, v)| format!("{k}={}", encode(v))).collect::<Vec<_>>().join("&")
}

/// Percent-encodes a value: everything but RFC 3986's unreserved characters.
pub(crate) fn encode(text: &str) -> String {
    text.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

/// A query's value as it was before being encoded (`+` a space).
pub(crate) fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok());
                match hex {
                    Some(b) => {
                        out.push(b);
                        i += 3;
                    }
                    None => {
                        out.push(b'%');
                        i += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).to_string()
}

/// One answer at the door: its query's values, and whether its `state` is
/// the sign-in's (compared in constant time; false when it has none).
pub(crate) struct Answer {
    pub params: HashMap<String, String>,
    pub state_ok: bool,
}

impl Answer {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.params.get(key).map(String::as_str)
    }
}

/// What a provider makes of an answer: the page the browser shows (a
/// message id), and either the wait's outcome or nothing (the wait goes on).
pub(crate) enum Verdict {
    /// This sign-in's answer, or one that ends it: the wait is over.
    Done(&'static str, Result<String, SyncError>),
    /// Not this sign-in's: said in the browser, and the wait goes on.
    Ignored(&'static str),
}

/// How a wait ends without an answer: Cancel pressed, or the time up.
pub(crate) struct Ends {
    pub cancelled: SyncError,
    pub timed_out: SyncError,
}

/// Waits at the door for the provider's answer, at most `deadline`, or until
/// `stop` (Cancel): `judge` says what each answer means and which page the
/// browser shows. The code, else why there is none.
pub(crate) fn wait(listener: &TcpListener, state: &str, deadline: Duration, stop: &AtomicBool, ends: Ends, judge: impl Fn(&Answer) -> Verdict) -> Result<String, SyncError> {
    listener.set_nonblocking(true).map_err(|e| SyncError::Network(e.to_string()))?;
    let until = Instant::now() + deadline;
    loop {
        if stop.load(Ordering::Relaxed) {
            return Err(ends.cancelled);
        }
        // An answer waiting is read before the time is called: a phone
        // freezes Sioul while the browser is in front (its clock then jumps),
        // and the browser's request waits meanwhile in the system's queue.
        let (mut stream, _) = match listener.accept() {
            Ok(accepted) => accepted,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() > until {
                    return Err(ends.timed_out);
                }
                std::thread::sleep(Duration::from_millis(200));
                continue;
            }
            Err(e) => return Err(SyncError::Network(e.to_string())),
        };
        let _ = stream.set_nonblocking(false);
        let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
        let mut line = String::new();
        if BufReader::new(&stream).read_line(&mut line).is_err() {
            continue;
        }
        // "GET /?state=…&code=…&scope=… HTTP/1.1"; anything else (a favicon) is not it.
        let target = line.split_whitespace().nth(1).unwrap_or("");
        let Some(params) = target.strip_prefix("/?") else {
            let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
            continue;
        };
        let params: HashMap<String, String> = params.split('&').filter_map(|p| p.split_once('=')).map(|(k, v)| (k.to_string(), decode(v))).collect();
        let state_ok = params.get("state").is_some_and(|said| subtle::ConstantTimeEq::ct_eq(said.as_bytes(), state.as_bytes()).into());
        let (said, outcome) = match judge(&Answer { params, state_ok }) {
            Verdict::Done(said, outcome) => (said, Some(outcome)),
            Verdict::Ignored(said) => (said, None),
        };
        // The page your browser shows, in your language.
        let body = crate::translator().text(said, None).replace('&', "&amp;").replace('<', "&lt;");
        let page = format!("<!doctype html><meta charset=utf-8><title>Sioul</title><p style=\"font:1.2em sans-serif;margin:3em\">{body}</p>");
        let _ = stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{page}", page.len()).as_bytes());
        if let Some(outcome) = outcome {
            return outcome;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_go_and_come_back_whole() {
        assert_eq!(encode("a b/c@d.example"), "a%20b%2Fc%40d.example");
        assert_eq!(decode("a%20b+c%2Fd%zz"), "a b c/d%zz");
        assert_eq!(query(&[("a", "1 2"), ("b", "x&y")]), "a=1%202&b=x%26y");
        // RFC 7636: 43 characters at least for the verifier; its challenge is SHA-256's.
        let verifier = random(48);
        assert_eq!(verifier.len(), 64);
        assert_eq!(challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"), "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM", "RFC 7636's example, appendix B");
    }

    /// Microsoft takes no IPv6 loopback: without the fallback, the door is
    /// 127.0.0.1's or none.
    #[test]
    fn the_door_is_on_the_loopback_address() {
        let (listener, redirect) = open(false).unwrap();
        assert!(listener.local_addr().unwrap().ip().is_loopback());
        assert_eq!(redirect, format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port()));
    }
}
