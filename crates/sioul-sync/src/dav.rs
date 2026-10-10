// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Contacts and calendars on a server: CardDAV (RFC 6352) and CalDAV (RFC
//! 4791), both WebDAV (RFC 4918), synced both ways with the folders on disk
//! (`sioul_core::vdir`).
//!
//! - **Finding them**: the address you give, else `/.well-known/caldav` and
//!   `/.well-known/carddav` on the domain (RFC 6764), else the known places of
//!   some providers; then your principal (RFC 5397) and its home sets, and
//!   the address books and calendars in them.
//! - **What you changed here goes first**: a changed file is sent with
//!   `If-Match` and its ETag, so nobody's change is overwritten silently; a new
//!   one with `If-None-Match: *`; a deleted one is deleted there.
//! - **Then what changed there**: with the sync token when the server keeps
//!   one (RFC 6578), else by comparing ETags; changed items come in batches
//!   (`calendar-multiget`, `addressbook-multiget`) of 50 at most, half as
//!   many after a slow or cut answer, twice as many again after quick ones
//!   (`pace`).
//! - **A new item whose UID the server holds already** (an invitation
//!   answered on two devices): the item there takes its place here, said
//!   when its answer differs (`same_uid_there`, `adopt`).
//! - **When both changed the same item**, the server's version wins and yours
//!   is kept among this device's earlier versions (`history::ACCOUNTS`), which
//!   the window lists and puts back, said in the report.
//! - **Cut short** (the network lost, the app killed), a sync resumes where it
//!   stopped: what was sent or brought is known at once, never sent twice.
//!
//! Only HTTPS, with the password in the system keyring and sent as Basic
//! authentication over TLS; for Google, its OAuth access token instead
//! (google.rs). A small client on `ureq`, written for this: the protocol
//! needed is a handful of requests.

use crate::SyncError;
use crate::fetch::{Control, Parked};
use sioul_core::config::{Account, state_dir};
use sioul_core::vdir::{self, ItemState, Kind, State};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub(crate) const DAV: &str = "DAV:";
const CALDAV: &str = "urn:ietf:params:xml:ns:caldav";
const CARDDAV: &str = "urn:ietf:params:xml:ns:carddav";
const CALSERVER: &str = "http://calendarserver.org/ns/";
const APPLE: &str = "http://apple.com/ns/ical/";

/// Items asked for in one multiget, at most: the batch after quick answers.
const BATCH: usize = 50;

/// A multiget answered in this long or more is slow: the next batch of that
/// collection is half as large (one item at least). A batch of 50 events with
/// their attachments, or of 50 cards with their photos, on a weak signal, can
/// take longer than a request may (`BUDGET`): asked again whole, it would fail
/// again at every sync.
const SLOW: Duration = if cfg!(test) { Duration::from_millis(600) } else { Duration::from_secs(20) };

/// A multiget answered within this is quick: the next batch is twice as
/// large, up to `BATCH`.
const QUICK: Duration = if cfg!(test) { Duration::from_millis(300) } else { Duration::from_secs(5) };

/// Each collection's batch now, by its address, in this process: halved after
/// a slow answer or a cut one, doubled after a quick one (`pace`). Not kept
/// on disk: a new start begins at `BATCH`, and a slow line slows it again at
/// the first batch.
static PACE: Mutex<BTreeMap<String, usize>> = Mutex::new(BTreeMap::new());

/// The batch of the collection at `url` now.
fn batch_of(url: &str) -> usize {
    PACE.lock().ok().and_then(|pace| pace.get(&path_key(url)).copied()).unwrap_or(BATCH)
}

/// A multiget of that collection answered (`Some`: in that long) or cut by
/// the network (`None`): its next batch follows.
fn pace(url: &str, took: Option<Duration>) {
    let Ok(mut pace) = PACE.lock() else { return };
    let now = pace.get(&path_key(url)).copied().unwrap_or(BATCH);
    let next = match took {
        Some(took) if took <= QUICK => (now * 2).min(BATCH),
        Some(took) if took < SLOW => now,
        _ => (now / 2).max(1),
    };
    pace.insert(path_key(url), next);
}

/// A connection to one server, with its login.
pub struct Client {
    agent: ureq::Agent,
    authorization: Mutex<String>,
    /// A Google account's address: its access token ends after an hour, and a
    /// refused one is asked again, once.
    google: Option<String>,
    /// The servers the login goes to, by origin (`origin`), once the
    /// account's homes are known (`keep_to`): an address a server's answer
    /// names on any other is refused, never asked with the login.
    own: Mutex<Option<Vec<String>>>,
}

/// What a server answered.
struct Answer {
    status: u16,
    etag: Option<String>,
    location: Option<String>,
    body: String,
}

/// How long a request may wait: to connect; for each read or write, so a big
/// answer trickling in on a weak signal goes on as long as it moves, and one
/// that stops is given up soon; and for the whole of it, a bound for the worst.
pub(crate) struct Budget {
    pub(crate) connect: Duration,
    pub(crate) stall: Duration,
    pub(crate) whole: Duration,
}

const BUDGET: Budget = Budget { connect: Duration::from_secs(20), stall: Duration::from_secs(60), whole: Duration::from_secs(15 * 60) };

pub(crate) fn agent(budget: &Budget) -> ureq::Agent {
    // Each read and write bounded by `stall`, as long as the answer moves:
    // on ureq's transport, which follows no semver (`stalls`).
    crate::stalls::agent(config(budget), budget.stall, false)
}

/// The sharing's server's one agent in this process (`remote::Server`): its
/// connections and TLS sessions kept between the requests of a pull, of the
/// sends after it and of the next pull, so that a phone's step opens one
/// connection, not one per pull, per send and per file sent (on mobile data,
/// each new one wakes the radio for a TLS handshake). Each request says its
/// own budget (its whole and its connection's time, `configure_request`; its
/// stall, `stalls::ask`); ureq keeps an idle connection fifteen seconds.
pub(crate) fn shared_agent() -> ureq::Agent {
    static SHARED: std::sync::OnceLock<ureq::Agent> = std::sync::OnceLock::new();
    SHARED.get_or_init(|| crate::stalls::shared_agent(config(&BUDGET), BUDGET.stall)).clone()
}

fn config(budget: &Budget) -> ureq::config::Config {
    ureq::Agent::config_builder()
        .timeout_connect(Some(budget.connect))
        .timeout_global(Some(budget.whole))
        .http_status_as_error(false)
        .max_redirects(0)
        .allow_non_standard_methods(true)
        .build()
}

impl Client {
    pub fn new(login: &str, password: &str) -> Client {
        Client {
            agent: agent(&BUDGET),
            authorization: Mutex::new(format!("Basic {}", sioul_core::lines::base64_encode(format!("{login}:{password}").as_bytes()))),
            google: None,
            own: Mutex::new(None),
        }
    }

    /// A Google account, signed in with its access token.
    pub fn google(address: &str) -> Result<Client, SyncError> {
        let token = crate::google::access_token(address, false)?;
        Ok(Client { agent: agent(&BUDGET), authorization: Mutex::new(format!("Bearer {token}")), google: Some(address.to_string()), own: Mutex::new(None) })
    }

    pub fn is_google(&self) -> bool {
        self.google.is_some()
    }

    /// From now on, the login goes only to the servers of the account's
    /// homes: their scheme, host and port. Every address of its address
    /// books, calendars and items comes from a server's answer, which may
    /// name any server; one on another is refused, and said
    /// (`SyncError::Elsewhere`).
    pub(crate) fn keep_to(&self, homes: &Homes) {
        let own = [&homes.calendars, &homes.contacts].into_iter().flatten().filter_map(|home| origin(home)).collect();
        *self.own.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(own);
    }

    /// Whether a request may go to `url` with the login: before the homes
    /// are known, finding them (`discover`) keeps to the account's domain.
    fn mine(&self, url: &str) -> Result<(), SyncError> {
        match &*self.own.lock().unwrap_or_else(std::sync::PoisonError::into_inner) {
            Some(own) if !origin(url).is_some_and(|o| own.contains(&o)) => Err(SyncError::Elsewhere(url.to_string())),
            _ => Ok(()),
        }
    }

    fn send(&self, method: &str, url: &str, headers: &[(&str, &str)], body: Option<&str>) -> Result<Answer, SyncError> {
        match (self.send_once(method, url, headers, body), &self.google) {
            // The access token ended early: a new one, once.
            (Err(SyncError::Login(_)), Some(address)) => {
                let token = crate::google::access_token(address, true)?;
                if let Ok(mut authorization) = self.authorization.lock() {
                    *authorization = format!("Bearer {token}");
                }
                self.send_once(method, url, headers, body)
            }
            (result, _) => result,
        }
    }

    fn send_once(&self, method: &str, url: &str, headers: &[(&str, &str)], body: Option<&str>) -> Result<Answer, SyncError> {
        allowed(url)?;
        self.mine(url)?;
        let authorization = self.authorization.lock().map(|a| a.clone()).unwrap_or_default();
        let mut request = ureq::http::Request::builder().method(method).uri(url).header("Authorization", &authorization).header("User-Agent", "Sioul");
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        let request = request.body(body.unwrap_or("").to_string()).map_err(|e| SyncError::Server(e.to_string()))?;
        let mut response = self.agent.run(request).map_err(|e| SyncError::Network(e.to_string()))?;
        let header = |name: &str| response.headers().get(name).and_then(|v| v.to_str().ok()).map(str::to_string);
        let (etag, location) = (header("ETag"), header("Location"));
        let status = response.status().as_u16();
        let body = response.body_mut().with_config().limit(64 * 1024 * 1024).read_to_string().unwrap_or_default();
        if status == 401 {
            return Err(SyncError::Login(format!("{method} {url}: 401")));
        }
        Ok(Answer { status, etag, location, body })
    }

    fn propfind(&self, url: &str, depth: &str, props: &str) -> Result<(u16, Vec<Response>), SyncError> {
        let body = format!(
            r#"<?xml version="1.0" encoding="utf-8"?><d:propfind xmlns:d="DAV:" xmlns:c="{CALDAV}" xmlns:a="{CARDDAV}" xmlns:cs="{CALSERVER}" xmlns:ic="{APPLE}"><d:prop>{props}</d:prop></d:propfind>"#
        );
        let answer = self.send("PROPFIND", url, &[("Depth", depth), ("Content-Type", "application/xml; charset=utf-8")], Some(&body))?;
        if answer.status != 207 {
            return Ok((answer.status, Vec::new()));
        }
        Ok((answer.status, multistatus(&answer.body)?.0))
    }
}

/// Plain HTTP only to this computer, and only in a test build.
pub(crate) fn allowed(url: &str) -> Result<(), SyncError> {
    if url.starts_with("https://") {
        return Ok(());
    }
    #[cfg(feature = "insecure-test-tls")]
    if std::env::var_os("SIOUL_TEST_INSECURE_TLS").is_some() && (url.starts_with("http://localhost") || url.starts_with("http://127.0.0.1")) {
        return Ok(());
    }
    // This crate's own tests, against their stand-in (`stand_in`).
    #[cfg(test)]
    if url.starts_with("http://127.0.0.1:") {
        return Ok(());
    }
    Err(SyncError::Tls(format!("{url}: not encrypted")))
}

/// One `response` of a multistatus: an address and what was found there.
#[derive(Debug, Default, Clone)]
pub(crate) struct Response {
    pub(crate) href: String,
    /// The status of the whole response (sync-collection says 404 for what went).
    pub(crate) status: Option<u16>,
    pub(crate) props: Vec<Prop>,
}

/// One property, found (200) or not.
#[derive(Debug, Default, Clone)]
pub(crate) struct Prop {
    ns: String,
    name: String,
    status: u16,
    text: String,
    pub(crate) hrefs: Vec<String>,
    /// Child elements, by namespace and name (resourcetype, privileges).
    children: Vec<(String, String)>,
    /// `name` attributes of `comp` children (supported-calendar-component-set).
    comps: Vec<String>,
}

impl Response {
    pub(crate) fn prop(&self, ns: &str, name: &str) -> Option<&Prop> {
        self.props.iter().find(|p| p.ns == ns && p.name == name && p.status / 100 == 2)
    }

    pub(crate) fn text(&self, ns: &str, name: &str) -> Option<String> {
        self.prop(ns, name).map(|p| p.text.trim().to_string()).filter(|t| !t.is_empty())
    }

    pub(crate) fn is(&self, ns: &str, kind: &str) -> bool {
        self.prop(DAV, "resourcetype").is_some_and(|p| p.children.iter().any(|(n, k)| n == ns && k == kind))
    }
}

fn status_code(text: &str) -> Option<u16> {
    text.split_whitespace().nth(1).and_then(|c| c.parse().ok())
}

/// The responses of a 207 Multi-Status, and the sync token when one came.
pub(crate) fn multistatus(xml: &str) -> Result<(Vec<Response>, Option<String>), SyncError> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| SyncError::Server(format!("XML: {e}")))?;
    let mut responses = Vec::new();
    let mut token = None;
    for node in doc.root_element().children().filter(roxmltree::Node::is_element) {
        if node.has_tag_name((DAV, "sync-token")) {
            token = node.text().map(|t| t.trim().to_string());
            continue;
        }
        if !node.has_tag_name((DAV, "response")) {
            continue;
        }
        let mut response = Response::default();
        for part in node.children().filter(roxmltree::Node::is_element) {
            if part.has_tag_name((DAV, "href")) {
                response.href = part.text().unwrap_or("").trim().to_string();
            } else if part.has_tag_name((DAV, "status")) {
                response.status = status_code(part.text().unwrap_or(""));
            } else if part.has_tag_name((DAV, "propstat")) {
                let status = part.children().find(|c| c.has_tag_name((DAV, "status"))).and_then(|s| status_code(s.text().unwrap_or(""))).unwrap_or(200);
                for prop in part.children().filter(|c| c.has_tag_name((DAV, "prop"))).flat_map(|p| p.children().filter(roxmltree::Node::is_element)) {
                    let tag = prop.tag_name();
                    let elements: Vec<roxmltree::Node> = prop.descendants().filter(|d| d.is_element() && *d != prop).collect();
                    response.props.push(Prop {
                        ns: tag.namespace().unwrap_or("").to_string(),
                        name: tag.name().to_string(),
                        status,
                        text: prop.children().filter(|c| c.is_text()).filter_map(|c| c.text()).collect::<String>(),
                        hrefs: elements.iter().filter(|d| d.has_tag_name((DAV, "href"))).filter_map(|d| d.text()).map(|t| t.trim().to_string()).collect(),
                        children: elements.iter().map(|d| (d.tag_name().namespace().unwrap_or("").to_string(), d.tag_name().name().to_string())).collect(),
                        comps: elements.iter().filter(|d| d.tag_name().name() == "comp").filter_map(|d| d.attribute("name")).map(str::to_string).collect(),
                    });
                }
            }
        }
        responses.push(response);
    }
    Ok((responses, token))
}

/// An address's origin, what a login is kept to: its scheme, host and port,
/// in lowercase, the port written even when it is the scheme's own
/// (`https://dav.example.org:443`). None without a scheme or a host.
pub(crate) fn origin(url: &str) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    let scheme = scheme.to_ascii_lowercase();
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    // What stands before an "@" is a login, not the host.
    let at = authority.rsplit_once('@').map_or(authority, |(_, host)| host);
    let (host, port) = match at.rsplit_once(':') {
        // "[2001:db8::1]" holds colons of its own.
        Some((host, port)) if !port.contains(']') => (host, port.parse::<u16>().ok()?),
        _ => (at, if scheme == "https" { 443 } else if scheme == "http" { 80 } else { return None }),
    };
    (!host.is_empty()).then(|| format!("{scheme}://{}:{port}", host.to_ascii_lowercase()))
}

/// `href` made absolute against `base` ("https://host/a/b/" + "/c/" → "https://host/c/").
/// An absolute `href` is kept as it is, whatever its server: the login goes
/// only to the account's own (`Client::keep_to`).
pub fn absolute(base: &str, href: &str) -> String {
    if href.starts_with("http://") || href.starts_with("https://") {
        return href.to_string();
    }
    let origin_end = base.find("://").map_or(0, |i| i + 3);
    let origin = match base[origin_end..].find('/') {
        Some(slash) => &base[..origin_end + slash],
        None => base,
    };
    if href.starts_with('/') {
        return format!("{origin}{href}");
    }
    let directory = base.rsplit_once('/').map_or(base, |(d, _)| d);
    format!("{directory}/{href}")
}

/// The path of an address, percent-decoded: what to compare hrefs by, however a server writes them.
pub(crate) fn path_key(url: &str) -> String {
    let path = url.find("://").and_then(|i| url[i + 3..].find('/').map(|j| &url[i + 3 + j..])).unwrap_or(url);
    let mut out = Vec::with_capacity(path.len());
    let bytes = path.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(byte) = path.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(byte);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    let decoded = String::from_utf8_lossy(&out).to_string();
    decoded.trim_end_matches('/').to_string()
}

/// Where to look for the server of `domain`: the address given, the
/// well-known places, then the providers that put it elsewhere.
fn candidates(kind: Kind, given: Option<&str>, address: &str, mail_host: Option<&str>) -> Vec<String> {
    let mut out: Vec<String> = given.map(|g| vec![g.trim().to_string()]).unwrap_or_default();
    let domain = address.rsplit_once('@').map(|(_, d)| d.to_ascii_lowercase()).unwrap_or_default();
    let service = match kind {
        Kind::Calendars => "caldav",
        Kind::Contacts => "carddav",
    };
    if !domain.is_empty() {
        out.push(format!("https://{domain}/.well-known/{service}"));
        out.push(format!("https://{service}.{domain}/.well-known/{service}"));
    }
    let known: &[(&str, &str, &str)] = &[
        ("murena.io", "https://murena.io/remote.php/dav/", "https://murena.io/remote.php/dav/"),
        ("e.email", "https://murena.io/remote.php/dav/", "https://murena.io/remote.php/dav/"),
        ("fastmail.com", "https://caldav.fastmail.com/dav/", "https://carddav.fastmail.com/dav/"),
        ("icloud.com", "https://caldav.icloud.com/", "https://contacts.icloud.com/"),
        ("me.com", "https://caldav.icloud.com/", "https://contacts.icloud.com/"),
        ("posteo.de", "https://posteo.de:8443/", "https://posteo.de:8843/"),
        ("mailbox.org", "https://dav.mailbox.org/", "https://dav.mailbox.org/"),
    ];
    if let Some((_, calendars, contacts)) = known.iter().find(|(d, _, _)| *d == domain) {
        out.push(match kind {
            Kind::Calendars => calendars.to_string(),
            Kind::Contacts => contacts.to_string(),
        });
    }
    // cPanel hosts serve both on port 2080 of the mail server.
    if let Some(host) = mail_host {
        out.push(format!("https://{host}:2080/.well-known/{service}"));
        out.push(format!("https://{host}/.well-known/{service}"));
    }
    out.dedup();
    out
}

/// Where the account's contacts or calendars live, as found.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Homes {
    #[serde(default)]
    pub calendars: Option<String>,
    #[serde(default)]
    pub contacts: Option<String>,
}

fn homes_path(account: &str) -> PathBuf {
    state_dir().join("dav").join(account).join("homes.toml")
}

impl Homes {
    pub fn load(account: &str) -> Homes {
        std::fs::read_to_string(homes_path(account)).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default()
    }

    pub fn save(&self, account: &str) -> Result<(), SyncError> {
        let path = homes_path(account);
        let fail = |e: std::io::Error| SyncError::Disk(format!("{}: {e}", path.display()));
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(fail)?;
        }
        std::fs::write(&path, toml::to_string(self).map_err(|e| SyncError::Disk(e.to_string()))?).map_err(fail)
    }

    pub fn is_empty(&self) -> bool {
        self.calendars.is_none() && self.contacts.is_none()
    }
}

/// Finds an account's calendar home and address book home: where the
/// calendars and address books are. `given` is a server address you typed, if any.
pub fn discover(client: &Client, address: &str, given: Option<&str>, mail_host: Option<&str>) -> Result<Homes, SyncError> {
    if client.is_google() {
        // Google's calendars from the address's principal, its contacts from its well-known address.
        let homes = Homes { calendars: home(client, Kind::Calendars, &crate::google::caldav_start(address))?, contacts: home(client, Kind::Contacts, crate::google::CARDDAV_START)? };
        return if homes.is_empty() { Err(SyncError::NotFound("google.com".into())) } else { Ok(homes) };
    }
    let mut homes = Homes::default();
    let mut last_error = None;
    for kind in [Kind::Calendars, Kind::Contacts] {
        for start in candidates(kind, given, address, mail_host) {
            match home(client, kind, &start) {
                Ok(Some(found)) => {
                    match kind {
                        Kind::Calendars => homes.calendars = Some(found),
                        Kind::Contacts => homes.contacts = Some(found),
                    }
                    break;
                }
                Ok(None) => {}
                // A refused password is said at once: retrying elsewhere could lock the account.
                Err(e @ SyncError::Login(_)) => return Err(e),
                Err(e) => last_error = Some(e),
            }
        }
    }
    if homes.is_empty() {
        let domain = address.rsplit_once('@').map_or(address, |(_, d)| d).to_string();
        return Err(last_error.unwrap_or(SyncError::NotFound(domain)));
    }
    Ok(homes)
}

/// The domain an address belongs to ("caldav.example.org" → "example.org").
fn domain(url: &str) -> String {
    sioul_core::sites::domain_of(&sioul_core::sites::host_of(url))
}

/// From a starting address: follows redirects, finds the principal, then its home for `kind`.
fn home(client: &Client, kind: Kind, start: &str) -> Result<Option<String>, SyncError> {
    let mut url = start.to_string();
    // Redirects are followed by hand: the well-known addresses answer with one (RFC 6764 §5).
    // Each request carries the password, and the addresses tried are guesses
    // from your domain: a redirect to another domain (a parked domain's
    // advertiser, a web host's) is not followed, so it never goes there; the
    // server's own address can be given instead. Google's are its own, fixed.
    for _ in 0..5 {
        let answer = client.send("PROPFIND", &url, &[("Depth", "0"), ("Content-Type", "application/xml; charset=utf-8")], Some(PRINCIPAL))?;
        match answer.status {
            301 | 302 | 303 | 307 | 308 => match answer.location.map(|location| absolute(&url, &location)) {
                Some(next) if client.is_google() || domain(&next) == domain(start) => url = next,
                _ => return Ok(None),
            },
            207 => {
                let (responses, _) = multistatus(&answer.body)?;
                // A principal that does not name itself is the one asked.
                let principal = responses
                    .iter()
                    .find_map(|r| r.prop(DAV, "current-user-principal").and_then(|p| p.hrefs.first().cloned()))
                    .map_or_else(|| url.clone(), |href| absolute(&url, &href));
                // The principal and the home are asked with the login too: on
                // the domain started from only, as redirects (iCloud's homes
                // are on another of its hosts).
                let ours = |next: &str| if client.is_google() || domain(next) == domain(start) { Ok(()) } else { Err(SyncError::Elsewhere(next.to_string())) };
                ours(&principal)?;
                let (ns, name) = match kind {
                    Kind::Calendars => (CALDAV, "calendar-home-set"),
                    Kind::Contacts => (CARDDAV, "addressbook-home-set"),
                };
                let prop = match kind {
                    Kind::Calendars => "<c:calendar-home-set/>",
                    Kind::Contacts => "<a:addressbook-home-set/>",
                };
                let (_, responses) = client.propfind(&principal, "0", prop)?;
                let home = responses.iter().find_map(|r| r.prop(ns, name).and_then(|p| p.hrefs.first().cloned())).map(|href| absolute(&principal, &href));
                if let Some(home) = &home {
                    ours(home)?;
                }
                return Ok(home);
            }
            _ => return Ok(None),
        }
    }
    Ok(None)
}

const PRINCIPAL: &str = r#"<?xml version="1.0" encoding="utf-8"?><d:propfind xmlns:d="DAV:"><d:prop><d:current-user-principal/></d:prop></d:propfind>"#;

/// An address book or calendar the server lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    pub kind: Kind,
    pub url: String,
    pub id: String,
    pub name: String,
    pub color: Option<String>,
    pub read_only: bool,
    pub ctag: Option<String>,
    pub sync_token: Option<String>,
    /// For calendars: what it holds ("VEVENT", "VTODO"); empty when the server does not say.
    pub components: Vec<String>,
}

/// The collections in a home.
pub fn collections(client: &Client, kind: Kind, home: &str) -> Result<Vec<Listed>, SyncError> {
    let props = "<d:resourcetype/><d:displayname/><cs:getctag/><d:sync-token/><d:current-user-privilege-set/><ic:calendar-color/><c:supported-calendar-component-set/>";
    let (status, responses) = client.propfind(home, "1", props)?;
    if status != 207 {
        return Err(SyncError::Server(format!("PROPFIND {home}: {status}")));
    }
    let (ns, wanted) = match kind {
        Kind::Calendars => (CALDAV, "calendar"),
        Kind::Contacts => (CARDDAV, "addressbook"),
    };
    let mut found: Vec<Listed> = Vec::new();
    for response in responses.iter().filter(|r| r.is(ns, wanted)) {
        let url = absolute(home, &response.href);
        let privileges = response.prop(DAV, "current-user-privilege-set");
        let writable = privileges.is_none_or(|p| p.children.iter().any(|(n, k)| n == DAV && matches!(k.as_str(), "write" | "write-content" | "all" | "bind")));
        let mut id = vdir::folder_id(&response.href);
        // Two collections may end alike on different paths.
        while found.iter().any(|l| l.id == id) {
            id.push('_');
        }
        found.push(Listed {
            kind,
            name: response.text(DAV, "displayname").unwrap_or_else(|| id.clone()),
            color: response.text(APPLE, "calendar-color").map(|c| c.chars().take(7).collect()),
            read_only: !writable,
            ctag: response.text(CALSERVER, "getctag"),
            sync_token: response.text(DAV, "sync-token"),
            components: response.prop(CALDAV, "supported-calendar-component-set").map(|p| p.comps.clone()).unwrap_or_default(),
            url,
            id,
        });
    }
    Ok(found)
}

/// What a sync of one account did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    /// Items sent to the server: changed, new or deleted here.
    pub sent: usize,
    /// Items brought from the server.
    pub received: usize,
    pub removed: usize,
    /// Changed on both sides: the server's kept, yours set aside.
    pub conflicts: Vec<PathBuf>,
    pub collections: usize,
    /// Invitations answered on another device first, and otherwise: each
    /// one's title, and the answer the server holds and this device now keeps
    /// ("ACCEPTED", "TENTATIVE", "DECLINED"), yours replaced by it (`adopt`).
    pub answered: Vec<(String, String)>,
}

/// An account's client: Google's access token, or its login and the password in the keyring.
pub fn client_for(account: &Account) -> Result<Client, SyncError> {
    let login = account.login().ok_or(SyncError::NoServer)?;
    if account.auth.as_deref() == Some("google") {
        return Client::google(account.address.as_deref().unwrap_or(login));
    }
    Ok(Client::new(login, &crate::secret::password(account)?))
}

/// Syncs every address book and calendar of a contacts-and-calendars account.
/// An error in one collection (refused, cut, timed out) is said at the end,
/// the first one, the others synced all the same; a lasting one (a refused
/// password, a certificate) stops the sync at once.
pub fn sync(account: &Account) -> Result<Report, SyncError> {
    sync_with(&client_for(account)?, account)
}

fn sync_with(client: &Client, account: &Account) -> Result<Report, SyncError> {
    let login = account.login().ok_or(SyncError::NoServer)?;
    // One sync of an account at a time, across processes too (the window's
    // watcher, `sioul task add`): both would send the same new item, and the
    // last to keep its state would forget what the other did.
    let _lock = crate::fetch::lock(&state_dir().join("dav").join(format!("{}.lock", account.id)))?;
    let mut homes = Homes::load(&account.id);
    if homes.is_empty() {
        homes = discover(client, account.address.as_deref().unwrap_or(login), account.url.as_deref(), None)?;
        homes.save(&account.id)?;
    }
    client.keep_to(&homes);
    let mut report = Report::default();
    let mut refused = None;
    for (kind, home) in [(Kind::Contacts, &homes.contacts), (Kind::Calendars, &homes.calendars)] {
        let Some(home) = home else { continue };
        // Google makes, renames and deletes no calendar or address book from here.
        if !client.is_google() {
            go_on(&mut refused, send_changes(client, &account.id, kind))?;
            go_on(&mut refused, create_pending(client, &account.id, kind, home))?;
        }
        let mut listed = match collections(client, kind, home) {
            Ok(listed) => listed,
            Err(e) => {
                go_on(&mut refused, Err(e))?;
                continue;
            }
        };
        // Google's calendars hold events only, whether or not they say it.
        if client.is_google() {
            listed.iter_mut().filter(|l| l.kind == Kind::Calendars && l.components.is_empty()).for_each(|l| l.components = vec!["VEVENT".into()]);
        }
        forget_gone(&account.id, kind, &listed);
        for collection in &listed {
            match sync_collection(client, &account.id, collection, &mut report) {
                Ok(()) => report.collections += 1,
                Err(e) => go_on(&mut refused, Err(e))?,
            }
        }
    }
    // Google's task lists, over Google Tasks.
    if client.is_google() {
        match crate::google_tasks::sync(account) {
            Ok(tasks) => {
                report.sent += tasks.sent;
                report.received += tasks.received;
                report.removed += tasks.removed;
                report.collections += tasks.collections;
            }
            Err(e) => go_on(&mut refused, Err(e))?,
        }
    }
    refused.map_or(Ok(report), Err)
}

/// Syncs an account now, then every `every` until stopped (see
/// [`Control::pause`]: counted on the wall clock; while quiet, only when
/// nudged), each sync reported. After a lasting error (a refused password…)
/// it waits for a nudge instead of ending, then tries again (see
/// `fetch::Parked`: a refused password, unchanged, not before an hour unless
/// [`Control::retry`]).
pub fn watch(account: &Account, control: &Control, every: Duration, report: impl FnMut(Result<Report, SyncError>)) {
    watch_with(control, every, || sync(account), || crate::secret::password(account).ok(), report);
}

/// `watch`, syncing with `sync`; `password` reads the one kept now.
fn watch_with(control: &Control, every: Duration, mut sync: impl FnMut() -> Result<Report, SyncError>, password: impl Fn() -> Option<String>, mut report: impl FnMut(Result<Report, SyncError>)) {
    while !control.stopped() {
        let result = sync();
        let parked = result.as_ref().err().filter(|e| e.is_lasting()).map(|e| Parked::after(e, if e.wants_password() { password() } else { None }));
        report(result);
        let Some(parked) = parked else {
            control.pause(every);
            continue;
        };
        // A "Sync now" asked before this error is not an answer to it.
        control.retried();
        loop {
            if !control.park() {
                return;
            }
            if !parked.refused() || parked.again(password().as_deref()) || control.retried() {
                break;
            }
        }
    }
}

/// Goes on after an error of one collection: the first is kept, to say at
/// the end. A lasting one (a refused password, a certificate) stops there:
/// the others would meet it too, and retries can lock an account.
fn go_on(first: &mut Option<SyncError>, result: Result<(), SyncError>) -> Result<(), SyncError> {
    match result {
        Err(e) if !e.is_lasting() => {
            first.get_or_insert(e);
            Ok(())
        }
        other => other,
    }
}

/// Collections deleted on the server go here too: they were copies. One made
/// here and not created there yet stays, and so does one holding items never
/// sent, new or changed since (as Google Tasks' lists do).
fn forget_gone(account: &str, kind: Kind, listed: &[Listed]) {
    let kept: BTreeSet<&str> = listed.iter().map(|l| l.id.as_str()).collect();
    for collection in vdir::collections(kind).into_iter().filter(|c| c.account == account && !kept.contains(c.id.as_str())) {
        let state = State::load(&collection.state_path());
        let unsent = has_local_changes(&collection.dir, kind, &state);
        // Google's task lists are not on its CalDAV: Google Tasks says when they go.
        if state.pending || unsent || crate::google_tasks::is_tasks_list(&state.url) {
            continue;
        }
        let _ = std::fs::remove_dir_all(&collection.dir);
        let _ = std::fs::remove_file(collection.state_path());
    }
}

/// Tells the server the collections renamed here (PROPPATCH, RFC 4918 §9.2),
/// and deletes those deleted here (RFC 4918 §9.6), only when the server
/// holds nothing in them either: one filled elsewhere meanwhile comes back.
/// One refused, the others are still told; the first refusal is returned.
fn send_changes(client: &Client, account: &str, kind: Kind) -> Result<(), SyncError> {
    let mut refused = None;
    for collection in vdir::every_collection(kind).into_iter().filter(|c| c.account == account) {
        let path = collection.state_path();
        let mut state = State::load(&path);
        if state.url.is_empty() {
            continue;
        }
        if state.renamed {
            let body = format!(r#"<?xml version="1.0" encoding="utf-8"?><d:propertyupdate xmlns:d="DAV:"><d:set><d:prop><d:displayname>{}</d:displayname></d:prop></d:set></d:propertyupdate>"#, xml_escape(&collection.name));
            let answer = client.send("PROPPATCH", &state.url, &[("Content-Type", "application/xml; charset=utf-8")], Some(&body))?;
            if !(200..300).contains(&answer.status) {
                refused.get_or_insert(SyncError::Server(format!("PROPPATCH {}: {}", state.url, answer.status)));
                continue;
            }
            state.renamed = false;
            go_on(&mut refused, state.save(&path).map_err(SyncError::Disk))?;
        }
        if state.deleted {
            let (_, found) = client.propfind(&state.url, "1", "<d:getetag/>")?;
            let held = found.iter().filter(|r| path_key(&r.href) != path_key(&state.url)).count();
            if held > 0 {
                state.deleted = false;
            } else {
                let answer = client.send("DELETE", &state.url, &[], None)?;
                if !(200..300).contains(&answer.status) && answer.status != 404 {
                    refused.get_or_insert(SyncError::Server(format!("DELETE {}: {}", state.url, answer.status)));
                    continue;
                }
                let _ = std::fs::remove_dir_all(&collection.dir);
                let _ = std::fs::remove_file(&path);
                continue;
            }
            go_on(&mut refused, state.save(&path).map_err(SyncError::Disk))?;
        }
    }
    refused.map_or(Ok(()), Err)
}

/// Creates on the server the collections made here: a calendar or task list
/// with MKCALENDAR (RFC 4791 §5.3.1), an address book with an extended MKCOL
/// (RFC 5689), at `<home><id>/`, with its name, colour and, for a calendar,
/// what it holds. Already there (405) counts as made. One refused, the others
/// are still made; the first refusal is returned.
fn create_pending(client: &Client, account: &str, kind: Kind, home: &str) -> Result<(), SyncError> {
    let mut refused = None;
    for collection in vdir::collections(kind).into_iter().filter(|c| c.account == account) {
        let path = collection.state_path();
        let mut state = State::load(&path);
        if !state.pending {
            continue;
        }
        let url = format!("{}/{}/", home.trim_end_matches('/'), url_segment(&collection.id));
        let name = format!("<d:displayname>{}</d:displayname>", xml_escape(&collection.name));
        let color = collection.color.as_deref().map(|c| format!("<ic:calendar-color>{}</ic:calendar-color>", xml_escape(c))).unwrap_or_default();
        let (method, body) = match kind {
            Kind::Calendars => {
                let comps: String = state.components.iter().map(|c| format!(r#"<c:comp name="{}"/>"#, xml_escape(c))).collect();
                let set = if comps.is_empty() { String::new() } else { format!("<c:supported-calendar-component-set>{comps}</c:supported-calendar-component-set>") };
                (
                    "MKCALENDAR",
                    format!(r#"<?xml version="1.0" encoding="utf-8"?><c:mkcalendar xmlns:d="DAV:" xmlns:c="{CALDAV}" xmlns:ic="{APPLE}"><d:set><d:prop>{name}{color}{set}</d:prop></d:set></c:mkcalendar>"#),
                )
            }
            Kind::Contacts => (
                "MKCOL",
                format!(r#"<?xml version="1.0" encoding="utf-8"?><d:mkcol xmlns:d="DAV:" xmlns:a="{CARDDAV}"><d:set><d:prop><d:resourcetype><d:collection/><a:addressbook/></d:resourcetype>{name}</d:prop></d:set></d:mkcol>"#),
            ),
        };
        let answer = client.send(method, &url, &[("Content-Type", "application/xml; charset=utf-8")], Some(&body))?;
        match answer.status {
            200..=299 | 405 => {
                state.pending = false;
                state.url = url;
                go_on(&mut refused, state.save(&path).map_err(SyncError::Disk))?;
            }
            status => {
                refused.get_or_insert(SyncError::Server(format!("{method} {url}: {status}")));
            }
        }
    }
    refused.map_or(Ok(()), Err)
}

fn hash_of(path: &Path) -> Option<String> {
    read_item(path).ok().map(|bytes| vdir::content_hash(&bytes))
}

/// An item's file, read whole.
fn read_item(path: &Path) -> std::io::Result<Vec<u8>> {
    #[cfg(test)]
    tests::READS.with(|reads| reads.set(reads.get() + 1));
    std::fs::read(path)
}

/// An item's file as text; what cannot be read is said with its path.
fn read_text(path: &Path) -> Result<String, SyncError> {
    let fail = |e: &dyn std::fmt::Display| SyncError::Disk(format!("{}: {e}", path.display()));
    String::from_utf8(read_item(path).map_err(|e| fail(&e))?).map_err(|e| fail(&e))
}

/// A file name for an item, from its address: safe on every system, unique in its folder.
fn file_name(href: &str, extension: &str, taken: &BTreeSet<String>) -> String {
    let last = path_key(href).rsplit('/').next().unwrap_or("item").to_string();
    let stem = last.strip_suffix(&format!(".{extension}")).unwrap_or(&last);
    let stem: String = stem.chars().map(|c| if c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | '@') { c } else { '_' }).take(120).collect();
    // Not hidden (folders are read without their hidden files), and not one of
    // the names Windows keeps for its devices, whatever follows them ("CON.ics").
    let mut stem = stem.trim_start_matches('.').to_string();
    let device = stem.split('.').next().unwrap_or("").to_ascii_uppercase();
    if matches!(device.as_str(), "CON" | "PRN" | "AUX" | "NUL") || (device.len() == 4 && (device.starts_with("COM") || device.starts_with("LPT")) && device.ends_with(|c: char| c.is_ascii_digit())) {
        stem.insert(0, '_');
    }
    if stem.is_empty() {
        stem = vdir::new_name();
    }
    let mut name = format!("{stem}.{extension}");
    let mut n = 2;
    while taken.contains(&name) {
        name = format!("{stem}-{n}.{extension}");
        n += 1;
    }
    name
}

/// One collection: what changed here goes first, then what changed there. Its
/// state follows every answer and is kept whatever happens: what was sent or
/// brought before an error is known at the next sync, never sent twice.
fn sync_collection(client: &Client, account: &str, listed: &Listed, report: &mut Report) -> Result<(), SyncError> {
    let kind = listed.kind;
    let dir = vdir::prepare(kind, account, &listed.id, &listed.name, listed.color.as_deref()).map_err(|e| SyncError::Disk(e.to_string()))?;
    let state_path = vdir::state_path(account, kind, &listed.id);
    let mut state = State::load(&state_path);
    state.url = listed.url.clone();
    state.read_only = listed.read_only;
    state.components = listed.components.clone();
    let synced = both_ways(client, &dir, listed, &mut state, &state_path, report);
    state.save(&state_path).map_err(SyncError::Disk)?;
    synced
}

/// What changed here, sent; then, unless nothing changed on either side, what
/// changed there, brought. The ctag is kept once both went through; what the
/// server refused of an item is said once the rest is done.
fn both_ways(client: &Client, dir: &Path, listed: &Listed, state: &mut State, state_path: &Path, report: &mut Report) -> Result<(), SyncError> {
    let mut refetch: BTreeSet<String> = BTreeSet::new();
    let refused = if listed.read_only { None } else { push(client, listed.kind, dir, listed, state, &mut refetch, report)? };
    let unchanged = state.ctag.is_some() && state.ctag == listed.ctag && refetch.is_empty() && refused.is_none();
    if !unchanged {
        pull(client, dir, listed, state, state_path, &refetch, report)?;
    }
    state.ctag = listed.ctag.clone();
    refused.map_or(Ok(()), Err)
}

fn has_local_changes(dir: &Path, kind: Kind, state: &State) -> bool {
    let known: BTreeSet<&str> = state.items.iter().map(|i| i.file.as_str()).collect();
    let files = local_files(dir, kind);
    files.iter().any(|f| !known.contains(f.as_str())) || state.items.iter().any(|i| !unchanged(&dir.join(&i.file), i))
}

/// Whether an item's file is as last synced: by its size and time when
/// noted, else by its hash.
fn unchanged(path: &Path, item: &ItemState) -> bool {
    std::fs::metadata(path).is_ok_and(|m| as_noted(&m, item)) || hash_of(path).is_some_and(|hash| hash == item.hash)
}

/// A file's time is trusted to tell it unchanged once this old: a change made
/// within the same tick of the file system's clock (two seconds on FAT)
/// would keep it.
const SETTLED: Duration = Duration::from_secs(2);

/// A file's size and modification time (nanoseconds since 1970), noted to
/// tell it unchanged later without reading it; the time 0 (unknown) while
/// too recent, or ahead of now.
fn stamp(metadata: &std::fs::Metadata) -> (u64, i64) {
    let settled = metadata.modified().ok().filter(|m| std::time::SystemTime::now().duration_since(*m).is_ok_and(|age| age >= SETTLED));
    let mtime = settled.and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok()).and_then(|d| i64::try_from(d.as_nanos()).ok()).unwrap_or(0);
    (metadata.len(), mtime)
}

/// Whether a file is as noted for its item: the same size and the same settled time.
fn as_noted(metadata: &std::fs::Metadata, item: &ItemState) -> bool {
    item.mtime != 0 && stamp(metadata) == (item.size, item.mtime)
}

fn local_files(dir: &Path, kind: Kind) -> BTreeSet<String> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.ends_with(&format!(".{}", kind.extension())) && !n.starts_with('.'))
        .collect()
}

/// Sends what changed here: edits with If-Match, new items with If-None-Match,
/// deletions. The state follows each answer, so a push cut short keeps what
/// it did. What the server refuses of one item, or what cannot be read of it,
/// is returned, the others sent all the same; a lost connection stops it.
fn push(client: &Client, kind: Kind, dir: &Path, listed: &Listed, state: &mut State, refetch: &mut BTreeSet<String>, report: &mut Report) -> Result<Option<SyncError>, SyncError> {
    let mut refused = None;
    let mut i = 0;
    while i < state.items.len() {
        match send_known(client, kind, dir, listed, &state.items[i], refetch, report)? {
            Sent::Deleted => {
                state.items.remove(i);
                continue;
            }
            Sent::Agreed(item) => state.items[i] = item,
            Sent::Kept => {}
            Sent::Refused(e) => {
                refused.get_or_insert(e);
            }
        }
        i += 1;
    }
    let known: BTreeSet<String> = state.items.iter().map(|i| i.file.clone()).collect();
    for file in local_files(dir, kind).into_iter().filter(|f| !known.contains(f)) {
        let path = dir.join(&file);
        // Noted before it is read: a change made meanwhile is seen next time.
        let (size, mtime) = std::fs::metadata(&path).map(|m| stamp(&m)).unwrap_or_default();
        let text = match read_text(&path) {
            Ok(text) => text,
            Err(e) => {
                refused.get_or_insert(e);
                continue;
            }
        };
        let href = format!("{}/{}", listed.url.trim_end_matches('/'), url_segment(&file));
        if client.is_google() {
            match create_on_google(client, kind, listed, &file, &href, &text, refetch) {
                Ok(item) => {
                    state.items.push(ItemState { size, mtime, ..item });
                    report.sent += 1;
                    // Google may file it under its own name: the whole list is compared at the pull.
                    state.sync_token = None;
                }
                Err(e @ SyncError::Server(_)) => {
                    refused.get_or_insert(e);
                }
                Err(e) => return Err(e),
            }
            continue;
        }
        let answer = client.send("PUT", &href, &[("If-None-Match", "*"), ("Content-Type", kind.media_type())], Some(&text))?;
        // Refused because another item there holds its UID: an invitation
        // answered on another device first. That item takes this one's place.
        if matches!(answer.status, 400 | 403 | 409 | 412)
            && let Some(theirs) = same_uid_there(client, kind, listed, &href, &text, &answer.body)?
        {
            state.items.push(adopt(dir, listed, &file, &text, theirs, report)?);
            continue;
        }
        match answer.status {
            200..=299 => {
                report.sent += 1;
                let etag = answer.etag.unwrap_or_default();
                if etag.is_empty() {
                    refetch.insert(path_key(&href));
                }
                state.items.push(ItemState { href: path_key(&href), file, etag, hash: vdir::content_hash(text.as_bytes()), size, mtime });
            }
            // Already there: sent before and its answer lost, or another item
            // at that address. Known, never agreed (no ETag, no hash): what the
            // server holds comes at the pull, and yours is set aside then if it differs.
            412 => {
                refetch.insert(path_key(&href));
                state.items.push(ItemState { href: path_key(&href), file, ..ItemState::default() });
            }
            status => {
                refused.get_or_insert(SyncError::Server(format!("PUT {href}: {status}")));
            }
        }
    }
    Ok(refused)
}

/// The item the server holds under the UID of `text`, a new item it refused
/// at `href` because another holds that UID (RFC 4791 §5.3.2.1 and RFC 6352
/// §6.3.2.1, `no-uid-conflict`, said with 403 or 409; Nextcloud says 400, and
/// a server may say 412): where the refusal names it, else found by a query
/// on the UID (`calendar-query`, `addressbook-query`). Its address, ETag and
/// text; None when no other item there holds that UID (a server that answers
/// no query included), and the refusal is what it was.
fn same_uid_there(client: &Client, kind: Kind, listed: &Listed, href: &str, text: &str, refusal: &str) -> Result<Option<(String, String, String)>, SyncError> {
    let Some(uid) = uid_of(text) else { return Ok(None) };
    let mut hrefs: Vec<String> = named_in_refusal(refusal).into_iter().collect();
    if hrefs.is_empty() {
        hrefs = holding_uid(client, kind, &listed.url, &uid, &component_of(text))?;
    }
    let mine = path_key(href);
    hrefs.retain(|h| path_key(&absolute(&listed.url, h)) != mine);
    if hrefs.is_empty() {
        return Ok(None);
    }
    Ok(fetch(client, kind, &listed.url, &hrefs)?.into_iter().find(|(_, _, data)| uid_of(data).as_deref() == Some(uid.as_str())))
}

/// An item's UID, as its first `UID` line says.
fn uid_of(text: &str) -> Option<String> {
    sioul_core::lines::unfold(text).iter().find(|l| sioul_core::lines::name(l) == "UID").map(|l| sioul_core::lines::value(l).trim().to_string()).filter(|uid| !uid.is_empty())
}

/// The component a calendar item holds ("VEVENT", "VTODO"…), for a query.
fn component_of(text: &str) -> String {
    sioul_core::lines::unfold(text)
        .iter()
        .filter(|l| sioul_core::lines::name(l) == "BEGIN")
        .map(|l| sioul_core::lines::value(l).trim().to_ascii_uppercase())
        .find(|c| !matches!(c.as_str(), "VCALENDAR" | "VTIMEZONE" | "STANDARD" | "DAYLIGHT" | "VALARM"))
        .unwrap_or_else(|| "VEVENT".into())
}

/// The item a `no-uid-conflict` refusal names, when it names one.
fn named_in_refusal(body: &str) -> Option<String> {
    let doc = roxmltree::Document::parse(body).ok()?;
    let clash = doc.descendants().find(|n| n.tag_name().name() == "no-uid-conflict")?;
    clash.descendants().find(|n| n.has_tag_name((DAV, "href"))).and_then(|n| n.text()).map(|h| h.trim().to_string()).filter(|h| !h.is_empty())
}

/// The items of a collection holding `uid`, by a query (RFC 4791 §7.8, RFC
/// 6352 §8.6); none from a server that answers no query.
fn holding_uid(client: &Client, kind: Kind, url: &str, uid: &str, component: &str) -> Result<Vec<String>, SyncError> {
    let (uid, component) = (xml_escape(uid), xml_escape(component));
    let body = match kind {
        Kind::Calendars => format!(
            r#"<?xml version="1.0" encoding="utf-8"?><c:calendar-query xmlns:d="DAV:" xmlns:c="{CALDAV}"><d:prop><d:getetag/></d:prop><c:filter><c:comp-filter name="VCALENDAR"><c:comp-filter name="{component}"><c:prop-filter name="UID"><c:text-match>{uid}</c:text-match></c:prop-filter></c:comp-filter></c:comp-filter></c:filter></c:calendar-query>"#
        ),
        Kind::Contacts => format!(
            r#"<?xml version="1.0" encoding="utf-8"?><a:addressbook-query xmlns:d="DAV:" xmlns:a="{CARDDAV}"><d:prop><d:getetag/></d:prop><a:filter><a:prop-filter name="UID"><a:text-match match-type="equals">{uid}</a:text-match></a:prop-filter></a:filter></a:addressbook-query>"#
        ),
    };
    let answer = client.send("REPORT", url, &[("Depth", "1"), ("Content-Type", "application/xml; charset=utf-8")], Some(&body))?;
    if answer.status != 207 {
        return Ok(Vec::new());
    }
    let Ok((responses, _)) = multistatus(&answer.body) else { return Ok(Vec::new()) };
    let collection = path_key(url);
    Ok(responses.into_iter().filter(|r| r.status.is_none_or(|s| s / 100 == 2) && path_key(&absolute(url, &r.href)) != collection).map(|r| r.href).collect())
}

/// A new item here (`file`, holding `mine`) whose UID the server holds
/// already under another address, `theirs`: that item takes its place, under
/// its file, never sent again. An invitation answered on two devices: the
/// same answers (`agenda::answers`), taken as it is; others, the server's
/// kept, and said (`Report::answered`): the organizer may have had both. An
/// item without guests (a contact, a task) that differs: yours set aside
/// first, as any item changed on both sides.
fn adopt(dir: &Path, listed: &Listed, file: &str, mine: &str, (href, etag, data): (String, String, String), report: &mut Report) -> Result<ItemState, SyncError> {
    let path = dir.join(file);
    let (ours, theirs) = (sioul_core::agenda::answers(mine), sioul_core::agenda::answers(&data));
    if ours != theirs {
        // The guest whose answer differs: you, answering on each device.
        let kept = theirs.iter().find(|(who, said)| ours.get(*who) != Some(*said)).map_or_else(String::new, |(_, said)| said.clone());
        report.answered.push((summary_of(&data), kept));
    } else if ours.is_empty() && !same_text(mine.as_bytes(), &data) {
        report.conflicts.push(set_aside(&path, mine.as_bytes())?);
    }
    let text = crlf(&data);
    vdir::write_item(&path, &text).map_err(SyncError::Disk)?;
    report.received += 1;
    // Written now, its time not trusted yet (`SETTLED`): read once at the next sync.
    Ok(ItemState { href: path_key(&absolute(&listed.url, &href)), file: file.to_string(), etag, hash: vdir::content_hash(text.as_bytes()), ..ItemState::default() })
}

/// An event's title, as its first `SUMMARY` line says.
fn summary_of(text: &str) -> String {
    sioul_core::lines::unfold(text).iter().find(|l| sioul_core::lines::name(l) == "SUMMARY").map(|l| sioul_core::lines::unescape(sioul_core::lines::value(l).trim())).unwrap_or_default()
}

/// What became of an item known to the server, at a push.
enum Sent {
    /// Deleted there as it was here: forgotten.
    Deleted,
    /// Its state now: sent (the server's new ETag), or found unchanged (its size and time noted).
    Agreed(ItemState),
    /// As it was: unchanged, or to read again at the pull.
    Kept,
    /// Refused by the server, or unreadable here.
    Refused(SyncError),
}

/// One item known to the server, sent if it changed here, deleted there if
/// deleted here. A file whose size and time are as noted is not read.
fn send_known(client: &Client, kind: Kind, dir: &Path, listed: &Listed, item: &ItemState, refetch: &mut BTreeSet<String>, report: &mut Report) -> Result<Sent, SyncError> {
    let path = dir.join(&item.file);
    let url = absolute(&listed.url, &item.href);
    let unreadable = |e: std::io::Error| Sent::Refused(SyncError::Disk(format!("{}: {e}", path.display())));
    let metadata = match std::fs::metadata(&path) {
        Ok(metadata) => metadata,
        // Deleted here: deleted there, unless it changed there meanwhile.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let answer = client.send("DELETE", &url, &[("If-Match", &item.etag)], None)?;
            return Ok(match answer.status {
                200..=299 | 404 => {
                    report.sent += 1;
                    Sent::Deleted
                }
                412 => {
                    refetch.insert(path_key(&url));
                    Sent::Kept
                }
                status => Sent::Refused(SyncError::Server(format!("DELETE {url}: {status}"))),
            });
        }
        Err(e) => return Ok(unreadable(e)),
    };
    if as_noted(&metadata, item) {
        return Ok(Sent::Kept);
    }
    // Noted before it is read: a change made meanwhile is seen next time.
    let (size, mtime) = stamp(&metadata);
    let bytes = match read_item(&path) {
        Ok(bytes) => bytes,
        Err(e) => return Ok(unreadable(e)),
    };
    let hash = vdir::content_hash(&bytes);
    if hash == item.hash {
        // Unchanged, its size and time noted: not read next time.
        return Ok(if (size, mtime) == (item.size, item.mtime) { Sent::Kept } else { Sent::Agreed(ItemState { size, mtime, ..item.clone() }) });
    }
    if item.hash.is_empty() {
        // Never agreed: found there when sent as new; compared at the pull.
        refetch.insert(path_key(&url));
        return Ok(Sent::Kept);
    }
    let text = match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(e) => return Ok(Sent::Refused(SyncError::Disk(format!("{}: {e}", path.display())))),
    };
    let answer = client.send("PUT", &url, &[("If-Match", &item.etag), ("Content-Type", kind.media_type())], Some(&text))?;
    Ok(match answer.status {
        200..=299 => {
            report.sent += 1;
            // Without an ETag, the server changed what it stored: it comes back at the pull.
            let etag = answer.etag.unwrap_or_default();
            if etag.is_empty() {
                refetch.insert(path_key(&url));
            }
            Sent::Agreed(ItemState { etag, hash, size, mtime, ..item.clone() })
        }
        // Changed on both sides: theirs comes back at the pull, and yours is set aside then.
        412 => {
            refetch.insert(path_key(&url));
            Sent::Kept
        }
        status => Sent::Refused(SyncError::Server(format!("PUT {url}: {status}"))),
    })
}

/// A new item on Google: no If-None-Match (it takes If-Match only), cards in
/// vCard 3.0, and a new contact by POST when its address book refuses the
/// PUT (RFC 5995). Where Google filed it (its Location) is where it is known;
/// what Google kept of it comes back at the pull.
fn create_on_google(client: &Client, kind: Kind, listed: &Listed, file: &str, href: &str, text: &str, refetch: &mut BTreeSet<String>) -> Result<ItemState, SyncError> {
    let sent = if kind == Kind::Contacts { sioul_core::contacts::as_vcard3(text) } else { text.to_string() };
    let mut answer = client.send("PUT", href, &[("Content-Type", kind.media_type())], Some(&sent))?;
    if kind == Kind::Contacts && matches!(answer.status, 403 | 405 | 409) {
        answer = client.send("POST", &listed.url, &[("Content-Type", kind.media_type())], Some(&sent))?;
    }
    if !(200..300).contains(&answer.status) {
        return Err(SyncError::Server(format!("PUT {href}: {}", answer.status)));
    }
    let at = answer.location.as_deref().map_or_else(|| path_key(href), |l| path_key(&absolute(&listed.url, l)));
    refetch.insert(at.clone());
    Ok(ItemState { href: at, file: file.to_string(), etag: answer.etag.unwrap_or_default(), hash: vdir::content_hash(text.as_bytes()), ..ItemState::default() })
}

/// A file name as a URL path segment.
fn url_segment(name: &str) -> String {
    name.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'@' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Your version of an item changed on both sides, kept where you can find it,
/// on a phone too: among this device's earlier versions (`history`, the part
/// `history::ACCOUNTS`, named by the item's place in the data folder), which
/// the window lists and puts back (Settings ▸ Your folder and sharing ▸ Show
/// earlier versions), sharing on or off. Until 10 October 2026 it went to
/// `$XDG_STATE_HOME/sioul/dav/conflicts`, out of reach on a phone. Not kept (a
/// full disk), the sync stops there: the server's version does not take its
/// place, and yours is never lost.
fn set_aside(path: &Path, mine: &[u8]) -> Result<PathBuf, SyncError> {
    let root = crate::history::root(&state_dir().join("share").join("memory.json"));
    let data = sioul_core::config::data_dir();
    let file = path.strip_prefix(&data).ok().map(|relative| format!("data/{}", relative.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/"))).unwrap_or_else(|| format!("data/{}", path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()));
    crate::history::keep_bytes(&root, crate::history::ACCOUNTS, &file, mine, jiff::Timestamp::now().as_millisecond()).map_err(SyncError::Disk)
}

/// An item's file, when it changed here since the server last agreed on it (or never did).
fn changed_here(path: &Path, item: &ItemState) -> Option<Vec<u8>> {
    if std::fs::metadata(path).is_ok_and(|m| as_noted(&m, item)) {
        return None;
    }
    read_item(path).ok().filter(|bytes| item.hash.is_empty() || vdir::content_hash(bytes) != item.hash)
}

/// Lines ended with CRLF, as items are written here (RFC 5545 §3.1, RFC 6350 §3.2).
fn crlf(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\n', "\r\n")
}

/// Whether two versions of an item say the same, however their lines end.
fn same_text(mine: &[u8], theirs: &str) -> bool {
    crlf(&String::from_utf8_lossy(mine)).trim_end() == crlf(theirs).trim_end()
}

/// Brings what changed there: by sync token, else by comparing ETags. The
/// state is kept after each batch, so a pull cut short loses one batch at
/// most; the sync token moves once everything came.
fn pull(client: &Client, dir: &Path, listed: &Listed, state: &mut State, state_path: &Path, refetch: &BTreeSet<String>, report: &mut Report) -> Result<(), SyncError> {
    let key = |href: &str| path_key(&absolute(&listed.url, href));
    let by_href: BTreeMap<String, usize> = state.items.iter().enumerate().map(|(i, item)| (key(&item.href), i)).collect();
    let (changed, gone, token) = match state.sync_token.clone().and_then(|token| changes_since(client, &listed.url, &token).ok().flatten()) {
        Some(found) => found,
        None => everything(client, &listed.url, state, &by_href)?,
    };
    // Gone there: gone here. Changed here meanwhile (an edit saved during the
    // sync, or one the server refused), yours is set aside first.
    let gone: BTreeSet<String> = gone.iter().map(|h| key(h)).collect();
    let mut i = 0;
    while i < state.items.len() {
        if !gone.contains(&key(&state.items[i].href)) {
            i += 1;
            continue;
        }
        let path = dir.join(&state.items[i].file);
        if let Some(mine) = changed_here(&path, &state.items[i]) {
            report.conflicts.push(set_aside(&path, &mine)?);
        }
        let _ = std::fs::remove_file(&path);
        state.items.remove(i);
        report.removed += 1;
    }
    // Changed there, or to read again after a send.
    let known: BTreeMap<String, usize> = state.items.iter().enumerate().map(|(i, item)| (key(&item.href), i)).collect();
    let mut wanted: BTreeMap<String, String> = changed
        .into_iter()
        .filter_map(|(href, etag)| {
            let at = key(&href);
            let stale = known.get(&at).is_none_or(|&i| etag.as_deref().is_none_or(|e| e != state.items[i].etag));
            (stale || refetch.contains(&at)).then_some((at, href))
        })
        .collect();
    for at in refetch {
        if let Some(&i) = known.get(at) {
            wanted.entry(at.clone()).or_insert_with(|| state.items[i].href.clone());
        }
    }
    let hrefs: Vec<String> = wanted.into_values().collect();
    let mut at = 0;
    while at < hrefs.len() {
        // As many as the last answers allow (`pace`): fewer on a slow line, 50 again once it is quick.
        let batch = &hrefs[at..(at + batch_of(&listed.url)).min(hrefs.len())];
        let started = Instant::now();
        let fetched = fetch(client, listed.kind, &listed.url, batch);
        match &fetched {
            Ok(_) => pace(&listed.url, Some(started.elapsed())),
            Err(SyncError::Network(_)) => pace(&listed.url, None),
            Err(_) => {}
        }
        for (href, etag, data) in fetched? {
            store(dir, listed.kind, state, &listed.url, (&href, &etag, &data), report)?;
        }
        // Kept as it goes: cut short, the next sync goes on from here.
        state.save(state_path).map_err(SyncError::Disk)?;
        at += batch.len();
    }
    state.sync_token = token.or_else(|| listed.sync_token.clone());
    Ok(())
}

/// RFC 6578: what changed since `token`: (changed with their ETags, gone, the new token).
/// None when the server has forgotten the token or does not keep them.
#[allow(clippy::type_complexity)]
fn changes_since(client: &Client, url: &str, token: &str) -> Result<Option<(Vec<(String, Option<String>)>, Vec<String>, Option<String>)>, SyncError> {
    let body = format!(
        r#"<?xml version="1.0" encoding="utf-8"?><d:sync-collection xmlns:d="DAV:"><d:sync-token>{}</d:sync-token><d:sync-level>1</d:sync-level><d:prop><d:getetag/></d:prop></d:sync-collection>"#,
        xml_escape(token)
    );
    let answer = client.send("REPORT", url, &[("Depth", "0"), ("Content-Type", "application/xml; charset=utf-8")], Some(&body))?;
    if answer.status != 207 {
        return Ok(None);
    }
    let (responses, new_token) = multistatus(&answer.body)?;
    let collection = path_key(url);
    let mut changed = Vec::new();
    let mut gone = Vec::new();
    for response in responses.into_iter().filter(|r| path_key(&absolute(url, &r.href)) != collection) {
        if response.status == Some(404) {
            gone.push(response.href);
        } else {
            changed.push((response.href.clone(), response.text(DAV, "getetag")));
        }
    }
    Ok(Some((changed, gone, new_token)))
}

/// Every item's ETag, compared with what is known: (changed, gone, no token).
#[allow(clippy::type_complexity)]
fn everything(client: &Client, url: &str, state: &State, known: &BTreeMap<String, usize>) -> Result<(Vec<(String, Option<String>)>, Vec<String>, Option<String>), SyncError> {
    let (status, responses) = client.propfind(url, "1", "<d:getetag/><d:resourcetype/>")?;
    if status != 207 {
        return Err(SyncError::Server(format!("PROPFIND {url}: {status}")));
    }
    let collection = path_key(url);
    let items: Vec<&Response> = responses.iter().filter(|r| path_key(&absolute(url, &r.href)) != collection && !r.is(DAV, "collection")).collect();
    let present: BTreeSet<String> = items.iter().map(|r| path_key(&absolute(url, &r.href))).collect();
    let changed = items.iter().map(|r| (r.href.clone(), r.text(DAV, "getetag"))).collect();
    let gone = known.keys().filter(|k| !present.contains(*k)).filter_map(|k| known.get(k).map(|&i| state.items[i].href.clone())).collect();
    Ok((changed, gone, None))
}

/// Text made safe in XML, between tags and between quotes (a component's `name="…"`).
fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Items by their addresses, with their ETags: a multiget, else one GET each.
fn fetch(client: &Client, kind: Kind, url: &str, hrefs: &[String]) -> Result<Vec<(String, String, String)>, SyncError> {
    if hrefs.is_empty() {
        return Ok(Vec::new());
    }
    let (root, ns, data) = match kind {
        Kind::Calendars => ("c:calendar-multiget", CALDAV, "calendar-data"),
        Kind::Contacts => ("a:addressbook-multiget", CARDDAV, "address-data"),
    };
    let prefix = if kind == Kind::Calendars { "c" } else { "a" };
    let list: String = hrefs.iter().map(|h| format!("<d:href>{}</d:href>", xml_escape(h))).collect();
    let body = format!(
        r#"<?xml version="1.0" encoding="utf-8"?><{root} xmlns:d="DAV:" xmlns:c="{CALDAV}" xmlns:a="{CARDDAV}"><d:prop><d:getetag/><{prefix}:{data}/></d:prop>{list}</{root}>"#
    );
    let answer = client.send("REPORT", url, &[("Depth", "1"), ("Content-Type", "application/xml; charset=utf-8")], Some(&body))?;
    if answer.status == 207 {
        let (responses, _) = multistatus(&answer.body)?;
        let found: Vec<(String, String, String)> = responses
            .iter()
            .filter_map(|r| Some((r.href.clone(), r.text(DAV, "getetag").unwrap_or_default(), r.prop(ns, data).map(|p| p.text.clone()).filter(|t| !t.trim().is_empty())?)))
            .collect();
        if !found.is_empty() || hrefs.is_empty() {
            return Ok(found);
        }
    }
    // Some servers answer multigets poorly: one GET each.
    let mut found = Vec::new();
    for href in hrefs {
        let answer = client.send("GET", &absolute(url, href), &[], None)?;
        if answer.status / 100 == 2 {
            found.push((href.clone(), answer.etag.unwrap_or_default(), answer.body));
        }
    }
    Ok(found)
}

/// Writes an item brought from the server (href, ETag, text). Your version,
/// changed here since the server last agreed on it (or never agreed), is set
/// aside first, unless it is the same.
fn store(dir: &Path, kind: Kind, state: &mut State, url: &str, (href, etag, data): (&str, &str, &str), report: &mut Report) -> Result<(), SyncError> {
    let key = path_key(&absolute(url, href));
    let position = state.items.iter().position(|i| path_key(&absolute(url, &i.href)) == key);
    let file = match position {
        Some(i) => state.items[i].file.clone(),
        None => file_name(href, kind.extension(), &state.items.iter().map(|i| i.file.clone()).chain(local_files(dir, kind)).collect()),
    };
    let path = dir.join(&file);
    if let Some(i) = position
        && let Some(mine) = changed_here(&path, &state.items[i])
        && !same_text(&mine, data)
    {
        report.conflicts.push(set_aside(&path, &mine)?);
    }
    let text = crlf(data);
    vdir::write_item(&path, &text).map_err(SyncError::Disk)?;
    // Written now, its time not trusted yet (`SETTLED`): read once at the next sync.
    let item = ItemState { href: href.to_string(), file, etag: etag.to_string(), hash: vdir::content_hash(text.as_bytes()), ..ItemState::default() };
    match position {
        Some(i) => state.items[i] = item,
        None => state.items.push(item),
    }
    report.received += 1;
    Ok(())
}

/// Tests the login and finds where the account's contacts and calendars are.
pub fn test(address: &str, login: &str, password: &str, given: Option<&str>, mail_host: Option<&str>) -> Result<Homes, SyncError> {
    discover(&Client::new(login, password), address, given, mail_host)
}

/// Finds a Google account's calendars and contacts, once signed in.
pub fn test_google(address: &str) -> Result<Homes, SyncError> {
    discover(&Client::google(address)?, address, None, None)
}

/// Servers on this computer, for this crate's tests: a small HTTP/1.1 server,
/// a CalDAV server in memory on it with faults to inject, and the home the
/// tests write in.
#[cfg(test)]
pub(crate) mod stand_in {
    use std::collections::{BTreeMap, BTreeSet};
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
    use std::time::Duration;

    /// The tests' home: the XDG folders point into it, set once, before any
    /// test reading them goes on (each asks for the home first, and waits here
    /// while it is made); the other tests of this crate read none. Each test
    /// syncs an account of its own, so they run side by side.
    pub(crate) fn home() -> &'static Path {
        static HOME: OnceLock<PathBuf> = OnceLock::new();
        HOME.get_or_init(|| {
            forget_old_homes();
            let root = std::env::temp_dir().join(format!("sioul-sync-tests-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            for (variable, folder) in [("XDG_CONFIG_HOME", "config"), ("XDG_DATA_HOME", "data"), ("XDG_STATE_HOME", "state"), ("XDG_CACHE_HOME", "cache")] {
                let dir = root.join(folder);
                std::fs::create_dir_all(&dir).unwrap();
                // SAFETY: set once, inside this initialiser, before any test
                // that reads these folders goes on (see above).
                unsafe { std::env::set_var(variable, &dir) };
            }
            root
        })
    }

    /// The homes of earlier runs, gone: those whose run is over (no such
    /// process, where that can be told), or an hour old; a run going on beside
    /// this one keeps its own.
    fn forget_old_homes() {
        let hour_ago = std::time::SystemTime::now() - Duration::from_secs(3600);
        for entry in std::fs::read_dir(std::env::temp_dir()).into_iter().flatten().filter_map(Result::ok) {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some(process) = name.strip_prefix("sioul-sync-tests-") else { continue };
            let over = cfg!(target_os = "linux") && !Path::new("/proc").join(process).exists();
            let old = entry.metadata().and_then(|m| m.modified()).is_ok_and(|t| t < hour_ago);
            if over || old {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
    }

    /// A request, as read.
    pub(crate) struct Request {
        pub method: String,
        /// Its path, as sent.
        pub path: String,
        headers: Vec<(String, String)>,
        pub body: String,
    }

    impl Request {
        pub fn header(&self, name: &str) -> Option<&str> {
            self.headers.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
        }
    }

    pub(crate) struct Reply {
        pub status: u16,
        pub headers: Vec<(&'static str, String)>,
        pub body: String,
    }

    impl Reply {
        pub fn new(status: u16, body: impl Into<String>) -> Reply {
            Reply { status, headers: Vec::new(), body: body.into() }
        }
    }

    /// How a server misbehaves, for one request.
    #[derive(Debug, Clone, Copy)]
    pub(crate) enum Fault {
        /// Answers this status, doing nothing.
        Status(u16),
        /// Closes the connection, doing nothing: the network dropped.
        Drop,
        /// Does it, then closes without answering: the answer was lost.
        Lost,
        /// Waits this long, then does it.
        Late(Duration),
        /// Does it, and sends the answer's body in pieces over this long.
        Trickle(Duration),
    }

    /// A stand-in: how it misbehaves for a request, if at all, and its answer, the request done.
    pub(crate) trait Server: Send + Sync + 'static {
        fn misbehaves(&self, request: &Request) -> Option<Fault>;
        fn answer(&self, request: &Request) -> Reply;
    }

    /// Serves on 127.0.0.1 until the tests end, a thread and a request per
    /// connection. Its address: "http://127.0.0.1:port".
    pub(crate) fn serve(server: Arc<dyn Server>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        std::thread::spawn(move || {
            for stream in listener.incoming().filter_map(Result::ok) {
                let server = Arc::clone(&server);
                std::thread::spawn(move || exchange(stream, server.as_ref()));
            }
        });
        base
    }

    fn exchange(mut stream: TcpStream, server: &dyn Server) {
        let Some(request) = read(&stream) else { return };
        let fault = server.misbehaves(&request);
        let reply = match fault {
            Some(Fault::Status(status)) => Reply::new(status, ""),
            Some(Fault::Drop) => return,
            Some(Fault::Lost) => {
                server.answer(&request);
                return;
            }
            Some(Fault::Late(wait)) => {
                std::thread::sleep(wait);
                server.answer(&request)
            }
            Some(Fault::Trickle(_)) | None => server.answer(&request),
        };
        let headers: String = reply.headers.iter().map(|(name, value)| format!("{name}: {value}\r\n")).collect();
        let head = format!("HTTP/1.1 {} Stand-in\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n", reply.status, reply.body.len());
        if stream.write_all(head.as_bytes()).is_err() {
            return;
        }
        match fault {
            Some(Fault::Trickle(over)) => {
                let pieces = 30;
                for piece in reply.body.as_bytes().chunks(reply.body.len() / pieces + 1) {
                    std::thread::sleep(over / pieces as u32);
                    if stream.write_all(piece).and_then(|()| stream.flush()).is_err() {
                        return;
                    }
                }
            }
            _ => {
                let _ = stream.write_all(reply.body.as_bytes());
            }
        }
    }

    fn read(stream: &TcpStream) -> Option<Request> {
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).ok()?;
        let mut words = line.split_whitespace();
        let (method, path) = (words.next()?.to_string(), words.next()?.to_string());
        let mut headers: Vec<(String, String)> = Vec::new();
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).ok()?;
            let Some((name, value)) = line.trim_end().split_once(':') else { break };
            headers.push((name.trim().to_string(), value.trim().to_string()));
        }
        let length = headers.iter().find(|(n, _)| n.eq_ignore_ascii_case("content-length")).and_then(|(_, v)| v.parse().ok()).unwrap_or(0);
        let mut body = vec![0; length];
        reader.read_exact(&mut body).ok()?;
        Some(Request { method, path, headers, body: String::from_utf8_lossy(&body).into_owned() })
    }

    /// A fault for the `nth` request (from 1) that `matches`, once.
    pub(crate) fn nth(n: usize, matches: impl Fn(&Request) -> bool + Send + 'static, fault: Fault) -> impl FnMut(&Request) -> Option<Fault> + Send + 'static {
        let mut count = 0;
        move |request| {
            if !matches(request) {
                return None;
            }
            count += 1;
            (count == n).then_some(fault)
        }
    }

    type Faults = Vec<Box<dyn FnMut(&Request) -> Option<Fault> + Send>>;

    /// A CalDAV server in memory: calendars under /cal/, every change numbered
    /// (ETags, ctags and sync tokens come from that), faults to inject, and
    /// every request seen.
    #[derive(Default)]
    pub(crate) struct Dav {
        held: Mutex<Held>,
        faults: Mutex<Faults>,
    }

    #[derive(Default)]
    struct Held {
        /// Calendar ("a") → item ("x.ics") → (ETag, text).
        calendars: BTreeMap<String, BTreeMap<String, (String, String)>>,
        version: u64,
        /// Every change: (version, calendar, item).
        changes: Vec<(u64, String, String)>,
        /// "PUT /cal/a/x.ics 201", in order.
        seen: Vec<String>,
        /// A new item whose UID another item of its calendar holds is refused
        /// with this status, the other named in a `no-uid-conflict` or not.
        uid_clash: Option<(u16, bool)>,
        /// Another server's address (`http://127.0.0.1:port`), which this one
        /// names as a hostile one would: a calendar "theirs" there among its
        /// own, and an item there in each of its calendars (`name_elsewhere`).
        elsewhere: Option<String>,
    }

    impl Dav {
        /// A server with these calendars, empty; and the address of their home.
        pub(crate) fn start(calendars: &[&str]) -> (Arc<Dav>, String) {
            let dav = Arc::new(Dav::default());
            for name in calendars {
                dav.held().calendars.insert(name.to_string(), BTreeMap::new());
            }
            let base = serve(dav.clone());
            (dav, format!("{base}/cal/"))
        }

        fn held(&self) -> MutexGuard<'_, Held> {
            self.held.lock().unwrap()
        }

        /// An item written there, as another device does.
        pub(crate) fn put(&self, calendar: &str, item: &str, text: &str) {
            self.held().store(calendar, item, text);
        }

        /// An item deleted there, as another device does.
        pub(crate) fn delete(&self, calendar: &str, item: &str) {
            self.held().delete(calendar, item);
        }

        /// A calendar's items: name → text.
        pub(crate) fn items(&self, calendar: &str) -> BTreeMap<String, String> {
            self.held().calendars[calendar].iter().map(|(name, (_, text))| (name.clone(), text.clone())).collect()
        }

        /// The requests seen, in order: "PUT /cal/a/x.ics 201".
        pub(crate) fn seen(&self) -> Vec<String> {
            self.held().seen.clone()
        }

        /// A fault, asked of each request until it says one.
        pub(crate) fn fault(&self, fault: impl FnMut(&Request) -> Option<Fault> + Send + 'static) {
            self.faults.lock().unwrap().push(Box::new(fault));
        }

        /// From now on, one UID per calendar: a new item holding another's
        /// is refused with `status`, the other named when `named` (RFC 4791
        /// §5.3.2.1), or not (Nextcloud's 400).
        pub(crate) fn one_uid_each(&self, status: u16, named: bool) {
            self.held().uid_clash = Some((status, named));
        }

        /// From now on, its answers name addresses on the server at `base` too.
        pub(crate) fn name_elsewhere(&self, base: &str) {
            self.held().elsewhere = Some(base.to_string());
        }
    }

    /// An item's UID line, as the tests write them.
    fn uid_line(text: &str) -> Option<&str> {
        text.lines().find_map(|l| l.trim_end_matches('\r').strip_prefix("UID:"))
    }

    impl Server for Dav {
        fn misbehaves(&self, request: &Request) -> Option<Fault> {
            let fault = self.faults.lock().unwrap().iter_mut().find_map(|f| f(request));
            // Late or slow, it is answered all the same, and seen then.
            if let Some(fault @ (Fault::Status(_) | Fault::Drop | Fault::Lost)) = fault {
                self.held().seen.push(format!("{} {} {fault:?}", request.method, request.path));
            }
            fault
        }

        fn answer(&self, request: &Request) -> Reply {
            let mut held = self.held();
            let reply = held.answer(request);
            held.seen.push(format!("{} {} {}", request.method, request.path, reply.status));
            reply
        }
    }

    fn found(href: &str, props: &str) -> String {
        format!("<d:response><d:href>{href}</d:href><d:propstat><d:prop>{props}</d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>")
    }

    fn missing(href: &str) -> String {
        format!("<d:response><d:href>{href}</d:href><d:status>HTTP/1.1 404 Not Found</d:status></d:response>")
    }

    fn multistatus(responses: &str) -> Reply {
        Reply::new(207, format!(r#"<?xml version="1.0" encoding="utf-8"?><d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav" xmlns:cs="http://calendarserver.org/ns/">{responses}</d:multistatus>"#))
    }

    fn etag_prop(etag: &str) -> String {
        format!("<d:getetag>{}</d:getetag>", super::xml_escape(etag))
    }

    impl Held {
        fn store(&mut self, calendar: &str, item: &str, text: &str) -> String {
            self.version += 1;
            let etag = format!("\"e{}\"", self.version);
            self.calendars.get_mut(calendar).expect("a calendar of the stand-in").insert(item.to_string(), (etag.clone(), text.to_string()));
            self.changes.push((self.version, calendar.to_string(), item.to_string()));
            etag
        }

        fn delete(&mut self, calendar: &str, item: &str) {
            if self.calendars.get_mut(calendar).and_then(|items| items.remove(item)).is_some() {
                self.version += 1;
                self.changes.push((self.version, calendar.to_string(), item.to_string()));
            }
        }

        /// The last change of a calendar: its ctag and its sync token.
        fn last(&self, calendar: &str) -> u64 {
            self.changes.iter().filter(|(_, c, _)| c == calendar).map(|(v, _, _)| *v).max().unwrap_or(0)
        }

        fn calendar_props(&self, calendar: &str) -> String {
            let last = self.last(calendar);
            format!("<d:resourcetype><d:collection/><c:calendar/></d:resourcetype><d:displayname>{calendar}</d:displayname><cs:getctag>ctag-{last}</cs:getctag><d:sync-token>token-{last}</d:sync-token>")
        }

        fn answer(&mut self, request: &Request) -> Reply {
            let path = super::path_key(&request.path);
            let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
            match (request.method.as_str(), parts.as_slice()) {
                ("PROPFIND", ["cal"]) => {
                    let mut out = found("/cal/", "<d:resourcetype><d:collection/></d:resourcetype>");
                    for calendar in self.calendars.keys() {
                        out += &found(&format!("/cal/{calendar}/"), &self.calendar_props(calendar));
                    }
                    if let Some(base) = &self.elsewhere {
                        out += &found(&format!("{base}/cal/theirs/"), "<d:resourcetype><d:collection/><c:calendar/></d:resourcetype><d:displayname>theirs</d:displayname>");
                    }
                    multistatus(&out)
                }
                (_, ["cal", calendar, ..]) if !self.calendars.contains_key(*calendar) => Reply::new(404, ""),
                ("PROPFIND", ["cal", calendar]) => {
                    let mut out = found(&format!("/cal/{calendar}/"), &self.calendar_props(calendar));
                    for (item, (etag, _)) in &self.calendars[*calendar] {
                        out += &found(&format!("/cal/{calendar}/{item}"), &format!("{}<d:resourcetype/>", etag_prop(etag)));
                    }
                    if let Some(base) = &self.elsewhere {
                        out += &found(&format!("{base}/cal/{calendar}/there.ics"), &format!("{}<d:resourcetype/>", etag_prop("\"there\"")));
                    }
                    multistatus(&out)
                }
                ("REPORT", ["cal", calendar]) if request.body.contains("sync-collection") => self.changes_since(calendar, &request.body),
                ("REPORT", ["cal", calendar]) if request.body.contains("calendar-query") => self.query(calendar, &request.body),
                ("REPORT", ["cal", calendar]) => self.multiget(calendar, &request.body),
                ("PUT", ["cal", calendar, item]) => {
                    let current = self.calendars[*calendar].get(*item).map(|(etag, _)| etag.clone());
                    let refused = match (request.header("If-None-Match"), request.header("If-Match")) {
                        (Some("*"), _) => current.is_some(),
                        (_, Some(wanted)) => current.as_deref() != Some(wanted),
                        _ => false,
                    };
                    if refused {
                        return Reply::new(412, "");
                    }
                    let holder = uid_line(&request.body).and_then(|uid| self.calendars[*calendar].iter().find(|(name, (_, text))| name.as_str() != *item && uid_line(text) == Some(uid))).map(|(name, _)| name.clone());
                    if current.is_none()
                        && let (Some((status, named)), Some(holder)) = (self.uid_clash, holder)
                    {
                        let body = if named {
                            format!(r#"<?xml version="1.0" encoding="utf-8"?><d:error xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav"><c:no-uid-conflict><d:href>/cal/{calendar}/{holder}</d:href></c:no-uid-conflict></d:error>"#)
                        } else {
                            "Calendar object with uid already exists in this calendar collection.".to_string()
                        };
                        return Reply::new(status, body);
                    }
                    let etag = self.store(calendar, item, &request.body);
                    Reply { status: if current.is_some() { 204 } else { 201 }, headers: vec![("ETag", etag)], body: String::new() }
                }
                ("DELETE", ["cal", calendar, item]) => match (self.calendars[*calendar].get(*item), request.header("If-Match")) {
                    (None, _) => Reply::new(404, ""),
                    (Some((etag, _)), Some(wanted)) if etag != wanted => Reply::new(412, ""),
                    _ => {
                        self.delete(calendar, item);
                        Reply::new(204, "")
                    }
                },
                ("GET", ["cal", calendar, item]) => match self.calendars[*calendar].get(*item) {
                    Some((etag, text)) => Reply { status: 200, headers: vec![("ETag", etag.clone())], body: text.clone() },
                    None => Reply::new(404, ""),
                },
                _ => Reply::new(405, ""),
            }
        }

        /// RFC 6578: what changed since a token of this calendar; a token it never gave is refused.
        fn changes_since(&self, calendar: &str, body: &str) -> Reply {
            let doc = roxmltree::Document::parse(body).unwrap();
            let token = doc.descendants().find(|n| n.has_tag_name(("DAV:", "sync-token"))).and_then(|n| n.text()).unwrap_or("");
            let Some(since) = token.trim().strip_prefix("token-").and_then(|t| t.parse::<u64>().ok()) else { return Reply::new(403, "") };
            let changed: BTreeSet<&str> = self.changes.iter().filter(|(v, c, _)| *v > since && c == calendar).map(|(_, _, item)| item.as_str()).collect();
            let mut out = String::new();
            for item in changed {
                let href = format!("/cal/{calendar}/{item}");
                out += &match self.calendars[calendar].get(item) {
                    Some((etag, _)) => found(&href, &etag_prop(etag)),
                    None => missing(&href),
                };
            }
            out += &format!("<d:sync-token>token-{}</d:sync-token>", self.last(calendar));
            multistatus(&out)
        }

        /// RFC 4791 §7.8: the items whose UID holds the text asked (a UID filter, as `holding_uid` asks).
        fn query(&self, calendar: &str, body: &str) -> Reply {
            let doc = roxmltree::Document::parse(body).unwrap();
            let uid = doc.descendants().find(|n| n.tag_name().name() == "text-match").and_then(|n| n.text()).unwrap_or("").to_string();
            let mut out = String::new();
            for (item, (etag, text)) in &self.calendars[calendar] {
                if !uid.is_empty() && uid_line(text).is_some_and(|u| u.contains(&uid)) {
                    out += &found(&format!("/cal/{calendar}/{item}"), &etag_prop(etag));
                }
            }
            multistatus(&out)
        }

        fn multiget(&self, calendar: &str, body: &str) -> Reply {
            let doc = roxmltree::Document::parse(body).unwrap();
            let mut out = String::new();
            for href in doc.descendants().filter(|n| n.has_tag_name(("DAV:", "href"))).filter_map(|n| n.text()) {
                let key = super::path_key(href);
                let item = key.rsplit('/').next().unwrap_or("");
                out += &match self.calendars[calendar].get(item) {
                    Some((etag, text)) => found(href, &format!("{}<c:calendar-data>{}</c:calendar-data>", etag_prop(etag), super::xml_escape(text))),
                    None => missing(href),
                };
            }
            multistatus(&out)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::stand_in::{Dav, Fault, home, nth};
    use super::*;
    use std::sync::Arc;

    /// A contacts-and-calendars account, the test's own.
    fn account(id: &str) -> Account {
        home();
        toml::from_str(&format!("id = \"{id}\"\nkind = \"dav\"\naddress = \"jane@example.org\"\nhost = \"127.0.0.1\"\n")).unwrap()
    }

    fn client() -> Client {
        Client::new("jane", "secret")
    }

    fn event(uid: &str, summary: &str) -> String {
        format!(
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Sioul//tests//EN\r\nBEGIN:VEVENT\r\nUID:{uid}\r\nDTSTAMP:20261005T120000Z\r\nDTSTART:20261006T090000Z\r\nDTEND:20261006T100000Z\r\nSUMMARY:{summary}\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n"
        )
    }

    /// A calendar as the stand-in lists it now.
    fn listed(client: &Client, home: &str, name: &str) -> Listed {
        collections(client, Kind::Calendars, home).unwrap().into_iter().find(|l| l.id == name).unwrap()
    }

    /// One calendar synced, as the account's sync does it.
    fn sync_one(client: &Client, account: &str, home: &str, name: &str) -> Result<Report, SyncError> {
        let mut report = Report::default();
        sync_collection(client, account, &listed(client, home, name), &mut report).map(|()| report)
    }

    fn folder(account: &str, calendar: &str) -> PathBuf {
        Kind::Calendars.root().join(account).join(calendar)
    }

    fn state_of(account: &str, calendar: &str) -> State {
        State::load(&vdir::state_path(account, Kind::Calendars, calendar))
    }

    fn puts(dav: &Dav) -> Vec<String> {
        dav.seen().into_iter().filter(|s| s.starts_with("PUT")).collect()
    }

    /// F1: the first pull of a big calendar, cut at its second batch (Android
    /// killed Sioul, the network dropped), resumes at the next sync: nothing
    /// sent back, nothing set aside.
    #[test]
    fn an_interrupted_pull_resumes() {
        home();
        let (dav, home) = Dav::start(&["big"]);
        for n in 0..120 {
            dav.put("big", &format!("e{n}.ics"), &event(&format!("uid-{n}"), &format!("Event {n}")));
        }
        dav.fault(nth(2, |r| r.method == "REPORT" && r.body.contains("multiget"), Fault::Drop));
        let client = client();
        assert!(matches!(sync_one(&client, "resumes", &home, "big"), Err(SyncError::Network(_))));
        let report = sync_one(&client, "resumes", &home, "big").unwrap();
        assert_eq!((report.sent, report.received, report.conflicts.len()), (0, 70, 0), "{report:?}");
        assert_eq!(local_files(&folder("resumes", "big"), Kind::Calendars).len(), 120);
        assert_eq!(dav.items("big").len(), 120);
        assert!(puts(&dav).is_empty(), "{:?}", dav.seen());
    }

    /// F1: an event made here is sent, then the network drops before the
    /// calendar is read: the next sync knows it was sent.
    #[test]
    fn a_sent_event_is_not_sent_again() {
        home();
        let (dav, home) = Dav::start(&["work"]);
        dav.put("work", "known.ics", &event("known", "Known"));
        let client = client();
        sync_one(&client, "sent-once", &home, "work").unwrap();
        dav.put("work", "theirs.ics", &event("theirs", "Theirs"));
        std::fs::write(folder("sent-once", "work").join("mine.ics"), event("mine", "Mine")).unwrap();
        dav.fault(nth(1, |r| r.method == "REPORT" && r.body.contains("sync-collection"), Fault::Drop));
        dav.fault(nth(1, |r| r.method == "PROPFIND" && r.path == "/cal/work/", Fault::Drop));
        assert!(sync_one(&client, "sent-once", &home, "work").is_err());
        let report = sync_one(&client, "sent-once", &home, "work").unwrap();
        assert_eq!((report.sent, report.received, report.conflicts.len()), (0, 1, 0), "{report:?}");
        assert_eq!(puts(&dav), ["PUT /cal/work/mine.ics 201"]);
        assert_eq!(dav.items("work").len(), 3);
    }

    /// F1: the answer to a new event's PUT is lost (stored there, the
    /// connection dropped): sent again, it is found there, the same: no
    /// conflict. Another item at that address is one: the server's kept, yours set aside.
    #[test]
    fn a_lost_answer_is_not_a_conflict() {
        home();
        let (dav, home) = Dav::start(&["home"]);
        let client = client();
        sync_one(&client, "lost", &home, "home").unwrap();
        let dir = folder("lost", "home");
        std::fs::write(dir.join("new.ics"), event("new", "New")).unwrap();
        dav.fault(nth(1, |r| r.method == "PUT", Fault::Lost));
        assert!(matches!(sync_one(&client, "lost", &home, "home"), Err(SyncError::Network(_))));
        let report = sync_one(&client, "lost", &home, "home").unwrap();
        assert!(report.conflicts.is_empty(), "{report:?}");
        assert_eq!(dav.items("home").len(), 1);
        assert_eq!(std::fs::read_to_string(dir.join("new.ics")).unwrap(), event("new", "New"));

        dav.put("home", "clash.ics", &event("theirs", "Theirs"));
        std::fs::write(dir.join("clash.ics"), event("mine", "Mine")).unwrap();
        let report = sync_one(&client, "lost", &home, "home").unwrap();
        assert_eq!(report.conflicts.len(), 1, "{report:?}");
        assert!(std::fs::read_to_string(&report.conflicts[0]).unwrap().contains("SUMMARY:Mine"));
        assert!(std::fs::read_to_string(dir.join("clash.ics")).unwrap().contains("SUMMARY:Theirs"));
        let report = sync_one(&client, "lost", &home, "home").unwrap();
        assert_eq!((report.sent, report.received, report.conflicts.len()), (0, 0, 0), "{report:?}");
    }

    /// A server that only notes what it is asked, and with what login.
    #[derive(Default)]
    struct Elsewhere {
        seen: Mutex<Vec<String>>,
    }

    impl stand_in::Server for Elsewhere {
        fn misbehaves(&self, _: &stand_in::Request) -> Option<Fault> {
            None
        }

        fn answer(&self, request: &stand_in::Request) -> stand_in::Reply {
            self.seen.lock().unwrap().push(format!("{} {} {}", request.method, request.path, request.header("Authorization").unwrap_or("")));
            stand_in::Reply::new(404, "")
        }
    }

    /// A server whose answers name another server (a calendar there, an item
    /// there, as a hostile or broken one would): the login never goes there,
    /// the sync says so, and its own calendars sync all the same.
    #[test]
    fn the_login_stays_on_the_account_s_server() {
        let account = account("kept-to");
        let elsewhere = Arc::new(Elsewhere::default());
        let there = stand_in::serve(elsewhere.clone());
        // "mine" holds an item of its own; "only-there" none but the one it names there,
        // which a multiget does not bring, so that it is asked for alone.
        let (dav, home) = Dav::start(&["mine", "only-there"]);
        Homes { calendars: Some(home.clone()), contacts: None }.save(&account.id).unwrap();
        dav.put("mine", "new.ics", &event("new", "New here"));
        dav.name_elsewhere(&there);
        let result = sync_with(&client(), &account);
        assert!(matches!(&result, Err(SyncError::Elsewhere(url)) if url.starts_with(&there)), "{result:?}");
        assert!(folder("kept-to", "mine").join("new.ics").exists());
        assert!(dav.seen().iter().any(|s| s.starts_with("REPORT /cal/only-there/")), "{:?}", dav.seen());
        assert!(elsewhere.seen.lock().unwrap().is_empty(), "{:?}", elsewhere.seen.lock().unwrap());
        // Said in words, with the address.
        let said = result.unwrap_err().sentence(&sioul_core::i18n::Translator::new("en"), "Calendars");
        assert!(said.starts_with("Calendars: the server named an address on another server") && said.contains(&there), "{said}");
        // Its own server's addresses go through, whatever their spelling.
        let kept = client();
        kept.keep_to(&Homes { calendars: Some(home.clone()), contacts: None });
        assert!(kept.mine(&format!("{home}mine/new.ics")).is_ok());
        assert!(kept.mine(&home.replacen("http://", "HTTP://", 1)).is_ok());
        assert!(kept.mine(&format!("{there}/cal/")).is_err());
        assert!(kept.mine("https://127.0.0.1/cal/").is_err());
        // Before the homes are known, finding them keeps to the domain started from.
        let pointing = stand_in::serve(Arc::new(Pointing(format!("{}/principals/jane/", there.replace("127.0.0.1", "localhost")))));
        assert!(matches!(super::home(&client(), Kind::Calendars, &format!("{pointing}/")), Err(SyncError::Elsewhere(url)) if url.contains("localhost")));
        assert!(elsewhere.seen.lock().unwrap().is_empty());
    }

    /// A server whose principal is at the address it is given.
    struct Pointing(String);

    impl stand_in::Server for Pointing {
        fn misbehaves(&self, _: &stand_in::Request) -> Option<Fault> {
            None
        }

        fn answer(&self, _: &stand_in::Request) -> stand_in::Reply {
            stand_in::Reply::new(
                207,
                format!(
                    r#"<?xml version="1.0" encoding="utf-8"?><d:multistatus xmlns:d="DAV:"><d:response><d:href>/</d:href><d:propstat><d:prop><d:current-user-principal><d:href>{}</d:href></d:current-user-principal></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response></d:multistatus>"#,
                    self.0
                ),
            )
        }
    }

    #[test]
    fn origins_are_scheme_host_and_port() {
        assert_eq!(origin("https://Dav.Example.org/remote.php/dav/").as_deref(), Some("https://dav.example.org:443"));
        assert_eq!(origin("https://dav.example.org:443/x"), origin("https://dav.example.org/y?z"));
        assert_ne!(origin("https://dav.example.org:8443/"), origin("https://dav.example.org/"));
        assert_ne!(origin("http://dav.example.org/"), origin("https://dav.example.org/"));
        // A login before the host is not the host.
        assert_eq!(origin("https://dav.example.org@other.example.org/").as_deref(), Some("https://other.example.org:443"));
        assert_eq!(origin("https://[2001:db8::1]:8443/").as_deref(), Some("https://[2001:db8::1]:8443"));
        assert_eq!(origin("https://[2001:db8::1]/").as_deref(), Some("https://[2001:db8::1]:443"));
        assert_eq!(origin("/cal/a/"), None);
        assert_eq!(origin("https://dav.example.org:x/"), None);
        assert_eq!(origin("ftp://dav.example.org/"), None);
    }

    /// F1: a calendar the server fails on, or whose connection drops, does
    /// not keep the others from syncing.
    #[test]
    fn one_bad_calendar_does_not_stop_the_others() {
        let account = account("bad-one");
        let (dav, home) = Dav::start(&["a", "b"]);
        Homes { calendars: Some(home), contacts: None }.save(&account.id).unwrap();
        dav.put("b", "new.ics", &event("new", "New in b"));
        dav.fault(nth(1, |r| r.method == "PROPFIND" && r.path == "/cal/a/", Fault::Status(500)));
        let result = sync_with(&client(), &account);
        assert!(matches!(&result, Err(SyncError::Server(e)) if e.contains("/cal/a/")), "{result:?}");
        assert!(folder("bad-one", "b").join("new.ics").exists());
        dav.put("b", "newer.ics", &event("newer", "Newer in b"));
        dav.fault(nth(1, |r| r.method == "PROPFIND" && r.path == "/cal/a/", Fault::Drop));
        assert!(matches!(sync_with(&client(), &account), Err(SyncError::Network(_))));
        assert!(folder("bad-one", "b").join("newer.ics").exists());
        assert_eq!(sync_with(&client(), &account).unwrap().collections, 2);
    }

    thread_local! {
        /// Item files read on this thread: each test syncs on its own.
        pub(super) static READS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }

    fn reads() -> usize {
        READS.with(std::cell::Cell::get)
    }

    /// F9: a sync with nothing changed here reads no file, each known by its
    /// size and time; a file edited since is read, and sent.
    #[test]
    fn unchanged_files_are_not_read() {
        home();
        let (dav, home) = Dav::start(&["many"]);
        for n in 0..1000 {
            dav.put("many", &format!("e{n}.ics"), &event(&format!("u{n}"), "Same"));
        }
        let client = client();
        sync_one(&client, "unread", &home, "many").unwrap();
        // Written by that sync a moment ago, their times are not trusted yet; an hour later they are.
        let dir = folder("unread", "many");
        let hour_ago = std::time::SystemTime::now() - Duration::from_secs(3600);
        for file in local_files(&dir, Kind::Calendars) {
            std::fs::File::options().write(true).open(dir.join(file)).unwrap().set_modified(hour_ago).unwrap();
        }
        sync_one(&client, "unread", &home, "many").unwrap();
        let before = reads();
        assert_eq!(sync_one(&client, "unread", &home, "many").unwrap().sent, 0);
        assert_eq!(reads() - before, 0, "nothing read");
        std::fs::write(dir.join("e7.ics"), event("u7", "Edited")).unwrap();
        let before = reads();
        assert_eq!(sync_one(&client, "unread", &home, "many").unwrap().sent, 1);
        assert_eq!(reads() - before, 1, "the edited file only");
        assert!(dav.items("many")["e7.ics"].contains("SUMMARY:Edited"));
    }

    /// F7: an answer that keeps coming, however slowly, is read to its end
    /// (here over six times the stall limit); one that stops coming is given
    /// up after one stall, not after the whole budget.
    #[test]
    fn a_slow_answer_is_read_while_it_moves() {
        home();
        let (dav, home) = Dav::start(&["slow"]);
        for n in 0..40 {
            dav.put("slow", &format!("e{n}.ics"), &event(&format!("u{n}"), "Slow"));
        }
        let budget = Budget { connect: Duration::from_secs(5), stall: Duration::from_millis(500), whole: Duration::from_secs(60) };
        let client = Client { agent: agent(&budget), ..client() };
        dav.fault(nth(1, |r| r.body.contains("multiget"), Fault::Trickle(Duration::from_secs(3))));
        let started = std::time::Instant::now();
        assert_eq!(sync_one(&client, "slow", &home, "slow").unwrap().received, 40);
        assert!(started.elapsed() > Duration::from_secs(3));
        dav.put("slow", "late.ics", &event("late", "Late"));
        dav.fault(nth(1, |r| r.body.contains("sync-collection"), Fault::Late(Duration::from_secs(5))));
        dav.fault(nth(1, |r| r.method == "PROPFIND" && r.path == "/cal/slow/", Fault::Late(Duration::from_secs(5))));
        let started = std::time::Instant::now();
        assert!(matches!(sync_one(&client, "slow", &home, "slow"), Err(SyncError::Network(_))));
        assert!(started.elapsed() < Duration::from_secs(3), "{:?}", started.elapsed());
    }

    /// Item 12 of 9 October: a slow multiget halves the next batch of its
    /// collection, a cut one too (at the next sync), and quick answers grow
    /// it back, twice as large each time, to 50.
    #[test]
    fn batches_follow_the_answers() {
        home();
        let (dav, home) = Dav::start(&["paced", "cut"]);
        for n in 0..200 {
            dav.put("paced", &format!("e{n}.ics"), &event(&format!("p{n}"), "Paced"));
        }
        for n in 0..120 {
            dav.put("cut", &format!("e{n}.ics"), &event(&format!("c{n}"), "Cut"));
        }
        // How many items each multiget asks for, by calendar.
        let asked = std::sync::Arc::new(Mutex::new(Vec::<(String, usize)>::new()));
        let noted = std::sync::Arc::clone(&asked);
        dav.fault(move |r| {
            if r.body.contains("multiget") {
                noted.lock().unwrap().push((r.path.clone(), r.body.matches("<d:href>").count()));
            }
            None
        });
        let of = |calendar: &str| asked.lock().unwrap().iter().filter(|(path, _)| *path == format!("/cal/{calendar}/")).map(|(_, n)| *n).collect::<Vec<_>>();
        dav.fault(nth(1, |r| r.path == "/cal/paced/" && r.body.contains("multiget"), Fault::Late(SLOW + Duration::from_millis(300))));
        let client = client();
        assert_eq!(sync_one(&client, "paced", &home, "paced").unwrap().received, 200);
        assert_eq!(of("paced"), [50, 25, 50, 50, 25]);
        dav.fault(nth(1, |r| r.path == "/cal/cut/" && r.body.contains("multiget"), Fault::Drop));
        assert!(matches!(sync_one(&client, "paced-cut", &home, "cut"), Err(SyncError::Network(_))));
        assert_eq!(sync_one(&client, "paced-cut", &home, "cut").unwrap().received, 120);
        assert_eq!(of("cut"), [50, 25, 50, 45]);
    }

    /// Item 5 of 9 October: an invitation answered on two devices. The phone
    /// answered first: its copy is on the server. The computer's, sent as new
    /// under another name before its sync brought the phone's, meets that
    /// UID there (403 or 409 naming the other item, RFC 4791; Nextcloud's 400
    /// or a 412 naming none: found by a query): the server's item takes its
    /// place, in one file, sent no more. The same answer is said nowhere;
    /// another answer, the server's is kept, and said.
    #[test]
    fn an_invitation_answered_on_two_devices() {
        home();
        let invitation = |uid: &str, answer: &str| {
            format!(
                "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Sioul//tests//EN\r\nBEGIN:VEVENT\r\nUID:{uid}\r\nDTSTAMP:20261005T120000Z\r\nDTSTART:20261012T090000Z\r\nDTEND:20261012T100000Z\r\nSUMMARY:Review\r\nORGANIZER;CN=Jane:mailto:jane@example.org\r\nATTENDEE;CN=Me;PARTSTAT={answer}:mailto:me@example.net\r\nATTENDEE;CN=Paul:mailto:paul@example.org\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n"
            )
        };
        for (n, (status, named)) in [(403, true), (409, true), (400, false), (412, false)].into_iter().enumerate() {
            let (dav, home) = Dav::start(&["invited"]);
            dav.one_uid_each(status, named);
            let (client, account) = (client(), format!("invited-{n}"));
            sync_one(&client, &account, &home, "invited").unwrap();
            let dir = folder(&account, "invited");
            // Accepted on the phone, then on the computer.
            dav.put("invited", "phone.ics", &invitation("review-1", "ACCEPTED"));
            std::fs::write(dir.join("desk.ics"), invitation("review-1", "ACCEPTED")).unwrap();
            let report = sync_one(&client, &account, &home, "invited").unwrap();
            assert!(report.answered.is_empty() && report.conflicts.is_empty(), "{status}: {report:?}");
            assert_eq!(local_files(&dir, Kind::Calendars), BTreeSet::from(["desk.ics".to_string()]), "{status}: one file, under its name here");
            assert_eq!(dav.items("invited").len(), 1, "{status}");
            let again = sync_one(&client, &account, &home, "invited").unwrap();
            assert_eq!((again.sent, again.received), (0, 0), "{status}: settled: {again:?}");
            // Maybe on the phone, accepted on the computer: the phone's answer is kept here, and said.
            dav.put("invited", "phone-2.ics", &invitation("review-2", "TENTATIVE"));
            std::fs::write(dir.join("desk-2.ics"), invitation("review-2", "ACCEPTED")).unwrap();
            let report = sync_one(&client, &account, &home, "invited").unwrap();
            assert_eq!(report.answered, [("Review".to_string(), "TENTATIVE".to_string())], "{status}: {report:?}");
            assert!(report.conflicts.is_empty(), "{status}: {report:?}");
            assert!(std::fs::read_to_string(dir.join("desk-2.ics")).unwrap().contains("PARTSTAT=TENTATIVE:mailto:me@example.net"), "{status}");
            assert_eq!(local_files(&dir, Kind::Calendars).len(), 2, "{status}");
            assert!(dav.items("invited")["phone-2.ics"].contains("PARTSTAT=TENTATIVE"), "{status}: the server's left as it was");
            assert!(!puts(&dav).iter().any(|p| p.ends_with(" 201") || p.ends_with(" 204")), "{status}: nothing written there: {:?}", dav.seen());
        }
    }

    /// A2: after a refused password the watcher waits, parked, instead of
    /// ending: nudged with the same password kept, it does not try again
    /// (retries can lock an account), unless "Sync now" asks; with another
    /// password, it does at once.
    #[test]
    fn a_refused_password_parks_the_watcher() {
        let control = Control::default();
        let syncs = std::sync::atomic::AtomicUsize::new(0);
        let kept = std::sync::Mutex::new("old".to_string());
        let (said, heard) = std::sync::mpsc::channel();
        std::thread::scope(|s| {
            s.spawn(|| {
                let sync = || match syncs.fetch_add(1, std::sync::atomic::Ordering::Relaxed) {
                    0 | 1 => Err(SyncError::Login("PROPFIND: 401".into())),
                    _ => Ok(Report::default()),
                };
                watch_with(&control, Duration::from_secs(3600), sync, || Some(kept.lock().unwrap().clone()), |result| said.send(result.is_ok()).unwrap());
            });
            assert_eq!(heard.recv_timeout(Duration::from_secs(5)), Ok(false), "refused");
            control.nudge();
            assert!(heard.recv_timeout(Duration::from_millis(300)).is_err(), "the same password: parked");
            control.retry();
            assert_eq!(heard.recv_timeout(Duration::from_secs(5)), Ok(false), "Sync now: tried, refused again");
            *kept.lock().unwrap() = "new".into();
            control.nudge();
            assert_eq!(heard.recv_timeout(Duration::from_secs(5)), Ok(true), "another: synced at once");
            control.stop();
        });
        assert_eq!(syncs.load(std::sync::atomic::Ordering::Relaxed), 3);
    }

    /// F8: an event deleted there while changed here: your version is set
    /// aside before its file goes, never lost.
    #[test]
    fn a_change_to_an_item_gone_there_is_set_aside() {
        home();
        let (dav, home) = Dav::start(&["gone"]);
        dav.put("gone", "x.ics", &event("x", "Before"));
        let client = client();
        sync_one(&client, "gone-edited", &home, "gone").unwrap();
        let file = folder("gone-edited", "gone").join("x.ics");
        dav.delete("gone", "x.ics");
        std::fs::write(&file, event("x", "Edited here")).unwrap();
        let report = sync_one(&client, "gone-edited", &home, "gone").unwrap();
        assert_eq!((report.removed, report.conflicts.len()), (1, 1), "{report:?}");
        assert!(std::fs::read_to_string(&report.conflicts[0]).unwrap().contains("SUMMARY:Edited here"));
        assert!(!file.exists());
        assert!(state_of("gone-edited", "gone").items.is_empty());
    }

    /// F5: two syncs of one account at once (the window's watcher and `sioul
    /// task add`) take turns: a new event is sent once, and known once.
    #[test]
    fn one_sync_of_an_account_at_a_time() {
        let account = account("one-at-a-time");
        let (dav, home) = Dav::start(&["a"]);
        Homes { calendars: Some(home), contacts: None }.save(&account.id).unwrap();
        sync_with(&client(), &account).unwrap();
        std::fs::write(folder("one-at-a-time", "a").join("new.ics"), event("new", "New")).unwrap();
        dav.fault(|r| (r.method == "PUT").then_some(Fault::Late(Duration::from_millis(300))));
        let both: Vec<_> = (0..2)
            .map(|_| {
                let account = account.clone();
                std::thread::spawn(move || sync_with(&client(), &account))
            })
            .collect();
        for sync in both {
            sync.join().unwrap().unwrap();
        }
        assert_eq!(puts(&dav), ["PUT /cal/a/new.ics 201"]);
        assert_eq!(state_of("one-at-a-time", "a").items.iter().filter(|i| i.file == "new.ics").count(), 1);
    }

    /// F1: a push cut by the network keeps its place: the items sent before
    /// carry their new ETags, the others are still known as they were.
    #[test]
    fn a_cut_push_keeps_its_place() {
        home();
        let (dav, home) = Dav::start(&["four"]);
        for n in 0..4 {
            dav.put("four", &format!("e{n}.ics"), &event(&format!("u{n}"), "Before"));
        }
        let client = client();
        sync_one(&client, "cut-push", &home, "four").unwrap();
        let dir = folder("cut-push", "four");
        for n in 0..4 {
            std::fs::write(dir.join(format!("e{n}.ics")), event(&format!("u{n}"), "After")).unwrap();
        }
        let before = state_of("cut-push", "four").items;
        dav.fault(nth(3, |r| r.method == "PUT", Fault::Drop));
        let mut state = state_of("cut-push", "four");
        let (mut refetch, mut report) = (BTreeSet::new(), Report::default());
        assert!(push(&client, Kind::Calendars, &dir, &listed(&client, &home, "four"), &mut state, &mut refetch, &mut report).is_err());
        assert_eq!(state.items.len(), 4, "every item still known: {:?}", state.items);
        for (now, then) in state.items.iter().zip(&before).take(2) {
            assert_ne!(now.etag, then.etag);
            assert_eq!(Some(now.hash.clone()), hash_of(&dir.join(&now.file)));
        }
        assert_eq!(state.items[2..], before[2..]);
    }

    #[test]
    fn reads_a_multistatus() {
        let xml = r#"<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:" xmlns:cal="urn:ietf:params:xml:ns:caldav" xmlns:x="http://apple.com/ns/ical/">
 <d:response><d:href>/dav/calendars/jane/personal/</d:href>
  <d:propstat><d:prop><d:resourcetype><d:collection/><cal:calendar/></d:resourcetype><d:displayname>Personal</d:displayname>
   <x:calendar-color>#4C6B5CFF</x:calendar-color><cal:supported-calendar-component-set><cal:comp name="VEVENT"/></cal:supported-calendar-component-set>
   <d:current-user-privilege-set><d:privilege><d:read/></d:privilege></d:current-user-privilege-set></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat>
  <d:propstat><d:prop><d:getctag/></d:prop><d:status>HTTP/1.1 404 Not Found</d:status></d:propstat></d:response>
 <d:response><d:href>/dav/calendars/jane/personal/gone.ics</d:href><d:status>HTTP/1.1 404 Not Found</d:status></d:response>
 <d:sync-token>http://example.org/sync/42</d:sync-token>
</d:multistatus>"#;
        let (responses, token) = multistatus(xml).unwrap();
        assert_eq!(token.as_deref(), Some("http://example.org/sync/42"));
        assert!(responses[0].is(CALDAV, "calendar"));
        assert_eq!(responses[0].text(DAV, "displayname").as_deref(), Some("Personal"));
        assert_eq!(responses[0].prop(CALDAV, "supported-calendar-component-set").unwrap().comps, vec!["VEVENT"]);
        assert!(responses[0].prop(CALSERVER, "getctag").is_none());
        assert_eq!(responses[1].status, Some(404));
    }

    #[test]
    fn addresses_and_names() {
        assert_eq!(absolute("https://h.example/remote.php/dav/", "/remote.php/dav/principals/jane/"), "https://h.example/remote.php/dav/principals/jane/");
        assert_eq!(absolute("https://h.example/a/b/", "c.ics"), "https://h.example/a/b/c.ics");
        assert_eq!(path_key("https://h.example/a%20b/c.ics"), "/a b/c.ics");
        assert_eq!(path_key("/a/b/"), "/a/b");
        let taken: BTreeSet<String> = ["abc.ics".to_string()].into_iter().collect();
        assert_eq!(file_name("/cal/abc.ics", "ics", &taken), "abc-2.ics");
        assert_eq!(file_name("/cal/a%20b:c.ics", "ics", &BTreeSet::new()), "a_b_c.ics");
        assert_eq!(file_name("/cal/..hidden.ics", "ics", &BTreeSet::new()), "hidden.ics", "never a hidden file");
        assert_eq!(file_name("/book/CON.vcf", "vcf", &BTreeSet::new()), "_CON.vcf", "nor a name Windows keeps");
        assert_eq!(file_name("/book/com1.x.vcf", "vcf", &BTreeSet::new()), "_com1.x.vcf");
        assert_eq!(url_segment("sioul-1.vcf"), "sioul-1.vcf");
        assert!(allowed("http://example.org/").is_err());
        // Redirects carry the password: followed within the domain asked only.
        assert_eq!(domain("https://caldav.example.org:8443/dav/"), domain("https://example.org/.well-known/caldav"));
        assert_ne!(domain("https://parking.example.net/x"), domain("https://example.org/.well-known/caldav"));
        assert_eq!(xml_escape(r#"a<"b"&"#), "a&lt;&quot;b&quot;&amp;");
    }
}
