// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Sioul's own checks of who sent a message: SPF, DKIM, DMARC, ARC and the
//! reverse DNS of the server that handed it to your provider, done when the
//! message is fetched, through the system's DNS (Stalwart's `mail-auth`).
//!
//! Some providers record no checks (a shared host may write only a spam score), and
//! those that do differ. The results are written on top of the stored message
//! as an Authentication-Results header (RFC 8601) under Sioul's own name for
//! this installation (`config::sioul_authserv_id`), which the Porch trusts
//! first. Checking when the message arrives also matters: DKIM keys rotate, and
//! some signatures expire within days.

use mail_auth::common::headers::HeaderWriter;
use mail_auth::spf::verify::SpfParameters;
use mail_auth::dmarc::verify::DmarcParameters;
use mail_auth::{AuthenticatedMessage, AuthenticationResults, DmarcResult, MessageAuthenticator};
use sioul_core::config::sioul_authserv_id;
use sioul_core::headers::RawHeaders;
use sioul_core::trust;
use std::path::Path;
use std::time::Duration;
use tokio::time::timeout;

/// The longest one message's checks may take: DNS can be slow, never blocking.
const CHECKS: Duration = Duration::from_secs(20);

pub struct Verifier {
    authenticator: MessageAuthenticator,
    id: String,
}

impl Verifier {
    pub fn new() -> Option<Verifier> {
        let authenticator = MessageAuthenticator::new_system_conf().ok()?;
        Some(Verifier { authenticator, id: sioul_authserv_id() })
    }

    /// For checking many messages in one run (the spam filter's training
    /// corpus, `sioul_learn::corpus`): the system's DNS, each question asked
    /// once and given `timeout`, each answer kept for the run (the
    /// resolver's cache, the names that do not exist too).
    pub fn quick(timeout: Duration) -> Option<Verifier> {
        let (config, mut options) = mail_auth::hickory_resolver::system_conf::read_system_conf().ok()?;
        options.timeout = timeout;
        options.attempts = 1;
        options.cache_size = options.cache_size.max(16_384);
        let authenticator = MessageAuthenticator::new(config, options).ok()?;
        Some(Verifier { authenticator, id: sioul_authserv_id() })
    }

    /// Sioul's results for a message, as `stamp` writes them on top of it
    /// ("Authentication-Results: sioul-….invalid; dkim=pass …"); none when
    /// it cannot be read. `whole` false: its header block alone (a message
    /// too large to fetch whole), so what needs the body is left unsaid
    /// (DKIM, ARC), and DMARC is said only when it passes (by SPF: a
    /// signature left unchecked might have passed it). No time limit: the
    /// caller's.
    pub async fn results_of(&self, raw: &[u8], whole: bool) -> Option<String> {
        self.results(raw, whole).await
    }

    /// Whether this verifier's DNS answers at all, asked for a name under
    /// `domain` made up for the question, so that no cache holds it: a
    /// server saying it does not exist answers too.
    pub async fn answers(&self, domain: &str) -> bool {
        let made_up = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
        let name = format!("sioul-{made_up:x}.{}.", domain.trim_end_matches('.'));
        match timeout(CHECKS, self.authenticator.resolver().lookup_ip(name.as_str())).await {
            Ok(Ok(_)) => true,
            Ok(Err(e)) => e.is_no_records_found(),
            Err(_) => false,
        }
    }

    /// The message with Sioul's results on top; unchanged if it cannot be read,
    /// is already stamped, or the checks do not finish in time (a later pass retries).
    pub async fn stamp(&self, raw: &[u8]) -> Vec<u8> {
        if self.stamped(raw) {
            return raw.to_vec();
        }
        match timeout(CHECKS, self.results(raw, true)).await {
            Ok(Some(header)) => [header.as_bytes(), raw].concat(),
            _ => raw.to_vec(),
        }
    }

    /// Whether the message already carries Sioul's results.
    pub fn stamped(&self, raw: &[u8]) -> bool {
        let head = &raw[..raw.len().min(16 * 1024)];
        let needle = format!("Authentication-Results: {}", self.id);
        String::from_utf8_lossy(head).contains(&needle)
    }

    async fn results(&self, raw: &[u8], whole: bool) -> Option<String> {
        let message = AuthenticatedMessage::parse(raw)?;
        let headers = RawHeaders::parse(raw);
        // Without the body, no signature can be checked: none is said.
        let dkim = if whole { self.authenticator.verify_dkim(&message).await } else { Vec::new() };
        let arc = if whole { Some(self.authenticator.verify_arc(&message).await) } else { None };
        let entry = trust::boundary(&headers);
        let mail_from = trust::mail_from(&headers).unwrap_or_default();
        let mail_from_domain = mail_from.rsplit_once('@').map_or(String::new(), |(_, d)| d.to_ascii_lowercase());
        let results = AuthenticationResults::new(&self.id).with_dkim_results(&dkim, message.from());
        // Mail that never came from outside (your provider's own notices, a message
        // to yourself delivered inside one provider) has no server to check SPF on,
        // so DMARC cannot judge it: only its signatures are recorded.
        let Some(e) = entry else { return Some(results.to_header()) };
        let spf = self.authenticator.verify_spf(SpfParameters::verify_mail_from(e.ip, &e.helo, &e.by, &mail_from)).await;
        let spf_domain = if mail_from_domain.is_empty() { e.helo.clone() } else { mail_from_domain };
        let dmarc = self.authenticator.verify_dmarc(DmarcParameters::new(&message, &dkim, &spf_domain, &spf)).await;
        let iprev = self.authenticator.verify_iprev(e.ip).await;
        let mut results = results.with_spf_mailfrom_result(&spf, e.ip, &mail_from, &e.helo);
        if let Some(arc) = &arc {
            results = results.with_arc_result(arc, e.ip);
        }
        results = results.with_iprev_result(&iprev, e.ip);
        if whole || dmarc.result() == DmarcResult::Pass {
            results = results.with_dmarc_result(&dmarc);
        }
        Some(results.to_header())
    }

    /// The message without Sioul's results, to check it again: they are the first header.
    pub fn unstamp(&self, raw: &[u8]) -> Vec<u8> {
        if !raw.starts_with(format!("Authentication-Results: {}", self.id).as_bytes()) {
            return raw.to_vec();
        }
        let mut end = 0;
        for line in raw.split_inclusive(|b| *b == b'\n') {
            if end > 0 && !line.starts_with(b" ") && !line.starts_with(b"\t") {
                break;
            }
            end += line.len();
        }
        raw[end..].to_vec()
    }
}

/// Checks every message of a Maildir that Sioul has not checked yet (with
/// `again`, every message, its earlier results replaced), and writes the
/// results on top of it, in place. Returns how many were checked.
pub fn verify_folder(root: &Path, again: bool) -> usize {
    let Some(verifier) = Verifier::new() else { return 0 };
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread().enable_all().build() else { return 0 };
    runtime.block_on(async {
        let mut checked = 0;
        for sub in ["new", "cur"] {
            let Ok(entries) = std::fs::read_dir(root.join(sub)) else { continue };
            for path in entries.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.is_file()) {
                let Ok(raw) = std::fs::read(&path) else { continue };
                let raw = if again { verifier.unstamp(&raw) } else { raw };
                if verifier.stamped(&raw) {
                    continue;
                }
                let stamped = verifier.stamp(&raw).await;
                if stamped.len() == raw.len() {
                    continue;
                }
                // Written beside, then moved over: the file keeps its name, so its flags and origin stay.
                let temporary = root.join("tmp").join(path.file_name().unwrap_or_default());
                let written = std::fs::create_dir_all(root.join("tmp")).and_then(|()| write_private(&temporary, &stamped));
                let modified = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
                if written.is_ok() && std::fs::rename(&temporary, &path).is_ok() {
                    if let (Some(time), Ok(file)) = (modified, std::fs::File::options().write(true).open(&path)) {
                        let _ = file.set_modified(time);
                    }
                    checked += 1;
                }
            }
        }
        checked
    })
}

/// Mail written yours alone, as it was fetched (`maildir::store`): not
/// readable by the other accounts of a shared computer, whatever the umask.
fn write_private(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}
