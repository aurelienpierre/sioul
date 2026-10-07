// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Finding an account's IMAP server from its address, the way Thunderbird does
//! (<https://wiki.mozilla.org/Thunderbird:Autoconfiguration>):
//!
//! 1. the provider's own settings, `autoconfig.<domain>` (cPanel hosts and
//!    Nextcloud providers publish them), then `<domain>/.well-known/autoconfig`;
//! 2. Thunderbird's list of providers (ISPDB), which knows Gmail and the big ones;
//! 3. a guess, `imap.<domain>` then `mail.<domain>` on port 993, for you to check.
//!
//! Only HTTPS: a settings file fetched in clear could send your password to
//! someone else's server. The ISPDB is told the domain, never the address.
//!
//! Google's mail is known before any of that: gmail.com and googlemail.com,
//! and any domain whose mail Google receives (Google Workspace: its MX is
//! smtp.google.com, or aspmx.l.google.com and its kin), asked of the system's
//! DNS. Google takes no account password there, only an app password or its
//! own sign-in, so the form must know it (docs/google.md, "Mail").

use crate::SyncError;
use sioul_core::config::Security;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

/// Google's mail servers, for Gmail and Google Workspace alike.
pub const GOOGLE_IMAP: &str = "imap.gmail.com";
pub const GOOGLE_SMTP: &str = "smtp.gmail.com";

/// IMAP settings found for an address, with the SMTP server when the settings name one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub host: String,
    pub port: u16,
    pub security: Security,
    pub username: String,
    pub by: FoundBy,
    pub smtp: Option<Smtp>,
}

/// An SMTP submission server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Smtp {
    pub host: String,
    pub port: u16,
    pub security: Security,
}

/// Where the settings came from, said to you before you type a password.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FoundBy {
    Provider,
    Ispdb,
    Guess,
    /// Google keeps this address's mail: Gmail, or a domain whose MX is Google's.
    Google,
}

impl FoundBy {
    /// The message naming it ("account-by-provider").
    pub fn message_id(self) -> &'static str {
        match self {
            FoundBy::Provider => "account-by-provider",
            FoundBy::Ispdb => "account-by-ispdb",
            FoundBy::Guess => "account-by-guess",
            FoundBy::Google => "account-by-google",
        }
    }
}

const HTTP: Duration = Duration::from_secs(10);
const PROBE: Duration = Duration::from_secs(5);

/// The IMAP settings for an address.
pub fn discover(address: &str) -> Result<Found, SyncError> {
    discover_with(address, mx_hosts)
}

/// `discover`, the domain's MX asked of `mx` (the system's DNS, or a stand-in in tests).
pub(crate) fn discover_with(address: &str, mx: impl Fn(&str) -> Option<Vec<String>>) -> Result<Found, SyncError> {
    let address = address.trim();
    let domain = domain_of(address).ok_or(SyncError::BadAddress)?;
    // Gmail: known here, nothing asked of anyone.
    if is_google_domain(&domain) {
        return Ok(google_found(address));
    }
    // HTTPS to the end: a redirect to a plain address is not followed.
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(HTTP)).https_only(true).build().into();
    let sources = [
        (format!("https://autoconfig.{domain}/mail/config-v1.1.xml?emailaddress={}", encode(address)), FoundBy::Provider),
        (format!("https://{domain}/.well-known/autoconfig/mail/config-v1.1.xml"), FoundBy::Provider),
        (format!("https://autoconfig.thunderbird.net/v1.1/{domain}"), FoundBy::Ispdb),
    ];
    for (url, by) in sources {
        let Some(xml) = fetch(&agent, &url) else { continue };
        if let Some(found) = parse_config(&xml, address, by) {
            return Ok(found);
        }
    }
    // Google Workspace: the domain's mail goes to Google, its servers are Google's.
    if mx(&domain).is_some_and(|hosts| hosts.iter().any(|h| is_google_mx(h))) {
        return Ok(google_found(address));
    }
    guess(&domain, address).ok_or(SyncError::NotFound(domain))
}

/// Google's servers for an address whose mail Google keeps: IMAP on 993 and
/// submission on 465, both encrypted from the first byte; the address as login.
pub fn google_found(address: &str) -> Found {
    Found {
        host: GOOGLE_IMAP.into(),
        port: 993,
        security: Security::Tls,
        username: address.trim().to_string(),
        by: FoundBy::Google,
        smtp: Some(Smtp { host: GOOGLE_SMTP.into(), port: 465, security: Security::Tls }),
    }
}

/// Gmail's own domains.
pub fn is_google_domain(domain: &str) -> bool {
    matches!(domain.trim().trim_end_matches('.').to_ascii_lowercase().as_str(), "gmail.com" | "googlemail.com")
}

/// Whether a mail exchanger is Google's: smtp.google.com (Workspace since
/// 2023), aspmx.l.google.com and altN.aspmx.l.google.com, aspmxN.googlemail.com.
/// Only Google names hosts under google.com and googlemail.com.
pub fn is_google_mx(host: &str) -> bool {
    let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    host.ends_with(".google.com") || host.ends_with(".googlemail.com")
}

/// Whether Google keeps an address's mail: its domain is Gmail's, or its MX
/// (asked of the system's DNS) is Google's.
pub fn is_google_mail(address: &str) -> bool {
    is_google_mail_with(address, mx_hosts)
}

/// `is_google_mail`, the MX asked of `mx`.
pub(crate) fn is_google_mail_with(address: &str, mx: impl Fn(&str) -> Option<Vec<String>>) -> bool {
    let Some(domain) = domain_of(address.trim()) else { return false };
    is_google_domain(&domain) || mx(&domain).is_some_and(|hosts| hosts.iter().any(|h| is_google_mx(h)))
}

/// The hosts that receive a domain's mail (its MX records), asked of the
/// system's DNS as the sender checks do (on Android, the network's servers,
/// which hickory reads through ConnectivityManager); None when it cannot be
/// asked or does not answer within a few seconds.
fn mx_hosts(domain: &str) -> Option<Vec<String>> {
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().ok()?;
    runtime.block_on(async {
        let resolver = mail_auth::MessageAuthenticator::new_system_conf().ok()?;
        let none = None::<&mail_auth::common::cache::NoCache<Box<str>, mail_auth::RecordSet<mail_auth::MX>>>;
        let found = tokio::time::timeout(PROBE, resolver.mx_lookup(domain, none)).await.ok()?.ok()?;
        Some(found.rrset.iter().flat_map(|mx| mx.exchanges.iter().map(|host| host.trim_end_matches('.').to_string())).collect())
    })
}

fn fetch(agent: &ureq::Agent, url: &str) -> Option<String> {
    agent.get(url).call().ok()?.body_mut().read_to_string().ok()
}

/// `imap.<domain>`, then `mail.<domain>`, if something answers on port 993.
fn guess(domain: &str, address: &str) -> Option<Found> {
    [format!("imap.{domain}"), format!("mail.{domain}")].into_iter().find(|host| answers(host, 993)).map(|host| Found {
        host,
        port: 993,
        security: Security::Tls,
        username: address.to_string(),
        by: FoundBy::Guess,
        smtp: None,
    })
}

fn answers(host: &str, port: u16) -> bool {
    let Ok(addresses) = (host, port).to_socket_addrs() else { return false };
    addresses.into_iter().any(|a| TcpStream::connect_timeout(&a, PROBE).is_ok())
}

fn domain_of(address: &str) -> Option<String> {
    let (local, domain) = address.rsplit_once('@')?;
    let valid = !local.is_empty() && domain.contains('.') && !domain.contains(['/', ' ', '?', '#']);
    valid.then(|| domain.to_ascii_lowercase())
}

/// Percent-encodes a query value (RFC 3986 §2.3 keeps only unreserved characters).
fn encode(value: &str) -> String {
    value
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Reads the IMAP servers of a `config-v1.1.xml` file; TLS from the first byte
/// is preferred to STARTTLS (RFC 8314 §3), and plain connections are ignored.
pub(crate) fn parse_config(xml: &str, address: &str, by: FoundBy) -> Option<Found> {
    let servers: Vec<Found> = blocks(xml, "incomingServer")
        .filter(|(attributes, _)| attributes.contains("\"imap\"") || attributes.contains("'imap'"))
        .filter_map(|(_, body)| server(body, address, by))
        .collect();
    let outgoing: Vec<Found> = blocks(xml, "outgoingServer")
        .filter(|(attributes, _)| attributes.contains("\"smtp\"") || attributes.contains("'smtp'"))
        .filter_map(|(_, body)| server(body, address, by))
        .collect();
    let smtp = outgoing.iter().find(|s| s.security == Security::Tls).or(outgoing.first()).map(|s| Smtp { host: s.host.clone(), port: s.port, security: s.security });
    let mut found = servers.iter().find(|s| s.security == Security::Tls).or(servers.first()).cloned()?;
    found.smtp = smtp;
    Some(found)
}

/// The SMTP server of an address: from its settings, else `smtp.<domain>`, then the IMAP host, on 465.
pub fn discover_smtp(address: &str, imap_host: Option<&str>) -> Result<Smtp, SyncError> {
    if let Ok(Found { smtp: Some(smtp), .. }) = discover(address) {
        return Ok(smtp);
    }
    let domain = domain_of(address).ok_or(SyncError::BadAddress)?;
    [format!("smtp.{domain}"), imap_host.unwrap_or(&format!("mail.{domain}")).to_string()]
        .into_iter()
        .find(|host| answers(host, 465))
        .map(|host| Smtp { host, port: 465, security: Security::Tls })
        .ok_or(SyncError::NotFound(domain))
}

fn server(body: &str, address: &str, by: FoundBy) -> Option<Found> {
    let security = match tag(body, "socketType")? {
        "SSL" => Security::Tls,
        "STARTTLS" => Security::Starttls,
        _ => return None,
    };
    Some(Found {
        host: substitute(tag(body, "hostname")?, address),
        port: tag(body, "port")?.parse().ok()?,
        security,
        username: substitute(tag(body, "username").unwrap_or("%EMAILADDRESS%"), address),
        by,
        smtp: None,
    })
}

/// Each `<name attributes>body</name>`, as (attributes, body).
fn blocks<'a>(xml: &'a str, name: &'a str) -> impl Iterator<Item = (&'a str, &'a str)> + 'a {
    let open = format!("<{name}");
    let close = format!("</{name}>");
    let mut rest = xml;
    std::iter::from_fn(move || {
        let start = rest.find(&open)? + open.len();
        let after = &rest[start..];
        let end_of_tag = after.find('>')?;
        let body_end = after.find(&close)?;
        let found = (&after[..end_of_tag], after.get(end_of_tag + 1..body_end)?);
        rest = &after[body_end + close.len()..];
        Some(found)
    })
}

/// The text of the first `<name>text</name>`, trimmed.
fn tag<'a>(body: &'a str, name: &str) -> Option<&'a str> {
    let start = body.find(&format!("<{name}>"))? + name.len() + 2;
    let end = start + body[start..].find(&format!("</{name}>"))?;
    Some(body[start..end].trim())
}

/// Thunderbird's placeholders: %EMAILADDRESS%, %EMAILLOCALPART%, %EMAILDOMAIN%.
fn substitute(template: &str, address: &str) -> String {
    let (local, domain) = address.rsplit_once('@').unwrap_or((address, ""));
    template.replace("%EMAILADDRESS%", address).replace("%EMAILLOCALPART%", local).replace("%EMAILDOMAIN%", domain)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shape a cPanel host publishes at autoconfig.<domain>.
    const CPANEL: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<clientConfig version="1.1">
  <emailProvider id="example.com">
    <domain>example.com</domain>
    <incomingServer type="imap">
      <hostname>mail.example.com</hostname>
      <port>143</port>
      <socketType>STARTTLS</socketType>
      <username>%EMAILADDRESS%</username>
      <authentication>password-cleartext</authentication>
    </incomingServer>
    <incomingServer type="imap">
      <hostname>mail.example.com</hostname>
      <port>993</port>
      <socketType>SSL</socketType>
      <username>%EMAILADDRESS%</username>
      <authentication>password-cleartext</authentication>
    </incomingServer>
    <outgoingServer type="smtp">
      <hostname>mail.example.com</hostname>
      <port>465</port>
      <socketType>SSL</socketType>
      <username>%EMAILADDRESS%</username>
    </outgoingServer>
    <incomingServer type="pop3">
      <hostname>mail.example.com</hostname>
      <port>995</port>
      <socketType>SSL</socketType>
    </incomingServer>
  </emailProvider>
</clientConfig>"#;

    /// The shape of the ISPDB's entries, with a plain server to ignore.
    const ISPDB: &str = r#"<clientConfig version="1.1"><emailProvider id="example.net">
      <incomingServer type='imap'><hostname>imap.%EMAILDOMAIN%</hostname><port>143</port>
      <socketType>plain</socketType><username>%EMAILLOCALPART%</username></incomingServer>
      <incomingServer type='imap'><hostname>imap.%EMAILDOMAIN%</hostname><port>143</port>
      <socketType>STARTTLS</socketType><username>%EMAILLOCALPART%</username></incomingServer>
    </emailProvider></clientConfig>"#;

    #[test]
    fn prefers_tls_from_the_first_byte() {
        let found = parse_config(CPANEL, "someone@example.com", FoundBy::Provider).unwrap();
        assert_eq!((found.host.as_str(), found.port, found.security), ("mail.example.com", 993, Security::Tls));
        assert_eq!(found.username, "someone@example.com");
        assert_eq!(found.smtp, Some(Smtp { host: "mail.example.com".into(), port: 465, security: Security::Tls }));
    }

    #[test]
    fn fills_placeholders_and_skips_plain() {
        let found = parse_config(ISPDB, "jane@example.net", FoundBy::Ispdb).unwrap();
        assert_eq!((found.host.as_str(), found.port, found.security), ("imap.example.net", 143, Security::Starttls));
        assert_eq!(found.username, "jane");
        assert_eq!(parse_config("<clientConfig/>", "a@example.org", FoundBy::Ispdb), None);
    }

    /// Google's mail, by its domains, else by the domain's MX (a stand-in
    /// for the DNS here); nothing asked for Gmail's own domains.
    #[test]
    fn knows_google_mail() {
        let never = |domain: &str| -> Option<Vec<String>> { panic!("{domain}: Gmail's domains need no DNS") };
        for address in ["someone@gmail.com", " Someone@GMail.COM ", "someone@googlemail.com"] {
            assert!(is_google_mail_with(address, never), "{address}");
            assert_eq!(discover_with(address, never).unwrap(), google_found(address));
        }
        let mx = |domain: &str| -> Option<Vec<String>> {
            match domain {
                // Google Workspace since 2023, then before (with a trailing dot, as DNS gives it).
                "new-workspace.example" => Some(vec!["smtp.google.com".into()]),
                "old-workspace.example" => Some(vec!["aspmx.l.google.com.".into(), "alt1.aspmx.l.google.com.".into(), "aspmx2.googlemail.com.".into()]),
                "elsewhere.example" => Some(vec!["mx.elsewhere.example".into()]),
                // Names that only look like Google's.
                "lookalike.example" => Some(vec!["google.com.lookalike.example".into(), "notgoogle.com".into(), "aspmx.l.google.com.evil.example".into()]),
                _ => None,
            }
        };
        assert!(is_google_mail_with("you@new-workspace.example", mx));
        assert!(is_google_mail_with("you@old-workspace.example", mx));
        assert!(!is_google_mail_with("you@elsewhere.example", mx));
        assert!(!is_google_mail_with("you@lookalike.example", mx));
        assert!(!is_google_mail_with("you@unanswered.example", mx), "no answer: not Google");
        assert!(!is_google_mail_with("not an address", mx));
        // Google's servers, encrypted from the first byte, the address as login.
        let found = google_found("you@new-workspace.example");
        assert_eq!((found.host.as_str(), found.port, found.security, found.username.as_str(), found.by), ("imap.gmail.com", 993, Security::Tls, "you@new-workspace.example", FoundBy::Google));
        assert_eq!(found.smtp, Some(Smtp { host: "smtp.gmail.com".into(), port: 465, security: Security::Tls }));
    }

    #[test]
    fn reads_addresses_carefully() {
        assert_eq!(domain_of("Someone@Example.org").as_deref(), Some("example.org"));
        assert_eq!(domain_of("nobody"), None);
        assert_eq!(domain_of("a@example.org/evil"), None);
        assert_eq!(encode("a+b@example.org"), "a%2Bb%40example.org");
    }
}
