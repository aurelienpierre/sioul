// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! An office's opening hours looked up, because the person pressed "Look
//! up" on its card (`sioul_core::hourslookup`, which builds the questions
//! and reads the answers): one request to each source per press, never by
//! batch or by timer, over https only, with the user agent that names Sioul
//! (`suggest::AGENT`). The map's search service (OpenStreetMap's Nominatim
//! by default, `map.search`) takes its turn with the contacts' map
//! (`geocode`): at most one question a second across the whole program, as
//! its usage policy asks; set to None, or after its refusal today, it is
//! not asked.

use crate::SyncError;
use sioul_core::hourslookup::{self, Asked, Found};
use std::path::Path;

/// The administration's directory's offices for `asked`; none asked when
/// the name has no word of its own (`Asked::directory_query`).
pub fn directory(asked: &Asked) -> Result<Vec<Found>, SyncError> {
    directory_at(hourslookup::DIRECTORY_ENDPOINT, asked)
}

/// The map's places for `asked`, in `language` ("fr"), its turn waited for:
/// at the map's search service's address (`geocode::search_address`),
/// nothing asked when it is set to None or refused today.
pub fn map(asked: &Asked, language: &str) -> Result<Vec<Found>, SyncError> {
    let endpoint = crate::geocode::search_address()?;
    crate::geocode::wait_turn();
    map_at(&endpoint, asked, language, &sioul_core::config::state_dir())
}

/// One GET with these parameters, over https only (redirects included); its
/// body and status.
fn asked(endpoint: &str, query: &[(&str, String)], language: Option<&str>) -> Result<(u16, String), SyncError> {
    let mut request = crate::geocode::agent().get(endpoint).header("User-Agent", crate::suggest::AGENT);
    for (key, value) in query {
        request = request.query(*key, value);
    }
    if let Some(language) = language {
        request = request.header("Accept-Language", language);
    }
    let mut response = request.call().map_err(|e| SyncError::Network(e.to_string()))?;
    let status = response.status().as_u16();
    Ok((status, response.body_mut().read_to_string().unwrap_or_default()))
}

pub(crate) fn directory_at(endpoint: &str, question: &Asked) -> Result<Vec<Found>, SyncError> {
    let Some(query) = question.directory_query() else { return Ok(Vec::new()) };
    let (status, text) = asked(endpoint, &query, None)?;
    if status != 200 {
        return Err(SyncError::Server(format!("service-public.gouv.fr: {status}")));
    }
    hourslookup::directory_answer(&text).map_err(SyncError::Server)
}

/// A refusal (403, 429) keeps the map's search service unasked until the
/// next local day, for the contacts' map too (`geocode::refusal`, in `state`).
pub(crate) fn map_at(endpoint: &str, question: &Asked, language: &str, state: &Path) -> Result<Vec<Found>, SyncError> {
    let (status, text) = asked(endpoint, &question.map_query(), Some(language))?;
    if let Some(refused) = crate::geocode::refusal(endpoint, status, state) {
        return Err(refused);
    }
    hourslookup::map_answer(&text).map_err(SyncError::Server)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::time::Duration;

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

    fn state(name: &str) -> std::path::PathBuf {
        let state = std::env::temp_dir().join(format!("sioul-hours-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&state);
        state
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
        let found = map_at(&url, &town_hall(), "fr", &state("sent")).unwrap();
        assert_eq!(found.len(), 3);
        let request = served.join().unwrap();
        let line = request.lines().next().unwrap();
        assert!(line.contains("extratags=1") && line.contains("format=jsonv2") && line.contains("addressdetails=1"), "{line}");
        assert!(request.to_lowercase().contains("accept-language: fr"), "{request}");
    }

    /// A refusal or a page that is no answer: said, never read as nothing
    /// found. The map's refusal (403, 429) stops it for the day, the
    /// contacts' map too; the directory's does not touch the map.
    #[test]
    fn a_refusal_is_no_answer() {
        let kept = state("refused");
        let (url, served) = stand_in("429 Too Many Requests", "slow down");
        let refused = map_at(&url, &town_hall(), "en", &kept).unwrap_err();
        served.join().unwrap();
        assert!(matches!(refused, SyncError::Said(crate::geocode::SAID_WAIT, _)), "{refused:?}");
        assert!(sioul_core::mapsearch::waiting(&kept, sioul_core::zone::now().date(), &url));
        let (url, served) = stand_in("403 Forbidden", "");
        assert!(matches!(directory_at(&url, &town_hall()), Err(SyncError::Server(_))));
        served.join().unwrap();
        let (url, served) = stand_in("200 OK", "<html>maintenance</html>");
        assert!(matches!(directory_at(&url, &town_hall()), Err(SyncError::Server(_))));
        served.join().unwrap();
        // A name made only of small words: the directory is not asked at all (no server needed).
        let small = Asked { name: "La".into(), ..town_hall() };
        assert_eq!(directory_at("http://127.0.0.1:9/never", &small).unwrap(), Vec::new());
        let _ = std::fs::remove_dir_all(&kept);
    }

    /// A server that answers more than a body may hold (ureq's 10 MB): said
    /// as no answer, never read whole (review 11).
    #[test]
    fn a_flood_is_no_answer() {
        // A body larger than ureq's 10 MB limit.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/records", listener.local_addr().unwrap());
        let served = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut request = vec![0u8; 8192];
            let _ = stream.read(&mut request);
            let size = 11 * 1024 * 1024;
            let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {size}\r\nConnection: close\r\n\r\n");
            let chunk = vec![b' '; 64 * 1024];
            for _ in 0..(size / chunk.len()) {
                if stream.write_all(&chunk).is_err() {
                    break;
                }
            }
        });
        assert!(directory_at(&url, &town_hall()).is_err(), "never read whole");
        let _ = served.join();
    }
}
