// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! An event's place beyond its words (the event card, 5 November 2026): its
//! point, its own `GEO` first, else the places kept on this device
//! (`sioul_core::places`), else one question to OpenStreetMap's geocoder,
//! only once you pressed Map; the way there, a `geo:` link when this system
//! has an app for it (checked each time, never assumed), else a page; and the
//! addresses suggested while you type a place: those you used before, from
//! this device alone, then a service made for it when you allow it (the Base
//! Adresse Nationale in France, else Photon), never Nominatim, whose policy
//! forbids it. Health's events, and agents, are never asked about here: only
//! your own press reaches it.

use crate::backend::{QtThread, json, load_config, offline, tr};
use cxx_qt_lib::QString;
use sioul_core::places::{Place, Places};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

/// Where `location` is, with no network: `geo` ("48.8566;2.3522", the
/// event's own `GEO`) first, else the places kept. {lat, lon, from: "event" or
/// "kept"}; {asked: true} when the geocoder was asked and found none; {} when
/// it is not known yet.
pub(crate) fn point(geo: &str, location: &str) -> String {
    if let Some((lat, lon)) = sioul_core::agenda::geo_of(&format!("BEGIN:VEVENT\r\nGEO:{}\r\nEND:VEVENT\r\n", geo.trim())) {
        return json(&serde_json::json!({ "lat": lat, "lon": lon, "from": "event" }));
    }
    let places = Places::load(&Places::default_path());
    match places.get(location) {
        Some(Place { lat, lon }) => json(&serde_json::json!({ "lat": lat, "lon": lon, "from": "kept" })),
        None if places.asked(location) => json(&serde_json::json!({ "asked": true })),
        None => "{}".into(),
    }
}

/// Addresses being asked of the geocoder now: each once at a time.
static LOCATING: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// `location` asked of OpenStreetMap's geocoder (Nominatim), because you
/// pressed Map: on a thread, at most one question a second for the whole
/// program (`geocode::locate` waits its turn), its answer kept with the places
/// (found or not, never asked twice), the window told (`places_changed`).
/// Returns what stops it, else "".
pub(crate) fn locate(qt: &QtThread, location: &str) -> String {
    let location = location.trim().to_string();
    if location.is_empty() {
        return String::new();
    }
    if offline() {
        return tr().text("map-offline", None);
    }
    let places = Places::load(&Places::default_path());
    if places.get(&location).is_some() || places.asked(&location) {
        return String::new();
    }
    {
        let mut asking = LOCATING.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if asking.contains(&location) {
            return String::new();
        }
        asking.push(location.clone());
    }
    let qt = qt.clone();
    std::thread::spawn(move || {
        let language = tr().text("qt-locale", None).replace('_', "-");
        let found = sioul_sync::geocode::locate(&location, &language);
        let said = match found {
            Ok(place) => {
                let path = Places::default_path();
                let mut places = Places::load(&path);
                places.set(&location, place);
                places.save(&path).err().unwrap_or_default()
            }
            // The network or the service is away: nothing kept, asked again at the next press.
            Err(e) => e.sentence(tr(), "OpenStreetMap"),
        };
        LOCATING.lock().unwrap_or_else(std::sync::PoisonError::into_inner).retain(|l| *l != location);
        let _ = qt.queue(move |mut sioul| {
            if !said.is_empty() {
                sioul.as_mut().set_status(QString::from(&said));
            }
            sioul.as_mut().places_changed();
        });
    });
    String::new()
}

/// Where the lines said once are kept, on this device.
fn said_path() -> std::path::PathBuf {
    sioul_core::config::state_dir().join("said-once.json")
}

/// Whether a line said once (`which`: "map", "suggest") is to be said now:
/// true the first time, which is then kept on this device.
pub(crate) fn first_time(which: &str) -> bool {
    let path = said_path();
    let mut said: Vec<String> = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    if said.iter().any(|s| s == which) {
        return false;
    }
    said.push(which.to_string());
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&path, serde_json::to_string(&said).unwrap_or_default());
    true
}

/// Whether this system has an app for `geo:` links (RFC 5870), as it says
/// now: Linux by `xdg-mime query default x-scheme-handler/geo`; Windows by
/// the `geo` URL protocol's key (`HKEY_CLASSES_ROOT\geo`, "URL Protocol");
/// macOS by LaunchServices (`NSWorkspace URLForApplicationToOpenURL:`, asked
/// through `osascript`). Android tries the link itself, and an intent that
/// resolves to nothing comes back false (`Qt.openUrlExternally`), so the
/// window falls back then.
pub(crate) fn geo_handler() -> bool {
    if cfg!(target_os = "android") {
        return true;
    }
    let output = |program: &str, args: &[&str]| std::process::Command::new(program).args(args).stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null()).output().ok();
    if cfg!(target_os = "windows") {
        return output("reg", &["query", r"HKEY_CLASSES_ROOT\geo", "/v", "URL Protocol"]).is_some_and(|o| o.status.success());
    }
    if cfg!(target_os = "macos") {
        let script = "ObjC.import('AppKit'); $.NSWorkspace.sharedWorkspace.URLForApplicationToOpenURL($.NSURL.URLWithString('geo:0,0')) ? 'yes' : 'no'";
        return output("osascript", &["-l", "JavaScript", "-e", script]).is_some_and(|o| String::from_utf8_lossy(&o.stdout).trim() == "yes");
    }
    output("xdg-mime", &["query", "default", "x-scheme-handler/geo"]).is_some_and(|o| o.status.success() && !String::from_utf8_lossy(&o.stdout).trim().is_empty())
}

/// The service your addresses are suggested from (`suggest::Service::for_region`):
/// the Base Adresse Nationale when your region is France, else Photon.
fn service() -> sioul_sync::suggest::Service {
    let region = sioul_core::phones::chosen(load_config().contacts.region.as_deref(), &tr().text("qt-locale", None)).map(|r| r.code);
    sioul_sync::suggest::Service::for_region(region)
}

/// Its name, for the line said once: "What you type in a place is sent to … to suggest addresses."
pub(crate) fn service_name() -> String {
    service().name().to_string()
}

/// The places you used before that hold what is typed (your events' places,
/// your contacts' postal addresses), from this device alone, five at most,
/// each once.
pub(crate) fn used_before(loaded: &sioul_core::links::Loaded, typed: &str) -> Vec<String> {
    let typed = sioul_core::places::normalize(typed).to_lowercase();
    if typed.chars().count() < 2 {
        return Vec::new();
    }
    let mut out: Vec<String> = Vec::new();
    let places = loaded.events.iter().map(|e| e.location.as_str()).chain(loaded.contacts.iter().flat_map(|c| c.addresses.iter().map(|a| a.value.as_str())));
    for place in places {
        let one = sioul_core::places::normalize(place);
        if one.is_empty() || !one.to_lowercase().contains(&typed) || out.iter().any(|o| o.eq_ignore_ascii_case(&one)) {
            continue;
        }
        out.push(one);
        if out.len() == sioul_sync::suggest::MOST {
            break;
        }
    }
    out
}

/// What is typed now, waiting for the service: the last one only.
static WANTED: Mutex<Option<String>> = Mutex::new(None);
/// A question to the service under way.
static ASKING: AtomicBool = AtomicBool::new(false);

/// The addresses suggested for `typed`, as the window asks after a pause: the
/// places used before at once, then, when allowed (Settings ▸ Agenda) and
/// three characters at least, the service's, on a thread, one question at a
/// time, the last typed only; each told (`addresses_suggested(typed, json)`:
/// `{used: [label], found: [{label, lat, lon}], service}`). Only what was
/// typed is sent; a network away sends nothing and says nothing.
pub(crate) fn suggest(qt: &QtThread, loaded: &sioul_core::links::Loaded, typed: &str) {
    let used = used_before(loaded, typed);
    let tell = |qt: &QtThread, typed: String, found: Vec<sioul_sync::suggest::Suggestion>, used: Vec<String>| {
        let said = json(&serde_json::json!({ "used": used, "found": found, "service": service_name() }));
        let _ = qt.queue(move |mut sioul| sioul.as_mut().addresses_suggested(QString::from(&typed), QString::from(&said)));
    };
    tell(qt, typed.to_string(), Vec::new(), used.clone());
    if !load_config().agenda.suggests_addresses() || offline() || typed.split_whitespace().collect::<String>().chars().count() < 3 {
        return;
    }
    *WANTED.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(typed.to_string());
    if ASKING.swap(true, Ordering::SeqCst) {
        return;
    }
    let qt = qt.clone();
    let service = service();
    let language = tr().text("qt-locale", None).chars().take(2).collect::<String>();
    let loaded_used = used;
    std::thread::spawn(move || {
        let mut first = Some(loaded_used);
        loop {
            let next = WANTED.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take();
            let Some(typed) = next else {
                ASKING.store(false, Ordering::SeqCst);
                // Asked again meanwhile, after the last look: this thread goes on.
                if WANTED.lock().unwrap_or_else(std::sync::PoisonError::into_inner).is_some() && !ASKING.swap(true, Ordering::SeqCst) {
                    continue;
                }
                break;
            };
            let found = sioul_sync::suggest::suggest(service, &typed, &language).unwrap_or_default();
            tell(&qt, typed, found, first.take().unwrap_or_default());
        }
    });
}

/// A suggestion chosen: its point kept for its words, so that its map needs no search.
pub(crate) fn keep_point(label: &str, lat: f64, lon: f64) {
    let path = Places::default_path();
    let mut places = Places::load(&path);
    places.set(label, Some(Place { lat, lon }));
    let _ = places.save(&path);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Its point: the event's own GEO first, then the places kept, else
    /// unknown; the places used before suggested from this device, each once.
    #[test]
    fn a_place_s_point_and_the_places_used_before() {
        let _files = crate::health::health_files_held();
        sioul_core::config::stand_ins_only();
        let _ = std::fs::remove_file(Places::default_path());
        let read = |text: &str| serde_json::from_str::<serde_json::Value>(text).unwrap();
        assert_eq!(read(&point("48.8566;2.3522", "3 rue des Exemples"))["from"], "event");
        assert_eq!(point("", "3 rue des Exemples"), "{}");
        keep_point("3 rue des Exemples", 45.5, 4.25);
        let kept = read(&point("", "3 rue des Exemples"));
        assert_eq!((kept["lat"].as_f64(), kept["from"].as_str()), (Some(45.5), Some("kept")));
        let mut loaded = sioul_core::links::Loaded::default();
        loaded.events = ["3 rue des Exemples", "3 Rue des Exemples", "Salle des fêtes", ""].iter().map(|l| sioul_core::agenda::EventRef { location: (*l).to_string(), ..Default::default() }).collect();
        assert_eq!(used_before(&loaded, "rue des ex"), vec!["3 rue des Exemples".to_string()]);
        assert_eq!(used_before(&loaded, "s"), Vec::<String>::new(), "two characters at least");
        let _ = std::fs::remove_file(Places::default_path());
        // Said once, then never again.
        let _ = std::fs::remove_file(said_path());
        assert!(first_time("map") && !first_time("map") && first_time("suggest"));
        let _ = std::fs::remove_file(said_path());
    }
}
