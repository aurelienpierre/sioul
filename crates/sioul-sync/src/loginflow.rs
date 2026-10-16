// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Signing in to a Nextcloud through its own page, in the browser: its Login
//! Flow v2 (docs/database.md, "Joining through your Nextcloud's sign-in"). A
//! device with nothing set up yet gets an app password of its own, made by
//! the server, without a password typed into Sioul; with it, the sharing
//! folder is found there and joined, and the contacts and calendars come.
//!
//! - **Begun** (`begin`): `POST <server>/index.php/login/v2`, signed in as
//!   nobody; the server answers the page to open (`login`) and where to ask
//!   for the result (`poll`: an address, and a token).
//! - **The page** opens in the system's browser: you sign in there as on the
//!   server's own site, its second step included, and grant Sioul access.
//! - **Asked** (`wait`): the poll address asked every three seconds, the
//!   token in the request's body, until it answers the server, your login
//!   name and an app password; stopped at Cancel, after twenty minutes (the
//!   server's own limit for a sign-in), or when it answers.
//! - **Kept to the server you chose**: HTTPS only; the page, the poll address
//!   and the server the answer names, each on the origin you chose (scheme,
//!   host and port); one elsewhere is refused, and nothing is kept. Never
//!   redirected.
//! - **Secret**: the poll token and the app password are never written to a
//!   file or a log, never shown by `Debug`, and go to that origin only. The
//!   window keeps the app password in the keyring (`secret`), as every
//!   account's password.
//! - **Given back** (`revoke`): an app password not used after all (Cancel
//!   pressed after it came) is deleted on the server
//!   (`DELETE /ocs/v2.php/core/apppassword`, Nextcloud's own way).
//!
//! Murena's server is `MURENA`: its sign-in answers there itself (16 October
//! 2026: the page and the poll address on `https://murena.io`, without
//! `index.php`). Any other Nextcloud by its address.

use crate::dav;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use zeroize::Zeroizing;

/// Murena's Nextcloud (the cloud of /e/OS).
pub const MURENA: &str = "https://murena.io";

/// How long a sign-in may take: the server forgets it after twenty minutes.
pub const LIMIT: Duration = Duration::from_secs(20 * 60);

/// How often the poll address is asked while you sign in.
pub const EVERY: Duration = Duration::from_secs(3);

/// Polls in a row that may fail (the network down a moment, the server busy)
/// before the sign-in says so: half a minute at `EVERY`.
const FAILURES: u32 = 10;

/// An answer is never read past this.
const ANSWER: u64 = 64 << 10;

/// A secret held in memory (a sign-in's page, an app password): zeroed when dropped.
pub type Secret = Zeroizing<String>;

/// A secret held in memory from `text`.
pub fn secret(text: &str) -> Secret {
    Zeroizing::new(text.to_string())
}

/// Why a sign-in did not go, for the window's words, each with the server's name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// What was typed is not a server's address.
    Address,
    /// The address is not on HTTPS: nothing was asked.
    NotHttps(String),
    /// The server could not be reached (the network, a timeout).
    Unreachable(String),
    /// Its certificate could not be checked: nothing was sent.
    Tls(String),
    /// No Nextcloud answered there (its sign-in not as Nextcloud makes it).
    NotNextcloud(String),
    /// The server named another server (its page, its poll address, or the
    /// server its answer names): refused, nothing kept. The chosen one's
    /// name, then the other's.
    Elsewhere(String, String),
    /// The server answered with an error.
    Server(String),
    /// Twenty minutes went by without the sign-in.
    TimedOut,
    /// Cancel was pressed.
    Cancelled,
}

impl Refusal {
    /// Its code for the window: "unreachable:murena.io", "elsewhere:murena.io:evil.example".
    pub fn code(&self) -> String {
        match self {
            Refusal::Address => "address".into(),
            Refusal::NotHttps(host) => format!("not-https:{host}"),
            Refusal::Unreachable(host) => format!("unreachable:{host}"),
            Refusal::Tls(host) => format!("tls:{host}"),
            Refusal::NotNextcloud(host) => format!("not-nextcloud:{host}"),
            Refusal::Elsewhere(chosen, named) => format!("elsewhere:{chosen}:{named}"),
            Refusal::Server(host) => format!("server:{host}"),
            Refusal::TimedOut => "timed-out".into(),
            Refusal::Cancelled => "cancelled".into(),
        }
    }
}

/// The server's address as typed, made whole: `cloud.example.org` →
/// `https://cloud.example.org`; `https://cloud.example.org/nextcloud/` →
/// `https://cloud.example.org/nextcloud`; the server's own pages after it
/// (`/index.php/…`, `/login…`, `/apps/files/…`) left out. A plain
/// `http://` is refused: the app password would travel unencrypted.
pub fn address_of(typed: &str) -> Result<String, Refusal> {
    let typed = typed.trim();
    let (scheme, rest) = match typed.split_once("://") {
        Some((scheme, rest)) => (scheme.to_ascii_lowercase(), rest),
        None => ("https".to_string(), typed),
    };
    let rest = rest.split(['?', '#']).next().unwrap_or("");
    let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
    if authority.is_empty() || authority.contains('@') || authority.chars().any(|c| c.is_whitespace() || c.is_control()) || authority.starts_with(':') {
        return Err(Refusal::Address);
    }
    let authority = authority.to_ascii_lowercase();
    let mut kept: Vec<&str> = Vec::new();
    for part in path.split('/').filter(|p| !p.is_empty()) {
        if matches!(part, "index.php" | "login" | "apps" | "remote.php" | "ocs" | "status.php" | "settings") {
            break;
        }
        kept.push(part);
    }
    let base = if kept.is_empty() { format!("{scheme}://{authority}") } else { format!("{scheme}://{authority}/{}", kept.join("/")) };
    if dav::allowed(&base).is_err() {
        return Err(Refusal::NotHttps(crate::remote::host_of(&base)));
    }
    if dav::origin(&base).is_none() {
        return Err(Refusal::Address);
    }
    Ok(base)
}

/// A sign-in under way: the page to open, and where to ask for its result.
/// Its token and the page's address are secrets: no `Debug` shows them.
pub struct Flow {
    /// The server chosen, as `address_of` made it.
    base: String,
    origin: String,
    login: Zeroizing<String>,
    endpoint: String,
    token: Zeroizing<String>,
    /// What Sioul says it is on the server's page and in its list of
    /// devices ("Sioul (Android)").
    client: String,
}

impl std::fmt::Debug for Flow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Flow").field("base", &self.base).field("login", &"…").field("token", &"…").finish()
    }
}

impl Flow {
    /// The page to open in the browser (its address holds the sign-in's own key: never logged).
    pub fn page(&self) -> &str {
        &self.login
    }

    /// The server's name: "murena.io".
    pub fn host(&self) -> String {
        crate::remote::host_of(&self.base)
    }
}

/// What the sign-in gave: the server, as it named itself (on the origin
/// chosen), the login name, and the app password. No `Debug` shows the password.
pub struct Grant {
    pub server: String,
    pub login: String,
    password: Zeroizing<String>,
}

impl std::fmt::Debug for Grant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Grant").field("server", &self.server).field("password", &"…").finish()
    }
}

impl Grant {
    /// For the requests to its server only, and the keyring.
    pub fn password(&self) -> &str {
        &self.password
    }

    /// The server's name: "murena.io".
    pub fn host(&self) -> String {
        crate::remote::host_of(&self.server)
    }

    /// The account's login to its server, with the app password, for the
    /// sharing's own requests (`remote`): its files there, under `remote.php/dav/`.
    pub fn login_for(&self, account: &str) -> crate::remote::Login {
        crate::remote::Login { account: account.to_string(), url: format!("{}/remote.php/dav/", self.server), user: self.login.clone(), password: Some(self.password.to_string()) }
    }

    /// A grant from its parts: the window's, holding one between the
    /// sign-in and the joining, and the tests'.
    pub fn of(server: &str, login: &str, password: &str) -> Grant {
        Grant { server: server.trim_end_matches('/').to_string(), login: login.to_string(), password: Zeroizing::new(password.to_string()) }
    }
}

/// The agent for the sign-in's requests: never redirected, each bounded.
fn agent() -> ureq::Agent {
    dav::agent(&dav::Budget { connect: Duration::from_secs(10), stall: Duration::from_secs(15), whole: Duration::from_secs(30) })
}

fn refused(e: &ureq::Error, host: &str) -> Refusal {
    match e {
        ureq::Error::Tls(_) => Refusal::Tls(host.to_string()),
        _ if format!("{e:?}").starts_with("Rustls") => Refusal::Tls(host.to_string()),
        _ => Refusal::Unreachable(host.to_string()),
    }
}

/// A request's body, read up to `ANSWER`, zeroed once dropped (it may hold a secret).
fn read(response: &mut ureq::http::Response<ureq::Body>) -> Zeroizing<String> {
    Zeroizing::new(response.body_mut().with_config().limit(ANSWER).read_to_string().unwrap_or_default())
}

/// Whether an address is on `origin`, over HTTPS (a test's own stand-in apart: `dav::allowed`).
fn on(url: &str, origin: &str) -> bool {
    dav::allowed(url).is_ok() && dav::origin(url).as_deref() == Some(origin)
}

#[derive(serde::Deserialize)]
struct Begun {
    login: String,
    poll: Poll,
}

#[derive(serde::Deserialize)]
struct Poll {
    token: String,
    endpoint: String,
}

#[derive(serde::Deserialize)]
struct Answered {
    server: String,
    #[serde(rename = "loginName")]
    login_name: String,
    #[serde(rename = "appPassword")]
    app_password: String,
}

/// A sign-in begun on `base` (`address_of`'s), Sioul saying it is `client`
/// there ("Sioul (Android)"): the page to open, and where to ask. Its page
/// and poll address both on `base`'s origin, over HTTPS, or it is refused.
pub fn begin(base: &str, client: &str) -> Result<Flow, Refusal> {
    let host = crate::remote::host_of(base);
    let origin = dav::origin(base).ok_or(Refusal::Address)?;
    let url = format!("{}/index.php/login/v2", base.trim_end_matches('/'));
    dav::allowed(&url).map_err(|_| Refusal::NotHttps(host.clone()))?;
    let request = ureq::http::Request::builder().method("POST").uri(&url).header("User-Agent", client).header("Accept", "application/json").body(Vec::new()).map_err(|_| Refusal::Address)?;
    let mut response = agent().run(request).map_err(|e| refused(&e, &host))?;
    let status = response.status().as_u16();
    let body = read(&mut response);
    match status {
        200 => {}
        404 | 405 => return Err(Refusal::NotNextcloud(host)),
        _ => return Err(Refusal::Server(host)),
    }
    let begun: Begun = serde_json::from_str(&body).map_err(|_| Refusal::NotNextcloud(host.clone()))?;
    let (login, token) = (Zeroizing::new(begun.login), Zeroizing::new(begun.poll.token));
    for named in [login.as_str(), begun.poll.endpoint.as_str()] {
        if !on(named, &origin) {
            return Err(Refusal::Elsewhere(host, crate::remote::host_of(named)));
        }
    }
    if token.is_empty() || token.len() > 1024 || !token.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(Refusal::NotNextcloud(host));
    }
    Ok(Flow { base: base.trim_end_matches('/').to_string(), origin, login, endpoint: begun.poll.endpoint, token, client: client.to_string() })
}

/// Asks once whether the sign-in is done: none while it is not (the server
/// answers 404 until you granted access); the grant when it is, its server
/// on the origin chosen (another is refused: nothing is kept). The server
/// answers it once only.
pub fn poll(flow: &Flow) -> Result<Option<Grant>, Refusal> {
    let host = crate::remote::host_of(&flow.base);
    let body = Zeroizing::new(format!("token={}", flow.token.as_str()).into_bytes());
    let request = ureq::http::Request::builder()
        .method("POST")
        .uri(&flow.endpoint)
        .header("User-Agent", flow.client.as_str())
        .header("Accept", "application/json")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body.to_vec())
        .map_err(|_| Refusal::Server(host.clone()))?;
    let mut response = agent().run(request).map_err(|e| refused(&e, &host))?;
    let status = response.status().as_u16();
    let answer = read(&mut response);
    match status {
        200 => {}
        404 => return Ok(None),
        _ => return Err(Refusal::Server(host)),
    }
    let answered: Answered = serde_json::from_str(&answer).map_err(|_| Refusal::NotNextcloud(host.clone()))?;
    let password = Zeroizing::new(answered.app_password);
    let server = answered.server.trim().trim_end_matches('/').to_string();
    if !on(&server, &flow.origin) {
        // Made by the server chosen all the same: given back to it, never kept.
        let named = crate::remote::host_of(&server);
        let _ = revoke(&Grant { server: flow.base.clone(), login: answered.login_name, password });
        return Err(Refusal::Elsewhere(host, named));
    }
    if answered.login_name.trim().is_empty() || password.is_empty() {
        return Err(Refusal::NotNextcloud(host));
    }
    Ok(Some(Grant { server, login: answered.login_name.trim().to_string(), password }))
}

/// Asks every `every` until the sign-in is done, `stop` is set (Cancel), or
/// `limit` went by. The network down a moment is asked again; for half a
/// minute in a row, it is said.
pub fn wait(flow: &Flow, stop: &AtomicBool, limit: Duration, every: Duration) -> Result<Grant, Refusal> {
    let end = Instant::now() + limit;
    let mut failures = 0;
    loop {
        if stop.load(Ordering::SeqCst) {
            return Err(Refusal::Cancelled);
        }
        match poll(flow) {
            Ok(Some(grant)) => {
                // Cancel pressed while the answer came: given back.
                if stop.load(Ordering::SeqCst) {
                    let _ = revoke(&grant);
                    return Err(Refusal::Cancelled);
                }
                return Ok(grant);
            }
            Ok(None) => failures = 0,
            Err(e @ (Refusal::Elsewhere(..) | Refusal::Tls(_) | Refusal::NotNextcloud(_))) => return Err(e),
            Err(e) => {
                failures += 1;
                if failures >= FAILURES {
                    return Err(e);
                }
            }
        }
        let next = Instant::now() + every;
        while Instant::now() < next {
            if stop.load(Ordering::SeqCst) {
                return Err(Refusal::Cancelled);
            }
            if Instant::now() >= end {
                return Err(Refusal::TimedOut);
            }
            std::thread::sleep(Duration::from_millis(100).min(every));
        }
        if Instant::now() >= end {
            return Err(Refusal::TimedOut);
        }
    }
}

/// An app password given back: deleted on its server, which made it (Cancel
/// pressed once it came, or a grant refused). Already gone there: done.
pub fn revoke(grant: &Grant) -> Result<(), Refusal> {
    let host = grant.host();
    let url = format!("{}/ocs/v2.php/core/apppassword", grant.server);
    dav::allowed(&url).map_err(|_| Refusal::NotHttps(host.clone()))?;
    let authorization = Zeroizing::new(format!("Basic {}", sioul_core::lines::base64_encode(format!("{}:{}", grant.login, grant.password()).as_bytes())));
    let request = ureq::http::Request::builder()
        .method("DELETE")
        .uri(&url)
        .header("Authorization", authorization.as_str())
        .header("OCS-APIREQUEST", "true")
        .header("User-Agent", "Sioul")
        .body(Vec::new())
        .map_err(|_| Refusal::Server(host.clone()))?;
    let response = agent().run(request).map_err(|e| refused(&e, &host))?;
    match response.status().as_u16() {
        200 | 204 | 401 | 403 | 404 => Ok(()),
        _ => Err(Refusal::Server(host)),
    }
}

/// The keyring's name for a sign-in's app password, kept for the sharing
/// (`remote::State::user`): "Nextcloud app password: jane on murena.io",
/// under the service `sioul` as every secret of Sioul's (`secret`).
pub fn keyring_name(login: &str, host: &str) -> String {
    format!("Nextcloud app password: {login} on {host}")
}

/// The grant's app password kept in the keyring (`keyring_name`), replacing
/// one kept there before for that login and server. Never in a file.
pub fn keep(grant: &Grant) -> Result<(), crate::SyncError> {
    crate::secret::save_named(&keyring_name(&grant.login, &grant.host()), grant.password())
}

/// The app password kept for that login and server, from the keyring.
pub fn kept(login: &str, host: &str) -> Option<String> {
    crate::secret::named(&keyring_name(login, host))
}

/// The app password kept for that login and server, forgotten here.
pub fn forget(login: &str, host: &str) -> Result<(), crate::SyncError> {
    crate::secret::forget_named(&keyring_name(login, host))
}

/// Where the contacts and calendars are on the grant's server, found there
/// alone, with its app password (`dav::discover_on`).
pub fn homes(grant: &Grant) -> Result<dav::Homes, crate::SyncError> {
    dav::discover_on(&dav::Client::new(&grant.login, grant.password()), &grant.server)
}

/// Whether the app password opens the account under `login` there (one
/// account's login may differ from the sign-in's own: its address rather
/// than its name): a principal asked of its server, on its origin alone.
pub fn opens(grant: &Grant, login: &str) -> Result<bool, crate::SyncError> {
    let client = dav::Client::new(login, grant.password());
    match dav::discover_on(&client, &grant.server) {
        Ok(_) => Ok(true),
        Err(crate::SyncError::Login(_)) => Ok(false),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_address_typed_is_made_whole_and_kept_to_https() {
        assert_eq!(address_of("murena.io"), Ok("https://murena.io".to_string()));
        assert_eq!(address_of(" https://Cloud.Example.org/nextcloud/ "), Ok("https://cloud.example.org/nextcloud".to_string()));
        assert_eq!(address_of("https://cloud.example.org/index.php/apps/files/?dir=/"), Ok("https://cloud.example.org".to_string()));
        assert_eq!(address_of("cloud.example.org:8443/nc/login"), Ok("https://cloud.example.org:8443/nc".to_string()));
        assert_eq!(address_of("http://cloud.example.org"), Err(Refusal::NotHttps("cloud.example.org".into())));
        for typed in ["", "   ", "https://", "jane@cloud.example.org", "https://jane:secret@cloud.example.org", "cloud example.org"] {
            assert_eq!(address_of(typed), Err(Refusal::Address), "{typed:?}");
        }
    }

    #[test]
    fn no_debug_shows_a_secret() {
        let grant = Grant::of("https://cloud.example.org", "jane", "app-password-secret");
        let said = format!("{grant:?}");
        assert!(!said.contains("app-password-secret") && said.contains("cloud.example.org"), "{said}");
        let flow = Flow { base: "https://cloud.example.org".into(), origin: "https://cloud.example.org:443".into(), login: Zeroizing::new("https://cloud.example.org/login/v2/flow/page-secret".into()), endpoint: "https://cloud.example.org/login/v2/poll".into(), token: Zeroizing::new("token-secret".into()), client: "Sioul".into() };
        let said = format!("{flow:?}");
        assert!(!said.contains("page-secret") && !said.contains("token-secret"), "{said}");
        assert_eq!(Refusal::Elsewhere("murena.io".into(), "evil.example".into()).code(), "elsewhere:murena.io:evil.example");
    }

    use crate::remote::fake::{APP_PASSWORD, Fake, USER};
    use std::sync::Arc;

    /// A stand-in Nextcloud with its sign-in, its files in a scratch folder.
    fn stand_in(name: &str) -> (Arc<Fake>, std::path::PathBuf) {
        let files = std::env::temp_dir().join(format!("sioul-loginflow-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&files);
        std::fs::create_dir_all(&files).unwrap();
        (Fake::start(&files), files)
    }

    #[test]
    fn a_sign_in_answered_gives_an_app_password_once_for_its_server_alone() {
        let (fake, files) = stand_in("answered");
        let base = address_of(&fake.base).unwrap();
        let flow = begin(&base, "Sioul (test)").unwrap();
        assert!(flow.page().starts_with(&format!("{base}/login/v2/flow/")), "the page on the server chosen");
        // Not signed in yet: nothing.
        assert!(poll(&flow).unwrap().is_none());
        fake.grant_flows();
        let grant = poll(&flow).unwrap().expect("signed in");
        assert_eq!((grant.server.as_str(), grant.login.as_str()), (base.as_str(), USER));
        assert!(grant.password().starts_with(APP_PASSWORD));
        // Answered once only, as Nextcloud does.
        assert!(poll(&flow).unwrap().is_none());
        // The poll's token went in the request's body, never in an address.
        assert!(fake.seen().iter().all(|line| !line.contains("polltoken")), "{:?}", fake.seen());
        // The contacts and calendars found there, with the app password.
        let homes = homes(&grant).unwrap();
        assert_eq!(homes.calendars.as_deref(), Some(format!("{base}/remote.php/dav/calendars/{USER}/").as_str()));
        assert_eq!(homes.contacts.as_deref(), Some(format!("{base}/remote.php/dav/addressbooks/users/{USER}/").as_str()));
        assert!(opens(&grant, USER).unwrap());
        assert!(!opens(&grant, "someone-else").unwrap(), "another login with it is refused there");
        // Waited for: the person signs in a moment later.
        let flow = begin(&base, "Sioul (test)").unwrap();
        let granting = Arc::clone(&fake);
        let signed = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            granting.grant_flows();
        });
        let stop = AtomicBool::new(false);
        let second = wait(&flow, &stop, Duration::from_secs(20), Duration::from_millis(50)).unwrap();
        signed.join().unwrap();
        assert_ne!(second.password(), grant.password(), "an app password of its own");
        // Given back: it opens nothing any more.
        revoke(&second).unwrap();
        assert!(!fake.app_passwords.lock().unwrap().iter().any(|p| p == second.password()));
        assert!(matches!(homes_of(&second), Err(crate::SyncError::Login(_))));
        let _ = std::fs::remove_dir_all(files);
    }

    fn homes_of(grant: &Grant) -> Result<crate::dav::Homes, crate::SyncError> {
        homes(grant)
    }

    #[test]
    fn a_sign_in_cancelled_or_left_twenty_minutes_keeps_nothing() {
        let (fake, files) = stand_in("cancelled");
        let base = address_of(&fake.base).unwrap();
        // Cancel pressed while the browser is open.
        let flow = begin(&base, "Sioul (test)").unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let pressing = Arc::clone(&stop);
        let pressed = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(200));
            pressing.store(true, Ordering::SeqCst);
        });
        let started = Instant::now();
        assert_eq!(wait(&flow, &stop, Duration::from_secs(20), Duration::from_millis(50)).unwrap_err(), Refusal::Cancelled);
        pressed.join().unwrap();
        assert!(started.elapsed() < Duration::from_secs(5), "stopped at once");
        // Granted, but Cancel pressed first: never asked for, so never made.
        fake.grant_flows();
        assert_eq!(wait(&flow, &stop, Duration::from_secs(20), Duration::from_millis(50)).unwrap_err(), Refusal::Cancelled);
        assert!(fake.app_passwords.lock().unwrap().is_empty());
        // Never granted: over at the limit (twenty minutes in Sioul, a moment here).
        let flow = begin(&base, "Sioul (test)").unwrap();
        let stop = AtomicBool::new(false);
        assert_eq!(wait(&flow, &stop, Duration::from_millis(300), Duration::from_millis(50)).unwrap_err(), Refusal::TimedOut);
        assert!(fake.app_passwords.lock().unwrap().is_empty());
        assert_eq!(LIMIT, Duration::from_secs(20 * 60));
        let _ = std::fs::remove_dir_all(files);
    }

    #[test]
    fn a_server_named_elsewhere_is_refused_and_nothing_kept() {
        let (fake, files) = stand_in("elsewhere");
        let (other, other_files) = stand_in("elsewhere-other");
        let base = address_of(&fake.base).unwrap();
        // Its answer names another server: refused, the app password given back to the one that made it.
        fake.signing.lock().unwrap().server = other.base.clone();
        let flow = begin(&base, "Sioul (test)").unwrap();
        fake.grant_flows();
        let refused = poll(&flow).unwrap_err();
        assert_eq!(refused, Refusal::Elsewhere(crate::remote::host_of(&base), crate::remote::host_of(&other.base)));
        assert!(fake.app_passwords.lock().unwrap().is_empty(), "given back");
        assert!(other.seen().is_empty(), "nothing asked of the other server");
        // On HTTPS there, refused all the same: another origin.
        fake.signing.lock().unwrap().server = "https://cloud.example.org".into();
        let flow = begin(&base, "Sioul (test)").unwrap();
        fake.grant_flows();
        assert!(matches!(poll(&flow), Err(Refusal::Elsewhere(_, named)) if named == "cloud.example.org"));
        // Its poll address on another server: refused before anything is asked there.
        fake.signing.lock().unwrap().server.clear();
        fake.signing.lock().unwrap().endpoint = format!("{}/login/v2/poll", other.base);
        assert!(matches!(begin(&base, "Sioul (test)"), Err(Refusal::Elsewhere(_, _))));
        assert!(other.seen().is_empty());
        // No Nextcloud there: said so.
        let none = crate::dav::stand_in::serve(Arc::new(Nothing));
        assert_eq!(begin(&none, "Sioul (test)").unwrap_err(), Refusal::NotNextcloud(crate::remote::host_of(&none)));
        // Plain HTTP elsewhere than a test's own stand-in: never asked.
        assert_eq!(begin("http://cloud.example.org", "Sioul (test)").unwrap_err(), Refusal::NotHttps("cloud.example.org".into()));
        let _ = std::fs::remove_dir_all(files);
        let _ = std::fs::remove_dir_all(other_files);
    }

    /// A web server that is no Nextcloud: 404 everywhere.
    struct Nothing;
    impl crate::dav::stand_in::Server for Nothing {
        fn misbehaves(&self, _: &crate::dav::stand_in::Request) -> Option<crate::dav::stand_in::Fault> {
            None
        }
        fn answer(&self, _: &crate::dav::stand_in::Request) -> crate::dav::stand_in::Reply {
            crate::dav::stand_in::Reply::new(404, "")
        }
    }

    /// A server that names its principal on another one: the app password
    /// never goes there, nor to your login's domain.
    #[test]
    fn the_app_password_never_leaves_its_server() {
        use crate::dav::stand_in::{Reply, Request, Server as StandIn, serve};
        struct Noted(std::sync::Mutex<Vec<String>>);
        impl StandIn for Noted {
            fn misbehaves(&self, _: &Request) -> Option<crate::dav::stand_in::Fault> {
                None
            }
            fn answer(&self, request: &Request) -> Reply {
                self.0.lock().unwrap().push(format!("{} {}", request.method, request.path));
                Reply::new(404, "")
            }
        }
        struct Pointing(String);
        impl StandIn for Pointing {
            fn misbehaves(&self, _: &Request) -> Option<crate::dav::stand_in::Fault> {
                None
            }
            fn answer(&self, _: &Request) -> Reply {
                Reply::new(207, format!(r#"<?xml version="1.0" encoding="utf-8"?><d:multistatus xmlns:d="DAV:"><d:response><d:href>/remote.php/dav/</d:href><d:propstat><d:prop><d:current-user-principal><d:href>{}</d:href></d:current-user-principal></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response></d:multistatus>"#, self.0))
            }
        }
        let noted = Arc::new(Noted(std::sync::Mutex::new(Vec::new())));
        let there = serve(noted.clone());
        let here = serve(Arc::new(Pointing(format!("{there}/remote.php/dav/principals/users/jane/"))));
        // The login an address of another domain: its domain is never asked either.
        let grant = Grant::of(&here, "jane@elsewhere.example", "app-password");
        assert!(homes(&grant).is_err());
        assert!(noted.0.lock().unwrap().is_empty(), "{:?}", noted.0.lock().unwrap());
    }
}
