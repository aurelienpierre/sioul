// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Whether an account's calendar server sends invitations by itself, and
//! the addresses it knows you by (decision 7 of the event card, 5 November
//! 2026): what tells who sends the invitation of a meeting you organize.
//!
//! - **Its own scheduling** (RFC 6638 §2): a server that schedules says
//!   `calendar-auto-schedule` in the DAV header of an OPTIONS answer. Then,
//!   for an event you organize in your own calendar there, it emails each
//!   guest itself (Nextcloud through its iMIP plugin), and Sioul drafts nothing.
//! - **Your addresses there** (RFC 6638 §2.4.1, `calendar-user-address-set`
//!   on your principal): such a server schedules only for an ORGANIZER that
//!   is one of them (Nextcloud: the address of your account there).
//!
//! Asked once a day per account, at the end of a calendars' sync, and kept
//! in `$XDG_STATE_HOME/sioul/dav/<account>/scheduling.toml`, this device's
//! own. A request that fails keeps what was known and is asked again at
//! the next sync; a server that answers without the word does not schedule.

use crate::SyncError;
use crate::dav::{CALDAV, Client, absolute, multistatus};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// What an account's calendar server says of its scheduling.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scheduling {
    /// Its DAV header says `calendar-auto-schedule`: it emails your guests itself.
    #[serde(default)]
    pub auto: bool,
    /// Your principal's `calendar-user-address-set`: the addresses it
    /// schedules for, lower case, without `mailto:`.
    #[serde(default)]
    pub addresses: Vec<String>,
    /// When it was asked, Unix seconds.
    #[serde(default)]
    pub checked: i64,
}

/// Asked again after this long.
const EVERY: i64 = 86_400;

fn path(account: &str) -> PathBuf {
    sioul_core::config::state_dir().join("dav").join(account).join("scheduling.toml")
}

/// What an account's server said last; none when it was never asked.
pub fn of(account: &str) -> Option<Scheduling> {
    std::fs::read_to_string(path(account)).ok().and_then(|text| toml::from_str(&text).ok())
}

fn save(account: &str, scheduling: &Scheduling) -> Result<(), String> {
    let path = path(account);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    let text = toml::to_string(scheduling).map_err(|e| e.to_string())?;
    sioul_core::filelock::replace(&path, text.as_bytes()).map_err(|e| format!("{}: {e}", path.display()))
}

/// Whether a server schedules for this organizer, as it said last: it
/// schedules by itself, and names this address among yours there. A server
/// that names none of your addresses, or was never asked, is not trusted to
/// send: Sioul drafts the invitation, each guest marked as left to you
/// (`organizer::Sender::Draft`).
pub fn schedules_for(account: &str, organizer: &str) -> bool {
    of(account).is_some_and(|s| s.auto && s.addresses.iter().any(|a| a.eq_ignore_ascii_case(organizer.trim())))
}

/// The DAV header's words, each alone (RFC 4918 §10.1: a list by commas).
fn says(header: &str, word: &str) -> bool {
    header.split(',').any(|w| w.trim().eq_ignore_ascii_case(word))
}

/// "mailto:Jane@Example.org" → "jane@example.org"; none for another kind of address.
fn mailto(href: &str) -> Option<String> {
    let href = href.trim();
    href.get(..7).filter(|p| p.eq_ignore_ascii_case("mailto:")).map(|_| href[7..].trim().to_lowercase()).filter(|a| !a.is_empty())
}

/// Asks the server at your calendars' home (`home`): its DAV header, then
/// your principal's addresses.
pub(crate) fn ask(client: &Client, home: &str, now: i64) -> Result<Scheduling, SyncError> {
    let options = client.send("OPTIONS", home, &[], None)?;
    let auto = options.status / 100 == 2 && options.dav.as_deref().is_some_and(|h| says(h, "calendar-auto-schedule"));
    let mut addresses = Vec::new();
    if auto {
        let current = r#"<?xml version="1.0" encoding="utf-8"?><d:propfind xmlns:d="DAV:"><d:prop><d:current-user-principal/></d:prop></d:propfind>"#;
        let answer = client.send("PROPFIND", home, &[("Depth", "0"), ("Content-Type", "application/xml; charset=utf-8")], Some(current))?;
        let principal = if answer.status == 207 { multistatus(&answer.body)?.0.iter().find_map(|r| r.prop("DAV:", "current-user-principal").and_then(|p| p.hrefs.first().cloned())) } else { None };
        let principal = principal.map_or_else(|| home.to_string(), |href| absolute(home, &href));
        let set = format!(r#"<?xml version="1.0" encoding="utf-8"?><d:propfind xmlns:d="DAV:" xmlns:c="{CALDAV}"><d:prop><c:calendar-user-address-set/></d:prop></d:propfind>"#);
        let answer = client.send("PROPFIND", &principal, &[("Depth", "0"), ("Content-Type", "application/xml; charset=utf-8")], Some(&set))?;
        if answer.status == 207 {
            for response in multistatus(&answer.body)?.0 {
                if let Some(prop) = response.prop(CALDAV, "calendar-user-address-set") {
                    addresses.extend(prop.hrefs.iter().filter_map(|h| mailto(h)));
                }
            }
        }
        addresses.sort();
        addresses.dedup();
    }
    Ok(Scheduling { auto, addresses, checked: now })
}

/// What an account's server says of its scheduling, asked again when a day
/// old or never asked (`ask`), kept; a request that fails keeps what was
/// known. Never an error of the sync.
pub(crate) fn keep_fresh(client: &Client, account: &str, home: &str) {
    let now = sioul_core::zone::now().timestamp().as_second();
    if of(account).is_some_and(|s| (0..EVERY).contains(&(now - s.checked))) {
        return;
    }
    if let Ok(found) = ask(client, home, now) {
        let _ = save(account, &found);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dav::stand_in::{Fault, Reply, Request, Server, home, serve};
    use std::sync::{Arc, Mutex};

    /// A server that schedules by itself (Nextcloud's sabre/dav, as its
    /// documentation and code show it: untested against a real Nextcloud),
    /// or a plain one.
    struct Scheduler {
        auto: bool,
        seen: Mutex<Vec<String>>,
    }

    impl Server for Scheduler {
        fn misbehaves(&self, _: &Request) -> Option<Fault> {
            None
        }

        fn answer(&self, request: &Request) -> Reply {
            self.seen.lock().unwrap().push(format!("{} {}", request.method, request.path));
            match (request.method.as_str(), request.path.as_str()) {
                ("OPTIONS", _) if self.auto => Reply { status: 200, headers: vec![("DAV", "1, 3, extended-mkcol, access-control, calendar-access, calendar-auto-schedule, calendar-availability".into())], body: String::new() },
                ("OPTIONS", _) => Reply { status: 200, headers: vec![("DAV", "1, 2, 3, calendar-access".into())], body: String::new() },
                ("PROPFIND", "/remote.php/dav/calendars/camille/") => Reply::new(207, r#"<?xml version="1.0"?><d:multistatus xmlns:d="DAV:"><d:response><d:href>/remote.php/dav/calendars/camille/</d:href><d:propstat><d:prop><d:current-user-principal><d:href>/remote.php/dav/principals/users/camille/</d:href></d:current-user-principal></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response></d:multistatus>"#),
                ("PROPFIND", "/remote.php/dav/principals/users/camille/") => Reply::new(207, r#"<?xml version="1.0"?><d:multistatus xmlns:d="DAV:" xmlns:cal="urn:ietf:params:xml:ns:caldav"><d:response><d:href>/remote.php/dav/principals/users/camille/</d:href><d:propstat><d:prop><cal:calendar-user-address-set><d:href>mailto:Camille@example.org</d:href><d:href>/remote.php/dav/principals/users/camille/</d:href></cal:calendar-user-address-set></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response></d:multistatus>"#),
                _ => Reply::new(405, ""),
            }
        }
    }

    /// A server that schedules says so and names your address there; a plain
    /// one says nothing, and nothing more is asked of it; kept a day, asked
    /// once; a failed request keeps what was known.
    #[test]
    fn a_server_s_scheduling_is_read_and_kept() {
        home();
        let scheduler = Arc::new(Scheduler { auto: true, seen: Mutex::new(Vec::new()) });
        let base = serve(scheduler.clone());
        let client = Client::new("camille", "secret");
        let at = format!("{base}/remote.php/dav/calendars/camille/");
        let found = ask(&client, &at, 100).unwrap();
        assert_eq!(found, Scheduling { auto: true, addresses: vec!["camille@example.org".into()], checked: 100 });
        keep_fresh(&client, "scheduling-auto", &at);
        assert!(schedules_for("scheduling-auto", "Camille@example.org") && !schedules_for("scheduling-auto", "other@example.org"));
        let asked = scheduler.seen.lock().unwrap().len();
        keep_fresh(&client, "scheduling-auto", &at);
        assert_eq!(scheduler.seen.lock().unwrap().len(), asked, "asked once a day");
        let plain = Arc::new(Scheduler { auto: false, seen: Mutex::new(Vec::new()) });
        let base = serve(plain.clone());
        keep_fresh(&client, "scheduling-plain", &format!("{base}/remote.php/dav/calendars/camille/"));
        assert_eq!(of("scheduling-plain").map(|s| (s.auto, s.addresses.len())), Some((false, 0)));
        assert_eq!(plain.seen.lock().unwrap().as_slice(), ["OPTIONS /remote.php/dav/calendars/camille/"], "nothing more asked");
        assert!(!schedules_for("scheduling-plain", "camille@example.org") && !schedules_for("scheduling-never", "camille@example.org"), "never asked: Sioul drafts");
        // A server gone: what was known stays.
        keep_fresh(&client, "scheduling-gone", "http://127.0.0.1:9/cal/");
        assert_eq!(of("scheduling-gone"), None);
    }
}
