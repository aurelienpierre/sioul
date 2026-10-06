// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Leaving a mailing list in one click, on the network: the HTTPS POST of
//! RFC 8058 (docs/client.md, "Unsubscribing"). When it may be sent is the
//! core's to say (`sioul_core::unsubscribe`); here it is sent as the RFC
//! asks: `List-Unsubscribe=One-Click` as a form, no cookie, no authorisation,
//! nothing of yours but what the list wrote in its own address. A redirect to
//! another site is never followed; the wait is short.

use crate::SyncError;
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
