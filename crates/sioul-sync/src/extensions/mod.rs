// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Extensions, the side that talks to the world (docs/extensions.md):
//!
//! - [`engine`]: an extension's file run in a sandbox, given only what its
//!   manifest grants, ended calmly at any limit;
//! - **what an extension may ask**, done for it ([`Real`]): a web request
//!   (`https`, the hosts its manifest names, public addresses only, no
//!   redirect followed, its answer's status, 4 MiB once unpacked, 30
//!   seconds), its own secret in the system keyring, a sign-in page shown by
//!   the window (on a computer or a phone), the time;
//! - **the store**: the manifest and its signature read ([`read_manifest`]),
//!   a file downloaded and kept only once its SHA-256 matched the signed
//!   manifest ([`install`]), and checked again before each run
//!   ([`checked_file`]).

pub mod engine;

use engine::{Limits, Outcome, Services, WebAnswer, WebRequest};
use sioul_core::extensions::manifest::{self, Manifest, Offer};
use sioul_core::extensions::{signature, state};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// What Sioul calls itself to the hosts an extension asks, unless the
/// extension says otherwise (W1): its name and version, nothing of the device
/// or the person; an extension of the repository names itself the same way.
const AGENT: &str = concat!("Sioul/", env!("CARGO_PKG_VERSION"));
/// The most a manifest, its signature and an extension's file weigh.
const MANIFEST_BYTES: usize = 1 << 20;
const SIGNATURE_BYTES: usize = 64 << 10;
const FILE_BYTES: usize = 8 << 20;

// ---------------------------------------------------------------- what an extension may ask

/// A sign-in page to show, and how long the person has.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignInPage {
    pub open: String,
    pub watch: String,
    /// The site's name, shown above the page.
    pub host: String,
    pub wait: Duration,
}

/// Who shows a sign-in page: the window, on a computer or a phone. The
/// address returned (beginning with `watch`), none when the person closed
/// it, or a refusal's word.
pub type SignInShower = Box<dyn FnMut(&SignInPage) -> Result<Option<String>, &'static str> + Send>;

/// The asks done for real: the network through the resolver that refuses
/// private addresses, the keyring under the extension's own name, the
/// window's sign-in page.
pub struct Real {
    id: String,
    shower: Option<SignInShower>,
    /// Whether this device's record says when the extension is removed, and when it was last read.
    watched: bool,
    looked: Option<std::time::Instant>,
    gone: bool,
}

impl Real {
    /// For the extension `id` (from its manifest); `shower` shows a sign-in page, when one may be shown.
    pub fn new(id: &str, shower: Option<SignInShower>) -> Self {
        Self { id: id.to_string(), shower, watched: false, looked: None, gone: false }
    }

    /// Its call ends once this device's record no longer holds the extension,
    /// or marks it as leaving (Remove, in this process or another): read
    /// once a second at most.
    pub fn removable(mut self) -> Self {
        self.watched = true;
        self
    }
}

/// A ureq error as one of the interface's words.
fn said(error: &ureq::Error) -> &'static str {
    match error {
        ureq::Error::Timeout(_) => "time",
        ureq::Error::BodyExceedsLimit(_) => "size",
        ureq::Error::Io(io) if io.to_string().contains("not a public address") => "address",
        _ => "network",
    }
}

impl Services for Real {
    fn web(&mut self, request: &WebRequest, wait: Duration, most: usize) -> Result<WebAnswer, &'static str> {
        // No proxy, whatever the environment says (W2): with one, the resolver would check
        // the proxy's address, never the host asked, and a public proxy would reach any.
        let config = ureq::Agent::config_builder().https_only(true).max_redirects(0).http_status_as_error(false).timeout_global(Some(wait)).user_agent(AGENT).proxy(None).build();
        let agent = crate::stalls::public_agent(config);
        let answer = if request.method == "POST" {
            let mut asked = agent.post(&request.url);
            for (name, value) in &request.headers {
                asked = asked.header(name.as_str(), value.as_str());
            }
            asked.send(request.body.as_deref().unwrap_or(""))
        } else {
            let mut asked = agent.get(&request.url);
            for (name, value) in &request.headers {
                asked = asked.header(name.as_str(), value.as_str());
            }
            asked.call()
        };
        let mut answer = answer.map_err(|e| said(&e))?;
        let status = answer.status().as_u16();
        let headers = answer.headers().iter().map(|(name, value)| (name.as_str().to_string(), String::from_utf8_lossy(value.as_bytes()).into_owned())).collect();
        // Counted once unpacked: a small compressed answer cannot fill the memory.
        let mut body = Vec::new();
        let most = most as u64;
        answer.body_mut().with_config().limit(most + 1).reader().take(most + 1).read_to_end(&mut body).map_err(|e| if e.to_string().contains("limit") { "size" } else { "network" })?;
        if body.len() as u64 > most {
            return Err("size");
        }
        Ok(WebAnswer { status, headers, body: String::from_utf8_lossy(&body).into_owned() })
    }

    // Kept as bytes (UTF-8 as it is): 2,400 bytes fit every system's keyring,
    // where a text takes two bytes a character on Windows (K3).
    fn secret(&mut self) -> Result<Option<String>, &'static str> {
        match crate::secret::named_bytes(&sioul_core::extensions::secret_name(&self.id)) {
            Ok(Some(bytes)) => String::from_utf8(bytes).map(Some).map_err(|_| "keyring"),
            Ok(None) => Ok(None),
            Err(_) => Err("keyring"),
        }
    }

    fn keep_secret(&mut self, secret: &str) -> Result<(), &'static str> {
        // Never written back once Remove began, whatever the last look said.
        self.looked = None;
        if self.removed() {
            return Err("not-now");
        }
        crate::secret::save_named_bytes(&sioul_core::extensions::secret_name(&self.id), secret.as_bytes()).map_err(|_| "keyring")
    }

    fn forget_secret(&mut self) -> Result<(), &'static str> {
        forget_secret(&self.id).map_err(|_| "keyring")
    }

    fn sign_in(&mut self, open: &str, watch: &str, host: &str, wait: Duration) -> Result<Option<String>, &'static str> {
        let shower = self.shower.as_mut().ok_or("not-now")?;
        shower(&SignInPage { open: open.to_string(), watch: watch.to_string(), host: host.to_string(), wait })
    }

    fn now(&self) -> i64 {
        jiff::Timestamp::now().as_second()
    }

    fn removed(&mut self) -> bool {
        if !self.watched || self.gone {
            return self.gone;
        }
        if self.looked.is_some_and(|at| at.elapsed() < Duration::from_secs(1)) {
            return false;
        }
        self.looked = Some(std::time::Instant::now());
        self.gone = state::load(&state::path()).get(&self.id).is_none_or(|i| i.leaving);
        self.gone
    }
}

/// Forgets an extension's secret (its removal, or its sign-in's).
pub fn forget_secret(id: &str) -> Result<(), crate::SyncError> {
    crate::secret::forget_named(&sioul_core::extensions::secret_name(id))
}

// ---------------------------------------------------------------- one call

/// A call Sioul makes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Call<'a> {
    /// The person pressed Sign in…, with what they chose if they were asked.
    SignIn(Option<&'a str>),
    /// Sioul wants the extension's data now.
    Refresh,
}

/// A call of the interface, as JSON, in Sioul's language now.
pub fn call_text(call: Call<'_>, french: bool) -> String {
    let lang = if french { "fr" } else { "en" };
    let text = match call {
        Call::SignIn(choice) => serde_json::json!({ "interface": sioul_core::extensions::INTERFACE, "call": "sign-in", "lang": lang, "choice": choice }),
        Call::Refresh => serde_json::json!({ "interface": sioul_core::extensions::INTERFACE, "call": "refresh", "lang": lang }),
    };
    text.to_string()
}

/// Runs one call of the extension `offer` (as accepted): its file checked
/// against its hash, then run with what its manifest grants. A sign-in page
/// may be shown only on a `sign-in` call. Never on the window's thread.
pub fn run(offer: &Offer, call: Call<'_>, french: bool, services: Real) -> (Outcome, Real) {
    let file = match checked_file(offer) {
        Ok(file) => file,
        Err(why) => return (Outcome { reply: Err(engine::Stop::NotRead(why)), log: Vec::new(), held: 0 }, services),
    };
    let may_sign_in = matches!(call, Call::SignIn(_));
    engine::call(&file, &offer.grants(), &call_text(call, french), services, &Limits::default(), may_sign_in)
}

// ---------------------------------------------------------------- the store

/// A file named by a local address: `file://…`, or an absolute path.
fn local_path(address: &str) -> Option<PathBuf> {
    if let Some(path) = address.strip_prefix("file://") {
        return Some(PathBuf::from(path));
    }
    let path = Path::new(address);
    path.is_absolute().then(|| path.to_path_buf())
}

/// What an address holds, `most` bytes at most: a file on this device, or
/// an `https` address (public addresses only, five redirects within https,
/// a minute).
fn fetch(address: &str, most: usize) -> Result<Vec<u8>, String> {
    if let Some(path) = local_path(address) {
        let size = std::fs::metadata(&path).map_err(|e| format!("{}: {e}", path.display()))?.len();
        if size > most as u64 {
            return Err(format!("{}: larger than {most} bytes", path.display()));
        }
        return std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()));
    }
    if !address.starts_with("https://") {
        return Err("not an https address".into());
    }
    let config = ureq::Agent::config_builder().https_only(true).max_redirects(5).http_status_as_error(false).timeout_global(Some(Duration::from_secs(60))).user_agent(AGENT).proxy(None).build();
    let mut answer = crate::stalls::public_agent(config).get(address).call().map_err(|e| format!("{address}: {e}"))?;
    if answer.status().as_u16() != 200 {
        return Err(format!("{address}: the server answered {}", answer.status().as_u16()));
    }
    let mut bytes = Vec::new();
    let most = most as u64;
    answer.body_mut().with_config().limit(most + 1).reader().take(most + 1).read_to_end(&mut bytes).map_err(|e| format!("{address}: {e}"))?;
    if bytes.len() as u64 > most {
        return Err(format!("{address}: larger than {most} bytes"));
    }
    Ok(bytes)
}

/// The manifest at `address` and its signature beside it (`.sig`), read
/// only when the signature holds with the publisher's key compiled in. The
/// serial is the caller's to check against what this device accepted
/// (`state::State::accept`).
pub fn read_manifest(address: &str, now: SystemTime) -> Result<Manifest, String> {
    read_manifest_with(address, now, signature::signer())
}

/// The same with `signer`'s key: the tests pass their own; the product's
/// paths call `read_manifest`, which trusts the publisher's alone.
pub fn read_manifest_with(address: &str, now: SystemTime, signer: &signature::Signer) -> Result<Manifest, String> {
    let text = fetch(address, MANIFEST_BYTES)?;
    let sig = fetch(&format!("{address}.sig"), SIGNATURE_BYTES)?;
    signature::verify(&text, &sig, signer, now).map_err(|refusal| format!("the manifest's signature: {refusal}"))?;
    let text = std::str::from_utf8(&text).map_err(|_| "the manifest is not UTF-8".to_string())?;
    manifest::parse(text)
}

/// Hexadecimal SHA-256.
pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// An extension's file downloaded, kept in Sioul's private data only once
/// its SHA-256 is the signed manifest's; where it is kept.
pub fn install(manifest_address: &str, offer: &Offer) -> Result<PathBuf, String> {
    let bytes = fetch(&manifest::file_address(manifest_address, &offer.file), FILE_BYTES)?;
    if sha256_hex(&bytes) != offer.sha256 {
        return Err(format!("{}: its file is not the one the manifest names (another SHA-256)", offer.id));
    }
    let path = state::file_path(offer);
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    }
    sioul_core::filelock::replace(&path, &bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
}

/// An extension's file as kept, when its SHA-256 is still the one accepted.
pub fn checked_file(offer: &Offer) -> Result<Vec<u8>, String> {
    let path = state::file_path(offer);
    let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    if bytes.len() > FILE_BYTES || sha256_hex(&bytes) != offer.sha256 {
        return Err(format!("{}: its file changed since it was checked", offer.id));
    }
    Ok(bytes)
}

/// An extension's file taken out of Sioul's private data.
pub fn discard_file(offer: &Offer) {
    let _ = std::fs::remove_file(state::file_path(offer));
}

#[cfg(test)]
mod tests;
