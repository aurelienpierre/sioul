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
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

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

/// The longest one settings file takes to come.
const HTTP: Duration = Duration::from_secs(8);
/// The longest one certificate check takes: its name looked up, its
/// connection made, STARTTLS asked where it is used, its handshake done.
const PROBE: Duration = Duration::from_secs(5);
/// The longest one question to the DNS takes.
const DNS: Duration = Duration::from_secs(4);
/// The longest a whole search takes, whatever the network does: a step
/// not started by then is not started, and what was not found is said so.
pub(crate) const WHOLE: Duration = Duration::from_secs(28);
/// With less time left than this, a step is not started.
const LEAST: Duration = Duration::from_millis(500);
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

/// How a server's certificate is reached for its check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Probe {
    /// TLS from the first byte (993, 465).
    Tls,
    /// IMAP's STARTTLS (RFC 9051 §6.2.1): the greeting, STARTTLS, its OK, then TLS.
    Imap,
    /// SMTP's STARTTLS (RFC 3207): the greeting, EHLO, STARTTLS, its 220, then TLS.
    Smtp,
}

impl Probe {
    fn receiving(security: Security) -> Probe {
        match security {
            Security::Tls => Probe::Tls,
            Security::Starttls => Probe::Imap,
        }
    }

    fn sending(security: Security) -> Probe {
        match security {
            Security::Tls => Probe::Tls,
            Security::Starttls => Probe::Smtp,
        }
    }
}

/// What discovery asks of the outside, each a seam for the tests, so that
/// none of them asks the network. Each question gets the longest it may
/// take, which the search gives from what is left of `WHOLE`.
pub(crate) struct Ask<'a> {
    /// A settings file, by its HTTPS address; None when there is none.
    pub fetch: &'a dyn Fn(&str, Duration) -> Option<String>,
    /// The hosts that receive a domain's mail (its MX records), the preferred first.
    pub mx: &'a dyn Fn(&str, Duration) -> Option<Vec<String>>,
    /// The SRV records of a name ("_imaps._tcp.example.org.").
    pub srv: &'a dyn Fn(&str, Duration) -> Vec<Service>,
    /// Whether a host answers on a port, reached as `Probe` says, with a
    /// certificate valid for that very name.
    pub certified: &'a dyn Fn(&str, u16, Probe, Duration) -> bool,
    /// How long since the search began.
    pub elapsed: &'a dyn Fn() -> Duration,
}

/// `run` with the real outside: HTTPS to the end (a redirect to a plain
/// address is not followed), the system's DNS, a TLS handshake.
fn with_the_network<T>(run: impl FnOnce(&Ask) -> T) -> T {
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(HTTP)).https_only(true).build().into();
    let began = Instant::now();
    run(&Ask {
        fetch: &|url: &str, limit: Duration| fetch(&agent, url, limit),
        mx: &mx_hosts,
        srv: &srv_records,
        certified: &|host: &str, port: u16, probe: Probe, limit: Duration| shows_its_own_certificate(host, (host, port), probe, crate::imap::tls_config(), limit),
        elapsed: &|| began.elapsed(),
    })
}

/// One search: what it asks of the outside, the time left of `WHOLE`, the
/// certificates already checked (a host and port is never checked twice in
/// one search), and the domains never told to Thunderbird's list.
struct Search<'a> {
    ask: &'a Ask<'a>,
    own: Vec<String>,
    checked: RefCell<HashMap<(String, u16, Probe), bool>>,
    /// A step was not started for want of time.
    late: std::cell::Cell<bool>,
}

impl<'a> Search<'a> {
    /// A search for an address at `domain`, the person's other domains `own`.
    fn new(ask: &'a Ask<'a>, domain: &str, own: &[String]) -> Search<'a> {
        let mut own: Vec<String> = own.iter().filter_map(|d| domain_of(&format!("x@{d}"))).collect();
        own.push(domain.to_string());
        Search { ask, own, checked: RefCell::new(HashMap::new()), late: std::cell::Cell::new(false) }
    }

    /// The time a step may take: at most `most`, at most what is left; none when too little is.
    fn limit(&self, most: Duration) -> Option<Duration> {
        let left = WHOLE.saturating_sub((self.ask.elapsed)());
        let limit = (left >= LEAST).then(|| most.min(left));
        self.late.set(self.late.get() || limit.is_none());
        limit
    }

    /// Nothing found for `domain`: said as such, or, when the time ran out
    /// before every step was asked, as the network's slowness.
    fn nothing(&self, domain: String) -> SyncError {
        if self.late.get() { SyncError::Network(format!("no answer within {} s", WHOLE.as_secs())) } else { SyncError::NotFound(domain) }
    }

    fn fetch(&self, url: &str) -> Option<String> {
        (self.ask.fetch)(url, self.limit(HTTP)?)
    }

    fn mx(&self, domain: &str) -> Vec<String> {
        self.limit(DNS).and_then(|limit| (self.ask.mx)(domain, limit)).unwrap_or_default()
    }

    fn srv(&self, name: &str) -> Vec<Service> {
        self.limit(DNS).map(|limit| (self.ask.srv)(name, limit)).unwrap_or_default()
    }

    /// Whether `host` shows, on `port`, a certificate valid for that very
    /// name: asked once per search, a name no host has never asked.
    fn certified(&self, host: &str, port: u16, probe: Probe) -> bool {
        let key = (host.to_string(), port, probe);
        if let Some(known) = self.checked.borrow().get(&key) {
            return *known;
        }
        let Some(limit) = self.limit(PROBE) else { return false };
        let passed = is_host_name(host) && (self.ask.certified)(host, port, probe, limit);
        self.checked.borrow_mut().insert(key, passed);
        passed
    }

    /// The settings a source gives for an address, if it has any.
    fn settings(&self, url: &str, address: &str, domain: &str, by: FoundBy) -> Option<Found> {
        self.fetch(url).and_then(|xml| parse_config(&xml, address, domain, by)).map(|found| microsoft_if_named(found, address))
    }

    /// The domain's own settings, then Thunderbird's list by the domain: HTTPS.
    fn by_the_domain(&self, address: &str, domain: &str) -> impl Iterator<Item = Found> {
        [
            (format!("https://autoconfig.{domain}/mail/config-v1.1.xml?emailaddress={}", encode(address)), FoundBy::Provider),
            (format!("https://{domain}/.well-known/autoconfig/mail/config-v1.1.xml"), FoundBy::Provider),
            (format!("{ISPDB}{domain}"), FoundBy::Ispdb),
        ]
        .into_iter()
        .filter_map(move |(url, by)| self.settings(&url, address, domain, by))
    }

    /// Thunderbird's list, asked by the names of whoever receives the
    /// domain's mail (`provider_names`): the entries it gives, in turn.
    fn by_the_exchangers<'b>(&'b self, exchangers: &'b [String], address: &'b str, domain: &'b str) -> impl Iterator<Item = Found> + 'b {
        provider_names(exchangers, &self.own).into_iter().filter_map(move |name| self.settings(&format!("{ISPDB}{name}"), address, domain, FoundBy::IspdbMx))
    }
}

/// The IMAP settings for an address.
pub fn discover(address: &str) -> Result<Found, SyncError> {
    let own = own_domains();
    with_the_network(|ask| discover_with(address, ask, &own))
}

/// `discover`, the outside asked through `ask` (the network, or stand-ins
/// in tests), the person's other domains `own` never told to the list.
pub(crate) fn discover_with(address: &str, ask: &Ask, own: &[String]) -> Result<Found, SyncError> {
    let address = tidy_address(address);
    let domain = domain_of(address).ok_or(SyncError::BadAddress)?;
    // Gmail: known here, nothing asked of anyone.
    if is_google_domain(&domain) {
        return Ok(google_found(address));
    }
    // Outlook.com's own domains: known here too.
    if is_microsoft_consumer_domain(&domain) {
        return Ok(microsoft_found(address, MicrosoftKind::Personal));
    }
    let search = Search::new(ask, &domain, own);
    // The domain's own settings, then Thunderbird's list by the domain.
    if let Some(found) = search.by_the_domain(address, &domain).next() {
        return Ok(found);
    }
    // Google Workspace, Outlook.com, Microsoft 365: the domain's mail goes
    // there (one question to the DNS for both), its servers are theirs.
    let exchangers = search.mx(&domain);
    if exchangers.iter().any(|h| is_google_mx(h)) {
        return Ok(google_found(address));
    }
    if let Some(kind) = exchangers.iter().find_map(|h| microsoft_mx_kind(h)) {
        return Ok(microsoft_found(address, kind));
    }
    // What unsigned DNS led to, from here on, is offered only when its
    // server shows a certificate valid for its very name, and said as
    // something to check. A domain kept at a host: the list, by the host's
    // names; its sending server is found at the first message sent, by
    // `discover_smtp`, which checks it too.
    let hosted = search.by_the_exchangers(&exchangers, address, &domain).find(|found| search.certified(&found.host, found.port, Probe::receiving(found.security)));
    if let Some(found) = hosted {
        return Ok(Found { smtp: None, ..found });
    }
    // A guess, which only the domain's owner can certify, then the domain's own records.
    guess(&search, &domain, address).or_else(|| from_records(&search, &domain, address)).ok_or_else(|| search.nothing(domain))
}

/// The names Thunderbird's list may know a domain's host by, as Thunderbird
/// asks them: each name above the hosts that receive the domain's mail, the
/// longest first ("mx1.mail.ovh.net": "mail.ovh.net", then "ovh.net"), down
/// to the name registered under its public suffix and never further
/// (`trust::organisational`: the Public Suffix List). A name under one of
/// the person's own domains (`own`: the address's, and the other
/// addresses' kept in the configuration) is left out: the list was asked
/// about the address's domain already, and is told nothing else of theirs.
/// A name no host can have (an IP address, a name in Unicode, which DNS
/// gives in its `xn--` form) is never asked. A few names at most.
fn provider_names(exchangers: &[String], own: &[String]) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for host in exchangers {
        let host = lower(host);
        let Some(registered) = trust::organisational(&host).filter(|_| is_host_name(&host)) else { continue };
        // The host itself only when it is the registered name: it has no name above it to ask.
        let mut name = if host == registered { Some(host.as_str()) } else { host.split_once('.').map(|(_, above)| above) };
        while let Some(candidate) = name.filter(|n| under(n, &registered)) {
            if !own.iter().any(|mine| under(candidate, mine) || under(mine, candidate)) && !names.iter().any(|n| n == candidate) {
                names.push(candidate.to_string());
            }
            name = candidate.split_once('.').map(|(_, above)| above);
        }
    }
    names.truncate(PROVIDER_NAMES);
    names
}

/// The domains of the person's other addresses, from the configuration:
/// never told to Thunderbird's list. Tests never read the configuration of
/// whoever runs them.
fn own_domains() -> Vec<String> {
    if cfg!(test) {
        return Vec::new();
    }
    let Ok(config) = sioul_core::config::Config::load(&sioul_core::config::default_path()) else { return Vec::new() };
    config.every_account().filter_map(|account| account.address.as_deref().and_then(domain_of)).collect()
}

/// A name as DNS compares it: no space around, no final dot, lower case.
fn lower(name: &str) -> String {
    name.trim().trim_end_matches('.').to_ascii_lowercase()
}

/// Whether `name` is `parent` or a name under it, by whole labels.
fn under(name: &str, parent: &str) -> bool {
    name == parent || name.strip_suffix(parent).is_some_and(|rest| rest.ends_with('.'))
}

/// Whether a name can be a host's, as discovery asks about or offers one:
/// ASCII letters, digits, hyphens and dots, with a dot inside, its last
/// label not all digits (no IP address). Anything else is never asked
/// about nor offered.
fn is_host_name(name: &str) -> bool {
    name.contains('.')
        && !name.starts_with(['.', '-'])
        && !name.ends_with(['.', '-'])
        && !name.contains("..")
        && name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'.')
        && name.rsplit('.').next().is_some_and(|last| !last.bytes().all(|b| b.is_ascii_digit()))
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
/// valid for its name: said as read in records nothing signs, to check.
/// Its sending server is found at the first message sent. The login is the
/// address.
fn from_records(search: &Search, domain: &str, address: &str) -> Option<Found> {
    let imap = preferred(search.srv(&format!("_imaps._tcp.{domain}.")))?;
    search.certified(&imap.target, imap.port, Probe::Tls).then(|| Found { host: imap.target, port: imap.port, security: Security::Tls, username: address.to_string(), by: FoundBy::Records, smtp: None })
}

/// The sending server the domain's own records name, checked:
/// `_submissions._tcp` (RFC 8314), TLS from the first byte; else
/// `_submission._tcp` (RFC 6186), with STARTTLS, unless it names port 465,
/// where TLS starts at the first byte (some hosts write it so).
fn sending_from_records(search: &Search, domain: &str) -> Option<Smtp> {
    if let Some(service) = preferred(search.srv(&format!("_submissions._tcp.{domain}."))).filter(|s| search.certified(&s.target, s.port, Probe::Tls)) {
        return Some(Smtp { host: service.target, port: service.port, security: Security::Tls });
    }
    let service = preferred(search.srv(&format!("_submission._tcp.{domain}.")))?;
    let security = if service.port == 465 { Security::Tls } else { Security::Starttls };
    search.certified(&service.target, service.port, Probe::sending(security)).then(|| Smtp { host: service.target, port: service.port, security })
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
    is_google_mail_with(address, |domain| mx_hosts(domain, DNS))
}

/// `is_google_mail`, the MX asked of `mx`.
pub(crate) fn is_google_mail_with(address: &str, mx: impl Fn(&str) -> Option<Vec<String>>) -> bool {
    let Some(domain) = domain_of(address.trim()) else { return false };
    is_google_domain(&domain) || mx(&domain).is_some_and(|hosts| hosts.iter().any(|h| is_google_mx(h)))
}

/// The hosts that receive a domain's mail (its MX records), asked of the
/// system's DNS as the sender checks do (on Android, the network's servers,
/// which hickory reads through ConnectivityManager); None when it cannot be
/// asked or does not answer within `limit`.
fn mx_hosts(domain: &str, limit: Duration) -> Option<Vec<String>> {
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().ok()?;
    runtime.block_on(async {
        let resolver = mail_auth::MessageAuthenticator::new_system_conf().ok()?;
        let none = None::<&mail_auth::common::cache::NoCache<Box<str>, mail_auth::RecordSet<mail_auth::MX>>>;
        let found = tokio::time::timeout(limit, resolver.mx_lookup(domain, none)).await.ok()?.ok()?;
        Some(found.rrset.iter().flat_map(|mx| mx.exchanges.iter().map(|host| host.trim_end_matches('.').to_string())).collect())
    })
}

/// The SRV records of a name, asked of the system's DNS as `mx_hosts` asks
/// it (the same resolver, on every system Sioul builds for); none when it
/// cannot be asked, does not answer within `limit`, or has none.
fn srv_records(name: &str, limit: Duration) -> Vec<Service> {
    use mail_auth::hickory_resolver::proto::rr::{Name, RData};
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread().enable_all().build() else { return Vec::new() };
    runtime.block_on(async {
        let Ok(resolver) = mail_auth::MessageAuthenticator::new_system_conf() else { return Vec::new() };
        let Ok(name) = Name::from_str_relaxed(name) else { return Vec::new() };
        let Ok(Ok(found)) = tokio::time::timeout(limit, resolver.resolver().srv_lookup(name)).await else { return Vec::new() };
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

/// A settings file, within `limit`, the body read included.
fn fetch(agent: &ureq::Agent, url: &str, limit: Duration) -> Option<String> {
    agent.get(url).config().timeout_global(Some(limit)).build().call().ok()?.body_mut().read_to_string().ok()
}

/// `imap.<domain>`, then `mail.<domain>`, on port 993: a guess, offered only
/// when the host shows a certificate valid for that very name.
fn guess(search: &Search, domain: &str, address: &str) -> Option<Found> {
    [format!("imap.{domain}"), format!("mail.{domain}")].into_iter().find(|host| search.certified(host, 993, Probe::Tls)).map(|host| Found {
        host,
        port: 993,
        security: Security::Tls,
        username: address.to_string(),
        by: FoundBy::Guess,
        smtp: None,
    })
}

/// Whether the server reached at `at` (a host's name and a port) shows a
/// certificate valid for `name`, checked as a sign-in checks it (`config`:
/// `imap::tls_config`, the system's certificates; the tests give their
/// own), reached as `probe` says. Nothing is sent after the handshake. An
/// alias of a host's server (a CNAME whose certificate names the host's
/// server, not the alias) fails here, as its sign-in would. All of it
/// within `limit`, the name's lookup included; the first address that
/// connects decides.
fn shows_its_own_certificate(name: &str, at: (&str, u16), probe: Probe, config: Arc<rustls::ClientConfig>, limit: Duration) -> bool {
    let Ok(server) = ServerName::try_from(name.to_string()) else { return false };
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread().enable_all().build() else { return false };
    let check = async {
        for address in tokio::net::lookup_host(at).await.ok()? {
            let Ok(mut tcp) = TcpStream::connect(address).await else { continue };
            match probe {
                Probe::Tls => {}
                Probe::Imap => tcp = crate::imap::starttls(tcp).await.ok()?,
                Probe::Smtp => smtp_starttls(&mut tcp).await?,
            }
            return Some(tokio_rustls::TlsConnector::from(config).connect(server, tcp).await.is_ok());
        }
        None
    };
    runtime.block_on(async { tokio::time::timeout(limit, check).await.ok().flatten().unwrap_or(false) })
}

/// SMTP's STARTTLS, up to where TLS begins (RFC 3207): the greeting, EHLO
/// (an address literal, as sending gives it), STARTTLS, its 220. None when
/// the server says anything else.
async fn smtp_starttls(tcp: &mut TcpStream) -> Option<()> {
    (smtp_reply(tcp).await? == 220).then_some(())?;
    tcp.write_all(b"EHLO [127.0.0.1]\r\n").await.ok()?;
    (smtp_reply(tcp).await? == 250).then_some(())?;
    tcp.write_all(b"STARTTLS\r\n").await.ok()?;
    (smtp_reply(tcp).await? == 220).then_some(())
}

/// The code of an SMTP reply, its lines read to the last ("250-…", then
/// "250 …"), a byte at a time so that nothing meant for TLS is swallowed.
async fn smtp_reply(tcp: &mut TcpStream) -> Option<u16> {
    let mut read = 0usize;
    loop {
        let mut line = Vec::new();
        loop {
            let byte = tcp.read_u8().await.ok()?;
            read += 1;
            if byte == b'\n' || read > 8192 {
                break;
            }
            line.push(byte);
        }
        let line = String::from_utf8_lossy(&line);
        let code = line.get(..3)?.parse().ok()?;
        if line.as_bytes().get(3) != Some(&b'-') {
            return Some(code);
        }
    }
}

/// An address as discovery reads it: no space around, no final dot after its domain.
fn tidy_address(address: &str) -> &str {
    let address = address.trim();
    address.strip_suffix('.').unwrap_or(address)
}

/// An address's domain as DNS and Thunderbird's list read it: lower case,
/// without a final dot, in ASCII (a domain in Unicode in its `xn--` form,
/// by IDNA); None when the address has none, or its domain is no name a
/// host can have (`is_host_name`: an IP address is not).
pub(crate) fn domain_of(address: &str) -> Option<String> {
    let (local, domain) = tidy_address(address).rsplit_once('@')?;
    if local.is_empty() || domain.contains(['/', ' ', '?', '#', '@', ':', '[']) {
        return None;
    }
    let ascii = if domain.is_ascii() {
        domain.to_ascii_lowercase()
    } else {
        lower(&mail_auth::hickory_resolver::proto::rr::Name::from_utf8(domain).ok()?.to_ascii())
    };
    is_host_name(&ascii).then_some(ascii)
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
/// `domain`: the address's, in its ASCII form, which host names take.
pub(crate) fn parse_config(xml: &str, address: &str, domain: &str, by: FoundBy) -> Option<Found> {
    let servers: Vec<Found> = blocks(xml, "incomingServer")
        .filter(|(attributes, _)| attributes.contains("\"imap\"") || attributes.contains("'imap'"))
        .filter_map(|(_, body)| server(body, address, domain, by))
        .collect();
    let outgoing: Vec<Found> = blocks(xml, "outgoingServer")
        .filter(|(attributes, _)| attributes.contains("\"smtp\"") || attributes.contains("'smtp'"))
        .filter_map(|(_, body)| server(body, address, domain, by))
        .collect();
    let smtp = outgoing.iter().find(|s| s.security == Security::Tls).or(outgoing.first()).map(|s| Smtp { host: s.host.clone(), port: s.port, security: s.security });
    let mut found = servers.iter().find(|s| s.security == Security::Tls).or(servers.first()).cloned()?;
    found.smtp = smtp;
    Some(found)
}

/// The SMTP server of an account whose receiving server is `imap_host`,
/// found at its first message sent, when nobody reads the answer before a
/// password follows it. In this order:
///
/// 1. the domain's own settings, then Thunderbird's list by the domain (HTTPS);
/// 2. Google's or Microsoft's sending server, for an account whose receiving
///    server is theirs, never for another;
/// 3. the account's receiving server itself, the one its password goes to
///    already, on 465 (TLS from the first byte) then 587 (STARTTLS);
/// 4. Thunderbird's list by the names of whoever receives the domain's mail,
///    then the domain's own records, then a guess, `smtp.<domain>` on 465
///    then 587.
///
/// From 3 on, a server is taken only when it shows a certificate valid for
/// its very name; at 4, what unsigned DNS led to only when it is at the
/// keeper of the receiving server (`same_keeper`).
pub fn discover_smtp(address: &str, imap_host: Option<&str>) -> Result<Smtp, SyncError> {
    let own = own_domains();
    with_the_network(|ask| discover_smtp_with(address, imap_host, ask, &own))
}

/// `discover_smtp`, the outside asked through `ask`.
pub(crate) fn discover_smtp_with(address: &str, imap_host: Option<&str>, ask: &Ask, own: &[String]) -> Result<Smtp, SyncError> {
    let address = tidy_address(address);
    let domain = domain_of(address).ok_or(SyncError::BadAddress)?;
    let receiving = imap_host.map(lower).filter(|host| is_host_name(host));
    // Gmail's and Outlook.com's own domains: known here, nothing asked.
    if is_google_domain(&domain) || is_microsoft_consumer_domain(&domain) {
        return discover_with(address, ask, own)?.smtp.ok_or(SyncError::NotFound(domain));
    }
    let search = Search::new(ask, &domain, own);
    if let Some(smtp) = search.by_the_domain(address, &domain).find_map(|found| found.smtp) {
        return Ok(smtp);
    }
    // Google's and Microsoft's own accounts: their own sending servers.
    match receiving.as_deref() {
        Some(GOOGLE_IMAP) => return Ok(Smtp { host: GOOGLE_SMTP.into(), port: 465, security: Security::Tls }),
        Some(host) if crate::microsoft::is_imap_host(host) => {
            let kind = if is_microsoft_consumer_domain(&domain) { MicrosoftKind::Personal } else { search.mx(&domain).iter().find_map(|h| microsoft_mx_kind(h)).unwrap_or(MicrosoftKind::Work) };
            return microsoft_found(address, kind).smtp.ok_or(SyncError::NoServer);
        }
        _ => {}
    }
    // The receiving server, where the password goes already.
    if let Some(host) = &receiving {
        for (port, security) in [(465, Security::Tls), (587, Security::Starttls)] {
            if search.certified(host, port, Probe::sending(security)) {
                return Ok(Smtp { host: host.clone(), port, security });
            }
        }
    }
    // What unsigned DNS led to, at the receiving server's keeper alone.
    let kept = |smtp: &Smtp| same_keeper(&smtp.host, receiving.as_deref(), &domain);
    // Google's and Microsoft's exchangers lead to their own accounts alone (above).
    let exchangers: Vec<String> = search.mx(&domain).into_iter().filter(|h| !is_google_mx(h) && microsoft_mx_kind(h).is_none()).collect();
    let hosted = search.by_the_exchangers(&exchangers, address, &domain).filter_map(|found| found.smtp).find(|smtp| kept(smtp) && search.certified(&smtp.host, smtp.port, Probe::sending(smtp.security)));
    if let Some(smtp) = hosted.or_else(|| sending_from_records(&search, &domain).filter(kept)) {
        return Ok(smtp);
    }
    let guessed = format!("smtp.{domain}");
    [(465, Security::Tls), (587, Security::Starttls)]
        .into_iter()
        .find(|(port, security)| search.certified(&guessed, *port, Probe::sending(*security)))
        .map(|(port, security)| Smtp { host: guessed.clone(), port, security })
        .ok_or_else(|| search.nothing(domain))
}

fn server(body: &str, address: &str, domain: &str, by: FoundBy) -> Option<Found> {
    let security = match tag(body, "socketType")? {
        "SSL" => Security::Tls,
        "STARTTLS" => Security::Starttls,
        _ => return None,
    };
    let (local, typed) = address.rsplit_once('@').unwrap_or((address, ""));
    let host = lower(&substitute(tag(body, "hostname")?, address, local, domain));
    Some(Found {
        host: is_host_name(&host).then_some(host)?,
        port: tag(body, "port")?.parse().ok()?,
        security,
        username: substitute(tag(body, "username").unwrap_or("%EMAILADDRESS%"), address, local, typed),
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
fn substitute(template: &str, address: &str, local: &str, domain: &str) -> String {
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
        let found = parse_config(CPANEL, "someone@example.com", "example.com", FoundBy::Provider).unwrap();
        assert_eq!((found.host.as_str(), found.port, found.security), ("mail.example.com", 993, Security::Tls));
        assert_eq!(found.username, "someone@example.com");
        assert_eq!(found.smtp, Some(Smtp { host: "mail.example.com".into(), port: 465, security: Security::Tls }));
    }

    #[test]
    fn fills_placeholders_and_skips_plain() {
        let found = parse_config(ISPDB, "jane@example.net", "example.net", FoundBy::Ispdb).unwrap();
        assert_eq!((found.host.as_str(), found.port, found.security), ("imap.example.net", 143, Security::Starttls));
        assert_eq!(found.username, "jane");
        assert_eq!(parse_config("<clientConfig/>", "a@example.org", "example.org", FoundBy::Ispdb), None);
        // Host names take the domain's ASCII form, the login the address as typed;
        // a host name no host can have is no server.
        let unicode = parse_config(ISPDB, "jane@exämple.net", "xn--exmple-cua.net", FoundBy::Ispdb).unwrap();
        assert_eq!((unicode.host.as_str(), unicode.username.as_str()), ("imap.xn--exmple-cua.net", "jane"));
        let odd = "<clientConfig><emailProvider><incomingServer type=\"imap\"><hostname>192.0.2.1</hostname><port>993</port><socketType>SSL</socketType></incomingServer></emailProvider></clientConfig>";
        assert_eq!(parse_config(odd, "a@example.org", "example.org", FoundBy::Ispdb), None);
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
            "contoso.example",
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
        let found = parse_config(YAHOO, "jane@example.org", "example.org", FoundBy::Ispdb).unwrap();
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
        // A final dot, a domain in Unicode (in its IDNA form), IP addresses: refused or made ASCII.
        assert_eq!(domain_of(" jane@Example.org. ").as_deref(), Some("example.org"));
        assert_eq!(domain_of("jane@Exämple.org").as_deref(), Some("xn--exmple-cua.org"));
        for odd in ["jane@192.0.2.1", "jane@[192.0.2.1]", "jane@[IPv6:2001:db8::1]", "jane@example..org", "jane@-example.org", "jane@exa_mple.org", "@example.org"] {
            assert_eq!(domain_of(odd), None, "{odd}");
        }
        for (name, host) in [("mx.example.org", true), ("192.0.2.1", false), ("2001:db8::1", false), ("localhost", false), ("example.org.", false), ("xn--exmple-cua.org", true), ("exämple.org", false), ("a.b-c.d1", true)] {
            assert_eq!(is_host_name(name), host, "{name}");
        }
    }

    /// The outside, stood in for: the settings files that exist, the MX and
    /// SRV records, the hosts that show a certificate valid for their name;
    /// the person's other domains; and what was asked, in order. Slow, every
    /// question takes all the time it is given and answers nothing, on a
    /// clock of the test's own. No test asks the network.
    #[derive(Default)]
    struct Outside {
        pages: Vec<(&'static str, &'static str)>,
        mx: Vec<(&'static str, Vec<&'static str>)>,
        srv: Vec<(&'static str, Vec<(u16, u16, u16, &'static str)>)>,
        certified: Vec<(&'static str, u16)>,
        own: Vec<String>,
        slow: bool,
        clock: std::cell::Cell<Duration>,
        asked: std::cell::RefCell<Vec<String>>,
    }

    impl Outside {
        fn with<T>(&self, run: impl FnOnce(&Ask) -> T) -> T {
            self.clock.set(Duration::ZERO);
            let note = |line: String, limit: Duration| {
                self.asked.borrow_mut().push(line);
                if self.slow {
                    self.clock.set(self.clock.get() + limit);
                }
                !self.slow
            };
            let fetch = |url: &str, limit: Duration| {
                note(format!("GET {url}"), limit).then(|| self.pages.iter().find(|(known, _)| *known == url).map(|(_, xml)| xml.to_string())).flatten()
            };
            let mx = |domain: &str, limit: Duration| {
                note(format!("MX {domain}"), limit).then(|| self.mx.iter().find(|(known, _)| *known == domain).map(|(_, hosts)| hosts.iter().map(|h| h.to_string()).collect())).flatten()
            };
            let srv = |name: &str, limit: Duration| {
                let answers = note(format!("SRV {name}"), limit);
                let records = self.srv.iter().find(|(known, _)| answers && *known == name).map(|(_, records)| records.clone()).unwrap_or_default();
                records.into_iter().map(|(priority, weight, port, target)| Service { priority, weight, port, target: target.to_string() }).collect()
            };
            let certified = |host: &str, port: u16, probe: Probe, limit: Duration| {
                let way = match probe {
                    Probe::Tls => "TLS",
                    Probe::Imap => "IMAP",
                    Probe::Smtp => "SMTP",
                };
                note(format!("{way} {host}:{port}"), limit) && self.certified.iter().any(|(known, at)| *known == host && *at == port)
            };
            run(&Ask { fetch: &fetch, mx: &mx, srv: &srv, certified: &certified, elapsed: &|| self.clock.get() })
        }

        fn discover(&self, address: &str) -> Result<Found, SyncError> {
            self.with(|ask| discover_with(address, ask, &self.own))
        }

        fn discover_smtp(&self, address: &str, imap_host: Option<&str>) -> Result<Smtp, SyncError> {
            self.with(|ask| discover_smtp_with(address, imap_host, ask, &self.own))
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
    const LIST_OVH: &str = "https://autoconfig.thunderbird.net/v1.1/ovh.net";

    /// The order of discovery, each step asked only when those before found
    /// nothing: the domain's own HTTPS settings, Thunderbird's list by the
    /// domain, Google and Microsoft by the MX, the list by the MX's names
    /// (the longest first), the checked guess, then the domain's records.
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
                "TLS imap.example.org:993".to_string(),
                "TLS mail.example.org:993".to_string(),
                "SRV _imaps._tcp.example.org.".to_string(),
            ]
        );
        // Each step that finds ends the search there; what HTTPS gave is offered as it is.
        let own = Outside { pages: vec![(OWN_SETTINGS, CPANEL)], ..Outside::default() };
        assert_eq!(own.discover("jane@example.org").unwrap().by, FoundBy::Provider);
        assert_eq!(own.asked(), [format!("GET {OWN_SETTINGS}")]);
        let listed = Outside { pages: vec![(LIST_BY_DOMAIN, CPANEL)], ..Outside::default() };
        assert_eq!(listed.discover("jane@example.org").unwrap().by, FoundBy::Ispdb);
        assert_eq!(listed.asked().len(), 3, "no DNS asked, no certificate checked");
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
    /// alone: never the address, never one of the person's domains. What
    /// the entry names is offered only when it shows its own certificate,
    /// said as something to check; its sending server is left to the first
    /// message sent.
    #[test]
    fn a_domain_kept_at_a_host_is_found_by_the_host_s_entry() {
        let outside = Outside {
            pages: vec![(LIST_OVH, OVH)],
            mx: vec![("example.org", vec!["mx1.mail.ovh.net.", "mx2.mail.ovh.net.", "mx3.mail.ovh.net."])],
            // The aliases answer on their ports, with a certificate for the host's server: no guess.
            certified: vec![("ssl0.ovh.net", 993), ("ssl0.ovh.net", 465)],
            ..Outside::default()
        };
        let found = outside.discover(" jane@example.org ").unwrap();
        assert_eq!((found.host.as_str(), found.port, found.security, found.by), ("ssl0.ovh.net", 993, Security::Tls, FoundBy::IspdbMx));
        assert_eq!(found.username, "jane@example.org", "the whole address, as the list says");
        assert_eq!(found.smtp, None, "found, and checked, at the first message sent");
        assert_eq!(FoundBy::IspdbMx.message_id(), "account-by-ispdb-mx");
        let asked = outside.asked();
        assert_eq!(asked[3..], ["MX example.org", "GET https://autoconfig.thunderbird.net/v1.1/mail.ovh.net", "GET https://autoconfig.thunderbird.net/v1.1/ovh.net", "TLS ssl0.ovh.net:993"]);
        assert!(asked[4..].iter().all(|line| !line.contains("example.org") && !line.contains("jane")), "{asked:?}");
        // An entry written with the address's domain (`imap.%EMAILDOMAIN%`)
        // gives the alias again: not offered, since it shows no certificate of its own.
        let aliased = "<clientConfig><emailProvider><incomingServer type=\"imap\"><hostname>imap.%EMAILDOMAIN%</hostname><port>993</port><socketType>SSL</socketType></incomingServer></emailProvider></clientConfig>";
        let alias = Outside { pages: vec![(LIST_OVH, aliased)], mx: vec![("example.org", vec!["mx1.mail.ovh.net."])], ..Outside::default() };
        assert_eq!(alias.discover("jane@example.org"), Err(SyncError::NotFound("example.org".into())));
        assert_eq!(alias.asked().iter().filter(|line| *line == "TLS imap.example.org:993").count(), 1, "checked once, the guess included");
        // The names the list is asked about: above each exchanger, the longest
        // first, down to the registered name, never a public suffix alone.
        let own = |domains: &[&str]| domains.iter().map(|d| d.to_string()).collect::<Vec<_>>();
        let names = |hosts: &[&str]| provider_names(&own(hosts), &own(&["example.org"]));
        assert_eq!(names(&["mx1.mail.ovh.net.", "MX2.Mail.OVH.net"]), ["mail.ovh.net", "ovh.net"]);
        assert_eq!(names(&["mx.example.co.uk"]), ["example.co.uk"], "never co.uk");
        assert_eq!(names(&["host.example"]), ["host.example"], "a registered name receiving the mail itself");
        assert_eq!(names(&["a.b.c.host.example", "mx.other.example"]), ["b.c.host.example", "c.host.example", "host.example", "other.example"]);
        assert_eq!(names(&["a.b.c.d.e.host.example"]).len(), PROVIDER_NAMES, "a few names at most");
        // The address's own servers, and those of the person's other domains.
        assert!(names(&["mail.example.org", "mx.in.example.org", "example.org"]).is_empty());
        assert!(provider_names(&own(&["mx.jane-family.example"]), &own(&["example.org", "jane-family.example"])).is_empty());
        // Names no host has, and public suffixes alone, are never asked about.
        assert!(names(&["co.uk", "uk", "localhost", "mx_1.host.example", "mx.host.example/v1.1/x", "192.0.2.1", "mx.exämple.example", ""]).is_empty());
    }

    /// What Thunderbird's list and the DNS are told, in the edge cases: an
    /// address typed with a final dot, a domain in Unicode (asked in its
    /// IDNA form, and its own servers never told to the list), a mail
    /// exchanger under another of the person's domains, IP addresses.
    #[test]
    fn tells_the_list_provider_names_alone() {
        let dotted = Outside::default();
        let _ = dotted.discover("jane@example.org.");
        let asked = dotted.asked();
        assert_eq!(asked[..3], [format!("GET {OWN_SETTINGS}"), format!("GET {WELL_KNOWN}"), format!("GET {LIST_BY_DOMAIN}")], "{asked:?}");
        assert!(asked.contains(&"SRV _imaps._tcp.example.org.".to_string()), "{asked:?}");
        let unicode = Outside { mx: vec![("xn--exmple-cua.org", vec!["mx.xn--exmple-cua.org.", "mx1.mail.host.example."])], ..Outside::default() };
        let _ = unicode.discover("jane@Exämple.org");
        let asked = unicode.asked();
        assert!(asked.contains(&"GET https://autoconfig.thunderbird.net/v1.1/xn--exmple-cua.org".to_string()) && asked.contains(&"MX xn--exmple-cua.org".to_string()), "{asked:?}");
        assert_eq!(asked.iter().filter(|line| line.starts_with("GET https://autoconfig.thunderbird.net")).count(), 3, "the domain once, then the host's two names: {asked:?}");
        let family = Outside { mx: vec![("example.org", vec!["mx.jane-family.example."])], own: vec!["jane-family.example".into()], ..Outside::default() };
        let _ = family.discover("jane@example.org");
        assert!(family.asked().iter().all(|line| !line.contains("v1.1/jane-family.example")), "another of the person's domains");
        let numbers = Outside { mx: vec![("example.org", vec!["192.0.2.1"])], srv: vec![("_imaps._tcp.example.org.", vec![(0, 0, 993, "192.0.2.1")])], ..Outside::default() };
        assert_eq!(numbers.discover("jane@example.org"), Err(SyncError::NotFound("example.org".into())));
        let asked = numbers.asked();
        assert!(asked.iter().all(|line| !line.contains("192.0.2") && !line.contains("v1.1/0.2.1") && !line.contains("v1.1/2.1")), "{asked:?}");
        assert_eq!(Outside::default().discover("jane@192.0.2.1"), Err(SyncError::BadAddress));
    }

    /// The domain's own SRV records (RFC 6186, RFC 8314), asked after the
    /// guess: a server read there is offered only when it shows a
    /// certificate valid for its name, said as read in records nothing
    /// signs. A forged record cannot win over an `imap.<domain>` that shows
    /// its own certificate.
    #[test]
    fn reads_the_domain_s_own_records_after_the_guess() {
        let records = Outside {
            srv: vec![("_imaps._tcp.example.org.", vec![(10, 0, 993, "backup.host.example."), (0, 1, 993, "ssl0.host.example."), (0, 5, 993, "SSL1.host.example.")])],
            certified: vec![("ssl1.host.example", 993)],
            ..Outside::default()
        };
        let found = records.discover("jane@example.org").unwrap();
        assert_eq!((found.host.as_str(), found.port, found.security, found.username.as_str(), found.by), ("ssl1.host.example", 993, Security::Tls, "jane@example.org", FoundBy::Records), "the lowest priority, then the highest weight");
        assert_eq!(found.smtp, None);
        assert_eq!(FoundBy::Records.message_id(), "account-by-records");
        assert_eq!(records.asked()[4..], ["TLS imap.example.org:993", "TLS mail.example.org:993", "SRV _imaps._tcp.example.org.", "TLS ssl1.host.example:993"]);
        // A forged record, its server holding a valid certificate of its own: the guess wins.
        let forged = Outside {
            srv: vec![("_imaps._tcp.example.org.", vec![(0, 0, 993, "imap.attacker.example.")])],
            certified: vec![("imap.attacker.example", 993), ("imap.example.org", 993)],
            ..Outside::default()
        };
        let found = forged.discover("jane@example.org").unwrap();
        assert_eq!((found.host.as_str(), found.by), ("imap.example.org", FoundBy::Guess));
        assert!(forged.asked().iter().all(|line| !line.starts_with("SRV")), "the records are not asked once the guess passed");
        // A record whose server shows no certificate for its name is not offered; "." says no service.
        let unchecked = Outside { srv: vec![("_imaps._tcp.example.org.", vec![(0, 0, 993, "alias.host.example.")])], ..Outside::default() };
        assert_eq!(unchecked.discover("jane@example.org"), Err(SyncError::NotFound("example.org".into())));
        assert_eq!(preferred(vec![Service { priority: 0, weight: 0, port: 993, target: "bad name.example".into() }, Service { priority: 0, weight: 0, port: 0, target: "host.example".into() }, Service { priority: 0, weight: 0, port: 993, target: ".".into() }]), None);
    }

    /// The sending server, found at the first message sent: the domain's
    /// HTTPS settings, Google's or Microsoft's for their own accounts
    /// alone, the account's receiving server (465, then 587), then the list
    /// by the MX's names, the records and the guess, each checked, what
    /// unsigned DNS led to only at the receiving server's keeper. The
    /// receiving server's guesses are never made here.
    #[test]
    fn finds_the_sending_server_in_its_order() {
        let tls = |host: &str, port| Smtp { host: host.into(), port, security: Security::Tls };
        let starttls = |host: &str, port| Smtp { host: host.into(), port, security: Security::Starttls };
        // The domain's own settings first, as they are.
        let own = Outside { pages: vec![(OWN_SETTINGS, CPANEL)], ..Outside::default() };
        assert_eq!(own.discover_smtp("jane@example.org", Some("ssl0.host.example")), Ok(tls("mail.example.com", 465)));
        assert_eq!(own.asked(), [format!("GET {OWN_SETTINGS}")]);
        // Google's sending server for an account at Google's receiving server, nothing more asked.
        let gmail = Outside::default();
        assert_eq!(gmail.discover_smtp("jane@example.org", Some("imap.gmail.com")), Ok(tls("smtp.gmail.com", 465)));
        assert_eq!(gmail.asked().len(), 3, "the domain's HTTPS settings alone");
        // A forged MX naming Google's or Microsoft's servers is never followed for another account.
        for exchanger in ["aspmx.l.google.com.", "example-org.mail.protection.outlook.com."] {
            let forged = Outside { mx: vec![("example.org", vec![exchanger])], certified: vec![("smtp.gmail.com", 465), ("smtp.office365.com", 587)], ..Outside::default() };
            assert_eq!(forged.discover_smtp("jane@example.org", Some("ssl0.host.example")), Err(SyncError::NotFound("example.org".into())), "{exchanger}");
            assert!(forged.asked().iter().all(|line| !line.contains("gmail") && !line.contains("office365")));
        }
        // The receiving server, on 465 then 587, before anything unsigned DNS leads to:
        // a mailbox on one offer of a host whose MX it shares with another offer.
        let pro = Outside {
            pages: vec![(LIST_OVH, OVH)],
            mx: vec![("example.org", vec!["mx1.mail.ovh.net."])],
            certified: vec![("pro1.mail.ovh.net", 587), ("ssl0.ovh.net", 465)],
            ..Outside::default()
        };
        assert_eq!(pro.discover_smtp("jane@example.org", Some("pro1.mail.ovh.net")), Ok(starttls("pro1.mail.ovh.net", 587)));
        assert_eq!(pro.asked()[3..], ["TLS pro1.mail.ovh.net:465", "SMTP pro1.mail.ovh.net:587"]);
        // The receiving server takes no mail to send: the host's entry, checked, at its keeper.
        let hosted = Outside { pages: vec![(LIST_OVH, OVH)], mx: vec![("example.org", vec!["mx1.mail.ovh.net."])], certified: vec![("ssl0.ovh.net", 465)], ..Outside::default() };
        assert_eq!(hosted.discover_smtp("jane@example.org", Some("pro1.mail.ovh.net")), Ok(tls("ssl0.ovh.net", 465)));
        let asked = hosted.asked();
        assert!(!asked.iter().any(|line| line.contains(":993")), "no receiving guess: {asked:?}");
        // The records, checked, at the keeper of the receiving server or under the domain.
        let records = Outside { srv: vec![("_submission._tcp.example.org.", vec![(0, 0, 587, "smtp.host.example.")])], certified: vec![("smtp.host.example", 587)], ..Outside::default() };
        assert_eq!(records.discover_smtp("jane@example.org", Some("imap.host.example")), Ok(starttls("smtp.host.example", 587)));
        let hosts_465 = Outside { srv: vec![("_submission._tcp.example.org.", vec![(0, 0, 465, "ssl0.host.example.")])], certified: vec![("ssl0.host.example", 465)], ..Outside::default() };
        assert_eq!(hosts_465.discover_smtp("jane@example.org", Some("imap.host.example")), Ok(tls("ssl0.host.example", 465)), "465 in a submission record: TLS from the first byte");
        let unchecked = Outside { srv: vec![("_submission._tcp.example.org.", vec![(0, 0, 587, "smtp.host.example.")])], ..Outside::default() };
        assert_eq!(unchecked.discover_smtp("jane@example.org", Some("imap.host.example")), Err(SyncError::NotFound("example.org".into())), "no certificate of its own: not taken");
        // Forged answers, each server holding a valid certificate for its own name: not followed.
        let forged = Outside {
            pages: vec![(LIST_OVH, OVH)],
            mx: vec![("example.org", vec!["mx1.mail.ovh.net."])],
            srv: vec![("_submissions._tcp.example.org.", vec![(0, 0, 465, "smtp.elsewhere.example.")])],
            certified: vec![("ssl0.ovh.net", 465), ("smtp.elsewhere.example", 465)],
            ..Outside::default()
        };
        assert_eq!(forged.discover_smtp("jane@example.org", Some("imap.host.example")), Err(SyncError::NotFound("example.org".into())));
        // The guess last: smtp.<domain> with a certificate of its own, on 465 then 587.
        let guessed = Outside { certified: vec![("smtp.example.org", 587)], ..Outside::default() };
        assert_eq!(guessed.discover_smtp("jane@example.org", Some("imap.host.example")), Ok(starttls("smtp.example.org", 587)));
        let alias = Outside { certified: vec![("ssl0.host.example", 993)], ..Outside::default() };
        assert_eq!(alias.discover_smtp("jane@example.org", Some("imap.host.example")), Err(SyncError::NotFound("example.org".into())), "an alias shows no certificate of its own");
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

    /// The worst case: every question takes all the time it is given, and
    /// nothing answers. A whole search, for the receiving server or the
    /// sending one, ends within `WHOLE` (under 30 seconds), on the test's
    /// own clock; each question is given what is left at most. A host and
    /// port is checked once in a search.
    #[test]
    fn a_whole_search_ends_within_its_time() {
        assert!(WHOLE < Duration::from_secs(30));
        let slow = Outside { mx: vec![("example.org", vec!["mx1.mail.host.example."])], slow: true, ..Outside::default() };
        // Said as the network's slowness, not as nothing found.
        assert_eq!(slow.discover("jane@example.org"), Err(SyncError::Network("no answer within 28 s".into())));
        assert!(slow.clock.get() <= WHOLE, "{:?}", slow.clock.get());
        let asked = slow.asked();
        assert_eq!(asked[..3], [format!("GET {OWN_SETTINGS}"), format!("GET {WELL_KNOWN}"), format!("GET {LIST_BY_DOMAIN}")], "{asked:?}");
        assert!(asked.len() < 9, "the later steps are not started once the time is spent: {asked:?}");
        assert_eq!(slow.discover_smtp("jane@example.org", Some("ssl0.host.example")), Err(SyncError::Network("no answer within 28 s".into())));
        assert!(slow.clock.get() <= WHOLE, "{:?}", slow.clock.get());
        // The steps' own limits: a settings file 8 s at most, a check 5 s, a DNS question 4 s.
        assert!(HTTP <= Duration::from_secs(8) && PROBE <= Duration::from_secs(5) && DNS <= Duration::from_secs(4));
        // A host and port checked once in a search: the receiving server
        // on 465, then the same server named by the host's entry.
        let once = Outside { pages: vec![(LIST_OVH, OVH)], mx: vec![("example.org", vec!["mx1.mail.ovh.net."])], ..Outside::default() };
        let _ = once.discover_smtp("jane@example.org", Some("ssl0.ovh.net"));
        let asked = once.asked();
        assert_eq!(asked.iter().filter(|line| *line == "TLS ssl0.ovh.net:465").count(), 1, "{asked:?}");
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
    /// this test alone, from the first byte, after IMAP's STARTTLS and after
    /// SMTP's. Asked by that name, it passes; asked by an alias
    /// (`imap.example.org`, the same server behind another name), it does
    /// not, though the certificate is otherwise valid; nor does a server
    /// that answers without TLS, nor a port where nothing listens. A server
    /// that never answers ends the check at its limit.
    #[test]
    fn the_certificate_check_takes_the_name_asked_alone() {
        use base64::Engine;
        use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
        use std::io::{BufRead, BufReader, Read, Write};
        let decode = |text: &str| base64::engine::general_purpose::STANDARD.decode(text).unwrap();
        let certificate = CertificateDer::from(decode(STAND_IN_CERTIFICATE));
        let provider = || Arc::new(rustls::crypto::ring::default_provider());
        let mut roots = rustls::RootCertStore::empty();
        roots.add(certificate.clone()).unwrap();
        let trusting = Arc::new(rustls::ClientConfig::builder_with_provider(provider()).with_safe_default_protocol_versions().unwrap().with_root_certificates(roots).with_no_client_auth());
        let serving = Arc::new(
            rustls::ServerConfig::builder_with_provider(provider())
                .with_safe_default_protocol_versions()
                .unwrap()
                .with_no_client_auth()
                .with_single_cert(vec![certificate], PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(decode(STAND_IN_KEY))))
                .unwrap(),
        );
        let limit = Duration::from_secs(5);
        // A stand-in for each way: it speaks its protocol's words in clear,
        // then TLS, for two visits; nothing is said after the handshake.
        for (probe, before) in [(Probe::Tls, Vec::<(&str, &str)>::new()), (Probe::Imap, vec![("", "* OK stand-in IMAP\r\n"), ("s1 STARTTLS", "s1 OK Begin TLS\r\n")]), (Probe::Smtp, vec![("", "220 stand-in ESMTP\r\n"), ("EHLO [127.0.0.1]", "250-stand-in\r\n250-STARTTLS\r\n250 8BITMIME\r\n"), ("STARTTLS", "220 Go ahead\r\n")])] {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            let serving = serving.clone();
            let server = std::thread::spawn(move || {
                let mut handshakes = Vec::new();
                for _ in 0..2 {
                    let (mut stream, _) = listener.accept().unwrap();
                    let mut reader = BufReader::new(stream.try_clone().unwrap());
                    for (heard, answer) in &before {
                        if !heard.is_empty() {
                            let mut line = String::new();
                            reader.read_line(&mut line).unwrap();
                            assert_eq!(line.trim_end(), *heard);
                        }
                        stream.write_all(answer.as_bytes()).unwrap();
                    }
                    let mut tls = rustls::ServerConnection::new(serving.clone()).unwrap();
                    let mut done = true;
                    while tls.is_handshaking() {
                        if tls.complete_io(&mut stream).is_err() {
                            done = false;
                            break;
                        }
                    }
                    handshakes.push(done);
                }
                handshakes
            });
            assert!(shows_its_own_certificate("ssl0.stand-in.example", ("127.0.0.1", port), probe, trusting.clone(), limit), "{probe:?}: its own name");
            assert!(!shows_its_own_certificate("imap.example.org", ("127.0.0.1", port), probe, trusting.clone(), limit), "{probe:?}: an alias, the certificate naming the host's server");
            assert_eq!(server.join().unwrap(), [true, false], "{probe:?}: the second visit ended at the certificate");
        }
        // A server that answers without TLS; a port where nothing listens.
        let plain = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let plain_port = plain.local_addr().unwrap().port();
        let greeter = std::thread::spawn(move || {
            let (mut stream, _) = plain.accept().unwrap();
            let _ = stream.write_all(b"* OK stand-in, without TLS\r\n");
            let _ = stream.read(&mut [0; 512]);
        });
        assert!(!shows_its_own_certificate("ssl0.stand-in.example", ("127.0.0.1", plain_port), Probe::Tls, trusting.clone(), limit), "it answers, without TLS");
        greeter.join().unwrap();
        let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        assert!(!shows_its_own_certificate("ssl0.stand-in.example", ("127.0.0.1", closed), Probe::Tls, trusting.clone(), limit), "nothing listens");
        assert!(!shows_its_own_certificate("not a name", ("127.0.0.1", closed), Probe::Tls, crate::imap::tls_config(), limit));
        // A server that takes the connection and never answers: the check ends at its limit.
        let silent = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let silent_port = silent.local_addr().unwrap().port();
        let began = Instant::now();
        assert!(!shows_its_own_certificate("ssl0.stand-in.example", ("127.0.0.1", silent_port), Probe::Smtp, trusting, Duration::from_millis(300)));
        assert!(began.elapsed() < Duration::from_secs(2), "{:?}", began.elapsed());
        drop(silent);
    }
}
