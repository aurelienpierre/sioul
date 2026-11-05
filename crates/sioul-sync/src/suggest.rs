// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Addresses suggested while a place is typed (the event form's place, 5
//! November 2026), from a service made for it, never Nominatim, whose policy
//! forbids search as you type: France's Base Adresse Nationale (through the
//! Géoplateforme, `data.geopf.fr/geocodage`, which replaced
//! `api-adresse.data.gouv.fr` in January 2026; 50 requests a second for an
//! address) for addresses in France, else Photon (komoot's, fair use). Only
//! what was typed is sent, with Sioul named; five suggestions at most, each
//! with its point, so that the map needs no search later.

use crate::SyncError;
use std::time::Duration;

/// France's addresses, autocompletion built in.
pub const BAN: &str = "https://data.geopf.fr/geocodage/search";
/// The world's, from OpenStreetMap's data.
pub const PHOTON: &str = "https://photon.komoot.io/api/";

/// How Sioul names itself to these services, and to OpenStreetMap's.
pub const AGENT: &str = concat!("Sioul/", env!("CARGO_PKG_VERSION"), " (https://sioul.aurelienpierre.com/)");

/// Five at most.
pub const MOST: usize = 5;

/// Which service suggests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Service {
    /// The Base Adresse Nationale: for addresses in France.
    Ban,
    Photon,
}

impl Service {
    /// The service for a person whose addresses are in `region` ("FR"): the
    /// Base Adresse Nationale in France, Photon elsewhere.
    pub fn for_region(region: Option<&str>) -> Service {
        if region.is_some_and(|r| r.eq_ignore_ascii_case("FR")) { Service::Ban } else { Service::Photon }
    }

    /// Its name, as the line that says where what is typed goes names it.
    pub fn name(self) -> &'static str {
        match self {
            Service::Ban => "Base Adresse Nationale",
            Service::Photon => "Photon (komoot)",
        }
    }

    fn endpoint(self) -> &'static str {
        match self {
            Service::Ban => BAN,
            Service::Photon => PHOTON,
        }
    }
}

/// One address suggested, with its point.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Suggestion {
    /// "8 Boulevard du Port 95000 Cergy".
    pub label: String,
    pub lat: f64,
    pub lon: f64,
}

/// What `service` suggests for `typed` (three characters at least: fewer
/// asks nothing), in `language` ("fr") where the service takes one.
pub fn suggest(service: Service, typed: &str, language: &str) -> Result<Vec<Suggestion>, SyncError> {
    suggest_at(service.endpoint(), service, typed, language)
}

fn suggest_at(endpoint: &str, service: Service, typed: &str, language: &str) -> Result<Vec<Suggestion>, SyncError> {
    let typed = typed.split_whitespace().collect::<Vec<_>>().join(" ");
    if typed.chars().count() < 3 {
        return Ok(Vec::new());
    }
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(8))).http_status_as_error(false).build().into();
    let limit = MOST.to_string();
    let request = agent.get(endpoint).query("q", &typed).query("limit", &limit).header("User-Agent", AGENT);
    let request = match service {
        Service::Ban => request.query("autocomplete", "1").query("index", "address"),
        // Photon's languages: its own few; another, its default.
        Service::Photon if ["en", "de", "fr", "it"].contains(&language) => request.query("lang", language),
        Service::Photon => request,
    };
    let mut response = request.call().map_err(|e| SyncError::Network(e.to_string()))?;
    let status = response.status().as_u16();
    let text = response.body_mut().read_to_string().unwrap_or_default();
    if status != 200 {
        return Err(SyncError::Server(format!("{}: {status}", service.name())));
    }
    let found: serde_json::Value = serde_json::from_str(&text).map_err(|e| SyncError::Server(e.to_string()))?;
    let mut out: Vec<Suggestion> = Vec::new();
    for feature in found["features"].as_array().into_iter().flatten() {
        let point = &feature["geometry"]["coordinates"];
        let (Some(lon), Some(lat)) = (point[0].as_f64(), point[1].as_f64()) else { continue };
        let properties = &feature["properties"];
        let label = match service {
            Service::Ban => properties["label"].as_str().unwrap_or_default().trim().to_string(),
            Service::Photon => photon_label(properties),
        };
        if label.is_empty() || out.iter().any(|s| s.label == label) {
            continue;
        }
        out.push(Suggestion { label, lat, lon });
        if out.len() == MOST {
            break;
        }
    }
    Ok(out)
}

/// A Photon place as one line: its number and street (or its name), its
/// postcode and town, its country: "3 Rue des Exemples, 99999 Exempleville, France".
fn photon_label(properties: &serde_json::Value) -> String {
    let text = |key: &str| properties[key].as_str().unwrap_or_default().trim().to_string();
    let street = match (text("housenumber"), text("street")) {
        (number, street) if !number.is_empty() && !street.is_empty() => format!("{number} {street}"),
        (_, street) if !street.is_empty() && text("name").is_empty() => street,
        _ => text("name"),
    };
    let town = [text("postcode"), text("city")].into_iter().filter(|t| !t.is_empty()).collect::<Vec<_>>().join(" ");
    [street, town, text("country")].into_iter().filter(|t| !t.is_empty()).collect::<Vec<_>>().join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    /// A stand-in for a service: answers `body` once, and gives back the request it read.
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

    /// The Base Adresse Nationale's answer read: each label and point, five
    /// at most, the same label once; what was typed sent alone, autocompleted,
    /// Sioul named.
    #[test]
    fn the_ban_suggests_addresses_with_their_points() {
        let (url, served) = stand_in(r#"{"type":"FeatureCollection","features":[{"type":"Feature","geometry":{"type":"Point","coordinates":[4.25,45.5]},"properties":{"label":"3 Rue des Exemples 99999 Exempleville","type":"housenumber"}},{"type":"Feature","geometry":{"type":"Point","coordinates":[4.25,45.5]},"properties":{"label":"3 Rue des Exemples 99999 Exempleville"}},{"type":"Feature","geometry":{"type":"Point","coordinates":[1.5,47.25]},"properties":{"label":"3 Rue des Exemples 88888 Autreville"}}]}"#);
        let found = suggest_at(&url, Service::Ban, "3 rue des  ex", "fr").unwrap();
        assert_eq!(found, vec![Suggestion { label: "3 Rue des Exemples 99999 Exempleville".into(), lat: 45.5, lon: 4.25 }, Suggestion { label: "3 Rue des Exemples 88888 Autreville".into(), lat: 47.25, lon: 1.5 }]);
        let request = served.join().unwrap();
        assert!(request.contains("q=3+rue+des+ex") || request.contains("q=3%20rue%20des%20ex"), "{request}");
        assert!(request.contains("autocomplete=1") && request.contains("limit=5"), "{request}");
        assert!(request.to_lowercase().contains("user-agent: sioul/"), "{request}");
    }

    /// Photon's answer as one line a place; fewer than three characters ask nothing.
    #[test]
    fn photon_suggests_places_as_lines() {
        let (url, served) = stand_in(r#"{"type":"FeatureCollection","features":[{"type":"Feature","geometry":{"type":"Point","coordinates":[-0.1,51.5]},"properties":{"housenumber":"10","street":"Example Street","postcode":"EX1 1AA","city":"Exampleton","country":"United Kingdom"}},{"type":"Feature","geometry":{"type":"Point","coordinates":[2.0,48.0]},"properties":{"name":"Example Hall","city":"Exampleville","country":"France"}}]}"#);
        let found = suggest_at(&url, Service::Photon, "10 exam", "en").unwrap();
        assert_eq!(found.iter().map(|s| s.label.as_str()).collect::<Vec<_>>(), vec!["10 Example Street, EX1 1AA Exampleton, United Kingdom", "Example Hall, Exampleville, France"]);
        let request = served.join().unwrap();
        assert!(request.contains("lang=en") && !request.contains("autocomplete"), "{request}");
        assert_eq!(suggest_at("http://127.0.0.1:9/never", Service::Photon, "ab", "en").unwrap(), Vec::new(), "nothing asked");
        assert_eq!(Service::for_region(Some("fr")), Service::Ban);
        assert_eq!(Service::for_region(Some("BE")), Service::Photon);
        assert_eq!(Service::for_region(None), Service::Photon);
    }
}
