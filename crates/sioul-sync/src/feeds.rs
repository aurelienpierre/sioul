// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! A calendar fetched by its web address (sioul-core's `subscribed`;
//! docs/client.md, "Calendars by their address"). An address pasted by the
//! person can lead anywhere, so each request is held to what a calendar
//! needs (audit-calendars §11):
//!
//! - **https only**: a `webcal://` address is read as `https://`, and a
//!   redirect to anything but `https://` is refused;
//! - **public addresses only**, the first and each redirect's: every address
//!   a name resolves to is checked before it is reached, by the resolver the
//!   contact's photos use (`stalls::public_agent`), so no second lookup can
//!   answer otherwise; with a proxy named in the environment (`HTTPS_PROXY`,
//!   `ALL_PROXY`), which resolves the name itself, the name is checked here
//!   before the proxy is asked (`stalls::resolves_public`, F1), never the
//!   proxy's own address;
//! - **three redirects at most**, followed here one by one, each checked as
//!   the first; the server's validators are sent to the first address alone
//!   (RFC 9110 §15.4);
//! - **ten megabytes once unpacked**: no compression is asked
//!   (`Accept-Encoding: identity`), and what is read is counted after any
//!   unpacking all the same (ureq counts its limit before it), so a small
//!   packed answer never becomes a large one in memory (`read_capped`);
//! - **the timeouts of CalDAV**: twenty seconds to connect, a minute in all;
//! - the server's own word that nothing changed (`If-None-Match`, else
//!   `If-Modified-Since`; RFC 9110 §13.1).
//!
//! The address is never written in a log or an error: a problem is said by
//! its kind alone (`subscribed::Problem`). Never run on the window's thread.

use sioul_core::subscribed::{self, Answer, MOST_BYTES, MOST_REDIRECTS, Problem};
use std::io::Read;
use std::time::Duration;

/// One request's answer, as the fetch needs it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reply {
    pub status: u16,
    /// Where a redirect points (`Location`), "" otherwise.
    pub location: String,
    pub etag: String,
    pub last_modified: String,
    /// Its body when it is a success, ten megabytes and one byte at most once unpacked.
    pub body: Vec<u8>,
}

/// One request to one address: the network (`Network`), or a stand-in in tests.
pub trait Hop {
    /// GET `url`, with the copy's validators (ETag, date) when given.
    fn get(&self, url: &str, validators: Option<&(String, String)>) -> Result<Reply, Problem>;
}

/// The network, through an agent that reaches public addresses only, over
/// https alone, and follows no redirect itself; or, for an address a proxy
/// of the environment serves, through that proxy once its name was checked.
pub struct Network {
    direct: ureq::Agent,
    proxied: Option<(ureq::Proxy, ureq::Agent)>,
}

impl Network {
    /// The network as this device reaches it: its environment's proxy, if one is named.
    pub fn new() -> Network {
        Network::with_proxy(ureq::Proxy::try_from_env())
    }

    /// The network, through `proxy` for the addresses it serves (the tests').
    pub fn with_proxy(proxy: Option<ureq::Proxy>) -> Network {
        let config = |proxy: Option<ureq::Proxy>| {
            ureq::Agent::config_builder()
                .timeout_connect(Some(Duration::from_secs(20)))
                .timeout_global(Some(Duration::from_secs(60)))
                .http_status_as_error(false)
                .https_only(true)
                .max_redirects(0)
                .max_redirects_will_error(false)
                .proxy(proxy)
                .build()
        };
        // Directly: never the environment's proxy, whose own address the resolver would check.
        let direct = crate::stalls::public_agent(config(None));
        let proxied = proxy.map(|p| (p.clone(), ureq::Agent::new_with_config(config(Some(p)))));
        Network { direct, proxied }
    }

    /// The agent for `url`: through the proxy when it serves it and the name
    /// resolves to public addresses here (F1), else directly; refused when not public.
    fn agent_for(&self, url: &str) -> Result<&ureq::Agent, Problem> {
        let Some((proxy, agent)) = &self.proxied else { return Ok(&self.direct) };
        let uri: ureq::http::Uri = url.parse().map_err(|_| Problem::NotAddress)?;
        if proxy.is_no_proxy(&uri) {
            return Ok(&self.direct);
        }
        if crate::stalls::resolves_public(&uri) { Ok(agent) } else { Err(Problem::NotPublic) }
    }
}

impl Default for Network {
    fn default() -> Network {
        Network::new()
    }
}

impl Hop for Network {
    fn get(&self, url: &str, validators: Option<&(String, String)>) -> Result<Reply, Problem> {
        let mut request = self.agent_for(url)?.get(url).header("User-Agent", "Sioul").header("Accept", "text/calendar, text/plain;q=0.8, */*;q=0.5").header("Accept-Encoding", "identity");
        if let Some((etag, modified)) = validators {
            if !etag.is_empty() {
                request = request.header("If-None-Match", etag.as_str());
            } else if !modified.is_empty() {
                request = request.header("If-Modified-Since", modified.as_str());
            }
        }
        let mut answer = request.call().map_err(|e| problem_of(&e))?;
        let status = answer.status().as_u16();
        let header = |name: &str| answer.headers().get(name).and_then(|v| v.to_str().ok()).unwrap_or_default().trim().to_string();
        let (location, etag, last_modified) = (header("location"), header("etag"), header("last-modified"));
        let body = if (200..300).contains(&status) { read_capped(answer.body_mut().as_reader())? } else { Vec::new() };
        Ok(Reply { status, location, etag, last_modified, body })
    }
}

/// What a failed request was, by its kind alone (never its address).
fn problem_of(error: &ureq::Error) -> Problem {
    match error {
        ureq::Error::RequireHttpsOnly(_) => Problem::NotHttps,
        ureq::Error::Io(io) if io.to_string().contains("not a public address") => Problem::NotPublic,
        ureq::Error::BadUri(_) | ureq::Error::Http(_) => Problem::NotAddress,
        _ => Problem::Network,
    }
}

/// A body read to ten megabytes and one byte at most, counted as it comes
/// out of any unpacking: past the limit, refused, and no more read.
pub fn read_capped(reader: impl Read) -> Result<Vec<u8>, Problem> {
    let mut body = Vec::new();
    reader.take(MOST_BYTES + 1).read_to_end(&mut body).map_err(|_| Problem::Network)?;
    if body.len() as u64 > MOST_BYTES {
        return Err(Problem::TooLarge);
    }
    Ok(body)
}

/// A calendar fetched from the network (`fetch_with`).
pub fn fetch(url: &str, validators: Option<(String, String)>) -> Answer {
    fetch_with(url, validators, &Network::new())
}

/// A calendar fetched through `hop`: its address checked (`subscribed::address`),
/// then each redirect, three at most, each checked the same way; the copy's
/// validators sent to the first address alone; a success's body, a 304 to
/// them, or why nothing may be read.
pub fn fetch_with(url: &str, validators: Option<(String, String)>, hop: &dyn Hop) -> Answer {
    let mut at = match subscribed::address(url) {
        Ok(at) => at,
        Err(problem) => return Answer::Refused(problem),
    };
    for turn in 0..=MOST_REDIRECTS {
        let asked = if turn == 0 { validators.as_ref() } else { None };
        let reply = match hop.get(&at, asked) {
            Ok(reply) => reply,
            Err(problem) => return Answer::Refused(problem),
        };
        match reply.status {
            200..=299 if reply.body.len() as u64 > MOST_BYTES => return Answer::Refused(Problem::TooLarge),
            200..=299 => return Answer::Calendar { bytes: reply.body, etag: reply.etag, last_modified: reply.last_modified },
            304 if asked.is_some() => return Answer::Unchanged,
            301 | 302 | 303 | 307 | 308 => {
                let Some(next) = joined(&at, &reply.location) else { return Answer::Refused(Problem::NotCalendar) };
                // The scheme as it is: http refused, never upgraded; `webcal:` read as https.
                if next.get(..7).is_some_and(|h| h.eq_ignore_ascii_case("http://")) {
                    return Answer::Refused(Problem::NotHttps);
                }
                at = match subscribed::address(&next) {
                    Ok(next) => next,
                    Err(problem) => return Answer::Refused(problem),
                };
            }
            404 | 410 => return Answer::Refused(Problem::Gone),
            status => return Answer::Refused(Problem::Status(status)),
        }
    }
    Answer::Refused(Problem::Redirects)
}

/// A redirect's `Location` read against the address that gave it (RFC 3986
/// §5.2, its common forms): absolute, from the host (`//`), from the root
/// (`/`), or beside the last part; none when empty.
fn joined(base: &str, location: &str) -> Option<String> {
    let location = location.trim();
    if location.is_empty() {
        return None;
    }
    // Absolute when it begins with a scheme (RFC 3986 §3.1), never for a
    // `://` in its query (`/login?next=https://…`: F2).
    let scheme = location.split_once(':').map(|(s, _)| s).filter(|s| s.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')));
    if scheme.is_some() {
        return Some(location.to_string());
    }
    let (scheme, rest) = base.split_once("://")?;
    if let Some(host_on) = location.strip_prefix("//") {
        return Some(format!("{scheme}://{host_on}"));
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    if location.starts_with('/') {
        return Some(format!("{scheme}://{authority}{location}"));
    }
    let path = rest[authority.len()..].split(['?', '#']).next().unwrap_or_default();
    let folder = path.rfind('/').map_or("/", |at| &path[..=at]);
    Some(format!("{scheme}://{authority}{folder}{location}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    /// A stand-in of the network: each address's reply; any other address is
    /// only checked as the real network checks it before any connection
    /// (`stalls::resolves_public`: IP literals and `localhost` resolve without
    /// asking anyone), never reached, whatever the check says (F6). It notes
    /// each address asked, with the validators sent.
    struct StandIn {
        replies: BTreeMap<String, Reply>,
        asked: Mutex<Vec<(String, bool)>>,
    }

    impl StandIn {
        fn new(replies: &[(&str, Reply)]) -> StandIn {
            StandIn { replies: replies.iter().map(|(u, r)| (u.to_string(), r.clone())).collect(), asked: Mutex::new(Vec::new()) }
        }
    }

    impl Hop for StandIn {
        fn get(&self, url: &str, validators: Option<&(String, String)>) -> Result<Reply, Problem> {
            self.asked.lock().unwrap().push((url.to_string(), validators.is_some()));
            match self.replies.get(url) {
                Some(reply) => Ok(reply.clone()),
                None if url.contains("example.") => Err(Problem::Network),
                None => {
                    let uri: ureq::http::Uri = url.parse().map_err(|_| Problem::NotAddress)?;
                    Err(if crate::stalls::resolves_public(&uri) { Problem::Network } else { Problem::NotPublic })
                }
            }
        }
    }

    fn ok(body: &str) -> Reply {
        Reply { status: 200, etag: "\"v1\"".into(), body: body.as_bytes().to_vec(), ..Reply::default() }
    }

    fn moved(to: &str) -> Reply {
        Reply { status: 302, location: to.into(), ..Reply::default() }
    }

    const CALENDAR: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Example//EN\r\nBEGIN:VEVENT\r\nUID:a@example.org\r\nDTSTAMP:20261118T100000Z\r\nDTSTART;VALUE=DATE:20261225\r\nSUMMARY:Holiday\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";

    /// A calendar is fetched over https; `webcal://` read as https; an http
    /// address refused before any request.
    #[test]
    fn a_calendar_is_fetched_over_https_alone() {
        let hop = StandIn::new(&[("https://calendar.example.org/a.ics", ok(CALENDAR))]);
        let Answer::Calendar { bytes, etag, .. } = fetch_with("webcal://calendar.example.org/a.ics", None, &hop) else { panic!("a calendar") };
        assert_eq!((bytes.len(), etag.as_str()), (CALENDAR.len(), "\"v1\""));
        assert_eq!(fetch_with("http://calendar.example.org/a.ics", None, &hop), Answer::Refused(Problem::NotHttps));
        assert_eq!(fetch_with("https://user:secret@calendar.example.org/a.ics", None, &hop), Answer::Refused(Problem::NotAddress));
        assert_eq!(hop.asked.lock().unwrap().len(), 1, "nothing asked of a refused address");
    }

    /// Three redirects at most, each checked: a fourth refused; one to http
    /// refused; one to a private address refused by the resolver itself,
    /// before any connection; the validators sent to the first address alone.
    #[test]
    fn redirects_are_counted_and_checked_each() {
        let hop = StandIn::new(&[
            ("https://a.example.org/c.ics", moved("https://b.example.org/c.ics")),
            ("https://b.example.org/c.ics", moved("/moved/c.ics")),
            ("https://b.example.org/moved/c.ics", moved("d.ics")),
            ("https://b.example.org/moved/d.ics", ok(CALENDAR)),
            ("https://loop.example.org/x", moved("https://loop.example.org/x")),
            ("https://down.example.org/x", moved("http://down.example.org/x")),
            ("https://home.example.org/x", moved("https://192.168.1.10/calendar.ics")),
            ("https://local.example.org/x", moved("https://localhost/calendar.ics")),
            ("https://v6.example.org/x", moved("https://[::ffff:10.0.0.5]/calendar.ics")),
        ]);
        assert!(matches!(fetch_with("https://a.example.org/c.ics", Some(("\"v0\"".into(), String::new())), &hop), Answer::Calendar { .. }), "three redirects");
        let asked = hop.asked.lock().unwrap().clone();
        assert_eq!(asked.iter().map(|(_, v)| *v).collect::<Vec<_>>(), [true, false, false, false], "validators to the first address alone");
        assert_eq!(fetch_with("https://loop.example.org/x", None, &hop), Answer::Refused(Problem::Redirects));
        assert_eq!(fetch_with("https://down.example.org/x", None, &hop), Answer::Refused(Problem::NotHttps));
        for private in ["https://home.example.org/x", "https://local.example.org/x", "https://v6.example.org/x"] {
            assert_eq!(fetch_with(private, None, &hop), Answer::Refused(Problem::NotPublic), "{private}");
        }
    }

    /// The review's F1: with a proxy named in the environment, the name is
    /// checked here before the proxy is asked, and refused when it is not
    /// public; never a connection (the proxy at a port nothing listens on).
    #[test]
    fn a_proxy_never_takes_the_public_rule_away() {
        let proxy = ureq::Proxy::new("http://127.0.0.1:9").unwrap();
        let through = Network::with_proxy(Some(proxy));
        for private in ["https://192.168.1.1/x.ics", "https://localhost/x.ics", "https://[::ffff:10.0.0.5]/x.ics", "https://127.0.0.1:9/x.ics"] {
            assert_eq!(through.agent_for(private).err(), Some(Problem::NotPublic), "{private}");
        }
        // Without a proxy, directly through the public resolver; a local proxy's own address never checked.
        assert!(Network::with_proxy(None).agent_for("https://192.168.1.1/x.ics").is_ok());
        assert!(through.agent_for("https://9.9.9.9/x.ics").is_ok(), "a public address goes through the proxy (nothing asked of it here)");
    }

    /// The server's 304 to the copy's validators says unchanged; a 304 never
    /// asked is no calendar; 404 and 410 say gone; another status is said.
    #[test]
    fn the_server_s_answers_are_read() {
        let not_modified = Reply { status: 304, ..Reply::default() };
        let hop = StandIn::new(&[("https://same.example.org/c.ics", not_modified), ("https://gone.example.org/c.ics", Reply { status: 410, ..Reply::default() }), ("https://busy.example.org/c.ics", Reply { status: 503, ..Reply::default() })]);
        assert_eq!(fetch_with("https://same.example.org/c.ics", Some(("\"v1\"".into(), String::new())), &hop), Answer::Unchanged);
        assert_eq!(fetch_with("https://gone.example.org/c.ics", None, &hop), Answer::Refused(Problem::Gone));
        assert_eq!(fetch_with("https://busy.example.org/c.ics", None, &hop), Answer::Refused(Problem::Status(503)));
        assert_eq!(fetch_with("https://unreached.example.org/c.ics", None, &hop), Answer::Refused(Problem::Network));
    }

    /// Ten megabytes once unpacked: a small packed answer that unpacks
    /// beyond them is refused after ten megabytes and one byte, never more read.
    #[test]
    fn a_body_is_capped_once_unpacked() {
        use std::io::Write;
        let mut packer = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
        let zeros = vec![0u8; 1 << 20];
        for _ in 0..40 {
            packer.write_all(&zeros).unwrap();
        }
        let packed = packer.finish().unwrap();
        assert!(packed.len() < 200_000, "a small answer: {}", packed.len());
        let unpacking = flate2::read::GzDecoder::new(packed.as_slice());
        assert_eq!(read_capped(unpacking), Err(Problem::TooLarge));
        assert_eq!(read_capped(CALENDAR.as_bytes()).unwrap().len(), CALENDAR.len());
        // A stand-in's body past the limit, as a server without compression sends it.
        let hop = StandIn::new(&[("https://large.example.org/c.ics", Reply { status: 200, body: vec![b'x'; MOST_BYTES as usize + 1], ..Reply::default() })]);
        assert_eq!(fetch_with("https://large.example.org/c.ics", None, &hop), Answer::Refused(Problem::TooLarge));
    }

    #[test]
    fn a_location_is_read_against_its_address() {
        assert_eq!(joined("https://a.example.org/x/y.ics?k=1", "z.ics").as_deref(), Some("https://a.example.org/x/z.ics"));
        assert_eq!(joined("https://a.example.org/x/y.ics", "/r.ics").as_deref(), Some("https://a.example.org/r.ics"));
        assert_eq!(joined("https://a.example.org/x/y.ics", "//b.example.org/r.ics").as_deref(), Some("https://b.example.org/r.ics"));
        assert_eq!(joined("https://a.example.org", "r.ics").as_deref(), Some("https://a.example.org/r.ics"));
        assert_eq!(joined("https://a.example.org/x", " ").as_deref(), None);
        // F2: a relative address whose query holds another address stays relative.
        assert_eq!(joined("https://a.example.org/x/y.ics", "/login?next=https://b.example.org/cal").as_deref(), Some("https://a.example.org/login?next=https://b.example.org/cal"));
        assert_eq!(joined("https://a.example.org/x/y.ics", "z.ics?from=webcal://c").as_deref(), Some("https://a.example.org/x/z.ics?from=webcal://c"));
        assert_eq!(joined("https://a.example.org/x", "webcal://b.example.org/r.ics").as_deref(), Some("webcal://b.example.org/r.ics"));
    }
}
