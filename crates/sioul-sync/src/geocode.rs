// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Where an address is, asked of OpenStreetMap's geocoder (Nominatim), under
//! its usage policy: Sioul says who it is and where to find it, asks one
//! address at a time, at most one a second across the whole program (every
//! caller waits its turn: the contacts' map, an event's map), only when you
//! asked, and keeps the answers (`sioul_core::places`), so an address is
//! never asked twice. Never while you type: its policy forbids it
//! (`suggest` has the services made for that).
//!
//! Nominatim reads an address strictly: a word it doesn't know, such as a
//! venue's name before its street ("La Salle, boulevard des Exemples, Exempleville"),
//! and it finds nothing. When it finds nothing, the services that suggest
//! addresses are asked once, as a map app's own search would: the Base Adresse
//! Nationale for addresses in France, then Photon, each tolerant of words it
//! doesn't know and of typos. Their first answer is the place, often the
//! street rather than the venue.

use crate::SyncError;
use sioul_core::places::Place;
use std::time::Duration;

const ENDPOINT: &str = "https://nominatim.openstreetmap.org/search";

/// How long to wait between two questions.
pub const PAUSE: Duration = Duration::from_millis(1100);

/// The place of one address; None when no geocoder knows it. Nominatim first,
/// waiting its turn; then `fuzzy`, the service your region suggests from
/// (`suggest::Service::for_region`), and Photon after the Base Adresse Nationale.
pub fn locate(address: &str, language: &str, fuzzy: crate::suggest::Service) -> Result<Option<Place>, SyncError> {
    wait_turn();
    if let Some(place) = locate_at(ENDPOINT, address, language)? {
        return Ok(Some(place));
    }
    locate_fuzzy(address, language, fuzzy, crate::suggest::BAN, crate::suggest::PHOTON)
}

/// The first answer of the services that suggest addresses, `fuzzy`'s first.
fn locate_fuzzy(address: &str, language: &str, fuzzy: crate::suggest::Service, ban: &str, photon: &str) -> Result<Option<Place>, SyncError> {
    use crate::suggest::Service;
    let query = sioul_core::places::normalize(address);
    let ways: &[(Service, &str)] = match fuzzy {
        Service::Ban => &[(Service::Ban, ban), (Service::Photon, photon)],
        Service::Photon => &[(Service::Photon, photon)],
    };
    for (service, endpoint) in ways {
        if let Some(first) = crate::suggest::suggest_at(endpoint, *service, &query, language)?.into_iter().next() {
            return Ok(Some(Place { lat: first.lat, lon: first.lon }));
        }
    }
    Ok(None)
}

/// When Nominatim was last asked, by any caller of this program.
static LAST: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);

/// Waits until `PAUSE` has passed since the last question, then takes its turn.
fn wait_turn() {
    let mut last = LAST.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(at) = *last {
        let since = at.elapsed();
        if since < PAUSE {
            std::thread::sleep(PAUSE - since);
        }
    }
    *last = Some(std::time::Instant::now());
}

fn locate_at(endpoint: &str, address: &str, language: &str) -> Result<Option<Place>, SyncError> {
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(20))).http_status_as_error(false).build().into();
    let query = sioul_core::places::normalize(address);
    let mut response = agent
        .get(endpoint)
        .query("format", "jsonv2")
        .query("limit", "1")
        .query("q", &query)
        .header("User-Agent", crate::suggest::AGENT)
        .header("Accept-Language", language)
        .call()
        .map_err(|e| SyncError::Network(e.to_string()))?;
    let status = response.status().as_u16();
    let text = response.body_mut().read_to_string().unwrap_or_default();
    if status != 200 {
        return Err(SyncError::Server(format!("Nominatim: {status}")));
    }
    let found: serde_json::Value = serde_json::from_str(&text).map_err(|e| SyncError::Server(e.to_string()))?;
    let first = found.as_array().and_then(|a| a.first());
    let number = |key: &str| first.and_then(|f| f[key].as_str()).and_then(|v| v.parse::<f64>().ok());
    Ok(match (number("lat"), number("lon")) {
        (Some(lat), Some(lon)) => Some(Place { lat, lon }),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    fn stand_in(body: &'static str) -> (String, std::thread::JoinHandle<String>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/search", listener.local_addr().unwrap());
        let served = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut request = vec![0u8; 8192];
            let n = stream.read(&mut request).unwrap();
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            String::from_utf8_lossy(&request[..n]).to_string()
        });
        (url, served)
    }

    #[test]
    fn an_address_found_or_not() {
        let (url, served) = stand_in(r#"[{"place_id":1,"lat":"45.5","lon":"4.25","display_name":"3, Rue des Exemples, Exempleville"}]"#);
        let place = locate_at(&url, "3 rue des Exemples\n99999 Exempleville", "fr").unwrap().unwrap();
        assert_eq!((place.lat, place.lon), (45.5, 4.25));
        let request = served.join().unwrap();
        assert!(request.contains("q=3+rue+des+Exemples%2C+99999+Exempleville") || request.contains("q=3%20rue%20des%20Exemples%2C%2099999%20Exempleville"), "{request}");
        assert!(request.to_lowercase().contains("user-agent: sioul/"), "{request}");
        let (url, served) = stand_in("[]");
        assert_eq!(locate_at(&url, "Nowhere", "en").unwrap(), None);
        served.join().unwrap();
    }

    /// A venue's name before its street: Nominatim finds nothing, the Base
    /// Adresse Nationale finds the street, and Photon is not asked.
    #[test]
    fn a_venue_nominatim_does_not_know_is_found_by_its_street() {
        let (ban, ban_served) = stand_in(r#"{"features":[{"geometry":{"coordinates":[6.19,48.69]},"properties":{"label":"Boulevard des Exemples 99999 Exempleville","score":0.69}}]}"#);
        let place = locate_fuzzy("La Salle, boulevard des Exemples, Exempleville", "fr", crate::suggest::Service::Ban, &ban, "http://127.0.0.1:9/never").unwrap().unwrap();
        assert_eq!((place.lat, place.lon), (48.69, 6.19));
        ban_served.join().unwrap();
        // Nothing from the first service: the next one answers.
        let (ban, ban_served) = stand_in(r#"{"features":[]}"#);
        let (photon, photon_served) = stand_in(r#"{"features":[{"geometry":{"coordinates":[4.25,45.5]},"properties":{"name":"Boulevard des Exemples","city":"Exempleville"}}]}"#);
        let place = locate_fuzzy("La Salle, boulevard des Exemples, Exempleville", "fr", crate::suggest::Service::Ban, &ban, &photon).unwrap().unwrap();
        assert_eq!((place.lat, place.lon), (45.5, 4.25));
        ban_served.join().unwrap();
        photon_served.join().unwrap();
        // Nobody knows it: none.
        let (photon, photon_served) = stand_in(r#"{"features":[]}"#);
        assert_eq!(locate_fuzzy("Nowhere at all", "en", crate::suggest::Service::Photon, "http://127.0.0.1:9/never", &photon).unwrap(), None);
        photon_served.join().unwrap();
    }
}
