// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The door a browser comes back to after a sign-in on a provider's page, as
//! native apps should have it (OAuth 2.0 for native apps, RFC 8252 §7.3): a
//! port of this device's loopback address, chosen by the system, open for one
//! sign-in. Google's sign-in (`google.rs`) and Microsoft's (`microsoft.rs`)
//! share it; each provider says what an answer means (`Verdict`) and which
//! page the browser then shows, in your language.
//!
//! What counts is the request line, `GET /?state=…&code=… HTTP/1.1`; the
//! request is read whole, its headers too, at most `LINE_BYTES` of it,
//! within `LINE_TIME` of the connection, Cancel heard meanwhile: a
//! connection that says nothing, or says too much, is dropped, and cannot
//! hold the door past Cancel or the code's minute (the review of 15 October
//! 2026, L1). Read whole, so that closing it after the page is an orderly
//! end: unread headers would make it a reset, and the browser's tab might
//! say "connection reset" (the re-check, L1-bis). A request without a query
//! (a favicon) gets a 404 and the wait goes on. The state is compared in constant time.
//! An answer that waits in the system's queue is read even once the time is
//! up (a phone freezes Sioul while the browser is in front, and its clock
//! then jumps), for `LATE_CONNECTIONS` connections at most.
//!
//! Also here, what both providers' requests need: PKCE's verifier and
//! challenge (RFC 7636), a random state, and the form encoding of a query.

use crate::SyncError;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
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

/// The most a request may hold, its line and headers: a browser's answer,
/// its code and state included, and its headers (cookies for 127.0.0.1
/// among them) are a few kilobytes at most.
const LINE_BYTES: usize = 16 * 1024;
/// The most time a connection has to say its request: a browser says it at once.
const LINE_TIME: Duration = Duration::from_secs(2);
/// Connections still read once the time is up, as long as they wait in the
/// queue (the browser's answer among them, after a phone's freeze); past
/// them the wait ends, however many more keep coming.
const LATE_CONNECTIONS: usize = 16;

/// A connection's request, read whole (its line, then its headers up to the
/// blank line that ends them) within `LINE_TIME` and `LINE_BYTES`, Cancel
/// heard between reads: its request line. None when it does not come whole
/// in time, or when Cancel is pressed meanwhile. A request that ends with
/// the connection's own end after its line (a tool, not a browser) counts whole.
fn request_line(stream: &TcpStream, stop: &AtomicBool) -> Option<String> {
    let started = Instant::now();
    let mut head: Vec<u8> = Vec::with_capacity(4096);
    let mut chunk = [0u8; 4096];
    let line = |head: &[u8]| head.iter().position(|&b| b == b'\n').map(|end| String::from_utf8_lossy(&head[..end]).trim_end_matches('\r').to_string());
    loop {
        if stop.load(Ordering::Relaxed) {
            return None;
        }
        let left = LINE_TIME.checked_sub(started.elapsed())?;
        // Each read waits at most a fifth of a second, so that Cancel is heard.
        stream.set_read_timeout(Some(left.clamp(Duration::from_millis(1), Duration::from_millis(200)))).ok()?;
        match (&*stream).read(&mut chunk) {
            Ok(0) => return line(&head),
            Ok(read) => {
                // From a little before what came, for a blank line split between two reads.
                let from = head.len().saturating_sub(3);
                head.extend_from_slice(&chunk[..read]);
                let tail = &head[from..];
                if tail.windows(4).any(|w| w == b"\r\n\r\n") || tail.windows(2).any(|w| w == b"\n\n") {
                    return line(&head);
                }
                if head.len() > LINE_BYTES {
                    return None;
                }
            }
            Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut | std::io::ErrorKind::Interrupted) => continue,
            Err(_) => return None,
        }
    }
}

/// Waits at the door for the provider's answer, at most `deadline`, or until
/// `stop` (Cancel): `judge` says what each answer means and which page the
/// browser shows. The code, else why there is none.
pub(crate) fn wait(listener: &TcpListener, state: &str, deadline: Duration, stop: &AtomicBool, ends: Ends, judge: impl Fn(&Answer) -> Verdict) -> Result<String, SyncError> {
    listener.set_nonblocking(true).map_err(|e| SyncError::Network(e.to_string()))?;
    let until = Instant::now() + deadline;
    let mut late = 0;
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
        // Past the time, only so many: a stream of strangers cannot keep the wait going.
        if Instant::now() > until {
            late += 1;
            if late > LATE_CONNECTIONS {
                return Err(ends.timed_out);
            }
        }
        let _ = stream.set_nonblocking(false);
        let _ = stream.set_write_timeout(Some(LINE_TIME));
        let Some(line) = request_line(&stream, stop) else { continue };
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

/// A browser coming back to the door at `port`: `GET <target>` with the
/// headers a browser sends (Chrome's), a cookie making the request `size`
/// bytes long when it is shorter; what the door answered, read to its end,
/// or the error that ended it (a reset). For the tests of both providers.
#[cfg(test)]
pub(crate) fn browser_answer(port: u16, target: &str, size: usize) -> std::io::Result<String> {
    let head = format!(
        "GET {target} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: keep-alive\r\nUpgrade-Insecure-Requests: 1\r\nUser-Agent: Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36\r\nAccept: text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7\r\nSec-Fetch-Site: cross-site\r\nSec-Fetch-Mode: navigate\r\nSec-Fetch-Dest: document\r\nsec-ch-ua: \"Chromium\";v=\"141\", \"Not?A_Brand\";v=\"8\"\r\nsec-ch-ua-mobile: ?0\r\nsec-ch-ua-platform: \"Linux\"\r\nAccept-Encoding: gzip, deflate, br, zstd\r\nAccept-Language: fr-FR,fr;q=0.9,en-US;q=0.8,en;q=0.7\r\n"
    );
    let cookie = |pad: usize| format!("Cookie: pad={}\r\n", "c".repeat(pad));
    let pad = size.saturating_sub(head.len() + cookie(0).len() + 2);
    let request = format!("{head}{}\r\n", if pad > 0 { cookie(pad) } else { String::new() });
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port))?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.write_all(request.as_bytes())?;
    let mut answer = Vec::new();
    stream.read_to_end(&mut answer)?;
    Ok(String::from_utf8_lossy(&answer).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The re-check of 15 October 2026, L1-bis: the browser's whole request,
    /// its headers included (800 to 3,000 bytes, a cookie or two), is read
    /// before the door answers and closes, so the browser gets its page and
    /// an orderly end, never a reset ("connection reset" in its tab).
    #[test]
    fn a_browser_gets_its_page_and_an_orderly_end() {
        let judge = |answer: &Answer| match (answer.state_ok, answer.get("code")) {
            (true, Some(code)) => Verdict::Done("google-page-granted", Ok(code.to_string())),
            _ => Verdict::Ignored("google-page-foreign"),
        };
        for size in (800..=3000).step_by(110) {
            let (listener, _) = open(false).unwrap();
            let port = listener.local_addr().unwrap().port();
            let browser = std::thread::spawn(move || browser_answer(port, "/?state=the-state&code=the-code&scope=openid", size));
            let ends = Ends { cancelled: SyncError::Message("cancelled".into()), timed_out: SyncError::Message("timed out".into()) };
            assert_eq!(wait(&listener, "the-state", Duration::from_secs(10), &AtomicBool::new(false), ends, judge).unwrap(), "the-code", "{size} bytes");
            let answer = browser.join().unwrap();
            assert!(answer.as_ref().is_ok_and(|page| page.starts_with("HTTP/1.1 200") && page.contains("</p>")), "{size} bytes: {answer:?}");
        }
    }

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

    /// The review of 15 October 2026, L1: a stranger who connects first and
    /// dribbles, or sends a line without end, holds the door two seconds at
    /// most, and Cancel is heard meanwhile; the browser's answer, behind it,
    /// is read; a stream of strangers past the time does not keep the wait going.
    #[test]
    fn a_slow_or_endless_stranger_does_not_hold_the_door() {
        let judge = |answer: &Answer| match (answer.state_ok, answer.get("code")) {
            (true, Some(code)) => Verdict::Done("google-page-granted", Ok(code.to_string())),
            _ => Verdict::Ignored("google-page-foreign"),
        };
        let ends = || Ends { cancelled: SyncError::Message("cancelled".into()), timed_out: SyncError::Message("timed out".into()) };
        // A stranger dribbling a byte every 300 ms, never ending its line; Cancel at half a second.
        let (listener, _) = open(false).unwrap();
        let port = listener.local_addr().unwrap().port();
        let dribbler = std::thread::spawn(move || {
            let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
            for byte in b"GET /?state=".iter().cycle().take(40) {
                if stream.write_all(&[*byte]).is_err() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(300));
            }
        });
        let stop = std::sync::Arc::new(AtomicBool::new(false));
        let stopper = {
            let stop = std::sync::Arc::clone(&stop);
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(500));
                stop.store(true, Ordering::Relaxed);
            })
        };
        let started = Instant::now();
        assert_eq!(wait(&listener, "the-state", Duration::from_secs(60), &stop, ends(), judge).err(), Some(SyncError::Message("cancelled".into())));
        assert!(started.elapsed() < Duration::from_millis(1500), "Cancel heard within a fifth of a second: {:?}", started.elapsed());
        stopper.join().unwrap();
        // A stranger sending a line without end (64 KB), then the browser's answer behind it.
        let (listener, _) = open(false).unwrap();
        let port = listener.local_addr().unwrap().port();
        let endless = std::thread::spawn(move || {
            let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
            let _ = stream.write_all(&[b'A'; 64 * 1024]);
            let mut browser = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
            browser.write_all(b"GET /?state=the-state&code=the-code HTTP/1.1\r\n\r\n").unwrap();
            let mut answer = String::new();
            let _ = browser.read_to_string(&mut answer);
            answer
        });
        let started = Instant::now();
        assert_eq!(wait(&listener, "the-state", Duration::from_secs(60), &AtomicBool::new(false), ends(), judge).unwrap(), "the-code");
        assert!(started.elapsed() < Duration::from_secs(3), "{:?}", started.elapsed());
        assert!(endless.join().unwrap().starts_with("HTTP/1.1 200"));
        dribbler.join().unwrap();
        // Past the time, twenty strangers waiting: sixteen are read, then the wait ends.
        let (listener, _) = open(false).unwrap();
        let port = listener.local_addr().unwrap().port();
        let strangers: Vec<std::net::TcpStream> = (0..20)
            .map(|_| {
                let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
                stream.write_all(b"GET /?state=another HTTP/1.1\r\n\r\n").unwrap();
                stream
            })
            .collect();
        let heard = std::sync::atomic::AtomicUsize::new(0);
        let counting = |answer: &Answer| {
            heard.fetch_add(1, Ordering::Relaxed);
            judge(answer)
        };
        assert_eq!(wait(&listener, "the-state", Duration::ZERO, &AtomicBool::new(false), ends(), counting).err(), Some(SyncError::Message("timed out".into())));
        assert_eq!(heard.load(Ordering::Relaxed), LATE_CONNECTIONS);
        drop(strangers);
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
