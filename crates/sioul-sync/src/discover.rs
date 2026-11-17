// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Finding an account's IMAP server from its address, the way Thunderbird does
//! (<https://wiki.mozilla.org/Thunderbird:Autoconfiguration>), in this order:
//!
//! 1. the provider's own settings, `autoconfig.<domain>` (cPanel hosts and
//!    Nextcloud providers publish them), then `<domain>/.well-known/autoconfig`;
//! 2. Thunderbird's list of providers (ISPDB), which knows Gmail and the big
//!    ones, asked by the domain;
//! 3. Google's and Microsoft's servers, when the domain's MX is theirs;
//! 4. Thunderbird's list again, asked by the names of whoever receives the
//!    domain's mail (`provider_names`): a domain of your own kept at a host
//!    is found by its host's entry;
//! 5. the domain's own SRV records (RFC 6186, RFC 8314), said as such, to
//!    check: DNS is not signed;
//! 6. a guess, `imap.<domain>` then `mail.<domain>` on port 993, for you to
//!    check, offered only when the host shows a certificate valid for that
//!    very name (`certified`): an alias of a host's server, whose
//!    certificate names the host's server, is no guess.
//!
//! Only HTTPS: a settings file fetched in clear could send your password to
//! someone else's server. The ISPDB is told the domain, then the name of
//! whoever receives its mail, never the address; a name under the address's
//! own domain is never told to it a second time.
//!
//! The sending server is found by the same steps (`discover_smtp`). What
//! unsigned DNS names (an MX, an SRV target) is followed without anyone
//! reading it only when it agrees with the receiving server the account
//! already has (`same_keeper`).
//!
//! Google's mail is known before any of that: gmail.com and googlemail.com,
//! and any domain whose mail Google receives (Google Workspace: its MX is
//! smtp.google.com, or aspmx.l.google.com and its kin), asked of the system's
//! DNS. Google takes no account password there, only an app password or its
//! own sign-in, so the form must know it (docs/google.md, "Mail").
//!
//! So is Microsoft's, which takes no password at all, only its own sign-in
//! (docs/microsoft.md): Outlook.com's own domains (hotmail.com, live.fr…,
//! the list Thunderbird's ISPDB gives them), a domain whose MX is
//! Outlook.com's (`*.olc.protection.outlook.com`: a personal account) or
//! Microsoft 365's (`*.mail.protection.outlook.com` and its older and newer
//! forms: a work or school account), and settings that name Microsoft's
//! IMAP server. The ID token settles the kind at sign-in.
//!
//! Yahoo's mail is found through Thunderbird's list, whose entry `yahoo.com`
//! covers yahoo.fr and Yahoo's other domains, ymail.com, myyahoo.com and
//! rocketmail.com. Its servers are known here by their names
//! (`is_yahoo_host`), because they take an app password only, made in the
//! Yahoo account, from a program that does not use Yahoo's own sign-in page:
//! the form says so before a password is typed, and a refusal says where to
//! make one (docs/client.md, "Signing in to the servers").

use crate::SyncError;
pub use crate::microsoft::Kind as MicrosoftKind;
use rustls::pki_types::ServerName;
use sioul_core::config::Security;
use sioul_core::trust;
use std::net::ToSocketAddrs;
use std::sync::Arc;
use std::time::Duration;

/// Google's mail servers, for Gmail and Google Workspace alike.
pub const GOOGLE_IMAP: &str = "imap.gmail.com";
pub const GOOGLE_SMTP: &str = "smtp.gmail.com";

/// Yahoo's mail servers, as Thunderbird's list of providers names them: the
/// receiving one, its twin for exports, and the sending one. Each takes an
/// app password only (Yahoo's help, "Generate and manage 3rd-party app
/// passwords", <https://help.yahoo.com/kb/SLN15241.html>, and "IMAP server
/// settings for Yahoo Mail", <https://help.yahoo.com/kb/SLN4075.html>, read
/// 17 November 2026).
pub const YAHOO_IMAP: &str = "imap.mail.yahoo.com";
pub const YAHOO_IMAP_EXPORT: &str = "export.imap.mail.yahoo.com";
pub const YAHOO_SMTP: &str = "smtp.mail.yahoo.com";

/// Whether a server, receiving or sending, is Yahoo's: by its whole name,
/// whatever its case.
pub fn is_yahoo_host(host: &str) -> bool {
    [YAHOO_IMAP, YAHOO_IMAP_EXPORT, YAHOO_SMTP].iter().any(|known| host.eq_ignore_ascii_case(known))
}

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
    /// Thunderbird's list, asked by the name of whoever receives the
    /// domain's mail (its MX): a domain of your own, kept at a host.
    IspdbMx,
    /// The domain's own SRV records: unsigned, so to check before going on.
    Records,
    Guess,
    /// Google keeps this address's mail: Gmail, or a domain whose MX is Google's.
    Google,
    /// Microsoft keeps this address's mail: Outlook.com's domains, a domain
    /// whose MX is Microsoft's, or settings naming its server; and of which kind.
    Microsoft(MicrosoftKind),
}

impl FoundBy {
    /// The message naming it ("account-by-provider").
    pub fn message_id(self) -> &'static str {
        match self {
            FoundBy::Provider => "account-by-provider",
            FoundBy::Ispdb => "account-by-ispdb",
            FoundBy::IspdbMx => "account-by-ispdb-mx",
            FoundBy::Records => "account-by-records",
            FoundBy::Guess => "account-by-guess",
            FoundBy::Google => "account-by-google",
            FoundBy::Microsoft(_) => "account-by-microsoft",
        }
    }
}

const HTTP: Duration = Duration::from_secs(10);
const PROBE: Duration = Duration::from_secs(5);
/// Thunderbird's list of providers, asked by a name.
const ISPDB: &str = "https://autoconfig.thunderbird.net/v1.1/";
/// The most names of a domain's mail exchangers the list is asked about.
const PROVIDER_NAMES: usize = 4;

/// One SRV record (RFC 2782): where a domain says a service is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Service {
    pub priority: u16,
    pub weight: u16,
    pub port: u16,
    pub target: String,
}

/// What discovery asks of the outside, each a seam for the tests, so that
/// none of them asks the network.
pub(crate) struct Ask<'a> {
    /// A settings file, by its HTTPS address; None when there is none.
    pub fetch: &'a dyn Fn(&str) -> Option<String>,
    /// The hosts that receive a domain's mail (its MX records), the preferred first.
    pub mx: &'a dyn Fn(&str) -> Option<Vec<String>>,
    /// The SRV records of a name ("_imaps._tcp.example.org.").
    pub srv: &'a dyn Fn(&str) -> Vec<Service>,
    /// Whether a host answers on a port with TLS from the first byte and a
    /// certificate valid for that very name.
    pub certified: &'a dyn Fn(&str, u16) -> bool,
}

/// `run` with the real outside: HTTPS to the end (a redirect to a plain
/// address is not followed), the system's DNS, a TLS handshake.
fn with_the_network<T>(run: impl FnOnce(&Ask) -> T) -> T {
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(HTTP)).https_only(true).build().into();
    run(&Ask { fetch: &|url: &str| fetch(&agent, url), mx: &mx_hosts, srv: &srv_records, certified: &certified })
}

/// The IMAP settings for an address.
pub fn discover(address: &str) -> Result<Found, SyncError> {
    with_the_network(|ask| discover_with(address, ask))
}

/// `discover`, the outside asked through `ask` (the network, or stand-ins in tests).
pub(crate) fn discover_with(address: &str, ask: &Ask) -> Result<Found, SyncError> {
    let address = address.trim();
    let domain = domain_of(address).ok_or(SyncError::BadAddress)?;
    // Gmail: known here, nothing asked of anyone.
    if is_google_domain(&domain) {
        return Ok(google_found(address));
    }
    // Outlook.com's own domains: known here too.
    if is_microsoft_consumer_domain(&domain) {
        return Ok(microsoft_found(address, MicrosoftKind::Personal));
    }
    // The domain's own settings, then Thunderbird's list by the domain.
    let sources = [
        (format!("https://autoconfig.{domain}/mail/config-v1.1.xml?emailaddress={}", encode(address)), FoundBy::Provider),
        (format!("https://{domain}/.well-known/autoconfig/mail/config-v1.1.xml"), FoundBy::Provider),
        (format!("{ISPDB}{domain}"), FoundBy::Ispdb),
    ];
    for (url, by) in sources {
        if let Some(found) = settings(ask, &url, address, by) {
            return Ok(found);
        }
    }
    // Google Workspace, Outlook.com, Microsoft 365: the domain's mail goes
    // there (one question to the DNS for both), its servers are theirs.
    let exchangers = (ask.mx)(&domain).unwrap_or_default();
    if exchangers.iter().any(|h| is_google_mx(h)) {
        return Ok(google_found(address));
    }
    if let Some(kind) = exchangers.iter().find_map(|h| microsoft_mx_kind(h)) {
        return Ok(microsoft_found(address, kind));
    }
    // A domain kept at a host: Thunderbird's list, by the host's names.
    for name in provider_names(&exchangers, &domain) {
        if let Some(found) = settings(ask, &format!("{ISPDB}{name}"), address, FoundBy::IspdbMx) {
            return Ok(found);
        }
    }
    // What the domain's own records say, then a guess: both checked.
    from_records(&domain, address, ask).or_else(|| guess(&domain, address, ask)).ok_or(SyncError::NotFound(domain))
}

/// The settings a source gives for an address, if it has any.
fn settings(ask: &Ask, url: &str, address: &str, by: FoundBy) -> Option<Found> {
    (ask.fetch)(url).and_then(|xml| parse_config(&xml, address, by)).map(|found| microsoft_if_named(found, address))
}

/// The names Thunderbird's list may know a domain's host by, as Thunderbird
/// asks them: each name above the hosts that receive the domain's mail, the
/// longest first ("mx1.mail.ovh.net": "mail.ovh.net", then "ovh.net"), down
/// to the name registered under its public suffix and never further
/// (`trust::organisational`: the Public Suffix List). A name under the
/// address's own domain is left out: the list was asked about the domain
/// already, and is told nothing else about it. A few names at most.
fn provider_names(exchangers: &[String], domain: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for host in exchangers {
        let host = lower(host);
        let Some(registered) = trust::organisational(&host).filter(|_| is_host_name(&host)) else { continue };
        // The host itself only when it is the registered name: it has no name above it to ask.
        let mut name = if host == registered { Some(host.as_str()) } else { host.split_once('.').map(|(_, above)| above) };
        while let Some(candidate) = name.filter(|n| under(n, &registered)) {
            if !under(candidate, domain) && !names.iter().any(|n| n == candidate) {
                names.push(candidate.to_string());
            }
            name = candidate.split_once('.').map(|(_, above)| above);
        }
    }
    names.truncate(PROVIDER_NAMES);
    names
}

/// A name as DNS compares it: no space around, no final dot, lower case.
fn lower(name: &str) -> String {
    name.trim().trim_end_matches('.').to_ascii_lowercase()
}

/// Whether `name` is `parent` or a name under it, by whole labels.
fn under(name: &str, parent: &str) -> bool {
    name == parent || name.strip_suffix(parent).is_some_and(|rest| rest.ends_with('.'))
}

/// Whether a name read in a DNS answer can be a host's: letters, digits,
/// hyphens and dots, with a dot inside. Anything else is never asked about
/// nor offered.
fn is_host_name(name: &str) -> bool {
    name.contains('.') && !name.starts_with(['.', '-']) && !name.ends_with(['.', '-']) && !name.contains("..") && name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'.')
}

/// Whether `host` is at the keeper of the receiving server `kept`, or under
/// the address's own `domain`: the same name, or the same name registered
/// under a public suffix. What unsigned DNS names as a sending server is
/// followed without anyone reading it only then: a forged answer could
/// otherwise send a password to a server of its choosing, which shows a
/// valid certificate for its own name.
fn same_keeper(host: &str, kept: Option<&str>, domain: &str) -> bool {
    let host = lower(host);
    if under(&host, domain) {
        return true;
    }
    let Some(kept) = kept.map(lower) else { return false };
    host == kept || trust::organisational(&host).is_some_and(|registered| trust::organisational(&kept).as_deref() == Some(registered.as_str()))
}

/// The receiving server the domain's own records name (RFC 6186:
/// `_imaps._tcp`, TLS from the first byte), when it shows a certificate
/// valid for its name; with the sending server the records name, when that
/// one is at the same keeper. The login is the address.
fn from_records(domain: &str, address: &str, ask: &Ask) -> Option<Found> {
    let imap = preferred((ask.srv)(&format!("_imaps._tcp.{domain}.")))?;
    if !(ask.certified)(&imap.target, imap.port) {
        return None;
    }
    let smtp = sending_from_records(domain, ask).filter(|smtp| same_keeper(&smtp.host, Some(&imap.target), domain));
    Some(Found { host: imap.target, port: imap.port, security: Security::Tls, username: address.to_string(), by: FoundBy::Records, smtp })
}

/// The sending server the domain's own records name: `_submissions._tcp`
/// (RFC 8314), TLS from the first byte; else `_submission._tcp` (RFC 6186),
/// with STARTTLS, unless it names port 465, where TLS starts at the first
/// byte (some hosts write it so). A server on TLS from the first byte must
/// show a certificate valid for its name; one on STARTTLS shows its own at
/// the sign-in.
fn sending_from_records(domain: &str, ask: &Ask) -> Option<Smtp> {
    let certified = |service: &Service| (ask.certified)(&service.target, service.port);
    if let Some(service) = preferred((ask.srv)(&format!("_submissions._tcp.{domain}."))).filter(certified) {
        return Some(Smtp { host: service.target, port: service.port, security: Security::Tls });
    }
    let service = preferred((ask.srv)(&format!("_submission._tcp.{domain}.")))?;
    match service.port {
        465 => certified(&service).then(|| Smtp { host: service.target, port: 465, security: Security::Tls }),
        port => Some(Smtp { host: service.target, port, security: Security::Starttls }),
    }
}

/// The record to use among a service's: the lowest priority, then the
/// highest weight (RFC 2782), among those that name a host and a port (a
/// target "." says the service is not offered).
fn preferred(records: Vec<Service>) -> Option<Service> {
    records
        .into_iter()
        .map(|record| Service { target: lower(&record.target), ..record })
        .filter(|record| record.port != 0 && is_host_name(&record.target))
        .min_by_key(|record| (record.priority, std::cmp::Reverse(record.weight)))
}

/// Settings that name Microsoft's IMAP server are Microsoft's mail: its own
/// sign-in, its servers; a personal account's when they send through
/// Outlook.com's server, else a work or school one's.
fn microsoft_if_named(found: Found, address: &str) -> Found {
    if !crate::microsoft::is_imap_host(&found.host) {
        return found;
    }
    let personal = found.smtp.as_ref().is_some_and(|s| s.host.eq_ignore_ascii_case(crate::microsoft::SMTP_PERSONAL));
    microsoft_found(address, if personal { MicrosoftKind::Personal } else { MicrosoftKind::Work })
}

/// Microsoft's servers for an address whose mail Microsoft keeps: IMAP on
/// 993 encrypted from the first byte, submission on 587 with STARTTLS (no
/// 465 at Microsoft), the sending server of its kind; the address as login.
pub fn microsoft_found(address: &str, kind: MicrosoftKind) -> Found {
    Found {
        host: crate::microsoft::IMAP_HOST.into(),
        port: 993,
        security: Security::Tls,
        username: address.trim().to_string(),
        by: FoundBy::Microsoft(kind),
        smtp: Some(Smtp { host: kind.smtp_host().into(), port: crate::microsoft::SMTP_PORT, security: Security::Starttls }),
    }
}

/// Outlook.com's own domains, those of personal Microsoft accounts: the list
/// Thunderbird's ISPDB gives (`autoconfig.thunderbird.net/v1.1/outlook.com`,
/// read 13 October 2026), but for its exchangers' domain,
/// olc.protection.outlook.com, which `microsoft_mx_kind` knows. Exact names only.
const MICROSOFT_CONSUMER_DOMAINS: &[&str] = &[
    "hotmail.com", "live.com", "msn.com", "outlook.com", "windowslive.com", "livemail.tw",
    "outlook.at", "outlook.be", "outlook.cl", "outlook.cz", "outlook.de", "outlook.dk", "outlook.es", "outlook.fr", "outlook.hu", "outlook.ie", "outlook.in", "outlook.it", "outlook.jp", "outlook.kr", "outlook.lv", "outlook.my", "outlook.ph", "outlook.pt", "outlook.sa", "outlook.sg", "outlook.sk",
    "outlook.co.id", "outlook.co.il", "outlook.co.th", "outlook.com.ar", "outlook.com.au", "outlook.com.br", "outlook.com.gr", "outlook.com.tr", "outlook.com.vn",
    "hotmail.be", "hotmail.ca", "hotmail.cl", "hotmail.cz", "hotmail.de", "hotmail.dk", "hotmail.es", "hotmail.fi", "hotmail.fr", "hotmail.gr", "hotmail.hu", "hotmail.it", "hotmail.lt", "hotmail.lv", "hotmail.my", "hotmail.nl", "hotmail.no", "hotmail.ph", "hotmail.rs", "hotmail.se", "hotmail.sg", "hotmail.sk",
    "hotmail.co.id", "hotmail.co.il", "hotmail.co.in", "hotmail.co.jp", "hotmail.co.kr", "hotmail.co.th", "hotmail.co.uk", "hotmail.co.za", "hotmail.com.ar", "hotmail.com.au", "hotmail.com.br", "hotmail.com.hk", "hotmail.com.tr", "hotmail.com.tw", "hotmail.com.vn",
    "live.at", "live.be", "live.ca", "live.cl", "live.cn", "live.de", "live.dk", "live.fi", "live.fr", "live.hk", "live.ie", "live.in", "live.it", "live.jp", "live.nl", "live.no", "live.ru", "live.se",
    "live.co.jp", "live.co.kr", "live.co.uk", "live.co.za", "live.com.ar", "live.com.au", "live.com.mx", "live.com.my", "live.com.ph", "live.com.pt", "live.com.sg",
];

/// Whether a domain is one of Outlook.com's own (personal Microsoft accounts).
pub fn is_microsoft_consumer_domain(domain: &str) -> bool {
    let domain = domain.trim().trim_end_matches('.').to_ascii_lowercase();
    MICROSOFT_CONSUMER_DOMAINS.contains(&domain.as_str())
}

/// Whether a mail exchanger is Microsoft's, and of which kind: Outlook.com's
/// (`*.olc.protection.outlook.com`: personal accounts, and the addresses of a
/// domain of your own Outlook.com receives), Microsoft 365's
/// (`*.mail.protection.outlook.com`, older `*.mail.eo.outlook.com` and
/// `*.mail.protection.outlook.de`, and `*.mx.microsoft` where the domain
/// signs its mail exchange with DNSSEC). A whole label before each, so that
/// names only made to look like these are not taken.
pub fn microsoft_mx_kind(host: &str) -> Option<MicrosoftKind> {
    let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    let under = |suffix: &str| host.strip_suffix(suffix).is_some_and(|label| !label.is_empty() && !label.ends_with('.'));
    if under(".olc.protection.outlook.com") {
        Some(MicrosoftKind::Personal)
    } else if [".mail.protection.outlook.com", ".mail.eo.outlook.com", ".mail.protection.outlook.de", ".mx.microsoft"].iter().any(|s| under(s)) {
        Some(MicrosoftKind::Work)
    } else {
        None
    }
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

/// The SRV records of a name, asked of the system's DNS as `mx_hosts` asks
/// it (the same resolver, on every system Sioul builds for); none when it
/// cannot be asked, does not answer within a few seconds, or has none.
fn srv_records(name: &str) -> Vec<Service> {
    use mail_auth::hickory_resolver::proto::rr::{Name, RData};
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread().enable_all().build() else { return Vec::new() };
    runtime.block_on(async {
        let Ok(resolver) = mail_auth::MessageAuthenticator::new_system_conf() else { return Vec::new() };
        let Ok(name) = Name::from_str_relaxed(name) else { return Vec::new() };
        let Ok(Ok(found)) = tokio::time::timeout(PROBE, resolver.resolver().srv_lookup(name)).await else { return Vec::new() };
        found
            .answers()
            .iter()
            .filter_map(|record| match &record.data {
                RData::SRV(srv) => Some(Service { priority: srv.priority, weight: srv.weight, port: srv.port, target: srv.target.to_lowercase().to_ascii() }),
                _ => None,
            })
            .collect()
    })
}

fn fetch(agent: &ureq::Agent, url: &str) -> Option<String> {
    agent.get(url).call().ok()?.body_mut().read_to_string().ok()
}

/// `imap.<domain>`, then `mail.<domain>`, on port 993: a guess, offered only
/// when the host shows a certificate valid for that very name.
fn guess(domain: &str, address: &str, ask: &Ask) -> Option<Found> {
    [format!("imap.{domain}"), format!("mail.{domain}")].into_iter().find(|host| (ask.certified)(host, 993)).map(|host| Found {
        host,
        port: 993,
        security: Security::Tls,
        username: address.to_string(),
        by: FoundBy::Guess,
        smtp: None,
    })
}

/// Whether `host` answers on `port` with TLS from the first byte and shows a
/// certificate valid for that very name, checked as a sign-in checks it
/// (`imap::tls_config`: the system's certificates). Nothing is sent after
/// the handshake. An alias of a host's server (a CNAME whose certificate
/// names the host's server, not the alias) fails here, as its sign-in would.
fn certified(host: &str, port: u16) -> bool {
    shows_its_own_certificate(host, (host, port), crate::imap::tls_config())
}

/// `certified`, the name checked apart from where it is reached (`at`), with
/// the certificates `config` trusts: the tests' seam. Each wait is bounded
/// (`PROBE`); the first address that answers decides.
fn shows_its_own_certificate(name: &str, at: impl ToSocketAddrs, config: Arc<rustls::ClientConfig>) -> bool {
    let Ok(name) = ServerName::try_from(name.to_string()) else { return false };
    let Ok(addresses) = at.to_socket_addrs() else { return false };
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread().enable_all().build() else { return false };
    runtime.block_on(async {
        for address in addresses {
            let Ok(Ok(tcp)) = tokio::time::timeout(PROBE, tokio::net::TcpStream::connect(address)).await else { continue };
            let handshake = tokio_rustls::TlsConnector::from(config.clone()).connect(name.clone(), tcp);
            return matches!(tokio::time::timeout(PROBE, handshake).await, Ok(Ok(_)));
        }
        false
    })
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

/// The SMTP server of an address, by the steps that find its receiving one
/// (`discover`): its settings, Thunderbird's list by the domain, then by
/// whoever receives the domain's mail, the domain's own records; else a
/// guess, `smtp.<domain>`, then the IMAP host, on 465, each offered only when
/// it shows a certificate valid for its very name. What unsigned DNS led to
/// (the list by the MX, the records) is taken only when it agrees with the
/// receiving server the account has (`same_keeper`): nobody reads this
/// answer before a password follows it.
/// An account at Microsoft is never sent here: its sending server is set when it is added.
pub fn discover_smtp(address: &str, imap_host: Option<&str>) -> Result<Smtp, SyncError> {
    with_the_network(|ask| discover_smtp_with(address, imap_host, ask))
}

/// `discover_smtp`, the outside asked through `ask`.
pub(crate) fn discover_smtp_with(address: &str, imap_host: Option<&str>, ask: &Ask) -> Result<Smtp, SyncError> {
    let address = address.trim();
    let domain = domain_of(address).ok_or(SyncError::BadAddress)?;
    if let Ok(found) = discover_with(address, ask) {
        let agreed = !matches!(found.by, FoundBy::IspdbMx | FoundBy::Records) || same_keeper(&found.host, imap_host, &domain);
        if let (true, Some(smtp)) = (agreed, found.smtp) {
            return Ok(smtp);
        }
    }
    if let Some(smtp) = sending_from_records(&domain, ask).filter(|smtp| same_keeper(&smtp.host, imap_host, &domain)) {
        return Ok(smtp);
    }
    [format!("smtp.{domain}"), imap_host.map_or_else(|| format!("mail.{domain}"), lower)]
        .into_iter()
        .find(|host| (ask.certified)(host, 465))
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
            let outside = Outside::default();
            assert_eq!(outside.discover(address).unwrap(), google_found(address));
            assert!(outside.asked().is_empty(), "Gmail's domains: nothing asked of anyone");
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

    /// Part 8, 7: Microsoft's mail, by Outlook.com's own domains (no DNS
    /// asked), else by the domain's MX; names only made to look like
    /// Microsoft's are not taken; settings naming its server are Microsoft's.
    #[test]
    fn knows_microsoft_mail() {
        for address in ["someone@outlook.com", " Someone@Hotmail.FR ", "someone@live.co.uk", "someone@msn.com", "someone@hotmail.com.br", "someone@outlook.jp"] {
            let outside = Outside::default();
            assert_eq!(outside.discover(address).unwrap(), microsoft_found(address, MicrosoftKind::Personal), "{address}");
            assert!(outside.asked().is_empty(), "{address}: Outlook.com's domains: nothing asked of anyone");
        }
        assert_eq!(MICROSOFT_CONSUMER_DOMAINS.len(), 102, "the ISPDB's list, whole (its 103rd name, olc.protection.outlook.com, is an exchanger's)");
        assert!(!is_microsoft_consumer_domain("outlook.com.evil.example") && !is_microsoft_consumer_domain("notoutlook.com"));
        // Outlook.com's and Microsoft 365's exchangers, with the trailing dot DNS gives.
        assert_eq!(microsoft_mx_kind("outlook-com.olc.protection.outlook.com."), Some(MicrosoftKind::Personal));
        assert_eq!(microsoft_mx_kind("eur.olc.protection.outlook.com"), Some(MicrosoftKind::Personal));
        for work in ["contoso-com.mail.protection.outlook.com", "contoso-com.o-v1.mx.microsoft", "x.mail.eo.outlook.com", "contoso-de.mail.protection.outlook.de"] {
            assert_eq!(microsoft_mx_kind(work), Some(MicrosoftKind::Work), "{work}");
        }
        for lookalike in ["mail.protection.outlook.com.evil.example", "evilmx.microsoft.example", "mail.protection.outlook.com", ".mail.protection.outlook.com", "evilmail.protection.outlook.com", "olc.protection.outlook.com", "mx.microsoft"] {
            assert_eq!(microsoft_mx_kind(lookalike), None, "{lookalike}");
        }
        // Settings naming Microsoft's server: Microsoft's, of the kind its sending server says.
        let settings = parse_config(
            "<clientConfig><emailProvider><incomingServer type=\"imap\"><hostname>outlook.office365.com</hostname><port>993</port><socketType>SSL</socketType></incomingServer>\
             <outgoingServer type=\"smtp\"><hostname>smtp.office365.com</hostname><port>587</port><socketType>STARTTLS</socketType></outgoingServer></emailProvider></clientConfig>",
            "you@contoso.example",
            FoundBy::Provider,
        )
        .unwrap();
        assert_eq!(microsoft_if_named(settings, "you@contoso.example"), microsoft_found("you@contoso.example", MicrosoftKind::Work));
        // Microsoft's servers: 993 encrypted from the first byte, 587 with STARTTLS, the sending server by kind.
        let found = microsoft_found("you@contoso.example", MicrosoftKind::Work);
        assert_eq!((found.host.as_str(), found.port, found.security, found.by), ("outlook.office365.com", 993, Security::Tls, FoundBy::Microsoft(MicrosoftKind::Work)));
        assert_eq!(found.smtp, Some(Smtp { host: "smtp.office365.com".into(), port: 587, security: Security::Starttls }));
        assert_eq!(microsoft_found("you@hotmail.fr", MicrosoftKind::Personal).smtp.unwrap().host, "smtp-mail.outlook.com");
    }

    /// Thunderbird's entry for Yahoo (`yahoo.com`, which covers yahoo.fr and
    /// Yahoo's other domains), as its list gave it on 17 November 2026
    /// (<https://autoconfig.thunderbird.net/v1.1/yahoo.com>), trimmed: five
    /// of its twenty-one domains, no documentation links, no webmail block.
    /// Two IMAP blocks, a POP one, an SMTP one; OAuth2 named before the
    /// password, with Yahoo's own sign-in, which Sioul does not use.
    const YAHOO: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<clientConfig version="1.1">
  <emailProvider id="yahoo.com">
    <domain>yahoo.com</domain>
    <domain>yahoo.fr</domain>
    <domain>ymail.com</domain>
    <domain>myyahoo.com</domain>
    <domain>rocketmail.com</domain>
    <displayName>Yahoo! Mail</displayName>
    <displayShortName>Yahoo</displayShortName>
    <incomingServer type="imap">
      <hostname>imap.mail.yahoo.com</hostname>
      <port>993</port>
      <socketType>SSL</socketType>
      <username>%EMAILADDRESS%</username>
      <authentication>OAuth2</authentication>
      <authentication>password-cleartext</authentication>
    </incomingServer>
    <incomingServer type="imap">
      <hostname>export.imap.mail.yahoo.com</hostname>
      <port>993</port>
      <socketType>SSL</socketType>
      <username>%EMAILADDRESS%</username>
      <authentication>OAuth2</authentication>
      <authentication>password-cleartext</authentication>
    </incomingServer>
    <incomingServer type="pop3">
      <hostname>pop.mail.yahoo.com</hostname>
      <port>995</port>
      <socketType>SSL</socketType>
      <username>%EMAILADDRESS%</username>
      <authentication>OAuth2</authentication>
      <authentication>password-cleartext</authentication>
    </incomingServer>
    <outgoingServer type="smtp">
      <hostname>smtp.mail.yahoo.com</hostname>
      <port>465</port>
      <socketType>SSL</socketType>
      <username>%EMAILADDRESS%</username>
      <authentication>OAuth2</authentication>
      <authentication>password-cleartext</authentication>
    </outgoingServer>
  </emailProvider>
  <oAuth2>
    <issuer>login.yahoo.com</issuer>
    <scope>mail-w</scope>
    <authURL>https://api.login.yahoo.com/oauth2/request_auth</authURL>
    <tokenURL>https://api.login.yahoo.com/oauth2/get_token</tokenURL>
  </oAuth2>
</clientConfig>"#;

    /// What Thunderbird's list answers for a Yahoo address (the address here
    /// is made up, and only fills the entry's placeholders): Yahoo's first
    /// IMAP server (never the export twin, never POP) and its sending
    /// server, both encrypted from the first byte, the whole address as
    /// login; and servers known as Yahoo's, which take app passwords only.
    #[test]
    fn finds_yahoo_s_servers_in_thunderbird_s_list() {
        let found = parse_config(YAHOO, "jane@example.org", FoundBy::Ispdb).unwrap();
        assert_eq!((found.host.as_str(), found.port, found.security), ("imap.mail.yahoo.com", 993, Security::Tls));
        assert_eq!((found.username.as_str(), found.by), ("jane@example.org", FoundBy::Ispdb));
        assert_eq!(found.smtp, Some(Smtp { host: "smtp.mail.yahoo.com".into(), port: 465, security: Security::Tls }));
        assert!(is_yahoo_host(&found.host) && found.smtp.as_ref().is_some_and(|smtp| is_yahoo_host(&smtp.host)));
        // Settings that name Yahoo's server stay what the list gave: they are not Microsoft's.
        assert_eq!(microsoft_if_named(found.clone(), "jane@example.org"), found);
        for host in ["imap.mail.yahoo.com", "IMAP.Mail.Yahoo.COM", "export.imap.mail.yahoo.com", "smtp.mail.yahoo.com"] {
            assert!(is_yahoo_host(host), "{host}");
        }
        // Names that only look like Yahoo's, and other servers.
        for host in ["imap.mail.yahoo.com.evil.example", "notimap.mail.yahoo.com", "mail.yahoo.com", "yahoo.com", "imap.gmail.com", "mail.example.org", ""] {
            assert!(!is_yahoo_host(host), "{host}");
        }
    }

    #[test]
    fn reads_addresses_carefully() {
        assert_eq!(domain_of("Someone@Example.org").as_deref(), Some("example.org"));
        assert_eq!(domain_of("nobody"), None);
        assert_eq!(domain_of("a@example.org/evil"), None);
        assert_eq!(encode("a+b@example.org"), "a%2Bb%40example.org");
    }

    /// The outside, stood in for: the settings files that exist, the MX and
    /// SRV records, the hosts that show a certificate valid for their name;
    /// and what was asked, in order. No test asks the network.
    #[derive(Default)]
    struct Outside {
        pages: Vec<(&'static str, &'static str)>,
        mx: Vec<(&'static str, Vec<&'static str>)>,
        srv: Vec<(&'static str, Vec<(u16, u16, u16, &'static str)>)>,
        certified: Vec<(&'static str, u16)>,
        asked: std::cell::RefCell<Vec<String>>,
    }

    impl Outside {
        fn with<T>(&self, run: impl FnOnce(&Ask) -> T) -> T {
            let note = |line: String| self.asked.borrow_mut().push(line);
            let fetch = |url: &str| {
                note(format!("GET {url}"));
                self.pages.iter().find(|(known, _)| *known == url).map(|(_, xml)| xml.to_string())
            };
            let mx = |domain: &str| {
                note(format!("MX {domain}"));
                self.mx.iter().find(|(known, _)| *known == domain).map(|(_, hosts)| hosts.iter().map(|h| h.to_string()).collect())
            };
            let srv = |name: &str| {
                note(format!("SRV {name}"));
                let records = self.srv.iter().find(|(known, _)| *known == name).map(|(_, records)| records.clone()).unwrap_or_default();
                records.into_iter().map(|(priority, weight, port, target)| Service { priority, weight, port, target: target.to_string() }).collect()
            };
            let certified = |host: &str, port: u16| {
                note(format!("TLS {host}:{port}"));
                self.certified.iter().any(|(known, at)| *known == host && *at == port)
            };
            run(&Ask { fetch: &fetch, mx: &mx, srv: &srv, certified: &certified })
        }

        fn discover(&self, address: &str) -> Result<Found, SyncError> {
            self.with(|ask| discover_with(address, ask))
        }

        fn discover_smtp(&self, address: &str, imap_host: Option<&str>) -> Result<Smtp, SyncError> {
            self.with(|ask| discover_smtp_with(address, imap_host, ask))
        }

        /// What was asked since the last look, in order.
        fn asked(&self) -> Vec<String> {
            std::mem::take(&mut *self.asked.borrow_mut())
        }
    }

    /// Thunderbird's entry for OVH (`ovh.net`), as its list gave it on 17
    /// November 2026 (<https://autoconfig.thunderbird.net/v1.1/ovh.net>),
    /// without its documentation link: the server every OVH mailbox is kept
    /// on, whatever the domain.
    const OVH: &str = r#"<clientConfig version="1.1">
  <emailProvider id="ovh.net">
    <!-- Hosting customers, MX servers -->
    <domain>ovh.net</domain>

    <displayName>OVH</displayName>
    <displayShortName>OVH</displayShortName>
    <incomingServer type="imap">
      <hostname>ssl0.ovh.net</hostname>
      <port>993</port>
      <socketType>SSL</socketType>
      <authentication>password-cleartext</authentication>
      <username>%EMAILADDRESS%</username>
    </incomingServer>
    <incomingServer type="pop3">
      <hostname>ssl0.ovh.net</hostname>
      <port>995</port>
      <socketType>SSL</socketType>
      <authentication>password-cleartext</authentication>
      <username>%EMAILADDRESS%</username>
    </incomingServer>
    <outgoingServer type="smtp">
      <hostname>ssl0.ovh.net</hostname>
      <port>465</port>
      <socketType>SSL</socketType>
      <authentication>password-cleartext</authentication>
      <username>%EMAILADDRESS%</username>
    </outgoingServer>
  </emailProvider>
</clientConfig>"#;

    const OWN_SETTINGS: &str = "https://autoconfig.example.org/mail/config-v1.1.xml?emailaddress=jane%40example.org";
    const WELL_KNOWN: &str = "https://example.org/.well-known/autoconfig/mail/config-v1.1.xml";
    const LIST_BY_DOMAIN: &str = "https://autoconfig.thunderbird.net/v1.1/example.org";

    /// The order of discovery, each step asked only when those before found
    /// nothing: the domain's own HTTPS settings, Thunderbird's list by the
    /// domain, Google and Microsoft by the MX, the list by the MX's names
    /// (the longest first), the domain's records, then the checked guess.
    #[test]
    fn asks_each_source_in_its_turn() {
        let outside = Outside { mx: vec![("example.org", vec!["mx1.mail.host.example.", "mx2.mail.host.example."])], ..Outside::default() };
        assert_eq!(outside.discover("jane@example.org"), Err(SyncError::NotFound("example.org".into())));
        assert_eq!(
            outside.asked(),
            [
                format!("GET {OWN_SETTINGS}"),
                format!("GET {WELL_KNOWN}"),
                format!("GET {LIST_BY_DOMAIN}"),
                "MX example.org".to_string(),
                "GET https://autoconfig.thunderbird.net/v1.1/mail.host.example".to_string(),
                "GET https://autoconfig.thunderbird.net/v1.1/host.example".to_string(),
                "SRV _imaps._tcp.example.org.".to_string(),
                "TLS imap.example.org:993".to_string(),
                "TLS mail.example.org:993".to_string(),
            ]
        );
        // Each step that finds ends the search there.
        let own = Outside { pages: vec![(OWN_SETTINGS, CPANEL)], ..Outside::default() };
        assert_eq!(own.discover("jane@example.org").unwrap().by, FoundBy::Provider);
        assert_eq!(own.asked(), [format!("GET {OWN_SETTINGS}")]);
        let listed = Outside { pages: vec![(LIST_BY_DOMAIN, CPANEL)], ..Outside::default() };
        assert_eq!(listed.discover("jane@example.org").unwrap().by, FoundBy::Ispdb);
        assert_eq!(listed.asked().len(), 3, "no DNS asked");
        // Google's and Microsoft's mail by the MX, before the list is asked about the MX's names.
        let google = Outside { mx: vec![("example.org", vec!["aspmx.l.google.com."])], ..Outside::default() };
        assert_eq!(google.discover("jane@example.org").unwrap(), google_found("jane@example.org"));
        assert_eq!(google.asked().last().map(String::as_str), Some("MX example.org"));
        let microsoft = Outside { mx: vec![("example.org", vec!["example-org.mail.protection.outlook.com."])], ..Outside::default() };
        assert_eq!(microsoft.discover("jane@example.org").unwrap(), microsoft_found("jane@example.org", MicrosoftKind::Work));
        assert_eq!(microsoft.asked().last().map(String::as_str), Some("MX example.org"));
    }

    /// A domain of one's own whose mail a host keeps: its `imap.<domain>` is
    /// an alias of the host's server, whose certificate names the host's
    /// server only. Thunderbird's list knows the host by the names of the
    /// servers that receive the domain's mail, and is told those names
    /// alone: never the address, never the domain a second time.
    #[test]
    fn a_domain_kept_at_a_host_is_found_by_the_host_s_entry() {
        let outside = Outside {
            pages: vec![("https://autoconfig.thunderbird.net/v1.1/ovh.net", OVH)],
            mx: vec![("example.org", vec!["mx1.mail.ovh.net.", "mx2.mail.ovh.net.", "mx3.mail.ovh.net."])],
            // The aliases answer on their ports, with a certificate for the host's server: no guess.
            certified: vec![("ssl0.ovh.net", 993), ("ssl0.ovh.net", 465)],
            ..Outside::default()
        };
        let found = outside.discover(" jane@example.org ").unwrap();
        assert_eq!((found.host.as_str(), found.port, found.security, found.by), ("ssl0.ovh.net", 993, Security::Tls, FoundBy::IspdbMx));
        assert_eq!(found.username, "jane@example.org", "the whole address, as the list says");
        assert_eq!(found.smtp, Some(Smtp { host: "ssl0.ovh.net".into(), port: 465, security: Security::Tls }));
        assert_eq!(FoundBy::IspdbMx.message_id(), "account-by-ispdb-mx");
        let asked = outside.asked();
        assert_eq!(asked[3..], ["MX example.org", "GET https://autoconfig.thunderbird.net/v1.1/mail.ovh.net", "GET https://autoconfig.thunderbird.net/v1.1/ovh.net"]);
        assert!(asked[4..].iter().all(|line| !line.contains("example.org") && !line.contains("jane")), "{asked:?}");
        // The names the list is asked about: above each exchanger, the longest
        // first, down to the registered name, never a public suffix alone.
        let names = |hosts: &[&str]| provider_names(&hosts.iter().map(|h| h.to_string()).collect::<Vec<_>>(), "example.org");
        assert_eq!(names(&["mx1.mail.ovh.net.", "MX2.Mail.OVH.net"]), ["mail.ovh.net", "ovh.net"]);
        assert_eq!(names(&["mx.example.co.uk"]), ["example.co.uk"], "never co.uk");
        assert_eq!(names(&["host.example"]), ["host.example"], "a registered name receiving the mail itself");
        assert_eq!(names(&["a.b.c.host.example", "mx.other.example"]), ["b.c.host.example", "c.host.example", "host.example", "other.example"]);
        assert_eq!(names(&["a.b.c.d.e.host.example"]).len(), PROVIDER_NAMES, "a few names at most");
        // The domain's own servers: the list was asked about the domain already.
        assert!(names(&["mail.example.org", "mx.in.example.org", "example.org"]).is_empty());
        // Names no host has, and public suffixes alone, are never asked about.
        assert!(names(&["co.uk", "uk", "localhost", "mx_1.host.example", "mx.host.example/v1.1/x", ""]).is_empty());
    }

    /// The domain's own SRV records (RFC 6186, RFC 8314), when no list knows
    /// the domain: said as read in its records, the server on TLS from the
    /// first byte only if it shows a certificate valid for its name.
    #[test]
    fn reads_the_domain_s_own_records() {
        let records = |sending: Vec<(&'static str, Vec<(u16, u16, u16, &'static str)>)>| Outside {
            srv: [vec![("_imaps._tcp.example.org.", vec![(10, 0, 993, "backup.host.example."), (0, 1, 993, "ssl0.host.example."), (0, 5, 993, "SSL1.host.example.")])], sending].concat(),
            certified: vec![("ssl1.host.example", 993), ("ssl1.host.example", 465), ("ssl0.host.example", 465)],
            ..Outside::default()
        };
        // As a host writes them by default: submission on 465 is TLS from the first byte.
        let outside = records(vec![("_submission._tcp.example.org.", vec![(0, 0, 465, "ssl0.host.example.")])]);
        let found = outside.discover("jane@example.org").unwrap();
        assert_eq!((found.host.as_str(), found.port, found.security, found.username.as_str(), found.by), ("ssl1.host.example", 993, Security::Tls, "jane@example.org", FoundBy::Records), "the lowest priority, then the highest weight");
        assert_eq!(found.smtp, Some(Smtp { host: "ssl0.host.example".into(), port: 465, security: Security::Tls }));
        assert_eq!(FoundBy::Records.message_id(), "account-by-records");
        assert_eq!(outside.asked()[4..], ["SRV _imaps._tcp.example.org.", "TLS ssl1.host.example:993", "SRV _submissions._tcp.example.org.", "SRV _submission._tcp.example.org.", "TLS ssl0.host.example:465"]);
        // RFC 8314's record first; RFC 6186's on 587 is STARTTLS.
        let both = records(vec![("_submissions._tcp.example.org.", vec![(0, 0, 465, "ssl1.host.example.")]), ("_submission._tcp.example.org.", vec![(0, 0, 587, "ssl0.host.example.")])]);
        assert_eq!(both.discover("jane@example.org").unwrap().smtp, Some(Smtp { host: "ssl1.host.example".into(), port: 465, security: Security::Tls }));
        let starttls = records(vec![("_submission._tcp.example.org.", vec![(0, 0, 587, "smtp.host.example.")])]);
        assert_eq!(starttls.discover("jane@example.org").unwrap().smtp, Some(Smtp { host: "smtp.host.example".into(), port: 587, security: Security::Starttls }));
        // "." says the service is not offered; a sending server at another
        // keeper than the receiving one is not taken from unsigned records.
        let none = records(vec![("_submission._tcp.example.org.", vec![(0, 0, 0, ".")])]);
        assert_eq!(none.discover("jane@example.org").unwrap().smtp, None);
        let elsewhere = records(vec![("_submission._tcp.example.org.", vec![(0, 0, 587, "smtp.elsewhere.example.")])]);
        assert_eq!(elsewhere.discover("jane@example.org").unwrap().smtp, None);
        // A receiving server that shows no certificate for its name is not
        // offered: the guess is tried, checked the same way.
        let unchecked = Outside { srv: vec![("_imaps._tcp.example.org.", vec![(0, 0, 993, "alias.host.example.")])], certified: vec![("mail.example.org", 993)], ..Outside::default() };
        let found = unchecked.discover("jane@example.org").unwrap();
        assert_eq!((found.host.as_str(), found.by), ("mail.example.org", FoundBy::Guess));
        assert_eq!(unchecked.asked()[4..], ["SRV _imaps._tcp.example.org.", "TLS alias.host.example:993", "TLS imap.example.org:993", "TLS mail.example.org:993"]);
        assert_eq!(preferred(vec![Service { priority: 0, weight: 0, port: 993, target: "bad name.example".into() }, Service { priority: 0, weight: 0, port: 0, target: "host.example".into() }]), None);
    }

    /// The sending server, by the same steps. What unsigned DNS led to is
    /// taken only when it agrees with the receiving server the account has:
    /// nobody reads this answer before a password follows it.
    #[test]
    fn finds_the_sending_server_by_the_same_steps() {
        // The receiving server typed by hand (the host's own name): the list, by the MX.
        let hosted = Outside {
            pages: vec![("https://autoconfig.thunderbird.net/v1.1/ovh.net", OVH)],
            mx: vec![("example.org", vec!["mx1.mail.ovh.net."])],
            ..Outside::default()
        };
        assert_eq!(hosted.discover_smtp("jane@example.org", Some("ssl0.ovh.net")), Ok(Smtp { host: "ssl0.ovh.net".into(), port: 465, security: Security::Tls }));
        // No list knows the host: `smtp.<domain>` answers on 465 with the
        // host's certificate (an alias), so the receiving server is taken.
        let alias = Outside { certified: vec![("ssl0.host.example", 465)], ..Outside::default() };
        assert_eq!(alias.discover_smtp("jane@example.org", Some("SSL0.host.example")), Ok(Smtp { host: "ssl0.host.example".into(), port: 465, security: Security::Tls }));
        let asked = alias.asked();
        assert_eq!(asked[asked.len() - 2..], ["TLS smtp.example.org:465", "TLS ssl0.host.example:465"]);
        // `smtp.<domain>` with a certificate of its own comes first.
        let own = Outside { certified: vec![("smtp.example.org", 465), ("ssl0.host.example", 465)], ..Outside::default() };
        assert_eq!(own.discover_smtp("jane@example.org", Some("ssl0.host.example")).unwrap().host, "smtp.example.org");
        // The records, when they name the keeper of the receiving server, or the domain itself.
        let records = Outside { srv: vec![("_submission._tcp.example.org.", vec![(0, 0, 587, "smtp.host.example.")])], ..Outside::default() };
        assert_eq!(records.discover_smtp("jane@example.org", Some("imap.host.example")), Ok(Smtp { host: "smtp.host.example".into(), port: 587, security: Security::Starttls }));
        let within = Outside { srv: vec![("_submission._tcp.example.org.", vec![(0, 0, 587, "out.example.org.")])], ..Outside::default() };
        assert_eq!(within.discover_smtp("jane@example.org", Some("imap.host.example")).unwrap().host, "out.example.org");
        // Forged answers: an MX at another listed provider, records naming
        // another server (each with a valid certificate for its own name).
        // Neither is followed: the receiving server the account has decides.
        let forged = Outside {
            pages: vec![("https://autoconfig.thunderbird.net/v1.1/ovh.net", OVH)],
            mx: vec![("example.org", vec!["mx1.mail.ovh.net."])],
            srv: vec![("_imaps._tcp.example.org.", vec![(0, 0, 993, "imap.elsewhere.example.")]), ("_submissions._tcp.example.org.", vec![(0, 0, 465, "smtp.elsewhere.example.")])],
            certified: vec![("ssl0.ovh.net", 993), ("ssl0.ovh.net", 465), ("imap.elsewhere.example", 993), ("smtp.elsewhere.example", 465), ("imap.host.example", 465)],
            ..Outside::default()
        };
        assert_eq!(forged.discover_smtp("jane@example.org", Some("imap.host.example")), Ok(Smtp { host: "imap.host.example".into(), port: 465, security: Security::Tls }));
        let nothing = Outside { srv: vec![("_submissions._tcp.example.org.", vec![(0, 0, 465, "smtp.elsewhere.example.")])], certified: vec![("smtp.elsewhere.example", 465)], ..Outside::default() };
        assert_eq!(nothing.discover_smtp("jane@example.org", Some("imap.host.example")), Err(SyncError::NotFound("example.org".into())));
        // The same keeper: the same name, or the same registered name; or under the address's domain.
        assert!(same_keeper("ssl0.ovh.net.", Some("SSL0.ovh.net"), "example.org") && same_keeper("smtp.host.example", Some("imap.host.example"), "example.org"));
        assert!(same_keeper("smtp.example.org", None, "example.org") && same_keeper("example.org", None, "example.org"));
        assert!(!same_keeper("smtp.elsewhere.example", Some("imap.host.example"), "example.org") && !same_keeper("smtp.host.example", None, "example.org"));
        assert!(!same_keeper("evil-example.org", None, "example.org") && !same_keeper("mail.example.co.uk", Some("co.uk"), "example.org") && !same_keeper("mail.example.co.uk", Some("mail.example.org.uk"), "example.org"), "whole labels; never a public suffix");
    }

    /// A guess is offered only when the host shows a certificate valid for
    /// that very name: an alias that answers with its host's certificate is
    /// no guess.
    #[test]
    fn a_guess_must_show_its_own_certificate() {
        let alias = Outside { certified: vec![("ssl0.host.example", 993)], ..Outside::default() };
        assert_eq!(alias.discover("jane@example.org"), Err(SyncError::NotFound("example.org".into())));
        let second = Outside { certified: vec![("mail.example.org", 993)], ..Outside::default() };
        let found = second.discover("jane@example.org").unwrap();
        assert_eq!((found.host.as_str(), found.port, found.security, found.by, found.smtp.clone()), ("mail.example.org", 993, Security::Tls, FoundBy::Guess, None));
        let first = Outside { certified: vec![("imap.example.org", 993), ("mail.example.org", 993)], ..Outside::default() };
        assert_eq!(first.discover("jane@example.org").unwrap().host, "imap.example.org");
        assert_eq!(first.asked().last().map(String::as_str), Some("TLS imap.example.org:993"), "the first that passes");
    }

    /// A certificate made for these tests alone, for the made-up names
    /// `ssl0.stand-in.example` and `www.ssl0.stand-in.example`, signed by
    /// itself, valid from 2026 to 2126, and its key (PKCS #8): DER, in
    /// base64. They protect nothing: no build of Sioul trusts them.
    const STAND_IN_CERTIFICATE: &str = "MIIB1jCCAX2gAwIBAgIUab9Di+Xx1JxwDzOkodCsSl+CXGwwCgYIKoZIzj0EAwIwIDEeMBwGA1UEAwwVc3NsMC5zdGFuZC1pbi5l\
         eGFtcGxlMCAXDTI2MDEwMTAwMDAwMFoYDzIxMjYwMTAxMDAwMDAwWjAgMR4wHAYDVQQDDBVzc2wwLnN0YW5kLWluLmV4YW1wbGUw\
         WTATBgcqhkjOPQIBBggqhkjOPQMBBwNCAATU6hIIPnQkgtSeQdFPtApSZP3m77fuyJIbfpaoiVVnX32rlF8B4ARZCUVzBwMlUNKG\
         UPf+MSWPGsIIfXJjU6Ego4GSMIGPMAwGA1UdEwEB/wQCMAAwDgYDVR0PAQH/BAQDAgeAMBMGA1UdJQQMMAoGCCsGAQUFBwMBMDsG\
         A1UdEQQ0MDKCFXNzbDAuc3RhbmQtaW4uZXhhbXBsZYIZd3d3LnNzbDAuc3RhbmQtaW4uZXhhbXBsZTAdBgNVHQ4EFgQUbQsofXN9\
         mjMvHcXUrlwH0XC6IiYwCgYIKoZIzj0EAwIDRwAwRAIgL4wM0Y3YLufcLMoas/fswHEPv8iNkU3K3EGhHDERGBkCIGzWDUmGwTje\
         de+sfWMoakurC1xBVsEt0HOf8QAsDbpz";
    const STAND_IN_KEY: &str = "MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQg2fP2AXMXWuh4zKZK7v4xN+h1UXvVhYlm53KfSmL+Ba6hRANCAATU\
         6hIIPnQkgtSeQdFPtApSZP3m77fuyJIbfpaoiVVnX32rlF8B4ARZCUVzBwMlUNKGUPf+MSWPGsIIfXJjU6Eg";

    /// The check itself, against servers standing in on this computer: one
    /// that shows a certificate for `ssl0.stand-in.example`, trusted for
    /// this test alone. Asked by that name, it passes; asked by an alias
    /// (`imap.example.org`, the same server behind another name), it does
    /// not, though the certificate is otherwise valid; nor does a server
    /// that answers without TLS, nor a port where nothing listens.
    #[test]
    fn the_certificate_check_takes_the_name_asked_alone() {
        use base64::Engine;
        use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
        use std::io::{Read, Write};
        let decode = |text: &str| base64::engine::general_purpose::STANDARD.decode(text).unwrap();
        let certificate = CertificateDer::from(decode(STAND_IN_CERTIFICATE));
        let provider = || Arc::new(rustls::crypto::ring::default_provider());
        let mut roots = rustls::RootCertStore::empty();
        roots.add(certificate.clone()).unwrap();
        let trusting = Arc::new(rustls::ClientConfig::builder_with_provider(provider()).with_safe_default_protocol_versions().unwrap().with_root_certificates(roots).with_no_client_auth());
        let serving = rustls::ServerConfig::builder_with_provider(provider())
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_no_client_auth()
            .with_single_cert(vec![certificate], PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(decode(STAND_IN_KEY))))
            .unwrap();
        // The stand-in: TLS from the first byte, two visits, nothing said after the handshake.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let at = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
            runtime.block_on(async {
                listener.set_nonblocking(true).unwrap();
                let listener = tokio::net::TcpListener::from_std(listener).unwrap();
                let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(serving));
                let mut handshakes = Vec::new();
                for _ in 0..2 {
                    let (stream, _) = listener.accept().await.unwrap();
                    handshakes.push(acceptor.accept(stream).await.is_ok());
                }
                handshakes
            })
        });
        assert!(shows_its_own_certificate("ssl0.stand-in.example", at, trusting.clone()), "its own name");
        assert!(!shows_its_own_certificate("imap.example.org", at, trusting.clone()), "an alias: the certificate names the host's server");
        assert_eq!(server.join().unwrap(), [true, false], "the second visit ended at the certificate");
        // With the system's certificates, as Sioul checks: this certificate is nobody's.
        let plain = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let plain_at = plain.local_addr().unwrap();
        let greeter = std::thread::spawn(move || {
            let (mut stream, _) = plain.accept().unwrap();
            let _ = stream.write_all(b"* OK stand-in, without TLS\r\n");
            let _ = stream.read(&mut [0; 512]);
        });
        assert!(!shows_its_own_certificate("ssl0.stand-in.example", plain_at, trusting.clone()), "it answers, without TLS");
        greeter.join().unwrap();
        let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap();
        assert!(!shows_its_own_certificate("ssl0.stand-in.example", closed, trusting), "nothing listens");
        assert!(!shows_its_own_certificate("not a name", at, crate::imap::tls_config()));
    }
}
