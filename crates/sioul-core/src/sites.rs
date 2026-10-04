// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Sites kept open in Sioul: secure mailboxes (a bank's, a hospital's, the
//! tax office's), chats, a dating site. Each is a portal account; this module
//! says what each is, which mail announces it, and keeps what they notified,
//! to be shown at your pace (`$XDG_STATE_HOME/sioul/site-notices.toml`): a
//! site's notification is always accepted, then waits, unless the site is in
//! real time.

use crate::areas::Area;
use crate::config::{Account, AccountKind, Config};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// What a site is, as it behaves: a secure mailbox or a client area (a
/// bank's, the tax office's), a chat, video calls, a social network, a dating
/// site, another site. Your own categories ("Banque", "Santé") come on top.
pub const TYPES: [&str; 6] = ["mailbox", "chat", "video", "social", "dating", "other"];

/// A type as written, as one of `TYPES`; an office, a bank, a supplier is a mailbox.
pub fn type_of(text: &str) -> &'static str {
    match text.trim().to_lowercase().as_str() {
        "" | "mailbox" | "admin" | "bank" | "energy" | "telecom" => "mailbox",
        other => TYPES.iter().find(|t| **t == other).copied().unwrap_or("other"),
    }
}

/// What a site of a type is for, unless you said: secure mailboxes are your
/// admin; chats, social networks and dating, leisure; calls and the rest, yours either way.
pub fn usual_area(kind: &str) -> Area {
    match type_of(kind) {
        "mailbox" => Area::ADMIN,
        "chat" | "social" | "dating" => Area::LEISURE,
        _ => Area::PERSONAL,
    }
}

/// One site, as the Sites page shows it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Site {
    pub id: String,
    pub name: String,
    pub url: String,
    /// One of `TYPES`.
    pub kind: String,
    /// Your own categories: any words, the list filters by them.
    pub categories: Vec<String>,
    pub realtime: bool,
    pub muted: bool,
    /// Kept open while the window is: chats by default.
    pub background: bool,
    /// For calls: the microphone, the camera, sharing the screen; given to
    /// chats, video calls and dating sites unless you said otherwise, to no other.
    pub microphone: bool,
    pub camera: bool,
    pub screen: bool,
    /// The domains whose mail announces it.
    pub announced_by: Vec<String>,
    /// What it is for: as you said, else as its category goes (`usual_area`).
    pub area: Area,
    /// Yours outside work too: admin or leisure.
    pub personal: bool,
}

/// The host of an address: "https://mail.proton.me/u/0" → "mail.proton.me".
pub fn host_of(url: &str) -> String {
    let rest = url.split("://").nth(1).unwrap_or(url);
    rest.split(['/', '?', '#']).next().unwrap_or("").split('@').next_back().unwrap_or("").split(':').next().unwrap_or("").to_lowercase()
}

/// A site's address as compared with another's: its host, path and query,
/// without the scheme, a final "/" or a fragment. Two services on one host
/// (canada.ca's accounts, a network and its messages) stay two.
pub fn place_of(url: &str) -> String {
    let rest = url.split("://").nth(1).unwrap_or(url).split('#').next().unwrap_or("");
    let (host, path) = rest.split_at(rest.find(['/', '?']).unwrap_or(rest.len()));
    format!("{}{}", host_of(&format!("https://{host}")), path.trim_end_matches('/'))
}

/// The domain under which a host lives, roughly: its last two labels, three
/// for the few countries that register under a second level ("co.uk").
pub fn domain_of(host: &str) -> String {
    let labels: Vec<&str> = host.split('.').filter(|l| !l.is_empty()).collect();
    let second_level = labels.len() >= 3 && matches!(labels[labels.len() - 2], "co" | "com" | "gov" | "org" | "ac" | "net" | "gouv");
    let keep = if second_level { 3 } else { 2 };
    labels[labels.len().saturating_sub(keep)..].join(".")
}

impl Site {
    pub fn of(account: &Account) -> Option<Site> {
        if account.kind != AccountKind::Portal {
            return None;
        }
        let url = account.url.clone().unwrap_or_default();
        let kind = type_of(account.site.as_deref().unwrap_or("")).to_string();
        let area = account.area.as_deref().and_then(Area::parse).unwrap_or_else(|| usual_area(&kind));
        let calls = matches!(kind.as_str(), "chat" | "video" | "dating");
        let announced_by = if account.announced_by.is_empty() { vec![domain_of(&host_of(&url))] } else { account.announced_by.iter().map(|d| d.trim().trim_start_matches('@').to_lowercase()).collect() };
        Some(Site {
            id: account.id.clone(),
            name: account.name.clone().unwrap_or_else(|| account.id.clone()),
            background: account.background.unwrap_or(kind == "chat"),
            microphone: account.microphone.unwrap_or(calls),
            camera: account.camera.unwrap_or(calls),
            screen: account.screen.unwrap_or(calls),
            realtime: account.realtime,
            muted: account.muted,
            personal: area.admin || area.leisure,
            area,
            categories: account.categories.iter().map(|c| c.trim().to_string()).filter(|c| !c.is_empty()).collect(),
            url,
            kind,
            announced_by,
        })
    }

    /// Whether mail from this address announces something waiting on the site.
    pub fn announced(&self, address: &str) -> bool {
        let address = address.trim().to_lowercase();
        let domain = address.rsplit_once('@').map_or(address.as_str(), |(_, d)| d);
        self.announced_by.iter().any(|d| !d.is_empty() && (domain == d || domain.ends_with(&format!(".{d}")) || address == *d))
    }
}

/// Whether a site's notification rings: someone calling now, in the words
/// chats use. A call waits for nobody; a missed one can.
pub fn is_call(title: &str, text: &str) -> bool {
    let said: String = format!("{title} {text}").chars().map(crate::text::fold_char).collect();
    // A missed call can wait, video or voice: "Missed video call", "Appel vidéo manqué".
    let missed = [
        "missed call", "missed voice call", "missed video call", "missed audio call", "appel manque", "appel vocal manque", "appel video manque",
        "appel audio manque", "llamada perdida", "videollamada perdida", "llamada de voz perdida", "verpasster anruf", "verpasster videoanruf",
        "chiamata persa", "videochiamata persa",
    ];
    let ringing = ["incoming call", "incoming voice call", "incoming video call", "is calling", "calling you", "voice call", "video call", "appel entrant", "vous appelle", "appel vocal", "appel video", "llamada entrante", "eingehender anruf", "chiamata in arrivo"];
    !missed.iter().any(|m| said.contains(m)) && ringing.iter().any(|r| said.contains(r))
}

/// Every site, in the order of the configuration: `[[site]]`, and those an
/// older file kept among the accounts when it was not read by `Config::load`.
pub fn sites(config: &Config) -> Vec<Site> {
    config.sites.iter().chain(config.accounts.iter()).filter_map(Site::of).collect()
}

/// The site a message comes from, when its sender announces one.
pub fn announcing<'a>(sites: &'a [Site], sender: &str) -> Option<&'a Site> {
    sites.iter().find(|s| s.announced(sender))
}

/// One notification of a site, kept until you have seen it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Notice {
    pub site: String,
    pub title: String,
    #[serde(default)]
    pub text: String,
    /// When it came, Unix seconds.
    pub at: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Notices {
    #[serde(default, rename = "notice")]
    pub notices: Vec<Notice>,
}

impl Notices {
    pub fn default_path() -> PathBuf {
        crate::config::state_dir().join("site-notices.toml")
    }

    pub fn load(path: &Path) -> Notices {
        std::fs::read_to_string(path).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let fail = |e: std::io::Error| format!("{}: {e}", path.display());
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(fail)?;
        }
        let temporary = path.with_extension("toml.new");
        std::fs::write(&temporary, toml::to_string(self).map_err(|e| e.to_string())?).map_err(fail)?;
        std::fs::rename(&temporary, path).map_err(fail)
    }

    /// Keeps a notification; the same one told twice (a chat repeats itself) is kept once.
    pub fn add(&mut self, notice: Notice) {
        if !self.notices.iter().any(|n| n.site == notice.site && n.title == notice.title && n.text == notice.text) {
            self.notices.push(notice);
        }
        // A long absence does not pile up: the newest two hundred.
        let excess = self.notices.len().saturating_sub(200);
        self.notices.drain(..excess);
    }

    /// A site visited: what it notified is seen.
    pub fn seen(&mut self, site: &str) {
        self.notices.retain(|n| n.site != site);
    }

    pub fn of(&self, site: &str) -> Vec<&Notice> {
        self.notices.iter().filter(|n| n.site == site).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calls_ring_through() {
        assert!(is_call("Appel vidéo entrant", "Camille"));
        assert!(is_call("Camille", "is calling you…"));
        assert!(!is_call("Appel manqué", "Camille"), "a missed call can wait");
        assert!(!is_call("Missed video call", "Camille") && !is_call("Appel vidéo manqué", "Camille"), "a missed video call too");
        assert!(!is_call("Camille", "Did you call the bank?"));
    }

    #[test]
    fn places_compared() {
        assert_eq!(place_of("https://WWW.Instagram.com/"), "www.instagram.com");
        assert_eq!(place_of("https://www.instagram.com/direct/inbox/#x"), "www.instagram.com/direct/inbox");
        assert_ne!(place_of("https://www.canada.ca/en/revenue-agency/a.html"), place_of("https://www.canada.ca/en/immigration/b.html"), "two services on one host");
        assert_ne!(place_of("https://power.example.com/Login?op=AL"), place_of("https://power.example.com/Login?op=GA"));
    }

    #[test]
    fn types_and_areas() {
        assert_eq!(type_of(""), "mailbox", "the default");
        assert_eq!(type_of(" Bank "), "mailbox", "a bank's space is a mailbox");
        assert_eq!(type_of("Social"), "social");
        assert_eq!(type_of("forum"), "other");
        assert_eq!(usual_area("chat"), Area::LEISURE);
        assert_eq!(usual_area("social"), Area::LEISURE);
        assert_eq!(usual_area("mailbox"), Area::ADMIN);
        assert_eq!(usual_area("video"), Area::PERSONAL);
        let config: Config = toml::from_str("[[account]]\nid = \"chat\"\nkind = \"portal\"\nurl = \"https://chat.example.org\"\nsite = \"chat\"\ncamera = false\n\n[[account]]\nid = \"bank\"\nkind = \"portal\"\nurl = \"https://bank.example.org\"\nsite = \"bank\"\n").unwrap();
        let all = sites(&config);
        assert_eq!((all[0].microphone, all[0].camera, all[0].screen), (true, false, true), "a chat has them, the camera said no");
        assert_eq!((all[1].microphone, all[1].camera, all[1].screen), (false, false, false), "a bank has none");
    }

    #[test]
    fn sites_and_what_announces_them() {
        let config: Config = toml::from_str(
            "[[account]]\nid = \"proton\"\nkind = \"portal\"\nurl = \"https://mail.proton.me/u/0\"\n\n[[account]]\nid = \"element\"\nkind = \"portal\"\nname = \"Element\"\nurl = \"https://app.element.io\"\nsite = \"chat\"\nrealtime = true\n\n[[account]]\nid = \"bank\"\nkind = \"portal\"\nurl = \"https://secure.bank.example.co.uk/login\"\nannounced_by = [\"@alerts.example.net\"]\n",
        )
        .unwrap();
        let all = sites(&config);
        assert_eq!(all.iter().map(|s| (s.id.as_str(), s.kind.as_str(), s.background)).collect::<Vec<_>>(), vec![("proton", "mailbox", false), ("element", "chat", true), ("bank", "mailbox", false)]);
        assert_eq!(all[0].announced_by, vec!["proton.me"]);
        assert_eq!(announcing(&all, "notify@proton.me").map(|s| s.id.as_str()), Some("proton"));
        assert_eq!(announcing(&all, "no-reply@mail.alerts.example.net").map(|s| s.id.as_str()), Some("bank"));
        assert!(announcing(&all, "jane@example.org").is_none());
        assert_eq!(domain_of("secure.bank.example.co.uk"), "example.co.uk");
        assert_eq!(host_of("https://user@app.element.io:443/#/room"), "app.element.io");
        let mut notices = Notices::default();
        notices.add(Notice { site: "element".into(), title: "Jane".into(), text: "Lunch?".into(), at: 1 });
        notices.add(Notice { site: "element".into(), title: "Jane".into(), text: "Lunch?".into(), at: 2 });
        notices.add(Notice { site: "proton".into(), title: "New message".into(), text: String::new(), at: 3 });
        assert_eq!(notices.of("element").len(), 1, "told twice, kept once");
        notices.seen("element");
        assert_eq!(notices.notices.len(), 1);
    }
}
