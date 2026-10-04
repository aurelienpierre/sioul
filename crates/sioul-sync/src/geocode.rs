// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Where an address is, asked of OpenStreetMap's geocoder (Nominatim), under
//! its usage policy: Sioul says who it is, asks one address at a time, at
//! most one a second (the caller waits), and keeps the answers
//! (`sioul_core::places`), so an address is never asked twice.

use crate::SyncError;
use sioul_core::places::Place;
use std::time::Duration;

const ENDPOINT: &str = "https://nominatim.openstreetmap.org/search";

/// How long to wait between two questions.
pub const PAUSE: Duration = Duration::from_millis(1100);

/// The place of one address; None when the geocoder knows none.
pub fn locate(address: &str, language: &str) -> Result<Option<Place>, SyncError> {
    locate_at(ENDPOINT, address, language)
}

fn locate_at(endpoint: &str, address: &str, language: &str) -> Result<Option<Place>, SyncError> {
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(20))).http_status_as_error(false).build().into();
    let query = sioul_core::places::normalize(address);
    let mut response = agent
        .get(endpoint)
        .query("format", "jsonv2")
        .query("limit", "1")
        .query("q", &query)
        .header("User-Agent", concat!("Sioul/", env!("CARGO_PKG_VERSION"), " (desktop mail and contacts client)"))
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
}
