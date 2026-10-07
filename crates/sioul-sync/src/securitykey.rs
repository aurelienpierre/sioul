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
                args.set("detail", detail.clone());
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
    let mut command = std::process::Command::new(program);
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
}
