// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Your security key where it meets the world: its certificate looked up when
//! you ask, and GnuPG asked to let the key go when you press "Let GnuPG
//! release it". What is checked and kept is the core's
//! (`sioul_core::securitykey`).
//!
//! - **The certificate.** A security key holds no certificate, only its
//!   keys' fingerprints and public parts. Sioul looks for it only when you ask
//!   (at setup, and "Look for a newer version"), since each lookup tells a
//!   server that someone wants your key: the address written on the key
//!   (HTTPS only), then the Web Key Directory of your addresses' domains,
//!   then keys.openpgp.org by fingerprint. Each certificate found is checked
//!   against the key, and one that is not the key's is never kept.
//! - **GnuPG's hold.** GnuPG's smart card daemon keeps the key for itself
//!   after any use (scdaemon connects exclusively unless `pcsc-shared` is set
//!   in `scdaemon.conf`). "Let GnuPG release it" runs `gpgconf --kill
//!   scdaemon`, on that click only: gpg starts it again when it needs the key,
//!   and asks its PIN again then. Sioul never changes GnuPG's settings.
//! - **GnuPG's own steps, on a press** ([`Gnupg`]): "Import from GnuPG" runs
//!   `gpg --export --armor` for the key's fingerprint; "Renew for two years"
//!   runs `gpg --quick-set-expire` for the key, then its subkeys, GnuPG's own
//!   pinentry asking the card's PIN. The person's GnuPG, its home as GnuPG
//!   finds it; never a setting changed, never a key made or deleted.
//! - **keys.openpgp.org** ([`send_to_keys_openpgp`]): the public certificate
//!   sent on "Send it to keys.openpgp.org", over HTTPS, then a confirmation
//!   asked for each address it names that is not published yet: the server
//!   mails each a link, and publishes the address once the link is opened.
//! - **The key itself**, on a computer: the system's smart card service,
//!   PC/SC ([`smartcard`]). On a phone, not yet.
//! - **Logs**: [`cap_logging`], as each program starts.

use crate::SyncError;
use sioul_core::i18n::Translator;
use sioul_core::pgp;
#[cfg(target_os = "android")]
use sioul_core::securitykey::NoReaders;
use sioul_core::securitykey::{self, CardInfo, CertCheck, Certificate, Mismatch, Readers};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

#[cfg(not(target_os = "android"))]
pub mod smartcard;

/// Where security keys are found: the system's smart card service on a
/// computer; on a phone, nowhere yet (NFC and USB come later).
pub fn readers() -> &'static dyn Readers {
    #[cfg(not(target_os = "android"))]
    {
        static PCSC: smartcard::PcscReaders = smartcard::PcscReaders;
        &PCSC
    }
    #[cfg(target_os = "android")]
    {
        static NONE: NoReaders = NoReaders;
        &NONE
    }
}

/// Caps what any crate may log at "info", whatever level it asks, as a
/// program starts (`sioul-app`, `sioul`). `openpgp-card` writes whole APDUs
/// at "trace", a VERIFY's PIN included, and a card's answers, session keys
/// among them. Sioul installs no logger, so nothing is written anywhere; this
/// keeps "debug" and "trace" away even if one came, and never raises the
/// level (it starts at "off").
pub fn cap_logging() {
    log::set_max_level(log::max_level().min(log::LevelFilter::Info));
}

/// What looking for a security key's certificate found.
pub enum Looked {
    /// The key's certificate (public parts), what it is, and where it came from.
    Found {
        /// The certificate.
        cert: Certificate,
        /// What it is, checked against the key.
        check: CertCheck,
        /// The address it came from.
        source: String,
    },
    /// Certificates were found, none of them the key's: why.
    Mismatch(Mismatch),
    /// Nothing was found anywhere.
    NotFound,
}

/// The places to look, in order: the address written on the key (HTTPS only),
/// the Web Key Directory of each of `addresses`, then keys.openpgp.org by the
/// fingerprint of the key's signing key (else of its decryption key).
pub fn places(card: &CardInfo, addresses: &[String]) -> Vec<String> {
    let mut urls = Vec::new();
    let url = card.url.trim();
    if url.get(..8).is_some_and(|s| s.eq_ignore_ascii_case("https://")) {
        urls.push(url.to_string());
    }
    for address in addresses {
        urls.extend(pgp::wkd_urls(address).into_iter().flatten());
    }
    if let Some(slot) = card.sign.as_ref().or(card.decrypt.as_ref()) {
        // "MUST be uppercase, and MUST NOT be prefixed with 0x" (keys.openpgp.org/about/api).
        urls.push(format!("https://keys.openpgp.org/vks/v1/by-fingerprint/{}", slot.fingerprint.to_ascii_uppercase()));
    }
    let mut seen = std::collections::BTreeSet::new();
    urls.retain(|u| seen.insert(u.to_ascii_lowercase()));
    urls
}

/// Looks for the certificate of the security key `card`, only when you ask:
/// over HTTPS, in the order of [`places`], keeping the first that is the key's.
pub fn find_certificate(card: &CardInfo, addresses: &[String]) -> Result<Looked, SyncError> {
    // HTTPS to the end: an older certificate served in clear could hide a revocation.
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(15))).https_only(true).user_agent("Sioul").build().into();
    find_at(&agent, &places(card, addresses), card)
}

fn find_at(agent: &ureq::Agent, urls: &[String], card: &CardInfo) -> Result<Looked, SyncError> {
    let mut mismatch = None;
    let mut last_error = None;
    for url in urls {
        match agent.get(url).call() {
            Ok(mut response) => {
                let bytes = response.body_mut().read_to_vec().unwrap_or_default();
                match securitykey::certificate_in(&bytes, card) {
                    Ok((cert, check)) => return Ok(Looked::Found { cert, check, source: url.clone() }),
                    // Nothing of the key's there: the next place.
                    Err(Mismatch::Missing) => {}
                    Err(why) => mismatch = Some(why),
                }
            }
            Err(ureq::Error::StatusCode(_)) => {}
            Err(e) => last_error = Some(e.to_string()),
        }
    }
    match (mismatch, last_error) {
        (Some(why), _) => Ok(Looked::Mismatch(why)),
        (None, Some(e)) => Err(SyncError::Network(e)),
        (None, None) => Ok(Looked::NotFound),
    }
}

/// What "Let GnuPG release it" did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Released {
    /// GnuPG's smart card daemon stopped: the key is free until gpg needs it again.
    Done,
    /// No `gpgconf` here: GnuPG is not installed, or not where Sioul looks.
    NoGnupg,
    /// In a Flatpak, GnuPG runs outside Sioul's sandbox, out of its reach.
    Sandboxed,
    /// `gpgconf` refused: what it said.
    Failed(String),
}

impl Released {
    /// What happened, in words, and what to do when it did not work.
    pub fn sentence(&self, tr: &Translator) -> String {
        match self {
            Released::Done => tr.text("seckey-released", None),
            Released::NoGnupg => tr.text("seckey-release-no-gnupg", None),
            Released::Sandboxed => tr.text("seckey-release-sandboxed", None),
            Released::Failed(detail) => {
                let mut args = sioul_core::i18n::args();
                // GnuPG's words, never read as a command to copy.
                args.set("detail", detail.replace('`', "'"));
                tr.text("seckey-release-failed", Some(&args))
            }
        }
    }
}

/// Asks GnuPG to let the security key go: `gpgconf --kill scdaemon`. Run only
/// when you press "Let GnuPG release it", never by itself.
pub fn release_gnupg() -> Released {
    if Path::new("/.flatpak-info").exists() {
        return Released::Sandboxed;
    }
    for program in gpgconf_places() {
        match release_with(&program) {
            Released::NoGnupg => continue,
            done => return done,
        }
    }
    Released::NoGnupg
}

/// Where `gpgconf` may be: on the PATH, then where GnuPG's installers put it,
/// since a window started from the desktop may not have them on its PATH.
fn gpgconf_places() -> Vec<PathBuf> {
    let mut places = vec![PathBuf::from(if cfg!(windows) { "gpgconf.exe" } else { "gpgconf" })];
    if cfg!(target_os = "macos") {
        places.extend(["/opt/homebrew/bin/gpgconf", "/usr/local/bin/gpgconf", "/usr/local/MacGPG2/bin/gpgconf"].map(PathBuf::from));
    }
    if cfg!(windows) {
        places.extend([r"C:\Program Files (x86)\GnuPG\bin\gpgconf.exe", r"C:\Program Files\GnuPG\bin\gpgconf.exe"].map(PathBuf::from));
    }
    places
}

fn release_with(program: &Path) -> Released {
    release_at(program, None)
}

/// `gpgconf --kill scdaemon`, for the GnuPG home `home` when given (`--homedir`).
fn release_at(program: &Path, home: Option<&Path>) -> Released {
    let mut command = std::process::Command::new(program);
    if let Some(home) = home {
        command.arg("--homedir").arg(home);
    }
    command.args(["--kill", "scdaemon"]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped());
    // No console window flashing up from Sioul's.
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    match command.output() {
        Ok(out) if out.status.success() => Released::Done,
        Ok(out) => Released::Failed(String::from_utf8_lossy(&out.stderr).lines().map(str::trim).rfind(|l| !l.is_empty()).unwrap_or("").to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Released::NoGnupg,
        Err(e) => Released::Failed(e.to_string()),
    }
}

/// GnuPG, as Sioul runs it on your press: the `gpg` program, and the home it
/// reads (none: GnuPG's own, `GNUPGHOME` or `~/.gnupg`; a throwaway one in tests).
#[derive(Debug, Clone)]
pub struct Gnupg {
    program: PathBuf,
    home: Option<PathBuf>,
}

/// Why GnuPG did not do what was asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GnupgProblem {
    /// No `gpg` here (on a phone, never).
    Missing,
    /// In a Flatpak, GnuPG runs outside Sioul's sandbox, out of its reach.
    Sandboxed,
    /// GnuPG here holds no such key: none to export, or no secret part to renew it with.
    NoSuchKey,
    /// The security key waited for a touch and none came in time (GnuPG's
    /// "Timeout", whatever its language says).
    TouchMissed,
    /// It refused, or did not answer in time: its last word.
    Failed(String),
}

/// How long GnuPG may take when it asks for a PIN and a touch: then it is
/// said as not answering, and its process is ended.
const GNUPG_WAITS: Duration = Duration::from_secs(5 * 60);

/// libgpg-error's GPG_ERR_TIMEOUT: a security key that waited for a touch in vain.
const GPG_ERR_TIMEOUT: u32 = 62;

/// Where a GnuPG step on the security key is, for what the window says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GnupgStep {
    /// GnuPG's pinentry is open: the PIN is typed there.
    Pin,
    /// No pinentry open (none needed, or closed): the key may wait for a touch.
    Card,
}

/// A line of GnuPG's status (`--status-fd`), as far as Sioul needs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// `[GNUPG:] PINENTRY_LAUNCHED <pid> …`: its pinentry opened.
    Pinentry(u32),
    /// `[GNUPG:] FAILURE <where> <code>`, `[GNUPG:] ERROR <where> <code>`:
    /// the error's code, its source left out (libgpg-error's low 16 bits).
    Failed(u32),
}

/// A status line read, if it is one Sioul needs.
pub fn status_of(line: &str) -> Option<Status> {
    let mut words = line.trim().strip_prefix("[GNUPG:] ")?.split_whitespace();
    match words.next()? {
        "PINENTRY_LAUNCHED" => words.next()?.parse().ok().map(Status::Pinentry),
        "FAILURE" | "ERROR" => words.last()?.parse::<u32>().ok().map(|code| Status::Failed(code & 0xFFFF)),
        _ => None,
    }
}

/// Whether the process `pid` still runs (its pinentry): on Linux, as /proc
/// says; elsewhere not known, taken as running until GnuPG answers.
fn running(pid: u32) -> bool {
    if cfg!(target_os = "linux") { Path::new(&format!("/proc/{pid}")).exists() } else { true }
}

/// While Sioul's own GnuPG works with the security key, Sioul's own looks at
/// the keys wait (`Gnupg::renew`): a key opened by Sioul between two of
/// GnuPG's commands, and reset when closed, would undo the PIN GnuPG gave it.
static GNUPG_ON_CARD: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Held while Sioul's own GnuPG works with the security key.
pub struct OnCard;

impl OnCard {
    /// From now until dropped, Sioul's looks at the keys wait.
    pub fn start() -> OnCard {
        GNUPG_ON_CARD.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        OnCard
    }
}

impl Drop for OnCard {
    fn drop(&mut self) {
        GNUPG_ON_CARD.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }
}

/// Whether Sioul may look at the security keys now: not while its own GnuPG works with one.
pub fn may_look() -> bool {
    GNUPG_ON_CARD.load(std::sync::atomic::Ordering::SeqCst) == 0
}

impl Gnupg {
    /// GnuPG here: `gpg` on the path, else where its installers put it (a
    /// window started from the desktop may not have them on its path).
    pub fn find() -> Result<Gnupg, GnupgProblem> {
        if cfg!(target_os = "android") {
            return Err(GnupgProblem::Missing);
        }
        if Path::new("/.flatpak-info").exists() {
            return Err(GnupgProblem::Sandboxed);
        }
        let name = if cfg!(windows) { "gpg.exe" } else { "gpg" };
        let on_path = std::env::var_os("PATH").into_iter().flat_map(|path| std::env::split_paths(&path).collect::<Vec<_>>()).map(|dir| dir.join(name));
        let known: Vec<PathBuf> = if cfg!(target_os = "macos") {
            ["/opt/homebrew/bin/gpg", "/usr/local/bin/gpg", "/usr/local/MacGPG2/bin/gpg"].map(PathBuf::from).to_vec()
        } else if cfg!(windows) {
            [r"C:\Program Files (x86)\GnuPG\bin\gpg.exe", r"C:\Program Files\GnuPG\bin\gpg.exe"].map(PathBuf::from).to_vec()
        } else {
            vec![PathBuf::from("/usr/bin/gpg")]
        };
        on_path.chain(known).find(|p| p.is_file()).map(|program| Gnupg { program, home: None }).ok_or(GnupgProblem::Missing)
    }

    /// This `gpg` with its home in `home` (`--homedir`): tests, never yours.
    pub fn with_home(program: &Path, home: &Path) -> Gnupg {
        Gnupg { program: program.to_path_buf(), home: Some(home.to_path_buf()) }
    }

    /// `gpg` run with `args`, in batch mode, nothing on its input; its output,
    /// or why not. GnuPG's own pinentry may ask for a PIN meanwhile.
    fn run(&self, args: &[&str]) -> Result<Vec<u8>, GnupgProblem> {
        self.run_watching(args, None)
    }

    /// The same, saying where it is as it goes (`watch`: its status read as
    /// it comes): the pinentry open, then the key that may wait for a touch.
    /// A security key that waited in vain is said as such.
    fn run_watching(&self, args: &[&str], watch: Option<&(dyn Fn(GnupgStep) + Sync)>) -> Result<Vec<u8>, GnupgProblem> {
        let mut command = std::process::Command::new(&self.program);
        if let Some(home) = &self.home {
            command.arg("--homedir").arg(home);
        }
        command.args(["--batch", "--no-tty"]);
        if watch.is_some() {
            command.args(["--status-fd", "2"]);
        }
        command.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        // No console window flashing up from Sioul's.
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000);
        }
        let mut child = command.spawn().map_err(|e| if e.kind() == std::io::ErrorKind::NotFound { GnupgProblem::Missing } else { GnupgProblem::Failed(e.to_string()) })?;
        // Its output read as it comes, so that a long one never blocks it.
        let mut out = child.stdout.take();
        let mut err = child.stderr.take();
        let reading = std::thread::spawn(move || {
            use std::io::Read;
            let mut bytes = Vec::new();
            if let Some(out) = out.as_mut() {
                let _ = out.read_to_end(&mut bytes);
            }
            bytes
        });
        // Its words and its status, line by line as they come.
        let (tell, heard) = std::sync::mpsc::channel::<Status>();
        let said = std::thread::spawn(move || {
            use std::io::BufRead;
            let mut text = String::new();
            if let Some(err) = err.take() {
                for line in std::io::BufReader::new(err).lines().map_while(Result::ok) {
                    match status_of(&line) {
                        Some(status) => {
                            let _ = tell.send(status);
                        }
                        None if !line.starts_with("[GNUPG:]") => {
                            text.push_str(&line);
                            text.push('\n');
                        }
                        None => {}
                    }
                }
            }
            text
        });
        let started = std::time::Instant::now();
        let mut shown: Option<GnupgStep> = None;
        let mut pinentry: Option<u32> = None;
        let mut failed: Option<u32> = None;
        let status = loop {
            if let Some(watch) = watch {
                for status in heard.try_iter() {
                    match status {
                        Status::Pinentry(pid) => pinentry = Some(pid),
                        Status::Failed(code) => failed = Some(code),
                    }
                }
                if pinentry.is_some_and(|pid| !running(pid)) {
                    pinentry = None;
                }
                let step = if pinentry.is_some() { GnupgStep::Pin } else { GnupgStep::Card };
                if shown != Some(step) {
                    shown = Some(step);
                    watch(step);
                }
            }
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if started.elapsed() > GNUPG_WAITS => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(GnupgProblem::Failed("gpg did not answer".into()));
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(100)),
                Err(e) => return Err(GnupgProblem::Failed(e.to_string())),
            }
        };
        let output = reading.join().unwrap_or_default();
        let errors = said.join().unwrap_or_default();
        failed = heard.try_iter().fold(failed, |failed, status| if let Status::Failed(code) = status { Some(code) } else { failed });
        if status.success() {
            Ok(output)
        } else if failed == Some(GPG_ERR_TIMEOUT) {
            Err(GnupgProblem::TouchMissed)
        } else {
            Err(GnupgProblem::Failed(errors.lines().map(str::trim).rfind(|l| !l.is_empty()).unwrap_or("").to_string()))
        }
    }

    /// The certificate of the key `fingerprint`, armored, as GnuPG keeps it
    /// (`gpg --export --armor`): public parts only.
    pub fn export(&self, fingerprint: &str) -> Result<Vec<u8>, GnupgProblem> {
        let fingerprint = fingerprint.trim();
        if fingerprint.is_empty() || !fingerprint.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(GnupgProblem::NoSuchKey);
        }
        let out = self.run(&["--armor", "--export", fingerprint])?;
        if out.iter().all(u8::is_ascii_whitespace) { Err(GnupgProblem::NoSuchKey) } else { Ok(out) }
    }

    /// Whether GnuPG holds the secret part of `fingerprint`, or knows the card
    /// that does: then it can renew the key.
    pub fn holds_secret(&self, fingerprint: &str) -> bool {
        let fingerprint = fingerprint.trim();
        !fingerprint.is_empty()
            && fingerprint.chars().all(|c| c.is_ascii_hexdigit())
            && self.run(&["--with-colons", "--list-secret-keys", fingerprint]).is_ok_and(|out| String::from_utf8_lossy(&out).lines().any(|l| l.starts_with("sec:")))
    }

    /// The key `fingerprint` renewed for `period` ("2y"), then its subkeys:
    /// `gpg --quick-set-expire`, twice, GnuPG's own pinentry asking the PIN
    /// (a security key's: and a touch) each time it needs it.
    /// Its subkeys are named one by one, expired ones included: GnuPG's "*"
    /// sets only those not yet expired, and a subkey expired since long
    /// stayed so. `watch` is told where it is: GnuPG's pinentry open, then
    /// the key that may wait for a touch. Meanwhile Sioul's own looks at the
    /// keys wait ([`may_look`]); at the end, done or not, GnuPG lets the
    /// security key go ([`Gnupg::release`]), so that Sioul can read it next.
    pub fn renew(&self, fingerprint: &str, period: &str, watch: &(dyn Fn(GnupgStep) + Sync)) -> Result<(), GnupgProblem> {
        if !self.holds_secret(fingerprint) {
            return Err(GnupgProblem::NoSuchKey);
        }
        let fingerprint = fingerprint.trim();
        let subkeys = self.subkeys(fingerprint);
        let renewed = {
            let _on_card = OnCard::start();
            self.run_watching(&["--quick-set-expire", fingerprint, period], Some(watch)).and_then(|_| {
                if subkeys.is_empty() {
                    return Ok(Vec::new());
                }
                let mut args = vec!["--quick-set-expire", fingerprint, period];
                args.extend(subkeys.iter().map(String::as_str));
                self.run_watching(&args, Some(watch))
            })
        };
        // Sioul's own GnuPG used the key: it lets it go, not a setting changed.
        let _ = self.release();
        renewed.map(|_| ())
    }

    /// The subkeys of `fingerprint` that are not revoked, expired ones
    /// included, as GnuPG lists them; none when it does not list them.
    pub fn subkeys(&self, fingerprint: &str) -> Vec<String> {
        let Ok(listed) = self.run(&["--with-colons", "--list-keys", fingerprint.trim()]) else { return Vec::new() };
        let mut out = Vec::new();
        let mut next_is_sub = false;
        for line in String::from_utf8_lossy(&listed).lines() {
            let fields: Vec<&str> = line.split(':').collect();
            match fields.first().copied() {
                // Its validity: "r" revoked, left alone.
                Some("sub") => next_is_sub = fields.get(1).is_some_and(|v| *v != "r"),
                Some("fpr") if next_is_sub => {
                    out.extend(fields.get(9).filter(|f| !f.is_empty()).map(|f| f.to_string()));
                    next_is_sub = false;
                }
                Some("fpr") => {}
                _ => next_is_sub = false,
            }
        }
        out
    }

    /// GnuPG's smart card daemon stopped, after Sioul's own GnuPG used the
    /// security key: it holds the key for itself (unless `pcsc-shared` is in
    /// scdaemon.conf) and Sioul could not read it next. gpg starts it again
    /// when it needs the key. GnuPG's settings stay as they are.
    pub fn release(&self) -> Released {
        let gpgconf = self.program.with_file_name(if cfg!(windows) { "gpgconf.exe" } else { "gpgconf" });
        let program = if gpgconf.is_file() { gpgconf } else { PathBuf::from(if cfg!(windows) { "gpgconf.exe" } else { "gpgconf" }) };
        release_at(&program, self.home.as_deref())
    }
}

/// The commands that renew the key `fingerprint` for two years by hand: the
/// key, then its `subkeys`, each named (GnuPG's "*" leaves the expired ones).
pub fn renew_commands(fingerprint: &str, subkeys: &[String]) -> Vec<String> {
    let mut out = vec![format!("gpg --quick-set-expire {fingerprint} 2y")];
    if !subkeys.is_empty() {
        out.push(format!("gpg --quick-set-expire {fingerprint} 2y {}", subkeys.join(" ")));
    }
    out
}

/// The command that writes the key's certificate into a file by hand.
pub fn export_command(fingerprint: &str) -> String {
    format!("gpg --export --armor {fingerprint} > key.asc")
}

/// What keys.openpgp.org said of a certificate sent to it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Published {
    /// Its fingerprint, as the server read it.
    pub fingerprint: String,
    /// The addresses it mailed a link to: each published once its link is opened.
    pub mailed: Vec<String>,
    /// The addresses findable there already.
    pub published: Vec<String>,
}

/// keys.openpgp.org, where "Send it to keys.openpgp.org" sends.
pub const KEYS_OPENPGP: &str = "https://keys.openpgp.org";

/// Sends the public certificate `armored` to keys.openpgp.org, only when you
/// press its button: over HTTPS, then a link asked for each address it names
/// that is not published yet, the mail in your language (`locale`, "fr").
/// Anyone can then find the key there by its fingerprint; by an address,
/// only once that address's link is opened.
pub fn send_to_keys_openpgp(armored: &[u8], locale: &str) -> Result<Published, SyncError> {
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(30))).https_only(true).http_status_as_error(false).user_agent("Sioul").build().into();
    send_at(&agent, KEYS_OPENPGP, armored, locale)
}

fn send_at(agent: &ureq::Agent, base: &str, armored: &[u8], locale: &str) -> Result<Published, SyncError> {
    let call = |path: &str, body: serde_json::Value| -> Result<serde_json::Value, SyncError> {
        let mut response = agent.post(&format!("{base}{path}")).header("Content-Type", "application/json").send(body.to_string()).map_err(|e| SyncError::Network(e.to_string()))?;
        let status = response.status().as_u16();
        let answer: serde_json::Value = serde_json::from_str(&response.body_mut().read_to_string().unwrap_or_default()).unwrap_or_default();
        if status == 200 {
            Ok(answer)
        } else {
            Err(SyncError::Server(format!("keys.openpgp.org: {status} {}", answer["error"].as_str().unwrap_or("")).trim().to_string()))
        }
    };
    let uploaded = call("/vks/v1/upload", serde_json::json!({ "keytext": String::from_utf8_lossy(armored) }))?;
    let addresses = |answer: &serde_json::Value, states: &[&str]| -> Vec<String> {
        answer["status"].as_object().map(|status| status.iter().filter(|(_, state)| states.contains(&state.as_str().unwrap_or(""))).map(|(address, _)| address.clone()).collect()).unwrap_or_default()
    };
    let mut published = Published { fingerprint: uploaded["key_fpr"].as_str().unwrap_or("").to_string(), mailed: Vec::new(), published: addresses(&uploaded, &["published"]) };
    let waiting = addresses(&uploaded, &["unpublished", "pending"]);
    let token = uploaded["token"].as_str().unwrap_or("");
    if !waiting.is_empty() && !token.is_empty() {
        let locale: Vec<&str> = Some(locale).filter(|l| !l.is_empty()).into_iter().collect();
        let asked = call("/vks/v1/request-verify", serde_json::json!({ "token": token, "addresses": waiting, "locale": locale }))?;
        published.mailed = addresses(&asked, &["pending"]);
        published.published = addresses(&asked, &["published"]);
    }
    Ok(published)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sioul_core::securitykey::SoftCard;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;

    /// A server on this computer answering each request with the next of
    /// `answers`, and telling which paths were asked.
    fn stand_in(answers: Vec<Vec<u8>>) -> (String, std::sync::mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        let (tell, heard) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for answer in answers {
                let Ok((stream, _)) = listener.accept() else { return };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                loop {
                    let mut header = String::new();
                    reader.read_line(&mut header).unwrap();
                    if header.trim_end().is_empty() {
                        break;
                    }
                }
                let _ = tell.send(line.split_whitespace().nth(1).unwrap_or("").to_string());
                let mut stream = stream;
                let _ = stream.write_all(&answer);
            }
        });
        (address, heard)
    }

    fn ok(body: &[u8]) -> Vec<u8> {
        let mut answer = format!("HTTP/1.1 200 OK\r\nContent-Type: application/pgp-keys\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).into_bytes();
        answer.extend_from_slice(body);
        answer
    }

    const NOT_FOUND: &[u8] = b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";

    fn agent() -> ureq::Agent {
        ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(5))).build().into()
    }

    #[test]
    fn the_places_come_in_order_and_https_only() {
        let (mut card, _) = SoftCard::generate("0006:12345678", &["Card Holder <me@example.org>"], "123456").unwrap();
        card.info.url = "https://keys.example.org/me.asc".into();
        let urls = places(&card.info, &["me@example.org".to_string()]);
        assert_eq!(urls[0], "https://keys.example.org/me.asc");
        assert!(urls[1].starts_with("https://openpgpkey.example.org/.well-known/openpgpkey/example.org/hu/"), "{urls:?}");
        assert!(urls[2].starts_with("https://example.org/.well-known/openpgpkey/hu/"), "{urls:?}");
        let fingerprint = card.info.sign.as_ref().unwrap().fingerprint.clone();
        assert_eq!(urls[3], format!("https://keys.openpgp.org/vks/v1/by-fingerprint/{fingerprint}"));
        assert_eq!(fingerprint, fingerprint.to_ascii_uppercase());
        // An address in clear on the key is not used.
        card.info.url = "http://keys.example.org/me.asc".into();
        assert!(!places(&card.info, &[]).iter().any(|u| u.starts_with("http:")));
        assert_eq!(places(&card.info, &[]).len(), 1);
    }

    #[test]
    fn the_first_certificate_that_is_the_keys_is_kept() {
        let (card, public) = SoftCard::generate("0006:12345678", &["Card Holder <me@example.org>"], "123456").unwrap();
        let (_, stranger) = SoftCard::generate("0006:87654321", &["Someone Else <someone@example.net>"], "123456").unwrap();
        let (base, heard) = stand_in(vec![NOT_FOUND.to_vec(), ok(&stranger), ok(&public)]);
        let urls = vec![format!("{base}/card"), format!("{base}/wkd"), format!("{base}/vks")];
        match find_at(&agent(), &urls, &card.info).unwrap() {
            Looked::Found { check, source, .. } => {
                assert_eq!(source, format!("{base}/vks"));
                assert_eq!(check.addresses, vec!["me@example.org".to_string()]);
                assert!(check.signs && check.decrypts && !check.expired);
            }
            _ => panic!("not found"),
        }
        let asked: Vec<String> = heard.try_iter().collect();
        assert_eq!(asked, vec!["/card", "/wkd", "/vks"]);
        // Nothing of the key's anywhere: said as not found.
        let (base, _heard) = stand_in(vec![NOT_FOUND.to_vec(), ok(&stranger)]);
        assert!(matches!(find_at(&agent(), &[format!("{base}/a"), format!("{base}/b")], &card.info).unwrap(), Looked::NotFound));
    }

    #[test]
    fn no_network_is_said_as_such() {
        let (card, _) = SoftCard::generate("0006:12345678", &[], "123456").unwrap();
        // A port nobody listens on: bound, then let go.
        let closed = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap();
        assert!(matches!(find_at(&agent(), &[format!("http://{closed}/key")], &card.info), Err(SyncError::Network(_))));
    }

    #[test]
    fn logs_never_carry_a_pin() {
        // Raised by a test as anything could: capped again at "info", never higher.
        log::set_max_level(log::LevelFilter::Trace);
        cap_logging();
        assert_eq!(log::max_level(), log::LevelFilter::Info);
        assert!(!log::log_enabled!(target: "openpgp_card::ocard::apdu", log::Level::Trace));
        // Never raised by the cap: "off" stays off.
        log::set_max_level(log::LevelFilter::Off);
        cap_logging();
        assert_eq!(log::max_level(), log::LevelFilter::Off);
        // No crate of Sioul's sets a logger or a level but the cap, and no crate it builds with installs a logger.
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let lock = std::fs::read_to_string(root.join("Cargo.lock")).unwrap();
        for logger in ["env_logger", "simple_logger", "simplelog", "fern", "log4rs", "android_logger", "oslog", "syslog", "systemd-journal-logger", "tracing-log", "pretty_env_logger", "flexi_logger", "slog-stdlog", "stderrlog", "tracing-subscriber"] {
            assert!(!lock.contains(&format!("name = \"{logger}\"")), "{logger} would write what crates log");
        }
        let mut sources = vec![root.join("crates")];
        while let Some(path) = sources.pop() {
            if path.is_dir() {
                sources.extend(std::fs::read_dir(&path).unwrap().filter_map(Result::ok).map(|e| e.path()));
            } else if path.extension().is_some_and(|x| x == "rs") {
                let text = std::fs::read_to_string(&path).unwrap();
                // Spelt in two halves, so that this test does not find itself.
                for (call, here) in [(concat!("set_", "logger("), false), (concat!("set_boxed", "_logger("), false), (concat!("set_max", "_level("), true)] {
                    let calls = text.matches(call).count();
                    let allowed = if here && path.ends_with("sioul-sync/src/securitykey.rs") { calls } else { 0 };
                    assert_eq!(calls, allowed, "{} calls {call}", path.display());
                }
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn gnupg_is_asked_to_let_go_with_a_stand_in() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("sioul-gpgconf-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // Never GnuPG's own gpgconf: a script that only says what it was asked.
        let program = dir.join("gpgconf");
        std::fs::write(&program, format!("#!/bin/sh\necho \"$@\" > '{}'\n", dir.join("asked").display())).unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(release_with(&program), Released::Done);
        assert_eq!(std::fs::read_to_string(dir.join("asked")).unwrap().trim(), "--kill scdaemon");
        let refusing = dir.join("refusing");
        std::fs::write(&refusing, "#!/bin/sh\necho 'gpgconf: no such component' >&2\nexit 2\n").unwrap();
        std::fs::set_permissions(&refusing, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(release_with(&refusing), Released::Failed("gpgconf: no such component".into()));
        assert_eq!(release_with(&dir.join("nowhere")), Released::NoGnupg);
        let tr = Translator::new("en");
        for said in [Released::Done, Released::NoGnupg, Released::Sandboxed, Released::Failed("x".into())] {
            assert!(!said.sentence(&tr).starts_with("seckey-"), "{said:?}");
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A throwaway GnuPG home, never yours: its agent never starts the smart
    /// card daemon (no security key plugged in is touched), and is stopped
    /// with the folder when the test ends.
    struct ThrowawayHome {
        dir: PathBuf,
        gpg: PathBuf,
    }

    impl ThrowawayHome {
        fn new(name: &str) -> Option<ThrowawayHome> {
            let gpg = Gnupg::find().ok()?.program;
            // Short: GnuPG's sockets may live in it, 108 bytes at most.
            let dir = std::env::temp_dir().join(format!("sg-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
            }
            std::fs::write(dir.join("gpg-agent.conf"), "disable-scdaemon\n").unwrap();
            std::fs::write(dir.join("gpg.conf"), "no-auto-key-locate\nno-auto-key-retrieve\n").unwrap();
            Some(ThrowawayHome { dir, gpg })
        }

        /// `gpg` in this home, no passphrase asked (the keys made here have none).
        fn gpg(&self, args: &[&str]) -> String {
            let out = std::process::Command::new(&self.gpg).arg("--homedir").arg(&self.dir).args(["--batch", "--no-tty", "--pinentry-mode", "loopback", "--passphrase", ""]).args(args).stdin(Stdio::null()).output().unwrap();
            assert!(out.status.success(), "gpg {args:?}: {}", String::from_utf8_lossy(&out.stderr));
            String::from_utf8_lossy(&out.stdout).to_string()
        }

        fn gnupg(&self) -> Gnupg {
            Gnupg::with_home(&self.gpg, &self.dir)
        }
    }

    impl Drop for ThrowawayHome {
        fn drop(&mut self) {
            let gpgconf = self.gpg.with_file_name(if cfg!(windows) { "gpgconf.exe" } else { "gpgconf" });
            let _ = std::process::Command::new(gpgconf).arg("--homedir").arg(&self.dir).args(["--kill", "all"]).stdin(Stdio::null()).output();
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    /// When each key of `fingerprint`'s certificate expires (Unix seconds), from GnuPG's own listing.
    fn expiries(home: &ThrowawayHome, fingerprint: &str) -> Vec<i64> {
        home.gpg(&["--with-colons", "--list-keys", fingerprint]).lines().filter(|l| l.starts_with("pub:") || l.starts_with("sub:")).map(|l| l.split(':').nth(6).unwrap_or("").parse().unwrap_or(0)).collect()
    }

    /// "Import from GnuPG" and "Renew for two years", run against a GnuPG home
    /// made for the test, with a key made there without a passphrase: never
    /// yours, never a card, never the network. Skipped where GnuPG is not installed.
    #[test]
    fn gnupg_exports_and_renews_in_a_throwaway_home() {
        let _alone = ON_CARD_TESTS.lock().unwrap_or_else(|e| e.into_inner());
        let Some(home) = ThrowawayHome::new("gpg") else { return };
        // A security key's certificate, known to this GnuPG as it would be after `gpg --card-status` and a fetch.
        let (card, public) = SoftCard::generate("0006:12345678", &["Card Holder <me@example.org>"], "123456").unwrap();
        let asc = home.dir.join("card.asc");
        std::fs::write(&asc, &public).unwrap();
        home.gpg(&["--import", asc.to_str().unwrap()]);
        let fingerprint = card.info.sign.as_ref().unwrap().fingerprint.clone();
        let gnupg = home.gnupg();
        // Exported, then checked against the key as "Import from GnuPG" does: the key's own.
        let exported = gnupg.export(&fingerprint).unwrap();
        assert!(String::from_utf8_lossy(&exported).starts_with("-----BEGIN PGP PUBLIC KEY BLOCK-----"));
        let (_, check) = securitykey::certificate_in(&exported, &card.info).unwrap();
        assert_eq!(check.addresses, vec!["me@example.org".to_string()]);
        // A key GnuPG does not hold, or a fingerprint that is none: said so, nothing run with it.
        assert_eq!(gnupg.export("0123456789ABCDEF0123456789ABCDEF01234567"), Err(GnupgProblem::NoSuchKey));
        assert_eq!(gnupg.export("me; rm -rf ~"), Err(GnupgProblem::NoSuchKey));
        // Its secret part on the card, unknown to this GnuPG: it cannot renew it.
        assert!(!gnupg.holds_secret(&fingerprint));
        assert_eq!(gnupg.renew(&fingerprint, "2y", &|_| {}), Err(GnupgProblem::NoSuchKey));
        // A key of its own, valid a year: renewed for two, the key and its subkey.
        home.gpg(&["--quick-gen-key", "Test Key <test@example.org>", "ed25519", "cert,sign", "1y"]);
        let own = home.gpg(&["--with-colons", "--list-secret-keys", "test@example.org"]).lines().find(|l| l.starts_with("fpr:")).unwrap().split(':').nth(9).unwrap().to_string();
        home.gpg(&["--quick-add-key", &own, "cv25519", "encr", "1y"]);
        assert!(gnupg.holds_secret(&own));
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
        assert!(expiries(&home, &own).iter().all(|at| *at < now + 400 * 86_400), "a year first");
        // Where it is, as it goes: no pinentry for a key without a passphrase, so the key (here, none) at once;
        // Sioul's own looks at the keys wait meanwhile.
        let steps = std::sync::Mutex::new(Vec::new());
        let looks = std::sync::Mutex::new(Vec::new());
        gnupg
            .renew(&own, "2y", &|step| {
                steps.lock().unwrap().push(step);
                looks.lock().unwrap().push(may_look());
            })
            .unwrap();
        assert_eq!(*steps.lock().unwrap(), [GnupgStep::Card, GnupgStep::Card], "one for the key, one for its subkeys");
        assert!(looks.lock().unwrap().iter().all(|may| !may), "no look at the keys while GnuPG works with one");
        assert!(may_look(), "and again once it is done");
        let renewed = expiries(&home, &own);
        assert_eq!(renewed.len(), 2);
        assert!(renewed.iter().all(|at| *at > now + 700 * 86_400 && *at < now + 740 * 86_400), "{renewed:?}");
        // No GnuPG there: said as missing.
        assert_eq!(Gnupg::with_home(&home.dir.join("no-gpg"), &home.dir).export(&own), Err(GnupgProblem::Missing));
        // The commands, for doing it by hand.
        assert_eq!(renew_commands("ABCD", &["EF01".to_string(), "2345".to_string()]), ["gpg --quick-set-expire ABCD 2y".to_string(), "gpg --quick-set-expire ABCD 2y EF01 2345".to_string()]);
        assert_eq!(renew_commands("ABCD", &[]), ["gpg --quick-set-expire ABCD 2y".to_string()]);
        assert_eq!(export_command("ABCD"), "gpg --export --armor ABCD > key.asc");
    }

    /// A key whose primary key was renewed alone, in a terminal, its subkeys
    /// expired long ago (made in 2022 for a year, in a GnuPG home made for
    /// the test): GnuPG's "*" leaves them so; each said exactly; then
    /// "Renew for two years" names each subkey, and every part is renewed.
    #[test]
    fn subkeys_expired_are_said_and_renewed_one_by_one() {
        let _alone = ON_CARD_TESTS.lock().unwrap_or_else(|e| e.into_inner());
        let Some(home) = ThrowawayHome::new("sub") else { return };
        let past = "20220101T000000";
        home.gpg(&["--faked-system-time", past, "--quick-gen-key", "Old Key <old@example.org>", "ed25519", "cert,sign", "1y"]);
        let primary = home.gpg(&["--with-colons", "--list-secret-keys", "old@example.org"]).lines().find(|l| l.starts_with("fpr:")).unwrap().split(':').nth(9).unwrap().to_string();
        home.gpg(&["--faked-system-time", past, "--quick-add-key", &primary, "cv25519", "encr", "1y"]);
        home.gpg(&["--faked-system-time", past, "--quick-add-key", &primary, "ed25519", "auth", "1y"]);
        // As the owner did: the primary key renewed, then "*", which leaves expired subkeys as they are.
        home.gpg(&["--quick-set-expire", &primary, "2y"]);
        home.gpg(&["--quick-set-expire", &primary, "2y", "*"]);
        let gnupg = home.gnupg();
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
        let parts = securitykey::parts_in(&gnupg.export(&primary).unwrap());
        let roles: Vec<(securitykey::Role, bool)> = parts.iter().map(|p| (p.role, p.expires.is_some_and(|at| at <= now))).collect();
        assert_eq!(roles, [(securitykey::Role::Primary, false), (securitykey::Role::Encryption, true), (securitykey::Role::Authentication, true)]);
        let tr = Translator::new("en");
        let said = securitykey::expired_parts(&tr, &parts, now);
        assert_eq!(said.len(), 2, "{said:?}");
        let encryption = securitykey::short_fingerprint(&parts[1].fingerprint);
        assert!(said[0].starts_with(&format!("Its encryption subkey ({encryption}) expired on ")) && said[0].ends_with("2023: nobody can encrypt to you with it."), "{}", said[0]);
        assert!(said[1].starts_with("Its authentication subkey ("), "{}", said[1]);
        // GnuPG lists both subkeys, the expired ones too: Renew names them.
        let mut listed = gnupg.subkeys(&primary);
        listed.sort();
        let mut expected = vec![parts[1].fingerprint.clone(), parts[2].fingerprint.clone()];
        expected.sort();
        assert_eq!(listed, expected);
        gnupg.renew(&primary, "2y", &|_| {}).unwrap();
        let renewed = securitykey::parts_in(&gnupg.export(&primary).unwrap());
        assert!(renewed.iter().all(|p| p.expires.is_some_and(|at| at > now + 700 * 86_400)), "{renewed:?}");
        assert!(securitykey::expired_parts(&tr, &renewed, now).is_empty());
        let until = securitykey::parts_until(&tr, &renewed);
        assert!(until.starts_with("primary key until ") && until.contains("encryption subkey until ") && until.contains("authentication subkey until "), "{until}");
    }

    /// Sioul's own GnuPG step on the security key ends with GnuPG letting the
    /// key go, asked once, done or not; with stand-ins for gpg and gpgconf,
    /// never GnuPG's own.
    #[cfg(unix)]
    #[test]
    fn renewing_ends_with_the_key_let_go_once() {
        use std::os::unix::fs::PermissionsExt;
        let _alone = ON_CARD_TESTS.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("sioul-gpg-standin-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let primary = "0123456789ABCDEF0123456789ABCDEF01234567";
        let sub = "89ABCDEF0123456789ABCDEF0123456789ABCDEF";
        let asked = dir.join("asked");
        let script = |name: &str, body: String| {
            let path = dir.join(name);
            std::fs::write(&path, body).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
            path
        };
        let fail = dir.join("fail");
        let gpg = script(
            "gpg",
            format!(
                "#!/bin/sh\ncase \"$*\" in\n*--list-secret-keys*) echo 'sec:u:255:22:0123456789ABCDEF:1:::::::scESC:::#:::23::0:' ;;\n*--list-keys*) printf 'pub:u:255:22:0123456789ABCDEF:1:::::::scESC:\\nfpr:::::::::{primary}:\\nsub:e:255:18:89ABCDEF0123:1:::::::e:\\nfpr:::::::::{sub}:\\n' ;;\n*--quick-set-expire*) echo \"gpg $*\" >> '{asked}'; if [ -e '{fail}' ]; then echo '[GNUPG:] FAILURE sign 100663358' >&2; exit 2; fi ;;\nesac\nexit 0\n",
                asked = asked.display(),
                fail = fail.display()
            ),
        );
        script("gpgconf", format!("#!/bin/sh\necho \"gpgconf $*\" >> '{}'\n", asked.display()));
        let home = dir.join("home");
        let gnupg = Gnupg::with_home(&gpg, &home);
        gnupg.renew(primary, "2y", &|_| {}).unwrap();
        let lines: Vec<String> = std::fs::read_to_string(&asked).unwrap().lines().map(str::to_string).collect();
        let homedir = format!("--homedir {}", home.display());
        assert_eq!(
            lines,
            [
                format!("gpg {homedir} --batch --no-tty --status-fd 2 --quick-set-expire {primary} 2y"),
                format!("gpg {homedir} --batch --no-tty --status-fd 2 --quick-set-expire {primary} 2y {sub}"),
                format!("gpgconf {homedir} --kill scdaemon"),
            ],
            "the key and its expired subkey, each named; then the key let go, once"
        );
        // A touch missed: said as such, and the key let go all the same.
        std::fs::write(&asked, "").unwrap();
        std::fs::write(&fail, "").unwrap();
        assert_eq!(gnupg.renew(primary, "2y", &|_| {}), Err(GnupgProblem::TouchMissed));
        let lines = std::fs::read_to_string(&asked).unwrap();
        assert_eq!(lines.lines().filter(|l| l.starts_with("gpgconf")).count(), 1, "{lines}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// GnuPG's status lines, invented as GnuPG writes them: its pinentry
    /// opened, and a security key that waited in vain for a touch (the code
    /// GPG_ERR_TIMEOUT, from the smart card daemon or not), whatever the
    /// language of its words.
    /// The tests that hold the pause on the keys, one at a time: it is one for the whole program.
    static ON_CARD_TESTS: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn gnupg_says_where_it_is_and_a_touch_missed() {
        let _alone = ON_CARD_TESTS.lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(status_of("[GNUPG:] PINENTRY_LAUNCHED 41235 gnome3 1.3.1 - - - - 1000/1000 0"), Some(Status::Pinentry(41235)));
        assert_eq!(status_of("[GNUPG:] FAILURE sign 100663358"), Some(Status::Failed(GPG_ERR_TIMEOUT)), "6 << 24 | 62: the smart card daemon's timeout");
        assert_eq!(status_of("[GNUPG:] ERROR keyedit.setexpire 62"), Some(Status::Failed(GPG_ERR_TIMEOUT)));
        assert_eq!(status_of("[GNUPG:] FAILURE sign 83918950"), Some(Status::Failed(83918950 & 0xFFFF)));
        assert_eq!(status_of("[GNUPG:] KEY_CONSIDERED ABCD 0"), None);
        assert_eq!(status_of("gpg: signing failed: Délai d'attente dépassé"), None);
        // While Sioul's own GnuPG works with a key, nested or not: no look; then again.
        assert!(may_look());
        let first = OnCard::start();
        let second = OnCard::start();
        assert!(!may_look());
        drop(first);
        assert!(!may_look());
        drop(second);
        assert!(may_look());
    }

    /// A server on this computer answering each POST with the next of
    /// `answers`, and telling which path and body each was.
    fn stand_in_posts(answers: Vec<(u16, &'static str)>) -> (String, std::sync::mpsc::Receiver<(String, String)>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        let (tell, heard) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for (status, body) in answers {
                let Ok((stream, _)) = listener.accept() else { return };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let mut length = 0;
                loop {
                    let mut header = String::new();
                    reader.read_line(&mut header).unwrap();
                    if header.trim_end().is_empty() {
                        break;
                    }
                    if let Some((name, value)) = header.split_once(':')
                        && name.eq_ignore_ascii_case("content-length")
                    {
                        length = value.trim().parse().unwrap_or(0);
                    }
                }
                let mut sent = vec![0u8; length];
                std::io::Read::read_exact(&mut reader, &mut sent).unwrap();
                let _ = tell.send((line.split_whitespace().nth(1).unwrap_or("").to_string(), String::from_utf8_lossy(&sent).to_string()));
                let mut stream = stream;
                let _ = stream.write_all(format!("HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
            }
        });
        (address, heard)
    }

    /// "Send it to keys.openpgp.org", against a stand-in server: the
    /// certificate sent, then a link asked for each address not published
    /// yet, in your language; a refusal said with the server's words.
    #[test]
    fn keys_openpgp_gets_the_certificate_and_mails_each_address() {
        let (_, public) = SoftCard::generate("0006:12345678", &["Card Holder <me@example.org>", "Card Holder <old@example.org>"], "123456").unwrap();
        let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(5))).http_status_as_error(false).build().into();
        let (base, heard) = stand_in_posts(vec![
            (200, r#"{"key_fpr":"ABCD","status":{"me@example.org":"unpublished","old@example.org":"published"},"token":"t0k3n"}"#),
            (200, r#"{"key_fpr":"ABCD","status":{"me@example.org":"pending","old@example.org":"published"},"token":"t0k3n"}"#),
        ]);
        let published = send_at(&agent, &base, &public, "fr").unwrap();
        assert_eq!(published, Published { fingerprint: "ABCD".into(), mailed: vec!["me@example.org".into()], published: vec!["old@example.org".into()] });
        let (path, body) = heard.recv().unwrap();
        assert_eq!(path, "/vks/v1/upload");
        let sent: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(sent["keytext"].as_str().unwrap().as_bytes(), &public[..], "the public certificate, as it is");
        let (path, body) = heard.recv().unwrap();
        assert_eq!(path, "/vks/v1/request-verify");
        assert_eq!(serde_json::from_str::<serde_json::Value>(&body).unwrap(), serde_json::json!({ "token": "t0k3n", "addresses": ["me@example.org"], "locale": ["fr"] }));
        // Every address published already: nothing asked.
        let (base, heard) = stand_in_posts(vec![(200, r#"{"key_fpr":"ABCD","status":{"me@example.org":"published"},"token":"t"}"#)]);
        assert_eq!(send_at(&agent, &base, &public, "en").unwrap().mailed, Vec::<String>::new());
        assert_eq!(heard.try_iter().count(), 1);
        // Refused: the server's words.
        let (base, _heard) = stand_in_posts(vec![(400, r#"{"error":"Rate limit exceeded"}"#)]);
        assert!(matches!(send_at(&agent, &base, &public, "en"), Err(SyncError::Server(said)) if said.contains("Rate limit exceeded")));
        // Never in clear: the agent Sioul sends with refuses http.
        let https: ureq::Agent = ureq::Agent::config_builder().https_only(true).timeout_global(Some(Duration::from_secs(5))).build().into();
        let (base, _heard) = stand_in_posts(vec![]);
        assert!(send_at(&https, &base, &public, "en").is_err());
    }
}
