// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! An office's opening hours looked up, because the person pressed "Look
//! up" on its card (`sioul_core::hourslookup`, which builds the questions
//! and reads the answers): one request to each source per press, never by
//! batch or by timer, with the user agent that names Sioul
//! (`suggest::AGENT`). OpenStreetMap's Nominatim takes its turn with the
//! contacts' map (`geocode`): at most one question a second across the
//! whole program, as its usage policy asks.

use crate::SyncError;
use sioul_core::hourslookup::{self, Asked, Found};
use std::time::Duration;

/// The administration's directory's offices for `asked`.
pub fn directory(asked: &Asked) -> Result<Vec<Found>, SyncError> {
    directory_at(hourslookup::DIRECTORY_ENDPOINT, asked)
}

/// OpenStreetMap's places for `asked`, in `language` ("fr"), its turn waited for.
pub fn map(asked: &Asked, language: &str) -> Result<Vec<Found>, SyncError> {
    crate::geocode::wait_turn();
    map_at(hourslookup::MAP_ENDPOINT, asked, language)
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(20))).http_status_as_error(false).build().into()
}

/// One GET with these parameters; its body when the server says 200.
fn asked(endpoint: &str, query: &[(&str, String)], language: Option<&str>, name: &str) -> Result<String, SyncError> {
    let mut request = agent().get(endpoint).header("User-Agent", crate::suggest::AGENT);
    for (key, value) in query {
        request = request.query(*key, value);
    }
    if let Some(language) = language {
        request = request.header("Accept-Language", language);
    }
    let mut response = request.call().map_err(|e| SyncError::Network(e.to_string()))?;
    let status = response.status().as_u16();
    let text = response.body_mut().read_to_string().unwrap_or_default();
    if status != 200 {
        return Err(SyncError::Server(format!("{name}: {status}")));
    }
    Ok(text)
}

pub(crate) fn directory_at(endpoint: &str, question: &Asked) -> Result<Vec<Found>, SyncError> {
    let text = asked(endpoint, &question.directory_query(), None, "service-public.gouv.fr")?;
    hourslookup::directory_answer(&text).map_err(SyncError::Server)
}

pub(crate) fn map_at(endpoint: &str, question: &Asked, language: &str) -> Result<Vec<Found>, SyncError> {
    let text = asked(endpoint, &question.map_query(), Some(language), "Nominatim")?;
    hourslookup::map_answer(&text).map_err(SyncError::Server)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    /// A stand-in server that answers `body` once, with `status`, and gives back the request it read.
    fn stand_in(status: &'static str, body: &'static str) -> (String, std::thread::JoinHandle<String>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/records", listener.local_addr().unwrap());
        let served = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut request = vec![0u8; 8192];
            let n = stream.read(&mut request).unwrap();
            write!(stream, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            String::from_utf8_lossy(&request[..n]).to_string()
        });
        (url, served)
    }

    fn town_hall() -> Asked {
        Asked { name: "Mairie de Exempleville".into(), place: "1 place de l'Exemple\n99999 Exempleville".into(), france: true }
    }

    /// The name and the place, nothing else, with Sioul named; the answer read.
    #[test]
    fn only_the_office_is_sent() {
        let body = include_str!("../../sioul-core/tests/fixtures/hours-directory.json");
        let (url, served) = stand_in("200 OK", body);
        let found = directory_at(&url, &town_hall()).unwrap();
        assert_eq!(found[0].hours, "Mo-Fr 09:00-17:00; PH off");
        let request = served.join().unwrap();
        let line = request.lines().next().unwrap();
        assert!(line.contains("where=nom+LIKE+%22Mairie+Exempleville%22") || line.contains("where=nom%20LIKE%20%22Mairie%20Exempleville%22"), "{line}");
        assert!(line.contains("code_postal+99999") || line.contains("code_postal%2099999"), "{line}");
        assert!(request.to_lowercase().contains("user-agent: sioul/"), "{request}");
        assert!(!request.to_lowercase().contains("cookie") && !request.to_lowercase().contains("authorization"), "{request}");

        let body = include_str!("../../sioul-core/tests/fixtures/hours-nominatim.json");
        let (url, served) = stand_in("200 OK", body);
        let found = map_at(&url, &town_hall(), "fr").unwrap();
        assert_eq!(found.len(), 3);
        let request = served.join().unwrap();
        let line = request.lines().next().unwrap();
        assert!(line.contains("extratags=1") && line.contains("format=jsonv2"), "{line}");
        assert!(request.to_lowercase().contains("accept-language: fr"), "{request}");
    }

    /// A refusal or a page that is no answer: said, never read as nothing found.
    #[test]
    fn a_refusal_is_no_answer() {
        let (url, served) = stand_in("429 Too Many Requests", "slow down");
        assert!(matches!(map_at(&url, &town_hall(), "en"), Err(SyncError::Server(_))));
        served.join().unwrap();
        let (url, served) = stand_in("200 OK", "<html>maintenance</html>");
        assert!(matches!(directory_at(&url, &town_hall()), Err(SyncError::Server(_))));
        served.join().unwrap();
    }
}
