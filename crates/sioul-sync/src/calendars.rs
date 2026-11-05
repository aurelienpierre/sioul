// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Agendas made, changed, removed and shared on their servers (several
//! agendas, part 5; docs/client.md, "Agendas on their servers").
//!
//! **Google**, through its Calendar API v3 (Google's CalDAV makes, renames
//! and deletes no calendar): `calendars.insert`, `calendars.patch`,
//! `calendars.delete`, `calendarList.patch` for the colour and
//! `calendarList.delete` to stop showing one shared with you; its sharing is
//! the calendar's ACL (`acl.list`, `acl.insert`, `acl.delete`). The OAuth
//! scope Sioul asks already holds `…/auth/calendar`.
//!
//! **Nextcloud** (sabre/dav), by its DAV sharing: a calendar's shares are its
//! `oc:invite` property, changed by a POST of `oc:share` (`set`, `remove`);
//! its public link by `cs:publish-calendar` and `cs:unpublish-calendar`, read
//! back as `cs:publish-url` and given as its feed (`subscription`). A server says it shares by its DAV header:
//! Nextcloud's `oc-resource-sharing`, CalendarServer's `calendarserver-sharing`
//! (caldav-sharing §5.1), its links by `oc-calendar-publishing`; one that says
//! neither does not share from Sioul.
//!
//! What is made, renamed, recoloured or removed here is sent by the next sync
//! (`dav::sync`, the queued model of `vdir`): the window writes nothing on a
//! server itself but sharing, which is asked and answered at once.

use crate::SyncError;
use crate::dav::{CALDAV, CALSERVER, Client, DAV, OWNCLOUD, xml_escape};
use serde::Serialize;
use serde_json::{Value, json};
use std::sync::Mutex;
use std::time::Duration;

const GOOGLE_CALENDAR: &str = "https://www.googleapis.com/calendar/v3";

/// Google's CalDAV, where each calendar's events are (`caldav_url`).
const GOOGLE_CALDAV: &str = "https://apidata.googleusercontent.com/caldav/v2";

// ---------------------------------------------------------------- Google's Calendar API v3

/// A Google calendar's id, from its CalDAV address: the segment after
/// `/caldav/v2/` ("abc@group.calendar.google.com"), decoded.
pub fn google_id(url: &str) -> Option<String> {
    let after = url.split_once("/caldav/v2/")?.1;
    let id = after.split('/').next().filter(|s| !s.is_empty())?;
    Some(percent_decode(id))
}

/// A Google calendar's CalDAV address, from its id.
pub fn google_caldav_url(id: &str) -> String {
    format!("{GOOGLE_CALDAV}/{}/events/", crate::google::encode(id))
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(byte) = u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16)
        {
            out.push(byte);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

/// The Calendar API with an account's access token, asked again once when refused.
pub struct GoogleApi {
    address: String,
    agent: ureq::Agent,
    token: Mutex<String>,
    base: String,
}

impl GoogleApi {
    pub fn new(address: &str) -> Result<GoogleApi, SyncError> {
        let agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(60))).http_status_as_error(false).build().into();
        Ok(GoogleApi { address: address.to_string(), agent, token: Mutex::new(crate::google::access_token(address, false)?), base: base() })
    }

    /// At a stand-in, with a token of its own (tests).
    #[cfg(test)]
    pub(crate) fn at(base: &str) -> GoogleApi {
        let agent = ureq::Agent::config_builder().http_status_as_error(false).build().into();
        GoogleApi { address: "you@example.org".into(), agent, token: Mutex::new("test-token".into()), base: base.trim_end_matches('/').to_string() }
    }

    fn call(&self, method: &str, path: &str, body: Option<&Value>) -> Result<(u16, Value), SyncError> {
        let url = format!("{}{path}", self.base);
        crate::dav::allowed(&url)?;
        for attempt in 0..2 {
            let token = self.token.lock().map(|t| t.clone()).unwrap_or_default();
            let request = ureq::http::Request::builder()
                .method(method)
                .uri(&url)
                .header("Authorization", format!("Bearer {token}"))
                .header("User-Agent", "Sioul")
                .header("Content-Type", "application/json")
                .body(body.map(Value::to_string).unwrap_or_default())
                .map_err(|e| SyncError::Server(e.to_string()))?;
            let mut response = self.agent.run(request).map_err(|e| SyncError::Network(e.to_string()))?;
            let status = response.status().as_u16();
            if status == 401 && attempt == 0 {
                let fresh = crate::google::access_token(&self.address, true)?;
                if let Ok(mut token) = self.token.lock() {
                    *token = fresh;
                }
                continue;
            }
            if status == 401 {
                return Err(SyncError::Login(format!("{method} {url}: 401")));
            }
            let text = response.body_mut().with_config().limit(8 * 1024 * 1024).read_to_string().unwrap_or_default();
            return Ok((status, serde_json::from_str(&text).unwrap_or(Value::Null)));
        }
        Err(SyncError::Login(format!("{method} {url}: 401")))
    }

    fn ok(&self, method: &str, path: &str, body: Option<&Value>) -> Result<Value, SyncError> {
        match self.call(method, path, body)? {
            (200..=299, value) => Ok(value),
            (status, value) => Err(refusal(&format!("{method} {path}"), status, &value)),
        }
    }

    /// A new calendar (`calendars.insert`), its name; its id.
    pub fn insert(&self, name: &str) -> Result<String, SyncError> {
        let made = self.ok("POST", "/calendars", Some(&json!({ "summary": name })))?;
        made["id"].as_str().map(str::to_string).filter(|id| !id.is_empty()).ok_or_else(|| SyncError::Server("Google Calendar: a calendar made without its id".into()))
    }

    /// Its name (`calendars.patch`).
    pub fn rename(&self, id: &str, name: &str) -> Result<(), SyncError> {
        self.ok("PATCH", &format!("/calendars/{}", crate::google::encode(id)), Some(&json!({ "summary": name }))).map(|_| ())
    }

    /// Its colour in your calendar list (`calendarList.patch`, `colorRgbFormat`).
    pub fn colour(&self, id: &str, colour: &str) -> Result<(), SyncError> {
        let body = json!({ "backgroundColor": colour, "foregroundColor": "#000000" });
        self.ok("PATCH", &format!("/users/me/calendarList/{}?colorRgbFormat=true", crate::google::encode(id)), Some(&body)).map(|_| ())
    }

    /// Deleted, its events with it (`calendars.delete`): Google keeps no trash bin of calendars.
    pub fn delete(&self, id: &str) -> Result<(), SyncError> {
        match self.call("DELETE", &format!("/calendars/{}", crate::google::encode(id)), None)? {
            (200..=299 | 404 | 410, _) => Ok(()),
            (status, value) => Err(refusal(&format!("delete {id}"), status, &value)),
        }
    }

    /// One shared with you, out of your calendar list (`calendarList.delete`): the calendar stays its owner's.
    pub fn leave(&self, id: &str) -> Result<(), SyncError> {
        match self.call("DELETE", &format!("/users/me/calendarList/{}", crate::google::encode(id)), None)? {
            (200..=299 | 404 | 410, _) => Ok(()),
            (status, value) => Err(refusal(&format!("leave {id}"), status, &value)),
        }
    }

    /// Your role in each calendar of your list (`calendarList.list`, every
    /// page, hidden ones too): "owner", "writer", "reader", "freeBusyReader", by id.
    pub fn roles(&self) -> Result<std::collections::BTreeMap<String, String>, SyncError> {
        let mut out = std::collections::BTreeMap::new();
        let mut page: Option<String> = None;
        loop {
            let path = match &page {
                Some(token) => format!("/users/me/calendarList?showHidden=true&pageToken={}", crate::google::encode(token)),
                None => "/users/me/calendarList?showHidden=true".to_string(),
            };
            let answer = self.ok("GET", &path, None)?;
            for item in answer["items"].as_array().into_iter().flatten() {
                if let (Some(id), Some(role)) = (item["id"].as_str(), item["accessRole"].as_str()) {
                    out.insert(id.to_string(), role.to_string());
                }
            }
            match answer["nextPageToken"].as_str() {
                Some(token) if !token.is_empty() => page = Some(token.to_string()),
                _ => return Ok(out),
            }
        }
    }

    /// Its access rules (`acl.list`, every page).
    pub fn acl(&self, id: &str) -> Result<Vec<Value>, SyncError> {
        let mut out = Vec::new();
        let mut page: Option<String> = None;
        loop {
            let path = match &page {
                Some(token) => format!("/calendars/{}/acl?pageToken={}", crate::google::encode(id), crate::google::encode(token)),
                None => format!("/calendars/{}/acl", crate::google::encode(id)),
            };
            let answer = self.ok("GET", &path, None)?;
            out.extend(answer["items"].as_array().cloned().unwrap_or_default());
            match answer["nextPageToken"].as_str() {
                Some(token) if !token.is_empty() => page = Some(token.to_string()),
                _ => return Ok(out),
            }
        }
    }

    /// A rule added (`acl.insert`): `scope` "user" with an address, or "default" (anyone).
    pub fn grant(&self, id: &str, scope: &str, value: &str, role: &str) -> Result<(), SyncError> {
        let scope = if scope == "default" { json!({ "type": "default" }) } else { json!({ "type": scope, "value": value }) };
        self.ok("POST", &format!("/calendars/{}/acl", crate::google::encode(id)), Some(&json!({ "role": role, "scope": scope }))).map(|_| ())
    }

    /// A rule taken out (`acl.delete`).
    pub fn revoke(&self, id: &str, rule: &str) -> Result<(), SyncError> {
        match self.call("DELETE", &format!("/calendars/{}/acl/{}", crate::google::encode(id), crate::google::encode(rule)), None)? {
            (200..=299 | 404 | 410, _) => Ok(()),
            (status, value) => Err(refusal(&format!("unshare {rule}"), status, &value)),
        }
    }
}

/// Google's refusal, its own words kept. The Calendar API turned off in the
/// Google project of the key (a key of your own made before part 5, with the
/// CalDAV API alone) is marked, so that the window says what to turn on
/// (`api_off`): Google answers 403 with the reason `accessNotConfigured`, or
/// `SERVICE_DISABLED` in its details (not checked against Google's page).
fn refusal(what: &str, status: u16, value: &Value) -> SyncError {
    let error = &value["error"];
    let mut reasons = error["errors"].as_array().into_iter().flatten().chain(error["details"].as_array().into_iter().flatten()).filter_map(|e| e["reason"].as_str());
    let off = status == 403 && reasons.any(|r| r == "accessNotConfigured" || r == "SERVICE_DISABLED");
    SyncError::Server(format!("Google Calendar, {what}: {status}{} {}", if off { API_OFF } else { "" }, error["message"].as_str().unwrap_or_default()))
}

const API_OFF: &str = " (Calendar API off)";

/// Refused because the Google project of the key has the Calendar API turned off.
pub fn api_off(error: &SyncError) -> bool {
    matches!(error, SyncError::Server(m) if m.starts_with("Google Calendar, ") && m.contains(API_OFF))
}

/// Google's address, or a stand-in's in a test build.
fn base() -> String {
    #[cfg(feature = "insecure-test-tls")]
    if let Some(base) = std::env::var_os("SIOUL_TEST_GOOGLE_CALENDAR") {
        return base.to_string_lossy().trim_end_matches('/').to_string();
    }
    GOOGLE_CALENDAR.to_string()
}

// ---------------------------------------------------------------- sharing, as the line says it

/// What someone may do with an agenda shared with them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Access {
    /// Its busy times alone (Google's freeBusyReader).
    Busy,
    Read,
    /// Read and change.
    Write,
}

/// One person (or group) an agenda is shared with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Sharee {
    /// What names the share to take it back: Nextcloud's principal
    /// ("principal:principals/users/sam"), Google's rule id ("user:sam@example.org").
    pub id: String,
    /// "Sam", else the address or the user's name.
    pub name: String,
    pub access: Access,
    /// A group of the server rather than one person.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub group: bool,
}

/// How a server shares, if at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Sharer {
    /// Nextcloud's DAV sharing: users and groups of the same server, and a public link.
    Nextcloud,
    /// Google's ACL: any address, and a public calendar.
    Google,
    /// This server does not share from Sioul.
    None,
}

/// An agenda's sharing, as read from its server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Sharing {
    pub sharer: Sharer,
    pub people: Vec<Sharee>,
    /// Its public link, read only, when one is on: an address any agenda
    /// app subscribes to (an ICS feed; Nextcloud's `subscription`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    /// The server publishes a link at all.
    pub links: bool,
}

impl Sharing {
    fn none() -> Sharing {
        Sharing { sharer: Sharer::None, people: Vec::new(), link: None, links: false }
    }
}

/// A Nextcloud user's or group's principal, as its sharing names it; none
/// for a name that is no user's name (an address: someone elsewhere, whom
/// the link reaches).
pub fn principal(name: &str, group: bool) -> Option<String> {
    let name = name.trim();
    let fits = !name.is_empty() && name.len() <= 64 && name.chars().all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | ' ')) && !name.contains("..");
    fits.then(|| format!("principal:principals/{}/{name}", if group { "groups" } else { "users" }))
}

/// A calendar's sharing on a CalDAV server: Nextcloud's DAV sharing when its
/// DAV header says `calendarserver-sharing`, its shares from `oc:invite` and
/// its public link from `cs:publish-url`; else none.
pub fn dav_sharing(client: &Client, url: &str) -> Result<Sharing, SyncError> {
    let options = client.send("OPTIONS", url, &[], None)?;
    let header = options.dav.unwrap_or_default().to_ascii_lowercase();
    let says = |feature: &str| header.split(',').any(|f| f.trim() == feature);
    // Nextcloud's sharing plugin says `oc-resource-sharing`, its publishing one
    // `oc-calendar-publishing` and `calendarserver-sharing` (apps/dav, read 4 November 2026).
    if !says("oc-resource-sharing") && !says("calendarserver-sharing") {
        return Ok(Sharing::none());
    }
    let links = says("oc-calendar-publishing");
    let body = format!(r#"<?xml version="1.0" encoding="utf-8"?><d:propfind xmlns:d="{DAV}" xmlns:oc="{OWNCLOUD}" xmlns:cs="{CALSERVER}"><d:prop><oc:invite/><cs:publish-url/></d:prop></d:propfind>"#);
    let answer = client.send("PROPFIND", url, &[("Depth", "0"), ("Content-Type", "application/xml; charset=utf-8")], Some(&body))?;
    if answer.status != 207 {
        return Err(SyncError::Server(format!("PROPFIND {url}: {}", answer.status)));
    }
    let (people, link) = read_invite(&answer.body)?;
    Ok(Sharing { sharer: Sharer::Nextcloud, people, link, links })
}

/// The address to give for a Nextcloud calendar's `cs:publish-url`: its
/// feed, `?export`, which any agenda app subscribes to, as Nextcloud
/// Calendar's own "Copy subscription link" gives it (`PublishCalendar.vue`,
/// read 5 November 2026). The bare address is a CalDAV collection, which a
/// browser and most agenda apps cannot open.
pub fn subscription(publish_url: &str) -> String {
    let url = publish_url.trim();
    if url.contains('?') { url.to_string() } else { format!("{url}?export") }
}

/// Who `oc:invite` names, and the link `cs:publish-url` gives (its feed, `subscription`).
fn read_invite(xml: &str) -> Result<(Vec<Sharee>, Option<String>), SyncError> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| SyncError::Server(format!("XML: {e}")))?;
    let mut people = Vec::new();
    for user in doc.descendants().filter(|n| n.has_tag_name((OWNCLOUD, "user"))) {
        let child = |name: &str| user.children().find(|c| c.has_tag_name((OWNCLOUD, name)) || c.has_tag_name((DAV, name)));
        let Some(href) = child("href").and_then(|h| h.text()).map(|t| t.trim().to_string()).filter(|h| !h.is_empty()) else { continue };
        let write = child("access").is_some_and(|a| a.children().any(|c| c.has_tag_name((OWNCLOUD, "read-write"))));
        let name = child("common-name").and_then(|n| n.text()).map(|t| t.trim().to_string()).filter(|n| !n.is_empty()).unwrap_or_else(|| href.rsplit('/').next().unwrap_or(&href).to_string());
        people.push(Sharee { group: href.contains("/groups/"), id: href, name, access: if write { Access::Write } else { Access::Read } });
    }
    let link = doc.descendants().find(|n| n.has_tag_name((CALSERVER, "publish-url"))).and_then(|p| p.descendants().find(|c| c.has_tag_name((DAV, "href"))).and_then(|h| h.text()).or(p.text())).map(str::trim).filter(|t| !t.is_empty()).map(subscription);
    Ok((people, link))
}

/// Shared with a user or group of the same Nextcloud (`principal`), read
/// only or read and change: `oc:share`'s `set`.
pub fn dav_share(client: &Client, url: &str, who: &str, write: bool) -> Result<(), SyncError> {
    let access = if write { "<oc:read-write/>" } else { "" };
    let body = format!(r#"<?xml version="1.0" encoding="utf-8"?><oc:share xmlns:d="{DAV}" xmlns:oc="{OWNCLOUD}"><oc:set><d:href>{}</d:href>{access}</oc:set></oc:share>"#, xml_escape(who));
    post(client, url, &body)
}

/// No longer shared with them: `oc:share`'s `remove`.
pub fn dav_unshare(client: &Client, url: &str, who: &str) -> Result<(), SyncError> {
    let body = format!(r#"<?xml version="1.0" encoding="utf-8"?><oc:share xmlns:d="{DAV}" xmlns:oc="{OWNCLOUD}"><oc:remove><d:href>{}</d:href></oc:remove></oc:share>"#, xml_escape(who));
    post(client, url, &body)
}

/// Its public link on or off: `cs:publish-calendar`, `cs:unpublish-calendar`.
pub fn dav_publish(client: &Client, url: &str, on: bool) -> Result<(), SyncError> {
    let body = format!(r#"<?xml version="1.0" encoding="utf-8"?><cs:{} xmlns:cs="{CALSERVER}"/>"#, if on { "publish-calendar" } else { "unpublish-calendar" });
    post(client, url, &body)
}

fn post(client: &Client, url: &str, body: &str) -> Result<(), SyncError> {
    let answer = client.send("POST", url, &[("Content-Type", "application/xml; charset=utf-8")], Some(body))?;
    if (200..300).contains(&answer.status) {
        Ok(())
    } else {
        Err(SyncError::Server(format!("POST {url}: {}", answer.status)))
    }
}

/// A Google calendar's sharing, from its ACL: each person or group by its
/// rule, the account's own rule (its owner) left out; a public calendar
/// (scope "default") as its link.
pub fn google_sharing(api: &GoogleApi, id: &str, own: &str) -> Result<Sharing, SyncError> {
    let mut people = Vec::new();
    let mut link = None;
    for rule in api.acl(id)? {
        let role = rule["role"].as_str().unwrap_or_default();
        let access = match role {
            "freeBusyReader" => Access::Busy,
            "reader" => Access::Read,
            "writer" | "owner" => Access::Write,
            _ => continue,
        };
        let (kind, value) = (rule["scope"]["type"].as_str().unwrap_or_default(), rule["scope"]["value"].as_str().unwrap_or_default());
        match kind {
            "default" => link = Some(format!("https://calendar.google.com/calendar/ical/{}/public/basic.ics", crate::google::encode(id))),
            "user" | "group" if !value.eq_ignore_ascii_case(own) && !(role == "owner" && value.eq_ignore_ascii_case(id)) => {
                people.push(Sharee { id: rule["id"].as_str().unwrap_or_default().to_string(), name: value.to_string(), access, group: kind == "group" });
            }
            _ => {}
        }
    }
    Ok(Sharing { sharer: Sharer::Google, people, link, links: true })
}

/// Google's role for an access.
pub fn google_role(access: Access) -> &'static str {
    match access {
        Access::Busy => "freeBusyReader",
        Access::Read => "reader",
        Access::Write => "writer",
    }
}

// ---------------------------------------------------------------- your availability on its servers

/// A user of another Nextcloud (32 and later), as its federated calendar
/// sharing names them in `oc:share`: `principal:principals/remote-users/`
/// and their cloud id ("sam@cloud.example.org") in base64, as Nextcloud's
/// own calendar app writes it (`btoa(shareWith)`, SharingSearch.vue) and its
/// server reads it back (`RemoteUserPrincipalBackend::principalUriToPrincipal`,
/// `FederationSharingService::decodeRemoteUserPrincipal`; nextcloud/server
/// and nextcloud/calendar, read 5 November 2026). None for what is no cloud
/// id: a name and a server, no space, no slash.
pub fn federated_principal(cloud_id: &str) -> Option<String> {
    let cloud_id = cloud_id.trim();
    let (user, server) = cloud_id.rsplit_once('@')?;
    let fits = !user.is_empty() && !server.is_empty() && cloud_id.len() <= 255 && !cloud_id.chars().any(|c| c.is_whitespace() || c.is_control() || c == '/') && server.contains(['.', ':']);
    fits.then(|| format!("principal:principals/remote-users/{}", sioul_core::lines::base64_encode(cloud_id.as_bytes())))
}

/// Google: a calendar's public free/busy (its ACL's `default` rule, role
/// `freeBusyReader`: "See only free/busy (hide details)", Google Calendar
/// help 37083) on or off; its public iCal address while on. A calendar
/// already public with its details is never narrowed nor taken down here
/// (`public_with_details`): that share is yours, made in Google.
pub fn google_public_busy(api: &GoogleApi, id: &str, on: bool) -> Result<Option<String>, SyncError> {
    let public = api.acl(id)?.into_iter().find(|rule| rule["scope"]["type"].as_str() == Some("default"));
    let role = public.as_ref().and_then(|rule| rule["role"].as_str().map(str::to_string));
    let link = format!("https://calendar.google.com/calendar/ical/{}/public/basic.ics", crate::google::encode(id));
    match (on, role.as_deref()) {
        (true, Some("freeBusyReader")) => Ok(Some(link)),
        (true, Some(_)) => Err(SyncError::Server(format!("Google Calendar, public {id}:{PUBLIC_DETAILS}"))),
        (true, None) => {
            api.grant(id, "default", "", "freeBusyReader")?;
            Ok(Some(link))
        }
        (false, Some("freeBusyReader")) => {
            let rule = public.as_ref().and_then(|r| r["id"].as_str()).unwrap_or("default").to_string();
            api.revoke(id, &rule)?;
            Ok(None)
        }
        (false, _) => Ok(None),
    }
}

const PUBLIC_DETAILS: &str = " public with its details already";

/// Refused because the calendar is public with its details already (`google_public_busy`).
pub fn public_with_details(error: &SyncError) -> bool {
    matches!(error, SyncError::Server(m) if m.starts_with("Google Calendar, public ") && m.ends_with(PUBLIC_DETAILS))
}

/// A server's answer to a PROPPATCH: every property set or removed
/// (RFC 4918 §9.2: one 207, a propstat per status), else what it refused.
fn proppatch(client: &Client, url: &str, update: &str) -> Result<(), SyncError> {
    let body = format!(r#"<?xml version="1.0" encoding="utf-8"?><d:propertyupdate xmlns:d="{DAV}" xmlns:c="{CALDAV}">{update}</d:propertyupdate>"#);
    let answer = client.send("PROPPATCH", url, &[("Content-Type", "application/xml; charset=utf-8")], Some(&body))?;
    match answer.status {
        200 | 204 => Ok(()),
        207 => {
            let doc = roxmltree::Document::parse(&answer.body).map_err(|e| SyncError::Server(format!("XML: {e}")))?;
            let refused: Vec<String> = doc
                .descendants()
                .filter(|n| n.has_tag_name((DAV, "propstat")))
                .filter_map(|propstat| propstat.children().find(|c| c.has_tag_name((DAV, "status"))).and_then(|s| s.text()))
                .filter(|status| status.split_whitespace().nth(1).and_then(|c| c.parse::<u16>().ok()).is_none_or(|c| c / 100 != 2))
                .map(|status| status.trim().to_string())
                .collect();
            if refused.is_empty() { Ok(()) } else { Err(SyncError::Server(format!("PROPPATCH {url}: {}", refused.join(", ")))) }
        }
        status => Err(SyncError::Server(format!("PROPPATCH {url}: {status}"))),
    }
}

/// A calendar marked transparent, or opaque again (`schedule-calendar-transp`,
/// RFC 6638 §9.1): transparent, none of its events counts as your busy time
/// in the server's free/busy (sabre's `getFreeBusyForEmail` skips it), which
/// is what a mirror of blocks and an agenda of someone else's availability
/// should be (research/availability-sharing.md §5.3). Its events stay opaque
/// for whoever reads them.
pub fn set_transparent(client: &Client, url: &str, transparent: bool) -> Result<(), SyncError> {
    let mark = if transparent { "<c:transparent/>" } else { "<c:opaque/>" };
    proppatch(client, url, &format!("<d:set><d:prop><c:schedule-calendar-transp>{mark}</c:schedule-calendar-transp></d:prop></d:set>"))
}

/// A calendar's description (`calendar-description`, RFC 4791 §5.2.1): a
/// mirror's "From Sioul. As of 5 November."; empty, taken out.
pub fn set_description(client: &Client, url: &str, text: &str) -> Result<(), SyncError> {
    let update = if text.trim().is_empty() { "<d:remove><d:prop><c:calendar-description/></d:prop></d:remove>".to_string() } else { format!("<d:set><d:prop><c:calendar-description>{}</c:calendar-description></d:prop></d:set>", xml_escape(text.trim())) };
    proppatch(client, url, &update)
}

/// Your scheduling inbox (RFC 6638 §2.2): `schedule-inbox-URL` of your
/// principal (`current-user-principal`, asked of `home`); else `inbox/`
/// in your calendar home, where Nextcloud keeps it and its settings page
/// writes your working hours (apps/dav, CalendarService.ts).
pub fn schedule_inbox(client: &Client, home: &str) -> Result<String, SyncError> {
    let ask = |url: &str, prop: &str| -> Result<Option<String>, SyncError> {
        let body = format!(r#"<?xml version="1.0" encoding="utf-8"?><d:propfind xmlns:d="{DAV}" xmlns:c="{CALDAV}"><d:prop>{prop}</d:prop></d:propfind>"#);
        let answer = client.send("PROPFIND", url, &[("Depth", "0"), ("Content-Type", "application/xml; charset=utf-8")], Some(&body))?;
        if answer.status != 207 {
            return Ok(None);
        }
        let doc = roxmltree::Document::parse(&answer.body).map_err(|e| SyncError::Server(format!("XML: {e}")))?;
        let name = if prop.contains("schedule-inbox-URL") { (CALDAV, "schedule-inbox-URL") } else { (DAV, "current-user-principal") };
        Ok(doc.descendants().find(|n| n.has_tag_name(name)).and_then(|p| p.descendants().find(|c| c.has_tag_name((DAV, "href"))).and_then(|h| h.text())).map(|h| crate::dav::absolute(url, h.trim())))
    };
    if let Some(principal) = ask(home, "<d:current-user-principal/>")?
        && let Some(inbox) = ask(&principal, "<c:schedule-inbox-URL/>")?
    {
        return Ok(inbox);
    }
    Ok(format!("{}/inbox/", home.trim_end_matches('/')))
}

/// Your working hours written on your inbox for the server's free/busy
/// (`calendar-availability`, RFC 7953 §7.2.4; `availability::working_hours`),
/// as Nextcloud's own settings page writes them; none taken out, never
/// written empty, which RFC 7953 would read as never available.
pub fn set_working_hours(client: &Client, inbox: &str, calendar: Option<&str>) -> Result<(), SyncError> {
    let update = match calendar {
        Some(text) if !text.trim().is_empty() => format!("<d:set><d:prop><c:calendar-availability>{}</c:calendar-availability></d:prop></d:set>", xml_escape(text)),
        _ => "<d:remove><d:prop><c:calendar-availability/></d:prop></d:remove>".to_string(),
    };
    proppatch(client, inbox, &update)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dav::stand_in::{Reply, Request, Server, serve};
    use std::sync::Arc;

    /// A Nextcloud's calendar, as its DAV sharing answers (sabre/dav's
    /// SharingPlugin and Nextcloud's PublishPlugin, as their documentation and
    /// code show them): untested against a real Nextcloud.
    #[derive(Default)]
    struct Nextcloud {
        shares: Mutex<Vec<(String, bool)>>,
        published: Mutex<bool>,
        posts: Mutex<Vec<String>>,
    }

    impl Server for Nextcloud {
        fn misbehaves(&self, _: &Request) -> Option<crate::dav::stand_in::Fault> {
            None
        }

        fn answer(&self, request: &Request) -> Reply {
            match request.method.as_str() {
                "OPTIONS" => Reply { status: 200, headers: vec![("DAV", "1, 3, extended-mkcol, access-control, calendarserver-principal-property-search, calendar-access, calendar-proxy, calendarserver-subscribed, calendar-auto-schedule, oc-resource-sharing, oc-calendar-publishing, calendarserver-sharing".into())], body: String::new() },
                "PROPFIND" => {
                    let users: String = self.shares.lock().unwrap().iter().map(|(href, write)| format!("<oc:user><d:href>{href}</d:href><oc:common-name>{}</oc:common-name><oc:invite-accepted/><oc:access>{}</oc:access></oc:user>", href.rsplit('/').next().unwrap(), if *write { "<oc:read-write/>" } else { "<oc:read/>" })).collect();
                    let link = if *self.published.lock().unwrap() { "<cs:publish-url><d:href>https://cloud.example.org/remote.php/dav/public-calendars/AbC123</d:href></cs:publish-url>" } else { "" };
                    Reply::new(207, format!(r#"<?xml version="1.0"?><d:multistatus xmlns:d="DAV:" xmlns:oc="http://owncloud.org/ns" xmlns:cs="http://calendarserver.org/ns/"><d:response><d:href>/remote.php/dav/calendars/alex/swimming/</d:href><d:propstat><d:prop><oc:invite>{users}</oc:invite>{link}</d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response></d:multistatus>"#))
                }
                "POST" => {
                    let body = request.body.clone();
                    self.posts.lock().unwrap().push(body.clone());
                    let doc = roxmltree::Document::parse(&body).unwrap();
                    if doc.root_element().has_tag_name((CALSERVER, "publish-calendar")) {
                        *self.published.lock().unwrap() = true;
                    } else if doc.root_element().has_tag_name((CALSERVER, "unpublish-calendar")) {
                        *self.published.lock().unwrap() = false;
                    } else {
                        for set in doc.descendants().filter(|n| n.has_tag_name((OWNCLOUD, "set"))) {
                            let href = set.descendants().find(|n| n.has_tag_name((DAV, "href"))).and_then(|h| h.text()).unwrap().to_string();
                            let write = set.descendants().any(|n| n.has_tag_name((OWNCLOUD, "read-write")));
                            let mut shares = self.shares.lock().unwrap();
                            shares.retain(|(h, _)| *h != href);
                            shares.push((href, write));
                        }
                        for remove in doc.descendants().filter(|n| n.has_tag_name((OWNCLOUD, "remove"))) {
                            let href = remove.descendants().find(|n| n.has_tag_name((DAV, "href"))).and_then(|h| h.text()).unwrap().to_string();
                            self.shares.lock().unwrap().retain(|(h, _)| *h != href);
                        }
                    }
                    Reply::new(200, "")
                }
                _ => Reply::new(405, ""),
            }
        }
    }

    /// A plain CalDAV server, which says nothing of sharing.
    struct Plain;

    impl Server for Plain {
        fn misbehaves(&self, _: &Request) -> Option<crate::dav::stand_in::Fault> {
            None
        }

        fn answer(&self, request: &Request) -> Reply {
            match request.method.as_str() {
                "OPTIONS" => Reply { status: 200, headers: vec![("DAV", "1, 2, 3, calendar-access".into())], body: String::new() },
                _ => Reply::new(405, ""),
            }
        }
    }

    /// Nextcloud's DAV sharing (part 5): its shares read from `oc:invite`,
    /// a user and a group added read only or read and change, one taken out,
    /// the public link on and off and read back; a plain CalDAV server says
    /// it does not share; a principal only for a user's name.
    #[test]
    fn nextcloud_shares_and_publishes_a_calendar() {
        let server = Arc::new(Nextcloud::default());
        let base = serve(server.clone());
        let url = format!("{base}/remote.php/dav/calendars/alex/swimming/");
        let client = Client::new("alex", "secret");
        let read = || dav_sharing(&client, &url).unwrap();
        assert_eq!((read().sharer, read().people.len(), read().link.clone(), read().links), (Sharer::Nextcloud, 0, None, true));
        dav_share(&client, &url, &principal("sam", false).unwrap(), false).unwrap();
        dav_share(&client, &url, &principal("family", true).unwrap(), true).unwrap();
        let shared = read();
        assert_eq!(shared.people.iter().map(|p| (p.name.as_str(), p.access, p.group)).collect::<Vec<_>>(), [("sam", Access::Read, false), ("family", Access::Write, true)]);
        assert!(server.posts.lock().unwrap()[0].contains("<oc:set><d:href>principal:principals/users/sam</d:href></oc:set>"), "{:?}", server.posts.lock().unwrap());
        dav_unshare(&client, &url, "principal:principals/users/sam").unwrap();
        assert_eq!(read().people.len(), 1);
        dav_publish(&client, &url, true).unwrap();
        assert_eq!(read().link.as_deref(), Some("https://cloud.example.org/remote.php/dav/public-calendars/AbC123?export"), "its feed, which any agenda app reads");
        dav_publish(&client, &url, false).unwrap();
        assert_eq!(read().link, None);
        let plain = serve(Arc::new(Plain));
        assert_eq!(dav_sharing(&client, &format!("{plain}/cal/home/")).unwrap().sharer, Sharer::None);
        assert_eq!(principal("sam@example.org", false), None, "someone elsewhere: the link reaches them");
        assert_eq!(principal("../admin", false), None);
    }

    /// Google, as its Calendar API v3 reference shows it (calendars,
    /// calendarList, acl): untested against Google.
    #[derive(Default)]
    struct Google {
        calendars: Mutex<Vec<(String, String)>>,
        colours: Mutex<Vec<(String, String)>>,
        acl: Mutex<Vec<Value>>,
        listed: Mutex<Vec<String>>,
    }

    impl Server for Google {
        fn misbehaves(&self, _: &Request) -> Option<crate::dav::stand_in::Fault> {
            None
        }

        fn answer(&self, request: &Request) -> Reply {
            let body: Value = serde_json::from_str(&request.body).unwrap_or(Value::Null);
            let path = request.path.split('?').next().unwrap_or_default().to_string();
            let parts: Vec<String> = path.trim_matches('/').split('/').map(percent_decode).collect();
            let parts: Vec<&str> = parts.iter().map(String::as_str).collect();
            match (request.method.as_str(), parts.as_slice()) {
                ("POST", ["calendars"]) => {
                    let id = format!("c{}@group.calendar.google.com", self.calendars.lock().unwrap().len() + 1);
                    self.calendars.lock().unwrap().push((id.clone(), body["summary"].as_str().unwrap_or_default().to_string()));
                    self.listed.lock().unwrap().push(id.clone());
                    Reply::new(200, json!({ "id": id, "summary": body["summary"] }).to_string())
                }
                ("PATCH", ["calendars", id]) => {
                    for c in self.calendars.lock().unwrap().iter_mut().filter(|c| c.0 == *id) {
                        c.1 = body["summary"].as_str().unwrap_or_default().to_string();
                    }
                    Reply::new(200, "{}")
                }
                ("DELETE", ["calendars", id]) => {
                    self.calendars.lock().unwrap().retain(|c| c.0 != *id);
                    Reply::new(204, "")
                }
                ("PATCH", ["users", "me", "calendarList", id]) => {
                    assert!(request.path.contains("colorRgbFormat=true"));
                    self.colours.lock().unwrap().push((id.to_string(), body["backgroundColor"].as_str().unwrap_or_default().to_string()));
                    Reply::new(200, "{}")
                }
                // Your list: yours made here, owner; one shared with you, writer.
                ("GET", ["users", "me", "calendarList"]) => {
                    assert!(request.path.contains("showHidden=true"));
                    let mut items: Vec<Value> = self.listed.lock().unwrap().iter().map(|id| json!({ "id": id, "accessRole": "owner" })).collect();
                    items.push(json!({ "id": "sam@example.org", "accessRole": "writer" }));
                    Reply::new(200, json!({ "items": items }).to_string())
                }
                ("DELETE", ["users", "me", "calendarList", id]) => {
                    self.listed.lock().unwrap().retain(|c| c != id);
                    Reply::new(204, "")
                }
                ("GET", ["calendars", _, "acl"]) => Reply::new(200, json!({ "items": *self.acl.lock().unwrap() }).to_string()),
                ("POST", ["calendars", _, "acl"]) => {
                    let scope = &body["scope"];
                    let id = match scope["type"].as_str() {
                        Some("default") => "default".to_string(),
                        Some(kind) => format!("{kind}:{}", scope["value"].as_str().unwrap_or_default()),
                        None => return Reply::new(400, ""),
                    };
                    self.acl.lock().unwrap().push(json!({ "id": id, "role": body["role"], "scope": scope }));
                    Reply::new(200, "{}")
                }
                ("DELETE", ["calendars", _, "acl", rule]) => {
                    self.acl.lock().unwrap().retain(|r| r["id"].as_str() != Some(rule));
                    Reply::new(204, "")
                }
                _ => Reply::new(404, ""),
            }
        }
    }

    /// Google's Calendar API (part 5): a calendar made, renamed, recoloured
    /// and deleted; one shared with you left; its ACL read as people and a
    /// public link, a person added busy only, read or change, taken out, the
    /// account's own rule never listed. A calendar's id from its CalDAV address and back.
    #[test]
    fn google_makes_shares_and_deletes_a_calendar() {
        let google = Arc::new(Google::default());
        let api = GoogleApi::at(&serve(google.clone()));
        let id = api.insert("Swimming").unwrap();
        api.rename(&id, "Swimming club").unwrap();
        api.colour(&id, "#4c6b5c").unwrap();
        assert_eq!(google.calendars.lock().unwrap().as_slice(), [(id.clone(), "Swimming club".to_string())]);
        assert_eq!(google.colours.lock().unwrap().as_slice(), [(id.clone(), "#4c6b5c".to_string())]);
        // Whose each is, from your calendar list: yours owner, a calendar shared with you writer.
        let roles = api.roles().unwrap();
        assert_eq!((roles.get(&id).map(String::as_str), roles.get("sam@example.org").map(String::as_str)), (Some("owner"), Some("writer")));
        google.acl.lock().unwrap().push(json!({ "id": "user:you@example.org", "role": "owner", "scope": { "type": "user", "value": "you@example.org" } }));
        api.grant(&id, "user", "sam@example.org", google_role(Access::Busy)).unwrap();
        api.grant(&id, "default", "", "reader").unwrap();
        let shared = google_sharing(&api, &id, "you@example.org").unwrap();
        assert_eq!(shared.people.iter().map(|p| (p.name.as_str(), p.access)).collect::<Vec<_>>(), [("sam@example.org", Access::Busy)], "the owner's own rule left out");
        assert!(shared.link.as_deref().is_some_and(|l| l.contains("/public/basic.ics")));
        api.revoke(&id, &shared.people[0].id).unwrap();
        api.revoke(&id, "default").unwrap();
        let after = google_sharing(&api, &id, "you@example.org").unwrap();
        assert!(after.people.is_empty() && after.link.is_none());
        api.leave(&id).unwrap();
        assert!(google.listed.lock().unwrap().is_empty());
        api.delete(&id).unwrap();
        assert!(google.calendars.lock().unwrap().is_empty());
        let url = google_caldav_url("abc@group.calendar.google.com");
        assert_eq!(url, "https://apidata.googleusercontent.com/caldav/v2/abc%40group.calendar.google.com/events/");
        assert_eq!(google_id(&url).as_deref(), Some("abc@group.calendar.google.com"));
    }

    /// A Nextcloud's scheduling side, as RFC 6638 and Nextcloud's apps/dav
    /// answer it: your principal, its inbox, and each PROPPATCH kept; one
    /// calendar refuses its property (403 in its propstat).
    #[derive(Default)]
    struct Scheduling {
        patches: Mutex<Vec<(String, String)>>,
    }

    impl Server for Scheduling {
        fn misbehaves(&self, _: &Request) -> Option<crate::dav::stand_in::Fault> {
            None
        }

        fn answer(&self, request: &Request) -> Reply {
            let multistatus = |href: &str, prop: &str, status: &str| Reply::new(207, format!(r#"<?xml version="1.0"?><d:multistatus xmlns:d="DAV:" xmlns:cal="urn:ietf:params:xml:ns:caldav"><d:response><d:href>{href}</d:href><d:propstat><d:prop>{prop}</d:prop><d:status>HTTP/1.1 {status}</d:status></d:propstat></d:response></d:multistatus>"#));
            match (request.method.as_str(), request.path.as_str()) {
                ("PROPFIND", "/remote.php/dav/calendars/alex/") if request.body.contains("current-user-principal") => multistatus(&request.path, "<d:current-user-principal><d:href>/remote.php/dav/principals/users/alex/</d:href></d:current-user-principal>", "200 OK"),
                ("PROPFIND", "/remote.php/dav/principals/users/alex/") if request.body.contains("schedule-inbox-URL") => multistatus(&request.path, "<cal:schedule-inbox-URL><d:href>/remote.php/dav/calendars/alex/inbox/</d:href></cal:schedule-inbox-URL>", "200 OK"),
                ("PROPPATCH", path) => {
                    self.patches.lock().unwrap().push((path.to_string(), request.body.clone()));
                    multistatus(path, "<cal:schedule-calendar-transp/>", if path.contains("refuses") { "403 Forbidden" } else { "200 OK" })
                }
                _ => Reply::new(405, ""),
            }
        }
    }

    /// Your availability's marks on a Nextcloud (design §1): the inbox found
    /// through your principal, else beside your calendars; your working hours
    /// written there and taken out, never written empty; a mirror marked
    /// transparent and described, its text escaped; a refusal said. Someone on
    /// another Nextcloud named as its federated sharing names them.
    #[test]
    fn availability_marks_calendars_and_writes_working_hours_on_a_nextcloud() {
        let server = Arc::new(Scheduling::default());
        let base = serve(server.clone());
        let client = Client::new("alex", "secret");
        let inbox = schedule_inbox(&client, &format!("{base}/remote.php/dav/calendars/alex/")).unwrap();
        assert_eq!(inbox, format!("{base}/remote.php/dav/calendars/alex/inbox/"));
        let plain = serve(Arc::new(Plain));
        assert_eq!(schedule_inbox(&client, &format!("{plain}/cal/alex")).unwrap(), format!("{plain}/cal/alex/inbox/"), "no principal said: beside the calendars");
        set_working_hours(&client, &inbox, Some("BEGIN:VCALENDAR\r\nX-NOTE:a&b<c\r\nEND:VCALENDAR\r\n")).unwrap();
        set_working_hours(&client, &inbox, None).unwrap();
        set_working_hours(&client, &inbox, Some("  ")).unwrap();
        let mirror = format!("{base}/remote.php/dav/calendars/alex/busy-7f3c/");
        set_transparent(&client, &mirror, true).unwrap();
        set_description(&client, &mirror, "From Sioul. As of 5 November & after.").unwrap();
        assert!(set_transparent(&client, &format!("{base}/remote.php/dav/calendars/alex/refuses/"), true).is_err(), "a refused property is said");
        let patches = server.patches.lock().unwrap().clone();
        assert_eq!(patches.len(), 6);
        assert!(patches[0].1.contains("<d:set><d:prop><c:calendar-availability>BEGIN:VCALENDAR") && patches[0].1.contains("a&amp;b&lt;c"), "{}", patches[0].1);
        for empty in [&patches[1].1, &patches[2].1] {
            assert!(empty.contains("<d:remove><d:prop><c:calendar-availability/></d:prop></d:remove>"), "{empty}");
        }
        assert!(patches[3].1.contains("<c:schedule-calendar-transp><c:transparent/></c:schedule-calendar-transp>") && patches[3].1.contains("xmlns:c=\"urn:ietf:params:xml:ns:caldav\""));
        assert!(patches[4].1.contains("<c:calendar-description>From Sioul. As of 5 November &amp; after.</c:calendar-description>"));
        assert_eq!(federated_principal("sam@cloud.example.org").as_deref(), Some("principal:principals/remote-users/c2FtQGNsb3VkLmV4YW1wbGUub3Jn"));
        for no in ["sam", "sam@", "@cloud.example.org", "sam @cloud.example.org", "sam@cloud/x.org", "sam@localhost"] {
            assert_eq!(federated_principal(no), None, "{no}");
        }
    }

    /// Google's public free/busy (design §1): on, the `default` rule
    /// `freeBusyReader` and its iCal address, once however often asked; off,
    /// taken out; a calendar already public with its details neither narrowed
    /// nor taken down.
    #[test]
    fn google_s_public_free_busy_goes_on_and_off_and_never_touches_a_public_calendar() {
        let google = Arc::new(Google::default());
        let api = GoogleApi::at(&serve(google.clone()));
        let id = api.insert("Personal").unwrap();
        let link = google_public_busy(&api, &id, true).unwrap().unwrap();
        assert!(link.ends_with("/public/basic.ics") && link.contains("c1%40group.calendar.google.com"), "{link}");
        assert_eq!(google_public_busy(&api, &id, true).unwrap().as_deref(), Some(link.as_str()));
        let rules = google.acl.lock().unwrap().clone();
        assert_eq!((rules.len(), rules[0]["role"].as_str(), rules[0]["scope"]["type"].as_str()), (1, Some("freeBusyReader"), Some("default")), "once");
        assert_eq!(google_public_busy(&api, &id, false).unwrap(), None);
        assert!(google.acl.lock().unwrap().is_empty());
        api.grant(&id, "default", "", "reader").unwrap();
        let refused = google_public_busy(&api, &id, true).unwrap_err();
        assert!(public_with_details(&refused), "{refused:?}");
        assert_eq!(google_public_busy(&api, &id, false).unwrap(), None);
        assert_eq!(google.acl.lock().unwrap()[0]["role"].as_str(), Some("reader"), "the public share made in Google stays");
    }
}
