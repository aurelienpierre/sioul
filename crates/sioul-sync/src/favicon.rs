// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! A site's own icon, for the list of sites (docs/sites.md, "The list"):
//! asked of the site itself, never of an icon service that would learn which
//! sites you keep. Its page is read for the icon it names (`<link rel="icon">`,
//! the larger the better), else `/favicon.ico`; the icon is kept in the cache
//! folder, by host, and asked again after a week.

use std::path::{Path, PathBuf};
use std::time::Duration;
use ureq::ResponseExt;

/// Asked again after this long.
pub const FRESH: Duration = Duration::from_secs(7 * 24 * 3600);
/// Larger than this is no icon.
const LARGEST: u64 = 512 * 1024;
const AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64; rv:128.0) Gecko/20100101 Firefox/128.0";

/// Where a site's icon is kept: `<dir>/<host>.<png|ico|svg|…>`, if it is.
pub fn cached(dir: &Path, host: &str) -> Option<PathBuf> {
    ["png", "ico", "svg", "gif", "jpg", "webp"].iter().map(|ext| dir.join(format!("{}.{ext}", safe(host)))).find(|p| p.is_file())
}

/// Whether the icon kept for `host` is missing or older than a week.
pub fn stale(dir: &Path, host: &str) -> bool {
    cached(dir, host).and_then(|p| std::fs::metadata(p).ok()).and_then(|m| m.modified().ok()).and_then(|t| t.elapsed().ok()).is_none_or(|age| age > FRESH)
}

fn safe(host: &str) -> String {
    host.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '-').collect()
}

/// HTTPS only, redirects included: a page cannot send Sioul to a plain
/// address, where anyone on the network sees which sites you keep, nor to a
/// device of your network (a router's page has no certificate to show).
fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(15))).http_status_as_error(false).https_only(true).build().into()
}

/// The page's address after its redirects, and its HTML (its first 2 MB).
fn page(url: &str) -> Option<(String, String)> {
    let mut answer = agent().get(url).header("User-Agent", AGENT).header("Accept", "text/html").call().ok()?;
    if !answer.status().is_success() {
        return None;
    }
    let final_url = answer.get_uri().to_string();
    let html = answer.body_mut().with_config().limit(2 * 1024 * 1024).read_to_string().ok()?;
    Some((final_url, html))
}

/// One attribute of a tag's text: `rel="icon"`, `href='/i.png'`, `sizes=32x32`.
fn attribute(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let mut from = 0;
    while let Some(at) = lower[from..].find(name).map(|i| i + from) {
        from = at + name.len();
        // A whole attribute name: after a space, before "=".
        let before_ok = at > 0 && lower.as_bytes()[at - 1].is_ascii_whitespace();
        let rest = lower[from..].trim_start();
        if !before_ok || !rest.starts_with('=') {
            continue;
        }
        let start = tag.len() - rest.len() + 1;
        let value = tag[start..].trim_start();
        let value = match value.chars().next()? {
            q @ ('"' | '\'') => value[1..].split(q).next()?.to_string(),
            _ => value.split(|c: char| c.is_whitespace() || c == '>').next()?.to_string(),
        };
        return Some(value);
    }
    None
}

/// The icons a page names, best first: by declared size (up to 256), PNG
/// first; SVG last, as some draw nothing outside a browser.
fn named_icons(html: &str) -> Vec<String> {
    let lower = html.to_ascii_lowercase();
    let mut found: Vec<(i64, String)> = Vec::new();
    let mut from = 0;
    while let Some(at) = lower[from..].find("<link").map(|i| i + from) {
        let end = lower[at..].find('>').map_or(lower.len(), |e| at + e);
        let tag = &html[at..end];
        from = end;
        let Some(rel) = attribute(tag, "rel").map(|r| r.to_ascii_lowercase()) else { continue };
        // mask-icon is one colour, for Safari's tabs: not an icon to show.
        if !rel.split_whitespace().any(|w| w == "icon" || w == "apple-touch-icon") || rel.contains("mask-icon") {
            continue;
        }
        let Some(href) = attribute(tag, "href").filter(|h| !h.trim().is_empty() && !h.starts_with("data:")) else { continue };
        let size = attribute(tag, "sizes").and_then(|s| s.split(['x', 'X']).next().and_then(|n| n.trim().parse::<i64>().ok())).unwrap_or(if rel.contains("apple-touch-icon") { 180 } else { 16 });
        let kind = href.to_ascii_lowercase();
        let bonus = if kind.contains(".svg") { -1000 } else if kind.contains(".png") { 20 } else { 0 };
        let score = size.min(256) + bonus;
        found.push((score, href));
    }
    found.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
    found.into_iter().map(|(_, href)| href).collect()
}

/// `href` read from the page at `base`.
fn join(base: &str, href: &str) -> String {
    let href = href.trim();
    if href.starts_with("https://") || href.starts_with("http://") {
        return href.to_string();
    }
    let scheme = base.split("://").next().unwrap_or("https");
    if let Some(rest) = href.strip_prefix("//") {
        return format!("{scheme}://{rest}");
    }
    let after = base.split("://").nth(1).unwrap_or("");
    let host = after.split(['/', '?', '#']).next().unwrap_or("");
    if href.starts_with('/') {
        return format!("{scheme}://{host}{href}");
    }
    let path = after[host.len()..].split(['?', '#']).next().unwrap_or("/");
    let dir = path.rfind('/').map_or("/", |i| &path[..=i]);
    format!("{scheme}://{host}{dir}{href}")
}

/// What an icon's bytes are, by their first bytes: its file extension; None for no image.
fn kind_of(bytes: &[u8]) -> Option<&'static str> {
    let head = &bytes[..bytes.len().min(512)];
    if head.starts_with(b"\x89PNG") {
        Some("png")
    } else if head.starts_with(&[0, 0, 1, 0]) {
        Some("ico")
    } else if head.starts_with(b"GIF8") {
        Some("gif")
    } else if head.starts_with(&[0xff, 0xd8]) {
        Some("jpg")
    } else if head.starts_with(b"RIFF") && head.get(8..12) == Some(b"WEBP") {
        Some("webp")
    } else {
        // An error page drawing its logo in SVG is still a page, not an icon.
        let text = String::from_utf8_lossy(head).to_ascii_lowercase();
        (text.contains("<svg") && !text.contains("<html") && !text.contains("<!doctype html")).then_some("svg")
    }
}

fn download(url: &str) -> Option<(Vec<u8>, &'static str)> {
    let mut answer = agent().get(url).header("User-Agent", AGENT).header("Accept", "image/*").call().ok()?;
    if !answer.status().is_success() {
        return None;
    }
    let bytes = answer.body_mut().with_config().limit(LARGEST).read_to_vec().ok()?;
    let kind = kind_of(&bytes)?;
    Some((bytes, kind))
}

/// The icon of the site at `url`, asked of the site and kept in `dir`; its file.
pub fn fetch(url: &str, dir: &Path) -> Result<PathBuf, String> {
    let host = url.split("://").nth(1).unwrap_or("").split(['/', '?', '#', ':']).next().unwrap_or("").to_lowercase();
    if host.is_empty() {
        return Err("no host".into());
    }
    let (base, html) = page(url).unwrap_or_else(|| (url.to_string(), String::new()));
    // Raster icons first, then /favicon.ico, an SVG last: some draw nothing outside a browser.
    let (svg, raster): (Vec<String>, Vec<String>) = named_icons(&html).iter().map(|h| join(&base, h)).partition(|u| u.to_ascii_lowercase().contains(".svg"));
    let candidates: Vec<String> = raster.into_iter().chain([join(&base, "/favicon.ico"), format!("https://{host}/favicon.ico")]).chain(svg).collect();
    let (bytes, kind) = candidates.iter().find_map(|c| download(c)).ok_or_else(|| format!("{host}: no icon"))?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    // One icon per host: an older one of another format goes.
    if let Some(old) = cached(dir, &host) {
        let _ = std::fs::remove_file(old);
    }
    let path = dir.join(format!("{}.{kind}", safe(&host)));
    std::fs::write(&path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icons_a_page_names() {
        let html = r#"<head><link rel="stylesheet" href="/s.css"><link rel="icon" type="image/png" sizes="32x32" href="/i32.png"><LINK REL='shortcut icon' href=/favicon.ico><link rel="apple-touch-icon" href="https://cdn.example.org/touch.png"><link rel="mask-icon" href="/mask.svg"><link rel="icon" href="data:image/png;base64,AAAA"></head>"#;
        assert_eq!(named_icons(html), vec!["https://cdn.example.org/touch.png", "/i32.png", "/favicon.ico"]);
        assert_eq!(join("https://www.example.org/a/b?x=1", "/i.png"), "https://www.example.org/i.png");
        assert_eq!(join("https://www.example.org/a/b", "i.png"), "https://www.example.org/a/i.png");
        assert_eq!(join("https://www.example.org/", "//cdn.example.net/i.png"), "https://cdn.example.net/i.png");
        assert_eq!(kind_of(b"\x89PNG\r\n"), Some("png"));
        assert_eq!(kind_of(b"<?xml version=\"1.0\"?><svg xmlns"), Some("svg"));
        assert_eq!(kind_of(b"<html>not found</html>"), None, "an error page is no icon");
        assert_eq!(kind_of(b"<!DOCTYPE html><body><svg viewBox=\"0 0 9 9\"></svg>Not found</body>"), None, "nor one with a drawing in it");
        assert_eq!(attribute(r#"<link data-rel="x" rel="icon""#, "rel").as_deref(), Some("icon"), "a whole attribute name");
    }

    /// Icons of three real sites, kept in a folder of their own. Run by hand: it asks the sites.
    #[test]
    #[ignore]
    fn real_sites_give_their_icons() {
        let dir = std::env::temp_dir().join(format!("sioul-favicons-{}", std::process::id()));
        for url in ["https://app.element.io/", "https://www.urssaf.fr/accueil/se-connecter.html", "https://discord.com/app"] {
            let path = fetch(url, &dir).unwrap();
            assert!(std::fs::metadata(&path).unwrap().len() > 0, "{url}");
            println!("{url} → {}", path.display());
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
