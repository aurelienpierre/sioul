// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What a server offers for one address (docs/client.md, "Accounts"): mail
//! (the provider's own settings, as `discover` finds them), calendars and
//! contacts (DAV, at the domain's well-known addresses or on the server
//! already known), and, on a Nextcloud (Murena's among them), its apps:
//! files, notes, Talk, Deck… what Sioul takes, and what it does not yet.
//! Asked without a password, except a Nextcloud's list of apps, which needs
//! the one its calendars use.

use std::time::Duration;

/// What was found.
#[derive(Debug, Clone, Default)]
pub struct Scouted {
    pub mail: Option<crate::discover::Found>,
    /// The DAV server that answers, `https://host`, when one does.
    pub dav: Option<String>,
    pub nextcloud: Option<Nextcloud>,
}

/// A Nextcloud's name and version, and the apps it says it has (when it was asked with a password).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Nextcloud {
    pub server: String,
    pub product: String,
    pub version: String,
    /// Its capabilities' names: "files", "notes", "spreed" (Talk), "deck"…
    pub apps: Vec<String>,
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(15))).http_status_as_error(false).max_redirects(0).build().into()
}

/// Whether `https://<host>/.well-known/caldav` answers as a DAV server would:
/// a redirection, or asking who you are.
fn dav_at(host: &str) -> bool {
    let url = format!("https://{host}/.well-known/caldav");
    agent().get(&url).header("User-Agent", "Sioul").call().is_ok_and(|r| matches!(r.status().as_u16(), 207 | 301 | 302 | 307 | 308 | 401))
}

/// A Nextcloud's `status.php`: its product's name and version.
fn nextcloud_at(host: &str) -> Option<Nextcloud> {
    let url = format!("https://{host}/status.php");
    let mut answer = agent().get(&url).header("User-Agent", "Sioul").call().ok()?;
    if !answer.status().is_success() {
        return None;
    }
    let text = answer.body_mut().with_config().limit(64 * 1024).read_to_string().ok()?;
    let status: serde_json::Value = serde_json::from_str(&text).ok()?;
    status.get("installed")?.as_bool().filter(|i| *i)?;
    Some(Nextcloud {
        server: format!("https://{host}"),
        product: status.get("productname").and_then(|v| v.as_str()).unwrap_or("Nextcloud").to_string(),
        version: status.get("versionstring").or_else(|| status.get("version")).and_then(|v| v.as_str()).unwrap_or("").to_string(),
        apps: Vec::new(),
    })
}

/// A Nextcloud's capabilities, asked with a login and password: the apps it has.
fn nextcloud_apps(server: &str, login: &str, password: &str) -> Vec<String> {
    use base64::Engine;
    let url = format!("{server}/ocs/v2.php/cloud/capabilities?format=json");
    let authorization = format!("Basic {}", base64::engine::general_purpose::STANDARD.encode(format!("{login}:{password}")));
    let Ok(mut answer) = agent().get(&url).header("User-Agent", "Sioul").header("OCS-APIRequest", "true").header("Authorization", &authorization).call() else { return Vec::new() };
    if !answer.status().is_success() {
        return Vec::new();
    }
    let Some(body) = answer.body_mut().with_config().limit(2 * 1024 * 1024).read_to_string().ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()) else { return Vec::new() };
    let mut apps: Vec<String> = body.pointer("/ocs/data/capabilities").and_then(|c| c.as_object()).map(|c| c.keys().cloned().collect()).unwrap_or_default();
    apps.sort();
    apps
}

/// What the server of `address` offers: its mail's settings, a DAV server
/// (`dav_host` when one is known already, else the domain's), and a
/// Nextcloud's apps when `login` gives the password of the calendars at
/// `dav_host`: it goes to that server only, never to another one found.
pub fn scout(address: &str, dav_host: Option<&str>, login: Option<(&str, &str)>) -> Scouted {
    let domain = address.rsplit_once('@').map_or(address, |(_, d)| d).trim().to_ascii_lowercase();
    let mail = crate::discover::discover(address).ok();
    let hosts: Vec<String> = dav_host.map(str::to_string).into_iter().chain([domain.clone(), format!("cloud.{domain}"), format!("dav.{domain}")]).collect();
    let dav = hosts.iter().find(|h| dav_at(h)).map(|h| format!("https://{h}"));
    let mut nextcloud = hosts.iter().find_map(|h| nextcloud_at(h));
    if let (Some(cloud), Some((login, password)), Some(host)) = (nextcloud.as_mut(), login, dav_host)
        && cloud.server == format!("https://{host}")
    {
        cloud.apps = nextcloud_apps(&cloud.server, login, password);
    }
    Scouted { mail, dav, nextcloud }
}

/// What a Nextcloud app is, as the window says it, and whether Sioul takes it:
/// ("files", false) is Files, which Sioul does not use yet.
pub fn known_app(name: &str) -> Option<(&'static str, bool)> {
    Some(match name {
        "dav" => ("calendars-contacts", true),
        "files" => ("files", false),
        "notes" => ("notes", false),
        "spreed" => ("talk", false),
        "deck" => ("deck", false),
        "mail" => ("webmail", false),
        "bookmarks" => ("bookmarks", false),
        "tables" => ("tables", false),
        "forms" => ("forms", false),
        "photos" => ("photos", false),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apps_named() {
        assert_eq!(known_app("files"), Some(("files", false)));
        assert_eq!(known_app("dav"), Some(("calendars-contacts", true)));
        assert_eq!(known_app("theming"), None, "what is no app to you is not said");
    }

    /// Murena's servers, asked without a password. Run by hand: it asks them.
    #[test]
    #[ignore]
    fn murena_scouted() {
        let found = scout("someone@murena.io", Some("murena.io"), None);
        assert_eq!(found.mail.as_ref().map(|m| m.host.as_str()), Some("mail.ecloud.global"));
        assert_eq!(found.dav.as_deref(), Some("https://murena.io"));
        assert_eq!(found.nextcloud.as_ref().map(|n| n.product.as_str()), Some("Nextcloud"));
        println!("{found:?}");
    }
}
