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
//!   (`calendar-multiget`, `addressbook-multiget`).
//! - **When both changed the same item**, the server's version wins and yours
//!   is kept aside in `$XDG_STATE_HOME/sioul/dav/conflicts`, said in the report.
//!
//! Only HTTPS, with the password in the system keyring and sent as Basic
//! authentication over TLS; for Google, its OAuth access token instead
//! (google.rs). A small client on `ureq`, written for this: the protocol
//! needed is a handful of requests.

use crate::SyncError;
use sioul_core::config::{Account, state_dir};
use sioul_core::vdir::{self, ItemState, Kind, State};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

const DAV: &str = "DAV:";
const CALDAV: &str = "urn:ietf:params:xml:ns:caldav";
const CARDDAV: &str = "urn:ietf:params:xml:ns:carddav";
const CALSERVER: &str = "http://calendarserver.org/ns/";
const APPLE: &str = "http://apple.com/ns/ical/";

/// Items asked for in one multiget.
const BATCH: usize = 50;

/// A connection to one server, with its login.
pub struct Client {
    agent: ureq::Agent,
    authorization: Mutex<String>,
    /// A Google account's address: its access token ends after an hour, and a
    /// refused one is asked again, once.
    google: Option<String>,
}

/// What a server answered.
struct Answer {
    status: u16,
    etag: Option<String>,
    location: Option<String>,
    body: String,
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(60))).http_status_as_error(false).max_redirects(0).allow_non_standard_methods(true).build().into()
}

impl Client {
    pub fn new(login: &str, password: &str) -> Client {
        Client { agent: agent(), authorization: Mutex::new(format!("Basic {}", sioul_core::lines::base64_encode(format!("{login}:{password}").as_bytes()))), google: None }
    }

    /// A Google account, signed in with its access token.
    pub fn google(address: &str) -> Result<Client, SyncError> {
        let token = crate::google::access_token(address, false)?;
        Ok(Client { agent: agent(), authorization: Mutex::new(format!("Bearer {token}")), google: Some(address.to_string()) })
    }

    pub fn is_google(&self) -> bool {
        self.google.is_some()
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
fn allowed(url: &str) -> Result<(), SyncError> {
    if url.starts_with("https://") {
        return Ok(());
    }
    #[cfg(feature = "insecure-test-tls")]
    if std::env::var_os("SIOUL_TEST_INSECURE_TLS").is_some() && (url.starts_with("http://localhost") || url.starts_with("http://127.0.0.1")) {
        return Ok(());
    }
    Err(SyncError::Tls(format!("{url}: not encrypted")))
}

/// One `response` of a multistatus: an address and what was found there.
#[derive(Debug, Default, Clone)]
struct Response {
    href: String,
    /// The status of the whole response (sync-collection says 404 for what went).
    status: Option<u16>,
    props: Vec<Prop>,
}

/// One property, found (200) or not.
#[derive(Debug, Default, Clone)]
struct Prop {
    ns: String,
    name: String,
    status: u16,
    text: String,
    hrefs: Vec<String>,
    /// Child elements, by namespace and name (resourcetype, privileges).
    children: Vec<(String, String)>,
    /// `name` attributes of `comp` children (supported-calendar-component-set).
    comps: Vec<String>,
}

impl Response {
    fn prop(&self, ns: &str, name: &str) -> Option<&Prop> {
        self.props.iter().find(|p| p.ns == ns && p.name == name && p.status / 100 == 2)
    }

    fn text(&self, ns: &str, name: &str) -> Option<String> {
        self.prop(ns, name).map(|p| p.text.trim().to_string()).filter(|t| !t.is_empty())
    }

    fn is(&self, ns: &str, kind: &str) -> bool {
        self.prop(DAV, "resourcetype").is_some_and(|p| p.children.iter().any(|(n, k)| n == ns && k == kind))
    }
}

fn status_code(text: &str) -> Option<u16> {
    text.split_whitespace().nth(1).and_then(|c| c.parse().ok())
}

/// The responses of a 207 Multi-Status, and the sync token when one came.
fn multistatus(xml: &str) -> Result<(Vec<Response>, Option<String>), SyncError> {
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

/// `href` made absolute against `base` ("https://host/a/b/" + "/c/" → "https://host/c/").
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
fn path_key(url: &str) -> String {
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
                let (ns, name) = match kind {
                    Kind::Calendars => (CALDAV, "calendar-home-set"),
                    Kind::Contacts => (CARDDAV, "addressbook-home-set"),
                };
                let prop = match kind {
                    Kind::Calendars => "<c:calendar-home-set/>",
                    Kind::Contacts => "<a:addressbook-home-set/>",
                };
                let (_, responses) = client.propfind(&principal, "0", prop)?;
                return Ok(responses.iter().find_map(|r| r.prop(ns, name).and_then(|p| p.hrefs.first().cloned())).map(|href| absolute(&principal, &href)));
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
pub fn sync(account: &Account) -> Result<Report, SyncError> {
    let login = account.login().ok_or(SyncError::NoServer)?;
    let client = client_for(account)?;
    let mut homes = Homes::load(&account.id);
    if homes.is_empty() {
        homes = discover(&client, account.address.as_deref().unwrap_or(login), account.url.as_deref(), None)?;
        homes.save(&account.id)?;
    }
    let mut report = Report::default();
    for (kind, home) in [(Kind::Contacts, &homes.contacts), (Kind::Calendars, &homes.calendars)] {
        let Some(home) = home else { continue };
        // Google makes, renames and deletes no calendar or address book from here.
        if !client.is_google() {
            send_changes(&client, &account.id, kind)?;
            create_pending(&client, &account.id, kind, home)?;
        }
        let mut listed = collections(&client, kind, home)?;
        // Google's calendars hold events only, whether or not they say it.
        if client.is_google() {
            listed.iter_mut().filter(|l| l.kind == Kind::Calendars && l.components.is_empty()).for_each(|l| l.components = vec!["VEVENT".into()]);
        }
        forget_gone(&account.id, kind, &listed);
        for collection in &listed {
            sync_collection(&client, &account.id, collection, &mut report)?;
            report.collections += 1;
        }
    }
    // Google's task lists, over Google Tasks.
    if client.is_google() {
        let tasks = crate::google_tasks::sync(account)?;
        report.sent += tasks.sent;
        report.received += tasks.received;
        report.removed += tasks.removed;
        report.collections += tasks.collections;
    }
    Ok(report)
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
fn send_changes(client: &Client, account: &str, kind: Kind) -> Result<(), SyncError> {
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
                return Err(SyncError::Server(format!("PROPPATCH {}: {}", state.url, answer.status)));
            }
            state.renamed = false;
            state.save(&path).map_err(SyncError::Disk)?;
        }
        if state.deleted {
            let (_, found) = client.propfind(&state.url, "1", "<d:getetag/>")?;
            let held = found.iter().filter(|r| path_key(&r.href) != path_key(&state.url)).count();
            if held > 0 {
                state.deleted = false;
            } else {
                let answer = client.send("DELETE", &state.url, &[], None)?;
                if !(200..300).contains(&answer.status) && answer.status != 404 {
                    return Err(SyncError::Server(format!("DELETE {}: {}", state.url, answer.status)));
                }
                let _ = std::fs::remove_dir_all(&collection.dir);
                let _ = std::fs::remove_file(&path);
                continue;
            }
            state.save(&path).map_err(SyncError::Disk)?;
        }
    }
    Ok(())
}

/// Creates on the server the collections made here: a calendar or task list
/// with MKCALENDAR (RFC 4791 §5.3.1), an address book with an extended MKCOL
/// (RFC 5689), at `<home><id>/`, with its name, colour and, for a calendar,
/// what it holds. Already there (405) counts as made.
fn create_pending(client: &Client, account: &str, kind: Kind, home: &str) -> Result<(), SyncError> {
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
                state.save(&path).map_err(SyncError::Disk)?;
            }
            status => return Err(SyncError::Server(format!("{method} {url}: {status}"))),
        }
    }
    Ok(())
}

fn hash_of(path: &Path) -> Option<String> {
    std::fs::read(path).ok().map(|bytes| vdir::content_hash(&bytes))
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

/// One collection: what changed here goes first, then what changed there.
fn sync_collection(client: &Client, account: &str, listed: &Listed, report: &mut Report) -> Result<(), SyncError> {
    let kind = listed.kind;
    let dir = vdir::prepare(kind, account, &listed.id, &listed.name, listed.color.as_deref()).map_err(|e| SyncError::Disk(e.to_string()))?;
    let state_path = vdir::state_path(account, kind, &listed.id);
    let mut state = State::load(&state_path);
    state.url = listed.url.clone();
    state.read_only = listed.read_only;
    state.components = listed.components.clone();
    let mut refetch: BTreeSet<String> = BTreeSet::new();
    if !listed.read_only {
        push(client, kind, &dir, listed, &mut state, &mut refetch, report)?;
    }
    let unchanged = state.ctag.is_some() && state.ctag == listed.ctag && refetch.is_empty() && !has_local_changes(&dir, kind, &state);
    if !unchanged {
        pull(client, kind, &dir, listed, &mut state, &refetch, report)?;
    }
    state.ctag = listed.ctag.clone();
    state.save(&state_path).map_err(SyncError::Disk)
}

fn has_local_changes(dir: &Path, kind: Kind, state: &State) -> bool {
    let known: BTreeSet<&str> = state.items.iter().map(|i| i.file.as_str()).collect();
    let files = local_files(dir, kind);
    files.iter().any(|f| !known.contains(f.as_str())) || state.items.iter().any(|i| hash_of(&dir.join(&i.file)).as_deref() != Some(i.hash.as_str()))
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

/// Sends what changed here: edits with If-Match, new items with If-None-Match, deletions.
fn push(client: &Client, kind: Kind, dir: &Path, listed: &Listed, state: &mut State, refetch: &mut BTreeSet<String>, report: &mut Report) -> Result<(), SyncError> {
    let files = local_files(dir, kind);
    let mut kept: Vec<ItemState> = Vec::new();
    for item in std::mem::take(&mut state.items) {
        let path = dir.join(&item.file);
        let url = absolute(&listed.url, &item.href);
        let Some(hash) = hash_of(&path) else {
            // Deleted here: deleted there, unless it changed there meanwhile.
            let answer = client.send("DELETE", &url, &[("If-Match", &item.etag)], None)?;
            match answer.status {
                200..=299 | 404 => report.sent += 1,
                412 => {
                    refetch.insert(path_key(&url));
                    kept.push(item);
                }
                status => return Err(SyncError::Server(format!("DELETE {url}: {status}"))),
            }
            continue;
        };
        if hash == item.hash {
            kept.push(item);
            continue;
        }
        let text = std::fs::read_to_string(&path).map_err(|e| SyncError::Disk(e.to_string()))?;
        let answer = client.send("PUT", &url, &[("If-Match", &item.etag), ("Content-Type", kind.media_type())], Some(&text))?;
        match answer.status {
            200..=299 => {
                report.sent += 1;
                // Without an ETag, the server changed what it stored: it comes back at the pull.
                let etag = answer.etag.unwrap_or_default();
                if etag.is_empty() {
                    refetch.insert(path_key(&url));
                }
                kept.push(ItemState { etag, hash, ..item });
            }
            412 => {
                // Changed on both sides: theirs comes back, yours is kept aside.
                report.conflicts.push(set_aside(&path, &text)?);
                refetch.insert(path_key(&url));
                kept.push(item);
            }
            status => return Err(SyncError::Server(format!("PUT {url}: {status}"))),
        }
    }
    let known: BTreeSet<String> = kept.iter().map(|i| i.file.clone()).collect();
    for file in files.iter().filter(|f| !known.contains(*f)) {
        let path = dir.join(file);
        let text = std::fs::read_to_string(&path).map_err(|e| SyncError::Disk(e.to_string()))?;
        let href = format!("{}/{}", listed.url.trim_end_matches('/'), url_segment(file));
        if client.is_google() {
            create_on_google(client, kind, listed, file, &href, &text, &mut kept, refetch)?;
            report.sent += 1;
            // Google may file it under its own name: the whole list is compared at the pull.
            state.sync_token = None;
            continue;
        }
        let answer = client.send("PUT", &href, &[("If-None-Match", "*"), ("Content-Type", kind.media_type())], Some(&text))?;
        match answer.status {
            200..=299 => {
                report.sent += 1;
                let etag = answer.etag.unwrap_or_default();
                if etag.is_empty() {
                    refetch.insert(path_key(&href));
                }
                kept.push(ItemState { href: path_key(&href), file: file.clone(), etag, hash: vdir::content_hash(text.as_bytes()) });
            }
            status => return Err(SyncError::Server(format!("PUT {href}: {status}"))),
        }
    }
    state.items = kept;
    Ok(())
}

/// A new item on Google: no If-None-Match (it takes If-Match only), cards in
/// vCard 3.0, and a new contact by POST when its address book refuses the
/// PUT (RFC 5995). Where Google filed it (its Location) is where it is known;
/// what Google kept of it comes back at the pull.
#[allow(clippy::too_many_arguments)]
fn create_on_google(client: &Client, kind: Kind, listed: &Listed, file: &str, href: &str, text: &str, kept: &mut Vec<ItemState>, refetch: &mut BTreeSet<String>) -> Result<(), SyncError> {
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
    kept.push(ItemState { href: at, file: file.to_string(), etag: answer.etag.unwrap_or_default(), hash: vdir::content_hash(text.as_bytes()) });
    Ok(())
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

/// Your version of an item changed on both sides, kept where you can find it.
/// Not kept (a full disk), the sync stops there: the server's version does not
/// take its place, and yours is never lost.
fn set_aside(path: &Path, text: &str) -> Result<PathBuf, SyncError> {
    let folder = state_dir().join("dav").join("conflicts");
    let name = format!("{}-{}", jiff::Timestamp::now().as_second(), path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default());
    let target = folder.join(name);
    std::fs::create_dir_all(&folder).and_then(|()| std::fs::write(&target, text)).map_err(|e| SyncError::Disk(format!("{}: {e}", target.display())))?;
    Ok(target)
}

/// Brings what changed there: by sync token, else by comparing ETags.
fn pull(client: &Client, kind: Kind, dir: &Path, listed: &Listed, state: &mut State, refetch: &BTreeSet<String>, report: &mut Report) -> Result<(), SyncError> {
    let by_href: BTreeMap<String, usize> = state.items.iter().enumerate().map(|(i, item)| (path_key(&absolute(&listed.url, &item.href)), i)).collect();
    let (changed, gone, token) = match state.sync_token.clone().and_then(|token| changes_since(client, &listed.url, &token).ok().flatten()) {
        Some(found) => found,
        None => everything(client, &listed.url, state, &by_href)?,
    };
    // Gone there: gone here.
    let gone: BTreeSet<String> = gone.into_iter().map(|h| path_key(&absolute(&listed.url, &h))).collect();
    let mut removed_files = Vec::new();
    state.items.retain(|item| {
        let key = path_key(&absolute(&listed.url, &item.href));
        if gone.contains(&key) {
            removed_files.push(item.file.clone());
            false
        } else {
            true
        }
    });
    for file in removed_files {
        let _ = std::fs::remove_file(dir.join(file));
        report.removed += 1;
    }
    // Changed there, or to read again after a send.
    let mut wanted: BTreeMap<String, (String, Option<String>)> = changed
        .into_iter()
        .filter(|(href, etag)| {
            let key = path_key(&absolute(&listed.url, href));
            let known = state.items.iter().find(|i| path_key(&absolute(&listed.url, &i.href)) == key);
            known.is_none_or(|i| etag.as_deref().is_none_or(|e| e != i.etag)) || refetch.contains(&key)
        })
        .map(|(href, etag)| (path_key(&absolute(&listed.url, &href)), (href, etag)))
        .collect();
    for key in refetch {
        if let Some(item) = state.items.iter().find(|i| path_key(&absolute(&listed.url, &i.href)) == *key) {
            wanted.entry(key.clone()).or_insert((item.href.clone(), None));
        }
    }
    let hrefs: Vec<String> = wanted.values().map(|(href, _)| href.clone()).collect();
    for batch in hrefs.chunks(BATCH) {
        for (href, etag, data) in fetch(client, kind, &listed.url, batch)? {
            // Set aside already when sending it was refused.
            let aside = refetch.contains(&path_key(&absolute(&listed.url, &href)));
            store(dir, kind, state, &listed.url, (&href, &etag, &data), aside, report)?;
        }
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

/// Writes an item brought from the server (href, ETag, text); a version changed
/// here meanwhile is set aside first, unless it already was (`aside`).
fn store(dir: &Path, kind: Kind, state: &mut State, url: &str, (href, etag, data): (&str, &str, &str), aside: bool, report: &mut Report) -> Result<(), SyncError> {
    let key = path_key(&absolute(url, href));
    let taken: BTreeSet<String> = state.items.iter().map(|i| i.file.clone()).chain(local_files(dir, kind)).collect();
    let position = state.items.iter().position(|i| path_key(&absolute(url, &i.href)) == key);
    let file = match position {
        Some(i) => state.items[i].file.clone(),
        None => file_name(href, kind.extension(), &taken),
    };
    let path = dir.join(&file);
    if let (Some(i), Some(hash), false) = (position, hash_of(&path), aside)
        && hash != state.items[i].hash
        && let Ok(mine) = std::fs::read_to_string(&path)
    {
        report.conflicts.push(set_aside(&path, &mine)?);
    }
    let text = data.replace("\r\n", "\n").replace('\n', "\r\n");
    vdir::write_item(&path, &text).map_err(SyncError::Disk)?;
    let item = ItemState { href: href.to_string(), file, etag: etag.to_string(), hash: vdir::content_hash(text.as_bytes()) };
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

#[cfg(test)]
mod tests {
    use super::*;

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
