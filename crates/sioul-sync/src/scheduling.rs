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
//! own. Only a clear answer changes what is known: an OPTIONS answered 2xx,
//! whose DAV header says the word or not, and PROPFINDs answered 207. A
//! request that fails, an error answered (403, 404, 429, 5xx…) or a PROPFIND
//! answered otherwise keeps what was known, asked again at the next sync
//! (review of 1e8551e, F9).
//!
//! **What it decides** (the owner's word, 6 November 2026: "per meeting"):
//! only who tells the guests of a meeting made elsewhere, whose guests are
//! not marked `SCHEDULE-AGENT=CLIENT` (`organizer::teller`). How a meeting
//! sends is read from its own file, never from this: a guest marked so is
//! told by your drafts, whatever this says.

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
/// schedules by itself, and names this address among yours there. Read for
/// the guests of a meeting made elsewhere alone (not left to you); the
/// window tells "never asked" apart (`of`), and then changes nothing of such
/// a meeting (`organized::server_for`).
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

/// An answer that says nothing clear (an error, a PROPFIND not answered 207):
/// what was known stays (F9).
fn unclear(what: &str, status: u16) -> SyncError {
    SyncError::Server(format!("{what}: {status}"))
}

/// Asks the server at your calendars' home (`home`): its DAV header, then
/// your principal's addresses. An answer that says nothing clear is an
/// error: what was known stays (`keep_fresh`).
pub(crate) fn ask(client: &Client, home: &str, now: i64) -> Result<Scheduling, SyncError> {
    let options = client.send("OPTIONS", home, &[], None)?;
    if options.status / 100 != 2 {
        return Err(unclear("OPTIONS", options.status));
    }
    let auto = options.dav.as_deref().is_some_and(|h| says(h, "calendar-auto-schedule"));
    let mut addresses = Vec::new();
    if auto {
        let current = r#"<?xml version="1.0" encoding="utf-8"?><d:propfind xmlns:d="DAV:"><d:prop><d:current-user-principal/></d:prop></d:propfind>"#;
        let answer = client.send("PROPFIND", home, &[("Depth", "0"), ("Content-Type", "application/xml; charset=utf-8")], Some(current))?;
        if answer.status != 207 {
            return Err(unclear("PROPFIND", answer.status));
        }
        let principal = multistatus(&answer.body)?.0.iter().find_map(|r| r.prop("DAV:", "current-user-principal").and_then(|p| p.hrefs.first().cloned()));
        let principal = principal.map_or_else(|| home.to_string(), |href| absolute(home, &href));
        let set = format!(r#"<?xml version="1.0" encoding="utf-8"?><d:propfind xmlns:d="DAV:" xmlns:c="{CALDAV}"><d:prop><c:calendar-user-address-set/></d:prop></d:propfind>"#);
        let answer = client.send("PROPFIND", &principal, &[("Depth", "0"), ("Content-Type", "application/xml; charset=utf-8")], Some(&set))?;
        if answer.status != 207 {
            return Err(unclear("PROPFIND", answer.status));
        }
        for response in multistatus(&answer.body)?.0 {
            if let Some(prop) = response.prop(CALDAV, "calendar-user-address-set") {
                addresses.extend(prop.hrefs.iter().filter_map(|h| mailto(h)));
            }
        }
        addresses.sort();
        addresses.dedup();
    }
    Ok(Scheduling { auto, addresses, checked: now })
}

/// What an account's server says of its scheduling, asked again when a day
/// old or never asked (`ask`), kept; a request that fails, or an answer that
/// says nothing clear, keeps what was known and is asked again at the next
/// sync (F9). Never an error of the sync.
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
        /// What it answers instead, while set: a passing error (503, 429), or a PROPFIND refused.
        failing: Mutex<Option<(&'static str, u16)>>,
    }

    impl Server for Scheduler {
        fn misbehaves(&self, _: &Request) -> Option<Fault> {
            None
        }

        fn answer(&self, request: &Request) -> Reply {
            self.seen.lock().unwrap().push(format!("{} {}", request.method, request.path));
            if let Some((method, status)) = *self.failing.lock().unwrap()
                && method == request.method
            {
                return Reply::new(status, "");
            }
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
        let scheduler = Arc::new(Scheduler { auto: true, seen: Mutex::new(Vec::new()), failing: Mutex::new(None) });
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
        let plain = Arc::new(Scheduler { auto: false, seen: Mutex::new(Vec::new()), failing: Mutex::new(None) });
        let base = serve(plain.clone());
        keep_fresh(&client, "scheduling-plain", &format!("{base}/remote.php/dav/calendars/camille/"));
        assert_eq!(of("scheduling-plain").map(|s| (s.auto, s.addresses.len())), Some((false, 0)));
        assert_eq!(plain.seen.lock().unwrap().as_slice(), ["OPTIONS /remote.php/dav/calendars/camille/"], "nothing more asked");
        assert!(!schedules_for("scheduling-plain", "camille@example.org") && !schedules_for("scheduling-never", "camille@example.org"), "never asked: Sioul drafts");
        // A server gone: what was known stays.
        keep_fresh(&client, "scheduling-gone", "http://127.0.0.1:9/cal/");
        assert_eq!(of("scheduling-gone"), None);
    }

    /// A server known to schedule, asked again a day later: a 503, a 429, a
    /// 403 or a 404 to its OPTIONS, or a PROPFIND not answered 207, keeps
    /// what was known, and it is asked again at the next sync; a clear
    /// answer changes it (review of 1e8551e, F9, the review's 503-after-200).
    #[test]
    fn an_error_keeps_what_the_server_said() {
        home();
        let scheduler = Arc::new(Scheduler { auto: true, seen: Mutex::new(Vec::new()), failing: Mutex::new(None) });
        let base = serve(scheduler.clone());
        let client = Client::new("camille", "secret");
        let at = format!("{base}/remote.php/dav/calendars/camille/");
        let account = "scheduling-errors";
        keep_fresh(&client, account, &at);
        let known = of(account).unwrap();
        assert!(known.auto && schedules_for(account, "camille@example.org"));
        let day_old = |known: &Scheduling| save(account, &Scheduling { checked: known.checked - EVERY - 1, ..known.clone() }).unwrap();
        for failing in [("OPTIONS", 503), ("OPTIONS", 429), ("OPTIONS", 403), ("OPTIONS", 404), ("PROPFIND", 500), ("PROPFIND", 403)] {
            day_old(&known);
            *scheduler.failing.lock().unwrap() = Some(failing);
            let asked = scheduler.seen.lock().unwrap().len();
            keep_fresh(&client, account, &at);
            assert!(scheduler.seen.lock().unwrap().len() > asked, "{failing:?}: asked");
            let kept = of(account).unwrap();
            assert!(kept.auto && kept.addresses == known.addresses && kept.checked < known.checked, "{failing:?}: what was known kept, asked again next time: {kept:?}");
        }
        // A clear answer: a DAV header without the word.
        *scheduler.failing.lock().unwrap() = None;
        let plain = Arc::new(Scheduler { auto: false, seen: Mutex::new(Vec::new()), failing: Mutex::new(None) });
        let plain_at = format!("{}/remote.php/dav/calendars/camille/", serve(plain));
        keep_fresh(&client, account, &plain_at);
        assert!(!of(account).unwrap().auto, "a clear answer changes it");
    }
}
