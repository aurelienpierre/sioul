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
//!
//! **Your provider's subject tag** (part 2 of 4 November 2026): a filter that
//! calls a message spam may write a tag at the start of its Subject
//! (`***Possible-SPAM***`, `[SPAM]`), which breaks a signature over the
//! Subject though nothing its sender wrote changed. When a filter of your
//! provider's marked the message (`trust::filter_marked`) and a signature
//! failed, the tags found by their shape (`trust::subject_tag_ends`) are taken
//! off one at a time and DKIM checked again; a candidate counts only when a
//! signature verifies over it, which it can only do over the Subject its
//! signer signed. DMARC is then judged again from that result. The stamp says
//! so beside each result it changed: `dkim=pass (after your provider's subject
//! tag)`, `dmarc=pass (after your provider's subject tag)`; the Subject shown
//! is the signed one (`trust::restored_tags`). Any other change of the content
//! fails as it should. **A report around the original** (SpamAssassin's
//! `report_safe` 1 or 2): the original, attached whole, is checked instead,
//! and the stamp says so after its id.

use mail_auth::common::headers::HeaderWriter;
use mail_auth::spf::verify::SpfParameters;
use mail_auth::dmarc::verify::DmarcParameters;
use mail_auth::{AuthenticatedMessage, AuthenticationResults, DkimOutput, DkimResult, DmarcResult, MX, MessageAuthenticator, Parameters, RecordSet, ResolverCache, Txt};
use sioul_core::config::sioul_authserv_id;
use sioul_core::headers::RawHeaders;
use sioul_core::trust;
use std::borrow::Borrow;
use std::collections::HashMap;
use std::hash::Hash;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;
use tokio::time::timeout;

/// The longest one message's checks may take: DNS can be slow, never blocking.
const CHECKS: Duration = Duration::from_secs(20);

pub struct Verifier {
    authenticator: MessageAuthenticator,
    id: String,
}

/// Answers kept for one message's checks, by name: what is asked twice (a
/// key, once the Subject is restored; a DMARC record) is asked of the DNS
/// once; a test fills them with its own, so that no DNS is asked.
pub struct Kept<K, V>(Mutex<HashMap<K, V>>);

impl<K, V> Default for Kept<K, V> {
    fn default() -> Kept<K, V> {
        Kept(Mutex::new(HashMap::new()))
    }
}

impl<K: Hash + Eq, V: Clone> ResolverCache<K, V> for Kept<K, V> {
    fn get<Q>(&self, name: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.0.lock().ok()?.get(name).cloned()
    }

    fn remove<Q>(&self, name: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.0.lock().ok()?.remove(name)
    }

    fn insert(&self, key: K, value: V, _: std::time::Instant) {
        if let Ok(mut kept) = self.0.lock() {
            kept.insert(key, value);
        }
    }
}

/// The answers of one message's checks, each kind of record by its name.
#[derive(Default)]
pub struct Answers {
    pub txt: Kept<Box<str>, Txt>,
    pub mx: Kept<Box<str>, RecordSet<MX>>,
    pub ipv4: Kept<Box<str>, RecordSet<Ipv4Addr>>,
    pub ipv6: Kept<Box<str>, RecordSet<Ipv6Addr>>,
    pub ptr: Kept<IpAddr, RecordSet<Box<str>>>,
}

type Asked<'a, P> = Parameters<'a, P, Kept<Box<str>, Txt>, Kept<Box<str>, RecordSet<MX>>, Kept<Box<str>, RecordSet<Ipv4Addr>>, Kept<Box<str>, RecordSet<Ipv6Addr>>, Kept<IpAddr, RecordSet<Box<str>>>>;

/// A check's parameters, its answers looked for in `answers` first.
fn asked<P>(params: P, answers: &Answers) -> Asked<'_, P> {
    Parameters::new(params).with_txt_cache(&answers.txt).with_mx_cache(&answers.mx).with_ipv4_cache(&answers.ipv4).with_ipv6_cache(&answers.ipv6).with_ptr_cache(&answers.ptr)
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
        self.results_with(raw, whole, &Answers::default()).await
    }

    /// `results`, the DNS's answers looked for in `answers` first. A report
    /// your provider's filter made around the original (`trust::report_original`):
    /// the original's checks, said so after the stamp's id.
    pub async fn results_with(&self, raw: &[u8], whole: bool, answers: &Answers) -> Option<String> {
        if whole && let Some((original, tags)) = trust::report_original(raw) {
            let header = self.checks(&original, raw, true, answers, tags).await?;
            let id = format!("Authentication-Results: {}", self.id);
            return Some(header.replacen(&id, &format!("{id} (checked on the original your provider attached)"), 1));
        }
        self.checks(raw, raw, whole, answers, 0).await
    }

    /// The checks of `raw`, where it came in read from `route` (the message
    /// itself, or the report around it); `tagged`: the tags your provider's
    /// report put before the original's Subject, said beside the results that
    /// passed on the original. Your provider's subject tag taken off when a
    /// signature failed and a filter marked the message (see the module).
    async fn checks(&self, raw: &[u8], route: &[u8], whole: bool, answers: &Answers, tagged: usize) -> Option<String> {
        let message = AuthenticatedMessage::parse(raw)?;
        let headers = RawHeaders::parse(route);
        // Without the body, no signature can be checked: none is said.
        let dkim = if whole { self.authenticator.verify_dkim(asked(&message, answers)).await } else { Vec::new() };
        // The Subject without the tags of a filter that marked it: the first
        // candidate over which a signature that failed now verifies.
        let failed = |outputs: &[DkimOutput]| outputs.iter().any(|d| *d.result() != DkimResult::Pass);
        let mut restored_tags = 0;
        if whole && failed(&dkim) && trust::filter_marked(&headers) {
            for tags in 1..=trust::SUBJECT_TAGS {
                let Some(candidate) = trust::without_subject_tags(raw, tags) else { break };
                let Some(candidate) = AuthenticatedMessage::parse(&candidate) else { continue };
                let again = self.authenticator.verify_dkim(asked(&candidate, answers)).await;
                if newly_passed(&dkim, &again).iter().any(|n| *n) {
                    restored_tags = tags;
                    break;
                }
            }
        }
        let restored_raw = (restored_tags > 0).then(|| trust::without_subject_tags(raw, restored_tags)).flatten();
        let restored = restored_raw.as_deref().and_then(AuthenticatedMessage::parse);
        let after = match &restored {
            Some(candidate) => self.authenticator.verify_dkim(asked(candidate, answers)).await,
            None => Vec::new(),
        };
        let (judged, signatures) = match &restored {
            Some(candidate) => (candidate, &after),
            None => (&message, &dkim),
        };
        let marked = if restored.is_some() { newly_passed(&dkim, &after) } else { vec![tagged > 0; signatures.len()] };
        let tags = if restored.is_some() { restored_tags } else { tagged };
        let arc = if whole { Some(self.authenticator.verify_arc(asked(&message, answers)).await) } else { None };
        let entry = trust::boundary(&headers);
        let mail_from = trust::mail_from(&headers).unwrap_or_default();
        let mail_from_domain = mail_from.rsplit_once('@').map_or(String::new(), |(_, d)| d.to_ascii_lowercase());
        let results = AuthenticationResults::new(&self.id).with_dkim_results(signatures, judged.from());
        // Mail that never came from outside (your provider's own notices, a message
        // to yourself delivered inside one provider) has no server to check SPF on,
        // so DMARC cannot judge it: only its signatures are recorded.
        let Some(e) = entry else { return Some(said_after_tags(&results.to_header(), &marked, false, tags)) };
        let spf = self.authenticator.verify_spf(asked(SpfParameters::verify_mail_from(e.ip, &e.helo, &e.by, &mail_from), answers)).await;
        let spf_domain = if mail_from_domain.is_empty() { e.helo.clone() } else { mail_from_domain };
        let dmarc = self.authenticator.verify_dmarc(asked(DmarcParameters::new(judged, signatures, &spf_domain, &spf), answers)).await;
        // DMARC judged again from the signatures over the Subject restored: said when that made it pass.
        let dmarc_after = tags > 0
            && dmarc.result() == DmarcResult::Pass
            && match &restored {
                Some(_) => self.authenticator.verify_dmarc(asked(DmarcParameters::new(&message, &dkim, &spf_domain, &spf), answers)).await.result() != DmarcResult::Pass,
                None => true,
            };
        let iprev = self.authenticator.verify_iprev(asked(e.ip, answers)).await;
        let mut results = results.with_spf_mailfrom_result(&spf, e.ip, &mail_from, &e.helo);
        if let Some(arc) = &arc {
            results = results.with_arc_result(arc, e.ip);
        }
        results = results.with_iprev_result(&iprev, e.ip);
        if whole || dmarc.result() == DmarcResult::Pass {
            results = results.with_dmarc_result(&dmarc);
        }
        Some(said_after_tags(&results.to_header(), &marked, dmarc_after, tags))
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

/// For each signature, in order, whether it passed only `after` (with the
/// Subject restored) where it had not `before`.
fn newly_passed(before: &[DkimOutput], after: &[DkimOutput]) -> Vec<bool> {
    after.iter().enumerate().map(|(i, a)| *a.result() == DkimResult::Pass && before.get(i).is_none_or(|b| *b.result() != DkimResult::Pass)).collect()
}

/// The stamp with `trust::after_tags_comment(tags)` beside each DKIM result
/// that passed only with the Subject restored (`marked`, the signatures in
/// the order the header lists them), and beside DMARC when that made it pass.
fn said_after_tags(header: &str, marked: &[bool], dmarc: bool, tags: usize) -> String {
    if tags == 0 || (!marked.contains(&true) && !dmarc) {
        return header.to_string();
    }
    let comment = trust::after_tags_comment(tags);
    let mut out = String::with_capacity(header.len() + 64);
    let mut rest = header;
    let mut signature = 0;
    while let Some(at) = rest.find("\tdkim=") {
        let (before, after) = rest.split_at(at + "\tdkim=".len());
        out.push_str(before);
        if marked.get(signature).copied().unwrap_or(false) && after.starts_with("pass") {
            out.push_str("pass ");
            out.push_str(&comment);
            rest = &after["pass".len()..];
        } else {
            rest = after;
        }
        signature += 1;
    }
    out.push_str(rest);
    if dmarc {
        out = out.replacen("\tdmarc=pass", &format!("\tdmarc=pass {comment}"), 1);
    }
    out
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

/// Whether a stored message is one to check again for your provider's
/// subject tag (`recheck_tagged`): Sioul's stamp on it says no signature
/// passed though one is there, a filter of your provider's marked it, and its
/// Subject starts with a tag.
fn tagged_and_failed(raw: &[u8], id: &str) -> bool {
    let headers = RawHeaders::parse(&raw[..raw.len().min(256 * 1024)]);
    let results = trust::read_auth_results(&headers, &[id.to_string()]);
    let signed = headers.all("DKIM-Signature").next().is_some();
    signed && results.is_some_and(|r| r.dkim != Some(trust::Outcome::Pass)) && trust::filter_marked(&headers) && trust::without_subject_tags(raw, 1).is_some()
}

/// Messages stored before Sioul knew your provider's subject tags (part 2 of
/// 4 November 2026), checked again: those of one Maildir folder `root` that
/// `tagged_and_failed` takes. A stamp is written again (in place, its file's
/// name and time kept, as `verify_folder` writes) only when a signature
/// verifies with the Subject restored; the others keep the stamp they got as
/// they came, made with the keys of that day. Returns the files written
/// again, and whether the DNS answered (else nothing was decided).
pub fn recheck_tagged(root: &Path) -> (Vec<PathBuf>, bool) {
    let Some(verifier) = Verifier::new() else { return (Vec::new(), false) };
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread().enable_all().build() else { return (Vec::new(), false) };
    runtime.block_on(async {
        let mut written = Vec::new();
        let mut asked = false;
        for sub in ["new", "cur"] {
            let Ok(entries) = std::fs::read_dir(root.join(sub)) else { continue };
            for path in entries.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.is_file()) {
                let Ok(raw) = std::fs::read(&path) else { continue };
                if !verifier.stamped(&raw) || !tagged_and_failed(&raw, &verifier.id) {
                    continue;
                }
                // Once per folder: whether the DNS answers at all, under the sender's domain.
                if !asked {
                    let domain = RawHeaders::parse(&raw).first("From").and_then(|f| f.rsplit_once('@').map(|(_, d)| d.trim_end_matches(['>', ' ']).to_string())).unwrap_or_default();
                    if domain.is_empty() || !verifier.answers(&domain).await {
                        return (written, false);
                    }
                    asked = true;
                }
                let unstamped = verifier.unstamp(&raw);
                let Ok(Some(header)) = timeout(CHECKS, verifier.results(&unstamped, true)).await else { continue };
                if !header.contains("(after your provider's") {
                    continue;
                }
                let stamped = [header.as_bytes(), &unstamped[..]].concat();
                if write_in_place(root, &path, &stamped) {
                    written.push(path);
                }
            }
        }
        (written, true)
    })
}

/// The folders of your accounts checked again for your provider's subject
/// tags (`recheck_tagged`), once on this device: `$XDG_STATE_HOME/sioul/verify/subject-tags.toml`
/// names each folder done, by its key (`account/folder`). A folder is done
/// once its pass got answers from the DNS; offline, it waits for the next
/// time. Returns the files written again, with their folder's key.
pub fn recheck_tagged_once(folders: &[(String, PathBuf)]) -> Vec<(String, PathBuf)> {
    #[derive(serde::Serialize, serde::Deserialize, Default)]
    struct Done {
        #[serde(default)]
        folders: Vec<String>,
    }
    let path = sioul_core::config::state_dir().join("verify").join("subject-tags.toml");
    let mut done: Done = std::fs::read_to_string(&path).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default();
    let mut written = Vec::new();
    let mut changed = false;
    let waiting: Vec<&(String, PathBuf)> = folders.iter().filter(|(key, _)| !done.folders.contains(key)).collect();
    for (key, root) in waiting {
        let (files, answered) = recheck_tagged(root);
        written.extend(files.into_iter().map(|f| (key.clone(), f)));
        if answered {
            done.folders.push(key.clone());
            changed = true;
        }
    }
    if changed && let Ok(text) = toml::to_string(&done) {
        let _ = std::fs::create_dir_all(path.parent().unwrap_or(Path::new("."))).and_then(|()| sioul_core::filelock::replace(&path, text.as_bytes()));
    }
    written
}

/// `bytes` over the stored message `path` of Maildir folder `root`: written
/// beside in its `tmp/`, then moved over, its name and time kept, so its
/// flags and origin stay. A file a sync renamed or took away meanwhile (a
/// flag changed, the message moved) is left as it is now: never a second copy.
fn write_in_place(root: &Path, path: &Path, bytes: &[u8]) -> bool {
    let temporary = root.join("tmp").join(path.file_name().unwrap_or_default());
    let written = std::fs::create_dir_all(root.join("tmp")).and_then(|()| write_private(&temporary, bytes));
    let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok();
    if written.is_ok() && modified.is_some() && std::fs::rename(&temporary, path).is_ok() {
        if let (Some(time), Ok(file)) = (modified, std::fs::File::options().write(true).open(path)) {
            let _ = file.set_modified(time);
        }
        return true;
    }
    let _ = std::fs::remove_file(&temporary);
    false
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

#[cfg(test)]
mod tests {
    use super::{Answers, Verifier, recheck_tagged_once, tagged_and_failed, write_in_place};
    use mail_auth::arc::ArcSealer;
    use mail_auth::common::headers::HeaderWriter;
    use mail_auth::common::crypto::Ed25519Key;
    use mail_auth::common::parse::TxtRecordParser;
    use mail_auth::common::verify::DomainKey;
    use mail_auth::{AuthenticatedMessage, AuthenticationResults, DkimResult, MessageAuthenticator, Parameters, ResolverCache, Txt};
    use sioul_core::headers::RawHeaders;
    use std::borrow::Borrow;
    use std::collections::HashMap;
    use std::hash::Hash;
    use std::sync::Mutex;
    use std::time::Instant;

    /// The test's own answers for the keys asked (a provider's ARC key): no DNS.
    struct Keys(Mutex<HashMap<Box<str>, Txt>>);

    impl ResolverCache<Box<str>, Txt> for Keys {
        fn get<Q>(&self, name: &Q) -> Option<Txt>
        where
            Box<str>: Borrow<Q>,
            Q: Hash + Eq + ?Sized,
        {
            self.0.lock().ok()?.get(name).cloned()
        }
        fn remove<Q>(&self, name: &Q) -> Option<Txt>
        where
            Box<str>: Borrow<Q>,
            Q: Hash + Eq + ?Sized,
        {
            self.0.lock().ok()?.remove(name)
        }
        fn insert(&self, key: Box<str>, value: Txt, _: Instant) {
            if let Ok(mut keys) = self.0.lock() {
                keys.insert(key, value);
            }
        }
    }

    // An Ed25519 key of mail-auth's own tests (public test vectors), as a stand-in provider's ARC key.
    const SEED: &str = "nWGxne/9WmC6hEr0kuwsxERJxWl7MmkZcDusAxyuf2A=";
    const PUBLIC: &str = "v=DKIM1; k=ed25519; p=11qYAYKxCrfVS/7TyWQHOg7hcvPapiMlrwIaaPcHURo=";

    /// What Sioul's own verifier (mail-auth) says of a lone arrival seal, as
    /// Gmail and Microsoft put on every message they receive (review of
    /// 01ca9df, 1): i=1, cv=none, valid, is a chain that passes. So arc=pass
    /// alone vouches for nothing, and trust reads the seal placed before the
    /// provider received the message: here none (`trust::arc_voucher`).
    #[tokio::test]
    async fn a_lone_arrival_seal_passes_and_vouches_for_nothing() {
        let raw = "From: Bank <news@bank.example>\r\nTo: noa@example.org\r\nSubject: Offers\r\nList-Id: <news.lists.bank.example>\r\n\r\nThis week.\r\n";
        let message = AuthenticatedMessage::parse(raw.as_bytes()).unwrap();
        let authenticator = MessageAuthenticator::new_system_conf().unwrap();
        let keys = Keys(Mutex::new(HashMap::new()));
        keys.insert("arrival._domainkey.provider.example.".into(), DomainKey::parse(PUBLIC.as_bytes()).unwrap().into(), Instant::now());
        // Sealed on arrival by the provider: the first and only set.
        let unsealed = authenticator.verify_arc(Parameters::new(&message).with_txt_cache(&keys)).await;
        assert_eq!(unsealed.result(), &DkimResult::None);
        let results = AuthenticationResults::new("mx.provider.example");
        let seed = sioul_core::lines::base64_decode(SEED).unwrap();
        let public = sioul_core::lines::base64_decode(PUBLIC.rsplit_once("p=").unwrap().1).unwrap();
        let key = Ed25519Key::from_seed_and_public_key(&seed, &public).unwrap();
        let set = ArcSealer::from_key(key).domain("provider.example").selector("arrival").headers(["From", "To", "Subject"]).seal(&message, &results, &unsealed).unwrap();
        let sealed = format!("{}{}{raw}", set.to_header(), results.to_header());
        let message = AuthenticatedMessage::parse(sealed.as_bytes()).unwrap();
        let checked = authenticator.verify_arc(Parameters::new(&message).with_txt_cache(&keys)).await;
        assert_eq!(checked.result(), &DkimResult::Pass, "a lone arrival seal is a chain that passes");
        // Trusted as it should be: the provider's own arrival seal answers for nothing below it.
        let headers = RawHeaders::parse(sealed.as_bytes());
        let sealers = sioul_core::trust::sealers(&["mx.provider.example".to_string()], &[]);
        assert_eq!(sioul_core::trust::arc_voucher(&headers, &sealers), None);
    }

    // Your provider's subject tag (part 2 of 4 November 2026): a notice
    // signed by its own domain and sent through a bulk sender, its Subject
    // tagged by your provider's SpamAssassin; stand-in domains, the test's own
    // key, every DNS answer the test's own.

    use mail_auth::dkim::{Canonicalization, DkimSigner};
    use mail_auth::dmarc::Dmarc;
    use mail_auth::spf::Spf;
    use mail_auth::{DnsError, DnssecStatus, RecordSet};
    use std::net::{IpAddr, Ipv4Addr};
    use std::sync::Arc;

    const SENDER_IP: Ipv4Addr = Ipv4Addr::new(203, 0, 112, 9);

    /// The notice as its sender signed it (`body` its text), relaxed or simple for its header.
    fn signed_notice(body: &str, simple: bool) -> String {
        let unsigned = format!(
            "From: Notices <notification@notif.post.example>\r\nTo: noa@example.org\r\nSubject: Proof of posting\r\nDate: Wed, 04 Nov 2026 09:10:00 +0100\r\nMessage-ID: <proof-1@notif.post.example>\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n{body}\r\n"
        );
        let seed = sioul_core::lines::base64_decode(SEED).unwrap();
        let public = sioul_core::lines::base64_decode(PUBLIC.rsplit_once("p=").unwrap().1).unwrap();
        let key = Ed25519Key::from_seed_and_public_key(&seed, &public).unwrap();
        let signer = DkimSigner::from_key(key).domain("notif.post.example").selector("s1").headers(["From", "To", "Subject", "Date", "Message-ID"]);
        let signer = if simple { signer.header_canonicalization(Canonicalization::Simple) } else { signer };
        format!("{}{unsigned}", signer.sign(unsigned.as_bytes()).unwrap().to_header())
    }

    /// The notice as your provider delivered it: its filter's verdict and the
    /// line where it came in on top; with `tag`, its tag in the Subject.
    fn delivered(signed: &str, tag: &str, marked: bool) -> Vec<u8> {
        let verdict = if marked { "X-Spam-Flag: YES\r\nX-Spam-Status: Yes, score=5.3 required=5.0 tests=LONG_HEX_URI,MPART_ALT_DIFF,MIME_HTML_ONLY\r\n" } else { "" };
        let tagged = if tag.is_empty() { signed.to_string() } else { signed.replacen("Subject: Proof of posting", &format!("Subject: {tag} Proof of posting"), 1) };
        format!("{verdict}Return-Path: <bounce@notif.post.example>\r\nReceived: from mail.bulk.example (mail.bulk.example [{SENDER_IP}]) by mx.provider.example with ESMTPS; Wed, 04 Nov 2026 09:10:02 +0100\r\n{tagged}").into_bytes()
    }

    /// Every DNS answer the checks ask, the test's own: the notice's key, its
    /// bounce domain's SPF (`spf`), the postal service's DMARC (reject), the
    /// sending server's reverse name.
    fn stand_in_dns(spf: &str) -> Answers {
        let (answers, at) = (Answers::default(), Instant::now());
        answers.txt.insert("s1._domainkey.notif.post.example.".into(), DomainKey::parse(PUBLIC.as_bytes()).unwrap().into(), at);
        answers.txt.insert("notif.post.example.".into(), Spf::parse(spf.as_bytes()).unwrap().into(), at);
        answers.txt.insert("_dmarc.notif.post.example.".into(), Dmarc::parse(b"v=DMARC1; p=reject").unwrap().into(), at);
        for none in ["_dmarc.post.example.", "_dmarc.example."] {
            answers.txt.insert(none.into(), Txt::Error(mail_auth::Error::Dns(DnsError::RecordNotFound(mail_auth::hickory_resolver::proto::op::ResponseCode::NXDomain))), at);
        }
        answers.ptr.insert(IpAddr::V4(SENDER_IP), RecordSet { rrset: Arc::from(vec![Box::<str>::from("mail.bulk.example.")]), dnssec_status: DnssecStatus::Indeterminate }, at);
        for name in ["mail.bulk.example.", "mail.bulk.example"] {
            answers.ipv4.insert(name.into(), RecordSet { rrset: Arc::from(vec![SENDER_IP]), dnssec_status: DnssecStatus::Indeterminate }, at);
        }
        answers
    }

    /// What Sioul's own stamp says, judged as the Porch judges a sender.
    fn judged(stamp: &str, id: &str) -> sioul_core::trust::Trust {
        let headers = RawHeaders::parse(format!("{stamp}From: notification@notif.post.example\r\n\r\n").as_bytes());
        let results = sioul_core::trust::read_auth_results(&headers, &[id.to_string()]);
        sioul_core::trust::judge_sender(results.as_ref(), None, Some("notif.post.example")).0
    }

    /// The tag in the Subject breaks the signature; taken off, the signature
    /// verifies and the stamp says so; DMARC passed through SPF as it came,
    /// and stays a pass. The body changed: no
    /// candidate verifies, the check fails as it should. Simple canonicalisation
    /// too. No filter's mark: no tag is taken off.
    #[tokio::test]
    async fn your_provider_s_subject_tag_is_taken_off_before_dkim() {
        sioul_core::config::stand_ins_only();
        let verifier = Verifier::new().expect("a verifier");
        let id = verifier.id.clone();
        let spf_pass = "v=spf1 ip4:203.0.112.9 -all";
        let tagged = delivered(&signed_notice("Your letter was posted.", false), "***Possible-SPAM***", true);
        let stamp = verifier.results_with(&tagged, true, &stand_in_dns(spf_pass)).await.unwrap();
        assert!(stamp.contains("dkim=pass (after your provider's subject tag) header.d=notif.post.example"), "{stamp}");
        assert!(stamp.contains("dmarc=pass header.from=notif.post.example") && !stamp.contains("dmarc=pass (after"), "DMARC passed through SPF already: {stamp}");
        assert_eq!(judged(&stamp, &id), sioul_core::trust::Trust::Verified);
        // The Subject shown is the signed one, marked, under this installation's id only.
        let card = sioul_core::card::Card::from_bytes(&[stamp.as_bytes(), &tagged[..]].concat()).unwrap();
        assert_eq!((card.subject.as_str(), card.provider_tag_restored()), ("Proof of posting", true));
        assert_eq!(sioul_core::card::Card::from_bytes(&tagged).unwrap().subject, "***Possible-SPAM*** Proof of posting", "no stamp of ours: as it came");
        // The body changed on the way: the signature fails, with the tag or without.
        let changed = delivered(&signed_notice("Your letter was posted.", false).replace("posted", "held"), "***Possible-SPAM***", true);
        let stamp = verifier.results_with(&changed, true, &stand_in_dns(spf_pass)).await.unwrap();
        assert!(!stamp.contains("dkim=pass") && !stamp.contains("(after your provider's"), "{stamp}");
        // Simple canonicalisation, the tag of another shape.
        let simple = delivered(&signed_notice("Your letter was posted.", true), "[SPAM]", true);
        let stamp = verifier.results_with(&simple, true, &stand_in_dns(spf_pass)).await.unwrap();
        assert!(stamp.contains("dkim=pass (after your provider's subject tag)"), "{stamp}");
        // Without a filter's mark, the tag is the sender's: nothing taken off.
        let unmarked = delivered(&signed_notice("Your letter was posted.", false), "[SPAM]", false);
        let stamp = verifier.results_with(&unmarked, true, &stand_in_dns(spf_pass)).await.unwrap();
        assert!(!stamp.contains("dkim=pass") && !stamp.contains("(after your provider's"), "{stamp}");
        // As it came, untagged: a pass, nothing said of tags.
        let plain = delivered(&signed_notice("Your letter was posted.", false), "", true);
        let stamp = verifier.results_with(&plain, true, &stand_in_dns(spf_pass)).await.unwrap();
        assert!(stamp.contains("dkim=pass header.d=") && !stamp.contains("(after"), "{stamp}");
    }

    /// DMARC judged again from the signature over the Subject restored: SPF
    /// failed (a forward), DKIM broke only for the tag, the domain asks to
    /// reject: as it came, forged; restored, DMARC passes, its stamp says
    /// why, and the sender is verified.
    #[tokio::test]
    async fn dmarc_is_judged_again_from_the_restored_signature() {
        sioul_core::config::stand_ins_only();
        let verifier = Verifier::new().expect("a verifier");
        let id = verifier.id.clone();
        let spf_fail = "v=spf1 -all";
        let signed = signed_notice("Your letter was posted.", false);
        // The same failure without the tag to take off: forged, as it should be.
        let forged = delivered(&signed.replace("posted", "held"), "***Possible-SPAM***", true);
        let stamp = verifier.results_with(&forged, true, &stand_in_dns(spf_fail)).await.unwrap();
        assert!(stamp.contains("dmarc=fail") && stamp.contains("spf=fail"), "{stamp}");
        assert_eq!(judged(&stamp, &id), sioul_core::trust::Trust::Forged);
        let tagged = delivered(&signed, "***Possible-SPAM***", true);
        let stamp = verifier.results_with(&tagged, true, &stand_in_dns(spf_fail)).await.unwrap();
        assert!(stamp.contains("spf=fail") && stamp.contains("dkim=pass (after your provider's subject tag)") && stamp.contains("dmarc=pass (after your provider's subject tag)"), "{stamp}");
        assert_eq!(judged(&stamp, &id), sioul_core::trust::Trust::Verified);
    }

    /// A report your provider's filter made around the original
    /// (SpamAssassin's `report_safe` 1 and 2): the outer message's checks
    /// fail, the original attached whole is checked instead, and the stamp
    /// says so; its tag before the original's Subject said too, the Subject
    /// shown the original's.
    #[tokio::test]
    async fn a_report_around_the_original_is_checked_on_the_original() {
        sioul_core::config::stand_ins_only();
        let verifier = Verifier::new().expect("a verifier");
        let id = verifier.id.clone();
        let original = signed_notice("Your letter was posted.", false);
        for (kind, encoding) in [("message/rfc822", "8bit"), ("text/plain", "8bit")] {
            let report = format!(
                "X-Spam-Flag: YES\r\nReturn-Path: <bounce@notif.post.example>\r\nReceived: from mail.bulk.example (mail.bulk.example [{SENDER_IP}]) by mx.provider.example with ESMTPS; Wed, 04 Nov 2026 09:10:02 +0100\r\n\
                 From: Notices <notification@notif.post.example>\r\nTo: noa@example.org\r\nSubject: ***SPAM*** Proof of posting\r\nDate: Wed, 04 Nov 2026 09:10:00 +0100\r\nMIME-Version: 1.0\r\n\
                 Content-Type: multipart/mixed; boundary=\"----------=_5F3A\"\r\n\r\nThis is a multi-part message in MIME format.\r\n\r\n\
                 ------------=_5F3A\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: 8bit\r\n\r\nSpam detection software has identified this incoming email as possible spam.\r\n\r\n\
                 ------------=_5F3A\r\nContent-Type: {kind}; x-spam-type=original\r\nContent-Description: original message before SpamAssassin\r\nContent-Disposition: attachment\r\nContent-Transfer-Encoding: {encoding}\r\n\r\n{original}\r\n------------=_5F3A--\r\n"
            );
            let stamp = verifier.results_with(report.as_bytes(), true, &stand_in_dns("v=spf1 -all")).await.unwrap();
            assert!(stamp.contains("(checked on the original your provider attached)") && stamp.contains("dkim=pass (after your provider's subject tag)"), "{kind}: {stamp}");
            assert_eq!(judged(&stamp, &id), sioul_core::trust::Trust::Verified, "{kind}");
            let card = sioul_core::card::Card::from_bytes(&[stamp.as_bytes(), report.as_bytes()].concat()).unwrap();
            assert_eq!(card.subject, "Proof of posting", "{kind}");
        }
    }

    /// Stored mail checked again once for its provider's tag (the migration):
    /// a stamp that said dkim=fail on a tagged, marked message is written again
    /// in place, its name and time kept; a folder done is not looked at again.
    #[test]
    fn stored_tagged_mail_is_checked_again_once() {
        sioul_core::config::stand_ins_only();
        let verifier = Verifier::new().expect("a verifier");
        let tagged = delivered(&signed_notice("Your letter was posted.", false), "***Possible-SPAM***", true);
        let raw = [format!("Authentication-Results: {};\r\n\tdkim=fail header.d=notif.post.example header.s=s1\r\n", verifier.id).as_bytes(), &tagged[..]].concat();
        assert!(tagged_and_failed(&raw, &verifier.id));
        assert!(!tagged_and_failed(&[format!("Authentication-Results: {};\r\n\tdkim=pass header.d=notif.post.example\r\n", verifier.id).as_bytes(), &tagged[..]].concat(), &verifier.id), "it passed: nothing to do");
        let untagged = delivered(&signed_notice("Your letter was posted.", false), "", true);
        assert!(!tagged_and_failed(&[format!("Authentication-Results: {};\r\n\tdkim=fail\r\n", verifier.id).as_bytes(), &untagged[..]].concat(), &verifier.id), "no tag");
        // In place: written beside, its name and time kept.
        let root = sioul_core::config::data_dir().join(format!("recheck-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("cur")).unwrap();
        let file = root.join("cur").join(format!("1793700000.U7-1.sioul{}2,S", sioul_core::maildir::INFO));
        std::fs::write(&file, &raw).unwrap();
        let time = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_793_700_000);
        std::fs::File::options().write(true).open(&file).unwrap().set_modified(time).unwrap();
        assert!(write_in_place(&root, &file, b"Authentication-Results: x;\r\n\tdkim=pass\r\n\r\nHi"));
        assert_eq!((std::fs::read(&file).unwrap(), std::fs::metadata(&file).unwrap().modified().unwrap()), (b"Authentication-Results: x;\r\n\tdkim=pass\r\n\r\nHi".to_vec(), time));
        // A folder done is named once, and not looked at again.
        let done = sioul_core::config::state_dir().join("verify").join("subject-tags.toml");
        let _ = std::fs::remove_file(&done);
        std::fs::create_dir_all(done.parent().unwrap()).unwrap();
        std::fs::write(&done, "folders = [\"home/.Junk\"]\n").unwrap();
        assert!(recheck_tagged_once(&[("home/.Junk".into(), root.clone())]).is_empty());
        assert_eq!(std::fs::read_to_string(&done).unwrap(), "folders = [\"home/.Junk\"]\n", "nothing asked again");
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_file(&done);
    }
}
