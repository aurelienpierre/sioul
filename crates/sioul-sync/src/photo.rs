// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! A contact's photo that its card names on the web (`PHOTO;VALUE=uri:https://…`),
//! loaded only when you ask for it on the person's card, then kept (decided
//! 3 November 2026): nothing remote loads by itself, and the server that holds
//! it learns that you looked once, never each time the contact shows. The copy
//! goes into the cache's `photos/`, named by the address
//! (`contacts::web_photo_path`); the card itself is never changed.

use std::path::{Path, PathBuf};
use std::time::Duration;

/// Larger than this is no contact's photo.
pub const LARGEST: u64 = 1 << 20;
const AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64; rv:128.0) Gecko/20100101 Firefox/128.0";

/// HTTPS only, redirects included, so that anyone on the network cannot see
/// whom you looked at; and public addresses only, every address of the
/// host checked before it is reached (`stalls::public_agent`): a card can
/// come from anyone, and a device of your network (a NAS, a router, even
/// with a valid certificate), or a public name pointing to a private
/// address, is never asked (review of 01ca9df, 3).
fn agent() -> ureq::Agent {
    crate::stalls::public_agent(ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(20))).http_status_as_error(false).https_only(true).build())
}

/// A picture's kind, from its type and its first bytes, which must agree:
/// "jpeg", "png", "webp", "gif"; none for anything else (a page, a script,
/// an SVG, which can hold one).
pub fn kind_of(content_type: &str, bytes: &[u8]) -> Option<&'static str> {
    let said = content_type.split(';').next().unwrap_or("").trim().to_ascii_lowercase();
    let (kind, magic): (&'static str, bool) = match said.as_str() {
        "image/jpeg" | "image/jpg" => ("jpeg", bytes.starts_with(&[0xFF, 0xD8, 0xFF])),
        "image/png" => ("png", bytes.starts_with(b"\x89PNG\r\n\x1a\n")),
        "image/webp" => ("webp", bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP"),
        "image/gif" => ("gif", bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a")),
        _ => return None,
    };
    magic.then_some(kind)
}

/// Whether an address may be asked for a photo: https, a host named.
pub fn may_ask(url: &str) -> bool {
    url.trim().strip_prefix("https://").is_some_and(|rest| rest.split(['/', '?', '#']).next().is_some_and(|host| !host.is_empty() && !host.contains('@')))
}

/// The photo at `url`, asked once and kept in `folder`; its file. Refused:
/// an address that is not https, an answer that is not a picture of a kind
/// above, larger than `LARGEST`, or slower than twenty seconds in all.
pub fn fetch(url: &str, folder: &Path) -> Result<PathBuf, String> {
    let url = url.trim();
    if !may_ask(url) {
        return Err("not an https address".into());
    }
    let mut answer = agent().get(url).header("User-Agent", AGENT).header("Accept", "image/jpeg, image/png, image/webp, image/gif").call().map_err(|e| e.to_string())?;
    if !answer.status().is_success() {
        return Err(format!("the server answered {}", answer.status().as_u16()));
    }
    let content_type = answer.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    let bytes = answer.body_mut().with_config().limit(LARGEST + 1).read_to_vec().map_err(|e| e.to_string())?;
    keep(url, &content_type, &bytes, folder)
}

/// A photo received, checked and written: beside, then renamed into its place.
pub fn keep(url: &str, content_type: &str, bytes: &[u8], folder: &Path) -> Result<PathBuf, String> {
    if bytes.len() as u64 > LARGEST {
        return Err("larger than a megabyte".into());
    }
    let kind = kind_of(content_type, bytes).ok_or("not a picture")?;
    std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    let path = sioul_core::contacts::web_photo_path(folder, url, kind);
    let part = path.with_extension("part");
    std::fs::write(&part, bytes).map_err(|e| format!("{}: {e}", part.display()))?;
    std::fs::rename(&part, &path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A photo is kept only as the picture it says it is, a megabyte at
    /// most, asked for over https alone; its copy is the one the card then shows.
    #[test]
    fn a_photo_is_kept_only_as_a_picture() {
        let png = b"\x89PNG\r\n\x1a\nrest";
        assert_eq!(kind_of("image/png", png), Some("png"));
        assert_eq!(kind_of("image/jpeg; charset=binary", &[0xFF, 0xD8, 0xFF, 0xE0]), Some("jpeg"));
        assert_eq!(kind_of("image/jpeg", png), None, "the type and the bytes must agree");
        assert_eq!(kind_of("image/svg+xml", b"<svg/>"), None, "an SVG can hold a script");
        assert_eq!(kind_of("text/html", b"<html>"), None);
        for url in ["http://example.org/jane.jpg", "file:///etc/passwd", "https://", "https://user@example.org/jane.jpg", "javascript:alert(1)"] {
            assert!(!may_ask(url), "{url}");
        }
        assert!(may_ask("https://example.org/jane.jpg"));
        assert!(fetch("http://example.org/jane.jpg", Path::new("/nonexistent")).is_err(), "refused before any network");
        // Only public addresses are reached: a device of your network never (review of 01ca9df, 3).
        let at = |ip: &str| vec![std::net::SocketAddr::new(ip.parse().unwrap(), 443)];
        for refused in ["127.0.0.1", "10.0.0.5", "192.168.1.10", "172.16.0.1", "169.254.1.1", "100.64.0.1", "0.0.0.0", "224.0.0.1", "203.0.113.9", "::1", "fd00::1", "fe80::1", "::ffff:192.168.1.10", "ff02::1"] {
            assert!(!crate::stalls::public_only(&at(refused)), "{refused}");
        }
        assert!(crate::stalls::public_only(&at("93.184.215.14")) && crate::stalls::public_only(&at("2606:2800:21f:cb07::1")));
        let mixed = [at("93.184.215.14"), at("192.168.1.10")].concat();
        assert!(!crate::stalls::public_only(&mixed), "one private address among public ones: refused");
        assert!(!crate::stalls::public_only(&[]));
        let folder = std::env::temp_dir().join(format!("sioul-photo-{}", std::process::id()));
        let url = "https://example.org/jane.png";
        let kept = keep(url, "image/png", png, &folder).unwrap();
        assert_eq!(sioul_core::contacts::web_photo_file(&folder, url), Some(kept.clone()));
        assert_eq!(std::fs::read(&kept).unwrap(), png.to_vec());
        assert!(keep(url, "image/png", &vec![0u8; (LARGEST + 1) as usize], &folder).is_err());
        assert!(keep("https://example.org/page", "text/html", b"<html>", &folder).is_err());
        assert_eq!(sioul_core::contacts::web_photo_file(&folder, "https://example.org/page"), None);
        let _ = std::fs::remove_dir_all(&folder);
    }
}
