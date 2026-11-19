// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The manifest of the extensions' repository (docs/extensions.md, "The
//! manifest"): what is offered, each extension's hosts, sign-in and file
//! with its hash. Read only once its signature held ([`super::signature`]).
//! Everything an extension will be given is read from here, never from the
//! extension itself: its hosts, whether it signs in and what it watches for.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// The form of manifest Sioul reads.
pub const FORMAT: u32 = 1;
/// The most hosts an extension may name.
const MOST_HOSTS: usize = 64;

/// Words in both of Sioul's languages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Words {
    pub en: String,
    pub fr: String,
}

impl Words {
    /// In French when `french`, else in English; the other when that one is empty.
    pub fn said(&self, french: bool) -> &str {
        let (first, other) = if french { (&self.fr, &self.en) } else { (&self.en, &self.fr) };
        if first.trim().is_empty() { other } else { first }
    }
}

/// One extension offered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Offer {
    pub id: String,
    pub version: String,
    pub interface: u32,
    pub name: Words,
    pub about: Words,
    pub brings: Vec<String>,
    #[serde(default)]
    pub hosts: Vec<String>,
    /// The beginning of the address a sign-in returns to; none: it keeps no sign-in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sign_in: Option<String>,
    /// An `https` address, or a name beside the manifest.
    pub file: String,
    pub sha256: String,
}

/// What an extension is given beyond the time and its log: the hosts it may
/// ask, and the sign-in it may watch for (and with it its secret).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Grants {
    pub hosts: BTreeSet<String>,
    pub sign_in: Option<String>,
}

impl Offer {
    pub fn grants(&self) -> Grants {
        Grants { hosts: self.hosts.iter().map(|h| h.to_ascii_lowercase()).collect(), sign_in: self.sign_in.clone() }
    }

    /// Whether this Sioul can run it: an interface it speaks, data it takes.
    pub fn usable(&self) -> bool {
        self.interface == super::INTERFACE && !self.brings.is_empty() && self.brings.iter().all(|b| super::BRINGS.contains(&b.as_str()))
    }
}

impl Grants {
    /// Whether `self` asks for more than `accepted`: a host not accepted, or
    /// a sign-in where there was none or another one.
    pub fn asks_more_than(&self, accepted: &Grants) -> bool {
        !self.hosts.is_subset(&accepted.hosts) || (self.sign_in.is_some() && self.sign_in != accepted.sign_in)
    }
}

/// A manifest read and checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    /// Grows with each manifest signed (`state::State::serial`).
    pub serial: u64,
    /// The SHA-256 of its exact bytes, hexadecimal: two manifests under one serial differ by it.
    pub sha256: String,
    pub offers: Vec<Offer>,
}

/// Fields this Sioul does not know are passed over: it grants only what it knows.
#[derive(Deserialize)]
struct Raw {
    format: u32,
    serial: u64,
    #[serde(default, rename = "extension")]
    extensions: Vec<Offer>,
}

/// The manifest's text read, each offer checked; the first fault said.
pub fn parse(text: &str) -> Result<Manifest, String> {
    let raw: Raw = toml::from_str(text).map_err(|e| format!("the manifest does not read: {e}"))?;
    if raw.format != FORMAT {
        return Err(format!("a manifest of form {} (this Sioul reads {FORMAT})", raw.format));
    }
    let mut seen = BTreeSet::new();
    for offer in &raw.extensions {
        check(offer)?;
        if !seen.insert(offer.id.clone()) {
            return Err(format!("{} is offered twice", offer.id));
        }
    }
    use sha2::Digest;
    let sha256 = sha2::Sha256::digest(text.as_bytes()).iter().map(|b| format!("{b:02x}")).collect();
    Ok(Manifest { serial: raw.serial, sha256, offers: raw.extensions })
}

/// An offer checked as the manifest's own are: also each offer this device
/// kept in its record, which nothing signs (`state::load`).
pub fn check(offer: &Offer) -> Result<(), String> {
    let id = &offer.id;
    if !super::valid_id(id) {
        return Err(format!("an id that is not one: {id:?}"));
    }
    let fault = |what: &str| Err(format!("{id}: {what}"));
    if offer.version.is_empty() || offer.version.len() > 32 || !offer.version.bytes().all(|b| b.is_ascii_alphanumeric() || b".-+".contains(&b)) {
        return fault("a version that is not one");
    }
    for words in [&offer.name, &offer.about] {
        if words.en.trim().is_empty() || words.fr.trim().is_empty() || words.en.len() > 600 || words.fr.len() > 600 {
            return fault("its name and what it does, in English and French, are needed");
        }
    }
    if offer.hosts.len() > MOST_HOSTS || !offer.hosts.iter().all(|h| host_name(h)) {
        return fault("a host that is not a name");
    }
    if let Some(prefix) = &offer.sign_in
        && !watched(prefix)
    {
        return fault("a sign-in's return that cannot be watched for");
    }
    if offer.sha256.len() != 64 || !offer.sha256.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
        return fault("a SHA-256 that is not 64 lower-case hexadecimal digits");
    }
    if !file_named(&offer.file) {
        return fault("a file that is neither an https address nor a name beside the manifest");
    }
    Ok(())
}

/// A host's name, exactly: labels of letters, digits and `-`, a dot between
/// them, at least two; no port, no pattern, no address.
pub fn host_name(host: &str) -> bool {
    let labels: Vec<&str> = host.split('.').collect();
    host.len() <= 253
        && labels.len() >= 2
        && labels.iter().all(|l| !l.is_empty() && l.len() <= 63 && !l.starts_with('-') && !l.ends_with('-') && l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'))
        && labels.last().is_some_and(|tld| tld.bytes().any(|b| b.is_ascii_alphabetic()))
}

/// A sign-in's return that may be watched for: an address of a scheme that
/// no web page loads (an application's own), with something after `://`.
/// Never `http`, `https`, `file`, `data`, `javascript`, `about`, `blob`.
pub fn watched(prefix: &str) -> bool {
    let Some((scheme, rest)) = prefix.split_once("://") else { return false };
    let scheme_ok = !scheme.is_empty() && scheme.bytes().next().is_some_and(|b| b.is_ascii_lowercase()) && scheme.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"+-.".contains(&b));
    let forbidden = ["http", "https", "file", "data", "javascript", "about", "blob", "content", "intent", "ftp", "ws", "wss", "mailto"];
    scheme_ok && !forbidden.contains(&scheme) && !rest.is_empty() && prefix.len() <= 200 && !prefix.bytes().any(|b| b.is_ascii_whitespace() || b.is_ascii_control())
}

/// An `https` address, or a plain file name beside the manifest.
fn file_named(file: &str) -> bool {
    if let Some(rest) = file.strip_prefix("https://") {
        return rest.split(['/', '?', '#']).next().is_some_and(|host| host_name(host)) && !file.bytes().any(|b| b.is_ascii_whitespace());
    }
    !file.is_empty() && file.len() <= 128 && !file.starts_with('.') && file.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}

/// Where a file named by the manifest is: as it is when an `https` address;
/// else beside the manifest, whose own address (or folder) is `manifest`.
pub fn file_address(manifest: &str, file: &str) -> String {
    if file.starts_with("https://") {
        return file.to_string();
    }
    match manifest.rfind('/') {
        Some(slash) => format!("{}/{file}", &manifest[..slash]),
        None => file.to_string(),
    }
}

/// A manifest of one extension, for the tests of this module and its neighbours.
#[cfg(test)]
pub(crate) const SAMPLE: &str = r#"
format = 1
serial = 7

[[extension]]
id = "timetable"
version = "0.1.0"
interface = 1
name = { en = "A timetable", fr = "Un emploi du temps" }
about = { en = "Lessons.", fr = "Des cours." }
brings = ["agenda"]
hosts = ["api.example.org", "sso.example.org"]
sign_in = "example-app://sign-in-callback"
file = "timetable-0.1.0.wasm"
sha256 = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
"#;

#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn a_manifest_reads_and_each_offer_is_checked() {
        let manifest = parse(SAMPLE).unwrap();
        assert_eq!(manifest.serial, 7);
        let offer = &manifest.offers[0];
        assert!(offer.usable());
        assert_eq!(offer.name.said(true), "Un emploi du temps");
        assert_eq!(file_address("https://example.org/ext/manifest.toml", &offer.file), "https://example.org/ext/timetable-0.1.0.wasm");
        assert_eq!(file_address("/home/x/ext/manifest.toml", &offer.file), "/home/x/ext/timetable-0.1.0.wasm");
        for (from, to) in [
            ("format = 1", "format = 2"),
            (r#"id = "timetable""#, r#"id = "../x""#),
            (r#""sso.example.org""#, r#""*.example.org""#),
            (r#""sso.example.org""#, r#""192.168.1.1""#),
            (r#""sso.example.org""#, r#""sso.example.org:8443""#),
            ("example-app://sign-in-callback", "https://example.org/back"),
            ("example-app://sign-in-callback", "javascript://x"),
            (r#"file = "timetable-0.1.0.wasm""#, r#"file = "../timetable.wasm""#),
            (r#"file = "timetable-0.1.0.wasm""#, r#"file = "http://example.org/t.wasm""#),
            ("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef", "0123"),
            (r#"fr = "Un emploi du temps""#, r#"fr = """#),
        ] {
            assert!(parse(&SAMPLE.replace(from, to)).is_err(), "{to}");
        }
        let twice = format!("{SAMPLE}\n{}", &SAMPLE[SAMPLE.find("[[extension]]").unwrap()..]);
        assert!(parse(&twice).is_err(), "an id offered twice");
        // An interface this Sioul does not speak is listed, not usable.
        assert!(!parse(&SAMPLE.replace("interface = 1", "interface = 2")).unwrap().offers[0].usable());
    }

    #[test]
    fn an_update_asks_more_by_a_host_or_a_sign_in() {
        let offer = parse(SAMPLE).unwrap().offers.remove(0);
        let accepted = offer.grants();
        assert!(!accepted.asks_more_than(&accepted));
        let mut fewer = accepted.clone();
        fewer.hosts.remove("sso.example.org");
        fewer.sign_in = None;
        assert!(!fewer.asks_more_than(&accepted), "less is never more");
        let mut host = accepted.clone();
        host.hosts.insert("other.example.org".into());
        assert!(host.asks_more_than(&accepted));
        let mut sign_in = fewer.clone();
        sign_in.sign_in = Some("example-app://sign-in-callback".into());
        assert!(sign_in.asks_more_than(&fewer), "a sign-in where there was none");
        let mut other = accepted.clone();
        other.sign_in = Some("example-app://elsewhere".into());
        assert!(other.asks_more_than(&accepted), "another sign-in");
    }
}
