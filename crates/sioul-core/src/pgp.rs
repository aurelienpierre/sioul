// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! OpenPGP (RFC 9580) for mail, with Sequoia and its pure-Rust cryptography.
//!
//! - **Your keys** live in `~/.local/share/sioul/pgp/own/`, their secret
//!   parts encrypted with a passphrase the system keyring keeps (`sioul-sync`
//!   asks it), so nothing is asked at each message. They come from an import
//!   (a file, GnuPG's export) or are made here.
//! - **Others' keys** live in `…/pgp/others/`: imported, found in their
//!   messages (Autocrypt), or looked up on request.
//! - **What this module does not do**: talk to the network, or to the
//!   keyring. Passphrases come in as arguments.

use crate::config::data_dir;
use sequoia_openpgp as openpgp;
use openpgp::cert::prelude::*;
use openpgp::crypto::{Password, SessionKey};
use openpgp::packet::{PKESK, SKESK};
use openpgp::parse::stream::{DecryptionHelper, DecryptorBuilder, DetachedVerifierBuilder, MessageLayer, MessageStructure, VerificationHelper};
use openpgp::parse::Parse;
use openpgp::policy::StandardPolicy;
use openpgp::serialize::stream::{Armorer, Encryptor, LiteralWriter, Message, Signer};
use openpgp::serialize::{Serialize, SerializeInto};
use openpgp::types::SymmetricAlgorithm;
use openpgp::{Cert, Fingerprint, KeyHandle};
use serde::Serialize as Serde;
use std::io::Write;
use std::path::PathBuf;

fn policy() -> StandardPolicy<'static> {
    StandardPolicy::new()
}

fn own_dir() -> PathBuf {
    data_dir().join("pgp").join("own")
}

fn others_dir() -> PathBuf {
    data_dir().join("pgp").join("others")
}

/// The keys Sioul knows: yours, with their secret parts, and others'.
#[derive(Default, Clone)]
pub struct Keys {
    pub own: Vec<Cert>,
    pub others: Vec<Cert>,
}

fn read_certs(dir: &std::path::Path) -> Vec<Cert> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "asc" || x == "pgp"))
        .filter_map(|p| std::fs::read(&p).ok())
        .flat_map(|bytes| CertParser::from_bytes(&bytes).map(|parser| parser.filter_map(Result::ok).collect::<Vec<_>>()).unwrap_or_default())
        .collect()
}

/// Whether a key holds `address` among its user IDs.
fn names(cert: &Cert, address: &str) -> bool {
    cert.userids().any(|u| u.userid().email().ok().flatten().is_some_and(|e| e.eq_ignore_ascii_case(address.trim())))
}

impl Keys {
    pub fn load() -> Keys {
        Keys { own: read_certs(&own_dir()), others: read_certs(&others_dir()) }
    }

    /// Your key for an address, able to sign now.
    pub fn own_for(&self, address: &str) -> Option<&Cert> {
        let policy = policy();
        self.own.iter().find(|c| names(c, address) && c.keys().secret().with_policy(&policy, None).alive().revoked(false).for_signing().next().is_some())
    }

    /// A key to encrypt to `address`: someone's, or yours.
    pub fn for_address(&self, address: &str) -> Option<&Cert> {
        let policy = policy();
        self.others.iter().chain(&self.own).filter(|c| names(c, address)).find(|c| c.keys().with_policy(&policy, None).alive().revoked(false).for_transport_encryption().next().is_some())
    }

    fn all(&self) -> impl Iterator<Item = &Cert> {
        self.own.iter().chain(&self.others)
    }

    fn by_handle(&self, handle: &KeyHandle) -> Option<&Cert> {
        self.all().find(|c| c.keys().any(|k| k.key().key_handle().aliases(handle)))
    }
}

/// A key as the window lists it.
#[derive(Debug, Clone, Serde, PartialEq, Eq)]
pub struct KeySummary {
    pub fingerprint: String,
    /// "Jane Exemple <jane@example.org>", one per user ID.
    pub names: Vec<String>,
    pub own: bool,
    /// "2026-10-03".
    pub created: String,
    /// "2029-10-03", or empty when it does not expire.
    pub expires: String,
    pub can_encrypt: bool,
}

fn summary(cert: &Cert, own: bool) -> KeySummary {
    let policy = policy();
    let valid = cert.with_policy(&policy, None).ok();
    let date = |t: std::time::SystemTime| jiff::Timestamp::try_from(t).map(|t| t.strftime("%Y-%m-%d").to_string()).unwrap_or_default();
    KeySummary {
        fingerprint: cert.fingerprint().to_hex(),
        names: cert.userids().map(|u| String::from_utf8_lossy(u.userid().value()).to_string()).collect(),
        own,
        created: date(cert.primary_key().key().creation_time()),
        expires: valid.as_ref().and_then(|v| v.primary_key().key_expiration_time()).map(date).unwrap_or_default(),
        can_encrypt: cert.keys().with_policy(&policy, None).alive().revoked(false).for_transport_encryption().next().is_some(),
    }
}

/// Every key, yours first.
pub fn summaries() -> Vec<KeySummary> {
    let keys = Keys::load();
    keys.own.iter().map(|c| summary(c, true)).chain(keys.others.iter().map(|c| summary(c, false))).collect()
}

/// Keeps a key, merged with what was known of it; secret parts go with yours.
fn store(cert: Cert) -> Result<KeySummary, String> {
    let own = cert.is_tsk();
    let dir = if own { own_dir() } else { others_dir() };
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{}.asc", cert.fingerprint().to_hex()));
    let merged = match std::fs::read(&path).ok().and_then(|bytes| Cert::from_bytes(&bytes).ok()) {
        Some(known) => known.merge_public_and_secret(cert).map_err(|e| e.to_string())?,
        None => cert,
    };
    let armored = if own { merged.as_tsk().armored().to_vec() } else { merged.armored().to_vec() }.map_err(|e| e.to_string())?;
    write_key(&path, &armored)?;
    Ok(summary(&merged, own))
}

/// Writes a key file beside, then moves it in place. On Unix it is readable by
/// you alone, whatever the umask: your secret keys, and whose keys you keep.
fn write_key(path: &std::path::Path, armored: &[u8]) -> Result<(), String> {
    let fail = |e: std::io::Error| format!("{}: {e}", path.display());
    let temporary = path.with_extension("tmp");
    // A file left by an interrupted write keeps its permissions when truncated: it goes first.
    let _ = std::fs::remove_file(&temporary);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temporary).map_err(fail)?;
    file.write_all(armored).and_then(|()| file.sync_all()).map_err(fail)?;
    drop(file);
    std::fs::rename(&temporary, path).map_err(fail)
}

/// Imports the keys of a file: armored or binary, public or secret.
pub fn import(bytes: &[u8]) -> Result<Vec<KeySummary>, String> {
    let certs: Vec<Cert> = CertParser::from_bytes(bytes).map_err(|e| e.to_string())?.filter_map(Result::ok).collect();
    if certs.is_empty() {
        return Err("no OpenPGP key".into());
    }
    certs.into_iter().map(store).collect()
}

/// Whether `passphrase` opens the secret parts of your key `fingerprint`.
pub fn opens(fingerprint: &str, passphrase: &str) -> bool {
    let keys = Keys::load();
    let Some(cert) = keys.own.iter().find(|c| c.fingerprint().to_hex() == fingerprint) else { return false };
    let policy = policy();
    let password = Password::from(passphrase);
    cert.keys().secret().with_policy(&policy, None).for_signing().next().is_some_and(|k| k.key().clone().decrypt_secret(&password).is_ok())
}

/// A long random passphrase for a key made here: nobody types it, the keyring keeps it.
pub fn new_passphrase() -> Result<String, String> {
    let mut bytes = [0u8; 24];
    openpgp::crypto::random(&mut bytes).map_err(|e| e.to_string())?;
    Ok(crate::lines::base64_encode(&bytes))
}

/// Makes a key for you: Curve25519, for signing and encrypting, valid three
/// years; its secret parts encrypted with `passphrase`.
pub fn generate(name: &str, address: &str, passphrase: &str) -> Result<KeySummary, String> {
    let userid = if name.trim().is_empty() { format!("<{}>", address.trim()) } else { format!("{} <{}>", name.trim(), address.trim()) };
    let (cert, _revocation) = CertBuilder::general_purpose(Some(userid))
        .set_cipher_suite(CipherSuite::Cv25519)
        .set_validity_period(std::time::Duration::from_secs(3 * 365 * 24 * 3600))
        .set_password(Some(Password::from(passphrase)))
        .generate()
        .map_err(|e| e.to_string())?;
    store(cert)
}

/// A key's public part, armored, to give to others.
pub fn export_public(fingerprint: &str) -> Option<String> {
    let keys = Keys::load();
    let cert = keys.all().find(|c| c.fingerprint().to_hex() == fingerprint)?;
    cert.armored().to_vec().ok().map(|b| String::from_utf8_lossy(&b).to_string())
}

/// Removes a key; yours are kept aside in `pgp/removed`, never deleted.
pub fn remove(fingerprint: &str) -> Result<(), String> {
    for (dir, own) in [(own_dir(), true), (others_dir(), false)] {
        let path = dir.join(format!("{fingerprint}.asc"));
        if !path.exists() {
            continue;
        }
        if own {
            let aside = data_dir().join("pgp").join("removed");
            std::fs::create_dir_all(&aside).map_err(|e| e.to_string())?;
            std::fs::rename(&path, aside.join(format!("{fingerprint}.asc"))).map_err(|e| e.to_string())?;
        } else {
            std::fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// The key that signs for you, its secret part opened.
fn signing_pair(cert: &Cert, passphrase: &str) -> Result<openpgp::crypto::KeyPair, String> {
    let policy = policy();
    let key = cert.keys().secret().with_policy(&policy, None).alive().revoked(false).for_signing().next().ok_or("no signing key")?.key().clone();
    let key = if key.has_unencrypted_secret() { key } else { key.decrypt_secret(&Password::from(passphrase)).map_err(|_| "wrong passphrase".to_string())? };
    key.into_keypair().map_err(|e| e.to_string())
}

/// A detached signature of `data`, armored, and the hash it used ("pgp-sha512"),
/// for multipart/signed (RFC 3156 §5).
pub fn sign_detached(data: &[u8], signer: &Cert, passphrase: &str) -> Result<(String, String), String> {
    let pair = signing_pair(signer, passphrase)?;
    let mut out = Vec::new();
    {
        let message = Message::new(&mut out);
        let message = Armorer::new(message).kind(openpgp::armor::Kind::Signature).build().map_err(|e| e.to_string())?;
        let mut signing = Signer::new(message, pair).map_err(|e| e.to_string())?.detached().build().map_err(|e| e.to_string())?;
        signing.write_all(data).map_err(|e| e.to_string())?;
        signing.finalize().map_err(|e| e.to_string())?;
    }
    let micalg = openpgp::Packet::from_bytes(&out)
        .ok()
        .and_then(|p| match p {
            openpgp::Packet::Signature(s) => Some(format!("pgp-{}", s.hash_algo().to_string().to_ascii_lowercase())),
            _ => None,
        })
        .unwrap_or_else(|| "pgp-sha512".into());
    Ok((String::from_utf8_lossy(&out).to_string(), micalg))
}

/// `data` encrypted to the recipients' keys, signed by yours when given; armored.
pub fn encrypt(data: &[u8], recipients: &[&Cert], signer: Option<(&Cert, &str)>) -> Result<String, String> {
    let policy = policy();
    let keys: Vec<_> = recipients.iter().flat_map(|c| c.keys().with_policy(&policy, None).supported().alive().revoked(false).for_transport_encryption()).collect();
    if keys.is_empty() {
        return Err("no encryption key".into());
    }
    let pair = signer.map(|(cert, passphrase)| signing_pair(cert, passphrase)).transpose()?;
    let mut out = Vec::new();
    {
        let message = Message::new(&mut out);
        let message = Armorer::new(message).build().map_err(|e| e.to_string())?;
        let message = Encryptor::for_recipients(message, keys).build().map_err(|e| e.to_string())?;
        let message = match pair {
            Some(pair) => Signer::new(message, pair).map_err(|e| e.to_string())?.build().map_err(|e| e.to_string())?,
            None => message,
        };
        let mut literal = LiteralWriter::new(message).build().map_err(|e| e.to_string())?;
        literal.write_all(data).map_err(|e| e.to_string())?;
        literal.finalize().map_err(|e| e.to_string())?;
    }
    Ok(String::from_utf8_lossy(&out).to_string())
}

/// A signature, checked.
#[derive(Debug, Clone, Serde, PartialEq, Eq)]
pub struct Checked {
    pub good: bool,
    /// The signer's first user ID, when their key is known.
    pub signer: String,
    pub fingerprint: String,
    /// Why it is not good: "unknown key", "bad signature"…
    pub problem: String,
}

/// What opening a message found.
#[derive(Debug, Clone, Default)]
pub struct Opened {
    pub data: Vec<u8>,
    pub signatures: Vec<Checked>,
}

struct Helper<'a> {
    keys: &'a Keys,
    passphrase: &'a dyn Fn(&Fingerprint) -> Option<String>,
    checked: Vec<Checked>,
}

impl VerificationHelper for Helper<'_> {
    fn get_certs(&mut self, ids: &[KeyHandle]) -> openpgp::Result<Vec<Cert>> {
        Ok(ids.iter().filter_map(|id| self.keys.by_handle(id).cloned()).collect())
    }

    fn check(&mut self, structure: MessageStructure) -> openpgp::Result<()> {
        for layer in structure {
            let MessageLayer::SignatureGroup { results } = layer else { continue };
            for result in results {
                self.checked.push(match result {
                    Ok(good) => Checked {
                        good: true,
                        signer: good.ka.cert().userids().next().map(|u| String::from_utf8_lossy(u.userid().value()).to_string()).unwrap_or_default(),
                        fingerprint: good.ka.cert().fingerprint().to_hex(),
                        problem: String::new(),
                    },
                    Err(error) => {
                        use openpgp::parse::stream::VerificationError as E;
                        let (fingerprint, problem) = match &error {
                            E::MissingKey { sig } => (sig.issuer_fingerprints().next().map(|f| f.to_hex()).unwrap_or_default(), "unknown key"),
                            E::BadSignature { .. } => (String::new(), "bad signature"),
                            E::BadKey { .. } => (String::new(), "bad key"),
                            E::MalformedSignature { .. } => (String::new(), "malformed signature"),
                            E::UnboundKey { .. } => (String::new(), "unbound key"),
                            _ => (String::new(), "unknown"),
                        };
                        Checked { good: false, signer: String::new(), fingerprint, problem: problem.to_string() }
                    }
                });
            }
        }
        // Whatever the signatures say, the text is shown with what they say.
        Ok(())
    }
}

impl DecryptionHelper for Helper<'_> {
    fn decrypt(&mut self, pkesks: &[PKESK], _skesks: &[SKESK], sym_algo: Option<SymmetricAlgorithm>, decrypt: &mut dyn FnMut(Option<SymmetricAlgorithm>, &SessionKey) -> bool) -> openpgp::Result<Option<Cert>> {
        let policy = policy();
        for pkesk in pkesks {
            for cert in &self.keys.own {
                let passphrase = (self.passphrase)(&cert.fingerprint()).unwrap_or_default();
                for key in cert.keys().secret().with_policy(&policy, None).for_transport_encryption().chain(cert.keys().secret().with_policy(&policy, None).for_storage_encryption()) {
                    let recipient_matches = pkesk.recipient().is_none_or(|r| key.key().key_handle().aliases(&r));
                    if !recipient_matches {
                        continue;
                    }
                    let secret = key.key().clone();
                    let secret = if secret.has_unencrypted_secret() { secret } else if let Ok(s) = secret.decrypt_secret(&Password::from(passphrase.as_str())) { s } else { continue };
                    let Ok(mut pair) = secret.into_keypair() else { continue };
                    if let Some((algo, session)) = pkesk.decrypt(&mut pair, sym_algo)
                        && decrypt(algo, &session)
                    {
                        return Ok(Some(cert.clone()));
                    }
                }
            }
        }
        Err(openpgp::Error::InvalidOperation("no key of yours opens this message".into()).into())
    }
}

/// The most a decrypted message may hold: anyone can encrypt to your key, and
/// a few kilobytes of compressed data can inflate to gigabytes.
const LARGEST_DECRYPTED: u64 = 256 * 1024 * 1024;

/// Decrypts an OpenPGP message (armored or not) with your keys, checking the
/// signatures inside. `passphrase` gives the passphrase of a key of yours.
pub fn decrypt(message: &[u8], keys: &Keys, passphrase: &dyn Fn(&Fingerprint) -> Option<String>) -> Result<Opened, String> {
    use std::io::Read as _;
    let policy = policy();
    let helper = Helper { keys, passphrase, checked: Vec::new() };
    let mut decryptor = DecryptorBuilder::from_bytes(message).map_err(|e| e.to_string())?.with_policy(&policy, None, helper).map_err(|e| e.to_string())?;
    let mut data = Vec::new();
    (&mut decryptor).take(LARGEST_DECRYPTED + 1).read_to_end(&mut data).map_err(|e| e.to_string())?;
    if data.len() as u64 > LARGEST_DECRYPTED {
        return Err(format!("larger than {LARGEST_DECRYPTED} bytes once decrypted"));
    }
    let helper = decryptor.into_helper();
    Ok(Opened { data, signatures: helper.checked })
}

/// Checks a detached signature of `data` (multipart/signed).
pub fn verify_detached(data: &[u8], signature: &[u8], keys: &Keys) -> Vec<Checked> {
    let policy = policy();
    let none = |_: &Fingerprint| None;
    let helper = Helper { keys, passphrase: &none, checked: Vec::new() };
    let Ok(builder) = DetachedVerifierBuilder::from_bytes(signature) else { return Vec::new() };
    let Ok(mut verifier) = builder.with_policy(&policy, None, helper) else { return Vec::new() };
    let _ = verifier.verify_bytes(data);
    verifier.into_helper().checked
}

/// Where a domain's Web Key Directory keeps the key of `address`: the
/// advanced address, then the direct one (draft-koch-openpgp-webkey-service §3.1).
pub fn wkd_urls(address: &str) -> Option<[String; 2]> {
    let (local, domain) = address.trim().rsplit_once('@')?;
    let domain = domain.to_ascii_lowercase();
    // A name in the DNS, nothing else: an address answered may come from anyone,
    // and "x@192.168.1.1", "x@[::1]" or "x@host:8080/path?" would send the
    // request elsewhere than to a domain's Web Key Directory.
    let labels: Vec<&str> = domain.split('.').collect();
    let named = labels.len() >= 2
        && labels.iter().all(|l| !l.is_empty() && !l.starts_with('-') && !l.ends_with('-') && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
        && labels.last().is_some_and(|tld| tld.chars().any(|c| c.is_ascii_alphabetic()) && *tld != "localhost");
    if !named {
        return None;
    }
    let mut context = openpgp::types::HashAlgorithm::SHA1.context().ok()?.for_digest();
    context.update(local.to_lowercase().as_bytes());
    let digest = context.into_digest().ok()?;
    let hash = zbase32(&digest);
    let query = local.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect::<String>();
    Some([
        format!("https://openpgpkey.{domain}/.well-known/openpgpkey/{domain}/hu/{hash}?l={query}"),
        format!("https://{domain}/.well-known/openpgpkey/hu/{hash}?l={query}"),
    ])
}

/// z-base-32 (RFC 6189 §5.1.6), as WKD names keys.
fn zbase32(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 32] = b"ybndrfg8ejkmcpqxot1uwisza345h769";
    let mut out = String::new();
    let (mut buffer, mut bits) = (0u32, 0u32);
    for &byte in bytes {
        buffer = (buffer << 8) | u32::from(byte);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(ALPHABET[((buffer >> bits) & 31) as usize] as char);
        }
    }
    if bits > 0 {
        out.push(ALPHABET[((buffer << (5 - bits)) & 31) as usize] as char);
    }
    out
}

/// Keeps keys found for `address`: public parts only, and only those naming it.
pub fn learn(bytes: &[u8], address: &str) -> Vec<KeySummary> {
    CertParser::from_bytes(bytes)
        .map(|parser| parser.filter_map(Result::ok).filter(|c| names(c, address)).filter_map(|c| store(c.strip_secret_key_material()).ok()).collect())
        .unwrap_or_default()
}

/// What a protected message showed when opened, for the reader's line.
#[derive(Debug, Clone, Default, Serde, PartialEq, Eq)]
pub struct PgpView {
    pub encrypted: bool,
    /// Decrypted; false when no key of yours opens it.
    pub opened: bool,
    pub signatures: Vec<Checked>,
}

/// Lone LF become CRLF: what was signed is the canonical form (RFC 3156 §5).
fn canonical(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len() + bytes.len() / 32);
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'\n' && (i == 0 || bytes[i - 1] != b'\r') {
            out.push(b'\r');
        }
        out.push(b);
    }
    out
}

fn kind_of(part: &mail_parser::MessagePart) -> (String, String) {
    use mail_parser::MimeHeaders;
    part.content_type().map_or_else(Default::default, |t| {
        (format!("{}/{}", t.ctype(), t.subtype().unwrap_or("")).to_ascii_lowercase(), t.attribute("protocol").unwrap_or("").to_ascii_lowercase())
    })
}

/// The message's own header lines but its MIME ones, which the decrypted body brings.
fn outer_headers(raw: &[u8], body_offset: usize) -> Vec<u8> {
    let head = String::from_utf8_lossy(&raw[..body_offset.min(raw.len())]).to_string();
    let mut out = String::new();
    let mut keep = false;
    for line in head.split_inclusive('\n') {
        if !line.starts_with([' ', '\t']) {
            let name = line.split(':').next().unwrap_or("").trim().to_ascii_lowercase();
            keep = !name.is_empty() && !name.starts_with("content-") && name != "mime-version";
        }
        if keep {
            out.push_str(line);
        }
    }
    canonical(out.trim_end().as_bytes()).into_iter().chain(b"\r\nMIME-Version: 1.0\r\n".iter().copied()).collect()
}

/// Opens a protected message: PGP/MIME encrypted or signed (RFC 3156), or
/// inline PGP. Gives back the message as it reads once decrypted (its own
/// headers, then what was inside), when it was encrypted, and what was found;
/// None when nothing is protected.
pub fn open(raw: &[u8], keys: &Keys, passphrase: &dyn Fn(&Fingerprint) -> Option<String>) -> Option<(Option<Vec<u8>>, PgpView)> {
    let message = mail_parser::MessageParser::default().parse(raw)?;
    let root = message.parts.first()?;
    let (kind, protocol) = kind_of(root);
    let children: Vec<&mail_parser::MessagePart> = root.sub_parts().map(|ids| ids.iter().filter_map(|&id| message.parts.get(id as usize)).collect()).unwrap_or_default();
    if kind == "multipart/encrypted" && protocol == "application/pgp-encrypted" {
        let payload = children.iter().find(|p| kind_of(p).0 == "application/octet-stream").map(|p| p.contents().to_vec())?;
        return Some(match decrypt(&payload, keys, passphrase) {
            Ok(opened) => {
                let mut virtual_message = outer_headers(raw, root.raw_body_offset() as usize);
                virtual_message.extend(canonical(&opened.data));
                let mut signatures = opened.signatures;
                // Signed, then encrypted as a multipart/signed (RFC 3156 §6.1): checked too.
                if let Some((_, inner)) = open(&virtual_message, keys, passphrase) {
                    signatures.extend(inner.signatures);
                }
                (Some(virtual_message), PgpView { encrypted: true, opened: true, signatures })
            }
            Err(_) => (None, PgpView { encrypted: true, opened: false, signatures: Vec::new() }),
        });
    }
    if kind == "multipart/signed" && protocol == "application/pgp-signature" && children.len() >= 2 {
        let signed = children[0];
        let (start, end) = (signed.raw_header_offset() as usize, signed.raw_end_offset() as usize);
        let data = canonical(raw.get(start..end)?);
        let signature = children.iter().find(|p| kind_of(p).0 == "application/pgp-signature")?.contents().to_vec();
        // The line break before the boundary belongs to the boundary; parsers
        // differ on where a part ends, so the part is checked with and without it.
        let mut checked = verify_detached(&data, &signature, keys);
        if !checked.iter().any(|c| c.good) && data.ends_with(b"\r\n") {
            let trimmed = verify_detached(&data[..data.len() - 2], &signature, keys);
            if trimmed.iter().any(|c| c.good) {
                checked = trimmed;
            }
        }
        return Some((None, PgpView { encrypted: false, opened: true, signatures: checked }));
    }
    // Inline: an armored message in the text.
    let text = message.body_text(0)?.to_string();
    let start = text.find("-----BEGIN PGP MESSAGE-----")?;
    let end = text[start..].find("-----END PGP MESSAGE-----").map(|e| start + e + "-----END PGP MESSAGE-----".len())?;
    Some(match decrypt(text[start..end].as_bytes(), keys, passphrase) {
        Ok(opened) => {
            let mut virtual_message = outer_headers(raw, root.raw_body_offset() as usize);
            virtual_message.extend_from_slice(b"Content-Type: text/plain; charset=utf-8\r\n\r\n");
            virtual_message.extend(canonical(&opened.data));
            (Some(virtual_message), PgpView { encrypted: true, opened: true, signatures: opened.signatures })
        }
        Err(_) => (None, PgpView { encrypted: true, opened: false, signatures: Vec::new() }),
    })
}

/// Autocrypt (Level 1, §2.1): the key a message's header carries for its sender.
pub fn autocrypt_key(header: &str, from: &str) -> Option<Cert> {
    let mut addr = None;
    let mut keydata = None;
    for part in header.split(';') {
        let (name, value) = part.split_once('=')?;
        match name.trim() {
            "addr" => addr = Some(value.trim().to_string()),
            "keydata" => keydata = Some(value.split_whitespace().collect::<String>()),
            _ => {}
        }
    }
    if !addr?.eq_ignore_ascii_case(from.trim()) {
        return None;
    }
    let bytes = crate::lines::base64_decode(&keydata?)?;
    let cert = Cert::from_bytes(&bytes).ok()?;
    names(&cert, from).then_some(cert)
}

/// Keeps a key from an Autocrypt header: only public parts, only when it names its sender.
pub fn learn_autocrypt(header: &str, from: &str) -> Option<KeySummary> {
    let cert = autocrypt_key(header, from)?;
    store(cert.strip_secret_key_material()).ok()
}

/// The Autocrypt header for your outgoing mail: your key, smallest form
/// (primary, encryption subkey, the user ID with your address).
pub fn autocrypt_header(cert: &Cert, address: &str) -> Option<String> {
    let policy = policy();
    let valid = cert.with_policy(&policy, None).ok()?;
    let userid = valid.userids().find(|u| u.userid().email().ok().flatten().is_some_and(|e| e.eq_ignore_ascii_case(address)))?;
    let encryption = valid.keys().subkeys().alive().revoked(false).for_transport_encryption().next()?;
    let mut packets: Vec<openpgp::Packet> = vec![cert.primary_key().key().clone().into()];
    packets.push(userid.userid().clone().into());
    packets.extend(userid.self_signatures().take(1).cloned().map(Into::into));
    packets.push(encryption.key().clone().into());
    packets.extend(encryption.self_signatures().take(1).cloned().map(Into::into));
    let minimal = Cert::from_packets(packets.into_iter()).ok()?;
    let mut bytes = Vec::new();
    minimal.serialize(&mut bytes).ok()?;
    // A space every 76 characters, where the header may be folded (Autocrypt §2.1):
    // a line holds 998 characters at most (RFC 5322 §2.1.1), and an RSA key
    // imported from GnuPG takes several thousand.
    let encoded: Vec<char> = crate::lines::base64_encode(&bytes).chars().collect();
    let keydata = encoded.chunks(76).map(|chunk| chunk.iter().collect::<String>()).collect::<Vec<_>>().join(" ");
    Some(format!("addr={address}; keydata={keydata}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sioul-pgp-{}-{}", std::process::id(), std::thread::current().name().unwrap_or("t").replace("::", "-")));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn key(address: &str) -> Cert {
        CertBuilder::general_purpose(Some(format!("Someone <{address}>")))
            .set_cipher_suite(CipherSuite::Cv25519)
            .set_password(Some(Password::from("secret words")))
            .generate()
            .unwrap()
            .0
    }

    #[test]
    fn signs_encrypts_decrypts_and_verifies() {
        let _ = temporary();
        let alice = key("alice@example.org");
        let bob = key("bob@example.org");
        let keys_of_bob = Keys { own: vec![bob.clone()], others: vec![alice.clone().strip_secret_key_material()] };
        let sealed = encrypt(b"Content-Type: text/plain\r\n\r\nHello Bob.\r\n", &[&bob, &alice], Some((&alice, "secret words"))).unwrap();
        assert!(sealed.starts_with("-----BEGIN PGP MESSAGE-----"));
        let passphrase = |_: &Fingerprint| Some("secret words".to_string());
        let opened = decrypt(sealed.as_bytes(), &keys_of_bob, &passphrase).unwrap();
        assert_eq!(opened.data, b"Content-Type: text/plain\r\n\r\nHello Bob.\r\n");
        assert_eq!(opened.signatures.len(), 1);
        assert!(opened.signatures[0].good && opened.signatures[0].signer.contains("alice@example.org"));
        let wrong = |_: &Fingerprint| Some("other words".to_string());
        assert!(decrypt(sealed.as_bytes(), &keys_of_bob, &wrong).is_err());
        let (signature, micalg) = sign_detached(b"signed part", &alice, "secret words").unwrap();
        assert!(micalg.starts_with("pgp-sha"), "{micalg}");
        let checked = verify_detached(b"signed part", signature.as_bytes(), &keys_of_bob);
        assert!(checked[0].good);
        let tampered = verify_detached(b"signed part!", signature.as_bytes(), &keys_of_bob);
        assert!(!tampered[0].good);
        let stranger = verify_detached(b"signed part", signature.as_bytes(), &Keys::default());
        assert_eq!(stranger[0].problem, "unknown key");
    }

    #[test]
    fn key_files_are_yours_alone() {
        let path = temporary().join("key.asc");
        write_key(&path, b"-----BEGIN PGP PRIVATE KEY BLOCK-----").unwrap();
        write_key(&path, b"-----BEGIN PGP PRIVATE KEY BLOCK----- again").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"-----BEGIN PGP PRIVATE KEY BLOCK----- again");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn web_key_directory_names() {
        // The example of draft-koch-openpgp-webkey-service §3.1.
        let [advanced, direct] = wkd_urls("Joe.Doe@Example.ORG").unwrap();
        assert_eq!(direct, "https://example.org/.well-known/openpgpkey/hu/iy9q119eutrkn8s1mk4r39qejnbu3n5q?l=Joe.Doe");
        assert!(advanced.starts_with("https://openpgpkey.example.org/.well-known/openpgpkey/example.org/hu/iy9q119eutrkn8s1mk4r39qejnbu3n5q"));
        // Only a domain's own directory: no address, no local name, no port or path.
        for elsewhere in ["joe@192.168.1.1", "joe@[::1]", "joe@localhost", "joe@host.example:8080/x?", "joe@1.2.3"] {
            assert!(wkd_urls(elsewhere).is_none(), "{elsewhere}");
        }
    }

    #[test]
    fn autocrypt_carries_the_smallest_key() {
        let alice = key("alice@example.org");
        let header = autocrypt_header(&alice, "alice@example.org").unwrap();
        assert!(header.starts_with("addr=alice@example.org; keydata="));
        assert!(header.split_whitespace().all(|word| word.len() <= 100), "foldable: {header}");
        let found = autocrypt_key(&header, "alice@example.org").unwrap();
        assert_eq!(found.fingerprint(), alice.fingerprint());
        assert!(!found.is_tsk());
        assert!(autocrypt_key(&header, "mallory@example.org").is_none());
    }
}
