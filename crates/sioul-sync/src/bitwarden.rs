// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Bitwarden, read here with no other program: your vault's logins and
//! their one-time codes, to fill a site's form when you ask. Read only:
//! nothing is ever written to the vault.
//!
//! As Bitwarden's own clients do (its security white paper; its SDK under
//! GPL-3.0, `bitwarden-crypto`): the master key is derived from your master
//! password and e-mail (PBKDF2-SHA256 or Argon2id, as your account says),
//! hashed once more to log in, stretched with HKDF to open your user key,
//! which opens each item (AES-256-CBC with HMAC-SHA256, or COSE for newer
//! accounts); an organisation's key comes through your RSA key. Or your
//! security key alone, as Bitwarden's "log in with passkey": the key's secret
//! (WebAuthn PRF) opens an RSA key Bitwarden keeps for that passkey, which
//! opens your user key. The master password and the keys live in memory
//! only, wiped when dropped; the keyring keeps the "remember this device"
//! token, nothing else.

use aes::cipher::{BlockDecryptMut, KeyIvInit, block_padding::Pkcs7};
use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use rsa::pkcs8::DecodePrivateKey;
use rsa::{Oaep, RsaPrivateKey};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::time::Duration;
use zeroize::Zeroizing;

type HmacSha256 = Hmac<Sha256>;

/// What went wrong, for the window to say.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    /// The master password does not open the vault.
    WrongPassword,
    /// A second step is asked: the providers offered (0 authenticator app,
    /// 1 e-mail, 3 YubiKey OTP, 7 security key (FIDO2), 8 recovery code; 2, 6
    /// Duo cannot be done here), and for the security key, its options, as
    /// the key is asked with them (`key_script`).
    SecondFactor(Vec<u8>, Option<String>),
    /// A new device: Bitwarden sent a code by e-mail.
    NewDevice,
    /// A passkey login whose key gave no secret to open the vault (no WebAuthn PRF).
    NoKeySecret,
    /// Bitwarden logs in with this passkey but keeps no key of the vault for it (its encryption is off).
    PasskeyNotForVault,
    /// The server could not be reached.
    Network(String),
    /// Anything else, in Bitwarden's words.
    Refused(String),
}

/// Bitwarden's addresses: its US or EU cloud, or your own server (Vaultwarden too).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Server {
    pub identity: String,
    pub api: String,
    /// The web vault: its security-key page asks a FIDO2 key for the second step.
    pub vault: String,
}

impl Server {
    /// "" or "bitwarden.com": the US cloud; "bitwarden.eu": the EU one (their
    /// vaults' addresses too, as a browser shows them); else a self-hosted base address.
    pub fn of(base: &str) -> Server {
        let base = base.trim().trim_end_matches('/');
        match base.to_ascii_lowercase().as_str() {
            "" | "bitwarden.com" | "vault.bitwarden.com" | "https://bitwarden.com" | "https://vault.bitwarden.com" => Server { identity: "https://identity.bitwarden.com".into(), api: "https://api.bitwarden.com".into(), vault: "https://vault.bitwarden.com".into() },
            "bitwarden.eu" | "vault.bitwarden.eu" | "https://bitwarden.eu" | "https://vault.bitwarden.eu" => Server { identity: "https://identity.bitwarden.eu".into(), api: "https://api.bitwarden.eu".into(), vault: "https://vault.bitwarden.eu".into() },
            _ => {
                let own = if base.contains("://") { base.to_string() } else { format!("https://{base}") };
                Server { identity: format!("{own}/identity"), api: format!("{own}/api"), vault: own }
            }
        }
    }
}

/// How the master key is derived, as the account says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kdf {
    Pbkdf2 { iterations: u32 },
    Argon2id { iterations: u32, memory_mib: u32, parallelism: u32 },
}

/// PBKDF2-HMAC-SHA256, 32 bytes (RFC 8018).
fn pbkdf2(secret: &[u8], salt: &[u8], iterations: u32) -> Zeroizing<[u8; 32]> {
    let prf = HmacSha256::new_from_slice(secret).expect("HMAC takes any key");
    let mut block = prf.clone();
    block.update(salt);
    block.update(&1u32.to_be_bytes());
    let mut u: [u8; 32] = block.finalize().into_bytes().into();
    let mut out = Zeroizing::new(u);
    for _ in 1..iterations {
        let mut next = prf.clone();
        next.update(&u);
        u = next.finalize().into_bytes().into();
        for (o, x) in out.iter_mut().zip(u) {
            *o ^= x;
        }
    }
    out
}

/// The master key, from the master password and the e-mail (trimmed, in lower case).
/// The settings come from the server, and are held to Bitwarden's own bounds:
/// below its clients' minimums, a server would make your password cheaper to
/// guess from what it is sent (its clients refuse them too); above its own
/// maximums, it would only exhaust this computer.
pub fn master_key(password: &str, email: &str, kdf: Kdf) -> Result<Zeroizing<[u8; 32]>, Problem> {
    let salt = email.trim().to_lowercase();
    match kdf {
        Kdf::Pbkdf2 { iterations } => {
            if !(5_000..=2_000_000).contains(&iterations) {
                return Err(Problem::Refused(format!("PBKDF2: {iterations} iterations")));
            }
            Ok(pbkdf2(password.as_bytes(), salt.as_bytes(), iterations))
        }
        Kdf::Argon2id { iterations, memory_mib, parallelism } => {
            if iterations < 2 || !(16..=1024).contains(&memory_mib) || !(1..=16).contains(&parallelism) {
                return Err(Problem::Refused(format!("Argon2id: {iterations} iterations, {memory_mib} MiB, {parallelism} lanes")));
            }
            let params = argon2::Params::new(memory_mib.saturating_mul(1024), iterations, parallelism, Some(32)).map_err(|e| Problem::Refused(e.to_string()))?;
            let argon = argon2::Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
            let mut key = Zeroizing::new([0u8; 32]);
            argon.hash_password_into(password.as_bytes(), &Sha256::digest(salt.as_bytes()), key.as_mut()).map_err(|e| Problem::Refused(e.to_string()))?;
            Ok(key)
        }
    }
}

/// The hash that logs in: the master key hashed once with the password.
pub fn password_hash(master: &[u8; 32], password: &str) -> String {
    let hash = pbkdf2(master, password.as_bytes(), 1);
    STANDARD.encode(&hash[..])
}

/// A symmetric key: AES-256 with its HMAC key, or a COSE key (newer accounts).
pub struct Key {
    enc: Zeroizing<Vec<u8>>,
    mac: Zeroizing<Vec<u8>>,
    /// The COSE algorithm, for a COSE key.
    cose: Option<i64>,
}

/// COSE algorithms Bitwarden uses (private use, `bitwarden-crypto/src/cose`).
const XCHACHA20_POLY1305: i64 = -70000;
const XAES_256_GCM: i64 = -70010;
const A256GCM: i64 = 3;

/// Bitwarden's padding: the last byte says how many to drop.
fn unpad(bytes: &[u8]) -> Option<&[u8]> {
    let n = usize::from(*bytes.last()?);
    (n >= 1 && n <= bytes.len()).then(|| &bytes[..bytes.len() - n])
}

impl Key {
    /// The master key stretched (HKDF-Expand-SHA256, "enc" and "mac").
    pub fn stretched(master: &[u8; 32]) -> Key {
        let hkdf = hkdf::Hkdf::<Sha256>::from_prk(master).expect("32 bytes is a valid PRK");
        let (mut enc, mut mac) = (Zeroizing::new(vec![0u8; 32]), Zeroizing::new(vec![0u8; 32]));
        hkdf.expand(b"enc", &mut enc).expect("32 bytes");
        hkdf.expand(b"mac", &mut mac).expect("32 bytes");
        Key { enc, mac, cose: None }
    }

    /// A key from its decrypted bytes: 64 (encryption and MAC), 32 (an old
    /// key without MAC), or a padded COSE key.
    pub fn from_bytes(bytes: &[u8]) -> Result<Key, Problem> {
        match bytes.len() {
            64 => Ok(Key { enc: Zeroizing::new(bytes[..32].to_vec()), mac: Zeroizing::new(bytes[32..].to_vec()), cose: None }),
            32 => Ok(Key { enc: Zeroizing::new(bytes.to_vec()), mac: Zeroizing::new(Vec::new()), cose: None }),
            _ => {
                let raw = unpad(bytes).unwrap_or(bytes);
                let value: ciborium::Value = ciborium::from_reader(raw).map_err(|e| Problem::Refused(format!("key: {e}")))?;
                let map = value.as_map().ok_or_else(|| Problem::Refused("key: not a COSE key".into()))?;
                let label = |n: i64| map.iter().find(|(k, _)| k.as_integer().is_some_and(|i| i128::from(i) == i128::from(n))).map(|(_, v)| v);
                let alg = label(3).and_then(|v| v.as_integer()).and_then(|i| i64::try_from(i).ok()).ok_or_else(|| Problem::Refused("key: no algorithm".into()))?;
                let k = label(-1).and_then(|v| v.as_bytes()).ok_or_else(|| Problem::Refused("key: no key".into()))?;
                Ok(Key { enc: Zeroizing::new(k.clone()), mac: Zeroizing::new(Vec::new()), cose: Some(alg) })
            }
        }
    }
}

fn b64(text: &str) -> Result<Vec<u8>, Problem> {
    STANDARD.decode(text.trim()).map_err(|e| Problem::Refused(format!("base64: {e}")))
}

/// AES-256-CBC with PKCS#7 padding.
fn cbc(key: &[u8], iv: &[u8], data: &[u8]) -> Result<Zeroizing<Vec<u8>>, Problem> {
    let decryptor = cbc::Decryptor::<aes::Aes256>::new_from_slices(key, iv).map_err(|_| Problem::Refused("AES key or IV".into()))?;
    decryptor.decrypt_padded_vec_mut::<Pkcs7>(data).map(Zeroizing::new).map_err(|_| Problem::WrongPassword)
}

/// An EncString opened: type 0 (AES-CBC), 2 (AES-CBC with HMAC, checked first), 7 (COSE).
pub fn decrypt(text: &str, key: &Key) -> Result<Zeroizing<Vec<u8>>, Problem> {
    let (kind, rest) = match text.split_once('.') {
        Some((kind, rest)) => (kind.parse::<u8>().map_err(|_| Problem::Refused("EncString type".into()))?, rest),
        None => (0, text),
    };
    let parts: Vec<&str> = rest.split('|').collect();
    match (kind, parts.as_slice()) {
        // Unchecked text only for the oldest keys, which have no MAC key: a key
        // with one opens checked text only, as in Bitwarden's clients, so a
        // server cannot strip the check off.
        (0, _) if !key.mac.is_empty() => Err(Problem::Refused("EncString type 0 for a key with a MAC key".into())),
        (0, [iv, data]) => cbc(&key.enc, &b64(iv)?, &b64(data)?),
        (2, [iv, data, mac]) => {
            let (iv, data, mac) = (b64(iv)?, b64(data)?, b64(mac)?);
            let mut check = HmacSha256::new_from_slice(&key.mac).map_err(|_| Problem::Refused("MAC key".into()))?;
            check.update(&iv);
            check.update(&data);
            // Checked in constant time, before anything is decrypted.
            check.verify_slice(&mac).map_err(|_| Problem::WrongPassword)?;
            cbc(&key.enc, &iv, &data)
        }
        (7, [message]) => cose(&b64(message)?, key),
        (kind, _) => Err(Problem::Refused(format!("EncString type {kind} is not read here"))),
    }
}

/// A COSE_Encrypt0 message (RFC 9052 §5.2): its algorithm, its nonce, its
/// text, opened with the protected header as additional data; padded UTF-8
/// unpadded.
fn cose(message: &[u8], key: &Key) -> Result<Zeroizing<Vec<u8>>, Problem> {
    use ciborium::Value;
    let bad = |what: &str| Problem::Refused(format!("COSE: {what}"));
    let mut value: Value = ciborium::from_reader(message).map_err(|_| bad("not CBOR"))?;
    if let Value::Tag(16, inner) = value {
        value = *inner;
    }
    let parts = value.as_array().ok_or_else(|| bad("not an array"))?;
    let [protected, unprotected, ciphertext] = parts.as_slice() else { return Err(bad("not Encrypt0")) };
    let protected = protected.as_bytes().ok_or_else(|| bad("protected header"))?;
    let header: Value = if protected.is_empty() { Value::Map(Vec::new()) } else { ciborium::from_reader(protected.as_slice()).map_err(|_| bad("protected header"))? };
    let find = |map: &Value, label: i64| map.as_map().and_then(|m| m.iter().find(|(k, _)| k.as_integer().is_some_and(|i| i128::from(i) == i128::from(label))).map(|(_, v)| v.clone()));
    let alg = find(&header, 1).and_then(|v| v.as_integer()).and_then(|i| i64::try_from(i).ok()).or(key.cose).ok_or_else(|| bad("no algorithm"))?;
    let padded = find(&header, 3).and_then(|v| v.as_text().map(str::to_string)).is_some_and(|t| t == "application/x.bitwarden.utf8-padded");
    let nonce = find(unprotected, 5).and_then(|v| v.as_bytes().cloned()).ok_or_else(|| bad("no nonce"))?;
    let ciphertext = ciphertext.as_bytes().ok_or_else(|| bad("no text"))?;
    // Enc_structure: ["Encrypt0", protected, external_aad].
    let mut aad = Vec::new();
    ciborium::into_writer(&Value::Array(vec![Value::Text("Encrypt0".into()), Value::Bytes(protected.clone()), Value::Bytes(Vec::new())]), &mut aad).map_err(|_| bad("AAD"))?;
    use aes_gcm::aead::{Aead, KeyInit, Payload};
    let payload = Payload { msg: ciphertext, aad: &aad };
    let plain = match alg {
        XCHACHA20_POLY1305 => {
            let cipher = chacha20poly1305::XChaCha20Poly1305::new_from_slice(&key.enc).map_err(|_| bad("key length"))?;
            if nonce.len() != 24 {
                return Err(bad("nonce length"));
            }
            cipher.decrypt(chacha20poly1305::XNonce::from_slice(&nonce), payload).map_err(|_| Problem::WrongPassword)?
        }
        A256GCM => {
            let cipher = aes_gcm::Aes256Gcm::new_from_slice(&key.enc).map_err(|_| bad("key length"))?;
            if nonce.len() != 12 {
                return Err(bad("nonce length"));
            }
            cipher.decrypt(aes_gcm::Nonce::from_slice(&nonce), payload).map_err(|_| Problem::WrongPassword)?
        }
        XAES_256_GCM => return Err(bad("XAES-256-GCM is not read here yet")),
        other => return Err(bad(&format!("algorithm {other}"))),
    };
    let plain = Zeroizing::new(plain);
    Ok(if padded { Zeroizing::new(unpad(&plain).ok_or_else(|| bad("padding"))?.to_vec()) } else { plain })
}

/// An EncString opened with an RSA key: type 3 (OAEP-SHA256), 4 (OAEP-SHA1), 5 and 6 (the same, their MAC unused).
pub fn decrypt_rsa(text: &str, private: &RsaPrivateKey) -> Result<Zeroizing<Vec<u8>>, Problem> {
    let (kind, rest) = text.split_once('.').ok_or_else(|| Problem::Refused("EncString type".into()))?;
    let data = b64(rest.split('|').next().unwrap_or(rest))?;
    let plain = match kind {
        "3" | "5" => private.decrypt(Oaep::new::<Sha256>(), &data),
        "4" | "6" => private.decrypt(Oaep::new::<sha1::Sha1>(), &data),
        other => return Err(Problem::Refused(format!("EncString type {other}"))),
    };
    plain.map(Zeroizing::new).map_err(|_| Problem::Refused("RSA".into()))
}

/// A string field: an EncString opened as UTF-8.
fn text(value: Option<&Value>, key: &Key) -> String {
    value.and_then(Value::as_str).and_then(|t| decrypt(t, key).ok()).map(|b| String::from_utf8_lossy(&b).to_string()).unwrap_or_default()
}

/// A field, in the camel case of current servers or the Pascal case of older ones.
fn field<'a>(value: &'a Value, name: &str) -> Option<&'a Value> {
    let mut pascal = name.to_string();
    if let Some(first) = pascal.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    value.get(name).or_else(|| value.get(&pascal)).filter(|v| !v.is_null())
}

/// HTTPS only, redirects included: what logs in (your password's hash) and
/// your vault never travel in clear, as mail never does.
fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(30))).http_status_as_error(false).https_only(true).build().into()
}

/// The version declared when the server does not say its own. Bitwarden
/// refuses clients too far behind it ("Please update your app to continue
/// using Bitwarden": 2024.12.0 was refused in October 2026), so the server's
/// own version is declared when it gives one (`client_version`).
const CLIENT_VERSION: &str = "2026.9.0";

/// "2026.9.2": a version as Bitwarden writes them.
fn is_version(text: &str) -> bool {
    let parts: Vec<&str> = text.split('.').collect();
    parts.len() >= 2 && parts.len() <= 4 && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

/// The version to declare to this server: its own (`GET {api}/config`, kept
/// for the session), so Sioul keeps within the versions it accepts; else
/// `CLIENT_VERSION`.
fn client_version(server: &Server) -> String {
    static KNOWN: std::sync::Mutex<Option<(String, String)>> = std::sync::Mutex::new(None);
    if let Ok(known) = KNOWN.lock()
        && let Some((api, version)) = known.as_ref()
        && *api == server.api
    {
        return version.clone();
    }
    let asked = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(10))).http_status_as_error(false).https_only(true).build();
    let asked: ureq::Agent = asked.into();
    let version = answer(asked.get(&format!("{}/config", server.api)).header("Accept", "application/json").call())
        .ok()
        .filter(|(status, _)| *status == 200)
        .and_then(|(_, json)| field(&json, "version").and_then(Value::as_str).map(str::to_string))
        .filter(|v| is_version(v));
    let version = version.unwrap_or_else(|| CLIENT_VERSION.to_string());
    if let Ok(mut known) = KNOWN.lock() {
        *known = Some((server.api.clone(), version.clone()));
    }
    version
}

fn device_type() -> &'static str {
    if cfg!(windows) {
        "6"
    } else if cfg!(target_os = "macos") {
        "7"
    } else {
        "8"
    }
}

/// Reads an answer: its status and its JSON. A large vault is more than
/// ureq's 10 MB; one that could not be read whole is not an empty vault.
fn answer(response: Result<ureq::http::Response<ureq::Body>, ureq::Error>) -> Result<(u16, Value), Problem> {
    let mut response = response.map_err(|e| Problem::Network(e.to_string()))?;
    let status = response.status().as_u16();
    let body = match response.body_mut().with_config().limit(64 * 1024 * 1024).read_to_string() {
        Ok(body) => body,
        Err(e) if status == 200 => return Err(Problem::Network(e.to_string())),
        Err(_) => String::new(),
    };
    Ok((status, serde_json::from_str(&body).unwrap_or(Value::Null)))
}

/// How the account derives its master key.
fn prelogin(server: &Server, email: &str) -> Result<Kdf, Problem> {
    let (status, json) = answer(agent().post(&format!("{}/accounts/prelogin", server.identity)).header("Content-Type", "application/json").send(serde_json::json!({ "email": email.trim() }).to_string()))?;
    if status != 200 {
        return Err(Problem::Refused(format!("prelogin: {status}")));
    }
    let settings = field(&json, "kdfSettings").unwrap_or(&json);
    let number = |names: &[&str]| names.iter().find_map(|n| field(settings, n).or_else(|| field(&json, n))).and_then(Value::as_u64).and_then(|n| u32::try_from(n).ok());
    Ok(match number(&["kdfType", "kdf"]).unwrap_or(0) {
        1 => Kdf::Argon2id { iterations: number(&["iterations", "kdfIterations"]).unwrap_or(3), memory_mib: number(&["memory", "kdfMemory"]).unwrap_or(64), parallelism: number(&["parallelism", "kdfParallelism"]).unwrap_or(4) },
        _ => Kdf::Pbkdf2 { iterations: number(&["iterations", "kdfIterations"]).unwrap_or(600_000) },
    })
}

/// The vault, open: the keys that read it, and how to ask for it.
pub struct Unlocked {
    access: String,
    user: Key,
    private: Option<RsaPrivateKey>,
    /// "Remember this device": the token that skips the second step next time.
    pub remember: Option<String>,
}

/// Logs in and opens the vault. `factor` is a second step asked before
/// (provider, code); `remembered` a token kept from a former login; `new_device`
/// the code Bitwarden mailed for a new device.
pub fn unlock(server: &Server, email: &str, password: &str, device: &str, factor: Option<(u8, &str)>, remembered: Option<&str>, new_device: Option<&str>) -> Result<Unlocked, Problem> {
    let kdf = prelogin(server, email)?;
    let master = master_key(password, email, kdf)?;
    let hash = Zeroizing::new(password_hash(&master, password));
    let mut form: Vec<(&str, String)> = vec![
        ("grant_type", "password".into()),
        ("username", email.trim().to_string()),
        ("password", hash.to_string()),
        ("scope", "api offline_access".into()),
        ("client_id", "desktop".into()),
        ("deviceType", device_type().into()),
        ("deviceIdentifier", device.to_string()),
        ("deviceName", "Sioul".into()),
    ];
    match (factor, remembered) {
        (Some((provider, code)), _) => {
            form.push(("twoFactorProvider", provider.to_string()));
            form.push(("twoFactorToken", code.trim().replace(' ', "")));
            form.push(("twoFactorRemember", "1".into()));
        }
        (None, Some(token)) => {
            form.push(("twoFactorProvider", "5".into()));
            form.push(("twoFactorToken", token.to_string()));
        }
        (None, None) => {}
    }
    if let Some(code) = new_device {
        form.push(("newDeviceOtp", code.trim().to_string()));
    }
    let (status, json) = ask_token(server, Some(email), &form)?;
    if status != 200 {
        // Which second steps are offered, or why it was refused.
        if let Some(providers) = field(&json, "twoFactorProviders").and_then(Value::as_array) {
            let ids: Vec<u8> = providers.iter().filter_map(|p| p.as_u64().or_else(|| p.as_str().and_then(|s| s.parse().ok()))).filter_map(|p| u8::try_from(p).ok()).collect();
            // The security key's challenge: what Bitwarden's page passes to the browser.
            let webauthn = field(&json, "twoFactorProviders2").and_then(|all| all.get("7")).filter(|c| !c.is_null()).map(|c| match c {
                Value::String(text) => text.clone(),
                other => other.to_string(),
            });
            return Err(Problem::SecondFactor(ids, webauthn));
        }
        return Err(refusal(&json, status));
    }
    // The user key and the private key: at the top, or where newer servers put them.
    let unlock_options = field(&json, "userDecryptionOptions").and_then(|o| field(o, "masterPasswordUnlock"));
    let protected = field(&json, "key")
        .or_else(|| unlock_options.and_then(|u| field(u, "masterKeyEncryptedUserKey")))
        .and_then(Value::as_str)
        .ok_or_else(|| Problem::Refused("no user key".into()))?;
    // The user key: opened by the stretched master key, or by the master key itself for the oldest accounts.
    let stretched = Key::stretched(&master);
    let opened = decrypt(protected, &stretched).or_else(|_| decrypt(protected, &Key { enc: Zeroizing::new(master.to_vec()), mac: Zeroizing::new(Vec::new()), cose: None }))?;
    session(&json, Key::from_bytes(&opened)?)
}

/// Asks Bitwarden for a session: `form` is the login; `email` goes in the
/// header Bitwarden expects with a password.
fn ask_token(server: &Server, email: Option<&str>, form: &[(&str, String)]) -> Result<(u16, Value), Problem> {
    let body: String = form.iter().map(|(k, v)| format!("{k}={}", form_encode(v))).collect::<Vec<_>>().join("&");
    let version = client_version(server);
    let mut request = agent()
        .post(&format!("{}/connect/token", server.identity))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Accept", "application/json")
        .header("Bitwarden-Client-Name", "desktop")
        .header("Bitwarden-Client-Version", &version)
        .header("Device-Type", device_type());
    if let Some(email) = email {
        request = request.header("auth-email", URL_SAFE_NO_PAD.encode(email.trim().as_bytes()));
    }
    answer(request.send(body))
}

/// Why Bitwarden refused a login, in its words.
fn refusal(json: &Value, status: u16) -> Problem {
    let said = [field(json, "errorModel").and_then(|m| field(m, "message")), field(json, "error_description"), field(json, "message")]
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .next()
        .unwrap_or("")
        .to_string();
    let lower = said.to_lowercase();
    if lower.contains("new device") {
        Problem::NewDevice
    } else if lower.contains("username or password") || lower.contains("invalid_username_or_password") {
        Problem::WrongPassword
    } else {
        Problem::Refused(if said.is_empty() { format!("login: {status}") } else { said })
    }
}

/// The session Bitwarden gave, with its user key opened: the account's RSA
/// key opened with it (at the top, or where newer servers put it).
fn session(json: &Value, user: Key) -> Result<Unlocked, Problem> {
    let access = field(json, "access_token").and_then(Value::as_str).ok_or_else(|| Problem::Refused("no access token".into()))?.to_string();
    let private = field(json, "privateKey")
        .or_else(|| field(json, "accountKeys").and_then(|k| field(k, "publicKeyEncryptionKeyPair")).and_then(|p| field(p, "wrappedPrivateKey")))
        .and_then(Value::as_str)
        .and_then(|p| decrypt(p, &user).ok())
        .and_then(|der| RsaPrivateKey::from_pkcs8_der(&der).ok());
    let remember = field(json, "twoFactorToken").and_then(Value::as_str).map(str::to_string);
    Ok(Unlocked { access, user, private, remember })
}

/// The account a session is for: the e-mail its access token names, in lower case.
pub fn account(unlocked: &Unlocked) -> Option<String> {
    let claims = URL_SAFE_NO_PAD.decode(unlocked.access.split('.').nth(1)?.trim_end_matches('=')).ok()?;
    let claims: Value = serde_json::from_slice(&claims).ok()?;
    claims.get("email").and_then(Value::as_str).map(|e| e.trim().to_lowercase())
}

/// What Bitwarden asks of a passkey to log in: the options the browser
/// takes (JSON), and the token that goes back with the key's answer.
pub struct PasskeyAsk {
    pub options: String,
    pub token: String,
}

/// A passkey login begun: asked of no account, the key says whose it is.
pub fn passkey_ask(server: &Server) -> Result<PasskeyAsk, Problem> {
    let (status, json) = answer(
        agent()
            .get(&format!("{}/accounts/webauthn/assertion-options", server.identity))
            .header("Accept", "application/json")
            .header("Bitwarden-Client-Name", "desktop")
            .header("Bitwarden-Client-Version", &client_version(server))
            .header("Device-Type", device_type())
            .call(),
    )?;
    match (status, field(&json, "options").filter(|o| o.is_object()), field(&json, "token").and_then(Value::as_str)) {
        (200, Some(options), Some(token)) => Ok(PasskeyAsk { options: options.to_string(), token: token.to_string() }),
        _ => Err(Problem::Refused(format!("passkey: {status}"))),
    }
}

/// The salt of the key's secret that opens the vault (WebAuthn PRF), as
/// Bitwarden's apps ask it: SHA-256 of "passwordless-login".
pub fn passkey_salt() -> [u8; 32] {
    Sha256::digest(b"passwordless-login").into()
}

/// Where a security key is asked: a page of the vault's own (its light
/// security-key page, with nothing to show unasked), so the key signs for the
/// one address Bitwarden takes keys' signatures from. Held unseen in the
/// window; `key_script` asks the key there.
pub fn key_page(server: &Server) -> String {
    format!("{}/webauthn-connector.html", server.vault)
}

/// The script that asks the key in that page: `options` as Bitwarden gave
/// them (a second step's, or `passkey_ask`'s for the key alone). The answer
/// waits in the page until taken (`window.sioulKey()`): for a second step,
/// {"token"}, the step's code; for the key alone, {"response", "secret"};
/// else {"error"} ("not-allowed": not used in time, or cancelled).
pub fn key_script(options: &str, passkey: bool) -> String {
    let options: Value = serde_json::from_str(options).unwrap_or(Value::Null);
    let mode = Value::String(if passkey { "passkey" } else { "factor" }.into());
    let salt = Value::String(URL_SAFE_NO_PAD.encode(passkey_salt()));
    format!("{}({mode}, {options}, {salt});\n", include_str!("bitwarden-key.js").trim_end())
}

/// Logs in with a passkey and opens the vault with the key's secret: `token`
/// as `passkey_ask` gave it, `response` the key's answer (JSON, as
/// Bitwarden's apps send it), `secret` the key's PRF output (base64url). No
/// second step: Bitwarden counts the key's PIN as one.
pub fn unlock_with_passkey(server: &Server, device: &str, token: &str, response: &str, secret: &str) -> Result<Unlocked, Problem> {
    let secret = Zeroizing::new(URL_SAFE_NO_PAD.decode(secret.trim()).unwrap_or_default());
    let secret: Zeroizing<[u8; 32]> = Zeroizing::new(secret.as_slice().try_into().map_err(|_| Problem::NoKeySecret)?);
    let form: Vec<(&str, String)> = vec![
        ("grant_type", "webauthn".into()),
        ("token", token.to_string()),
        ("deviceResponse", response.to_string()),
        ("scope", "api offline_access".into()),
        ("client_id", "desktop".into()),
        ("deviceType", device_type().into()),
        ("deviceIdentifier", device.to_string()),
        ("deviceName", "Sioul".into()),
    ];
    let (status, json) = ask_token(server, None, &form)?;
    if status != 200 {
        return Err(refusal(&json, status));
    }
    let option = field(&json, "userDecryptionOptions").and_then(|o| field(o, "webAuthnPrfOption")).ok_or(Problem::PasskeyNotForVault)?;
    session(&json, user_key_of_passkey(&secret, option)?)
}

/// The user key, opened with a passkey's secret: the secret stretched as a
/// master key is (HKDF-Expand, "enc" and "mac") opens the RSA key Bitwarden
/// keeps for the passkey, which opens the user key (`WebAuthnPrfOption`).
fn user_key_of_passkey(secret: &[u8; 32], option: &Value) -> Result<Key, Problem> {
    let wrapped = |name: &str| field(option, name).and_then(Value::as_str).ok_or(Problem::PasskeyNotForVault);
    let private = decrypt(wrapped("encryptedPrivateKey")?, &Key::stretched(secret))?;
    let private = RsaPrivateKey::from_pkcs8_der(&private).map_err(|_| Problem::Refused("passkey: RSA key".into()))?;
    Key::from_bytes(&decrypt_rsa(wrapped("encryptedUserKey")?, &private)?)
}

/// Sends the code of the e-mail second step (provider 1).
pub fn send_email_code(server: &Server, email: &str, password: &str, device: &str) -> Result<(), Problem> {
    let kdf = prelogin(server, email)?;
    let master = master_key(password, email, kdf)?;
    let body = serde_json::json!({ "email": email.trim(), "masterPasswordHash": password_hash(&master, password), "deviceIdentifier": device });
    let (status, json) = answer(
        agent()
            .post(&format!("{}/two-factor/send-email-login", server.api))
            .header("Content-Type", "application/json")
            .header("Bitwarden-Client-Name", "desktop")
            .header("Bitwarden-Client-Version", &client_version(server))
            .header("Device-Type", device_type())
            .send(body.to_string()),
    )?;
    if (200..300).contains(&status) {
        return Ok(());
    }
    let said = [field(&json, "errorModel").and_then(|m| field(m, "message")), field(&json, "message")].into_iter().flatten().filter_map(Value::as_str).next().unwrap_or("").to_string();
    Err(Problem::Refused(if said.is_empty() { format!("e-mail code: {status}") } else { said }))
}

fn form_encode(text: &str) -> String {
    text.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

/// A login of the vault.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Item {
    /// Bitwarden's id of the item, to come back to the one chosen.
    pub id: String,
    pub name: String,
    pub username: String,
    pub password: String,
    pub uris: Vec<String>,
    /// The one-time code's secret: base32, `otpauth://…` or `steam://…`.
    pub totp: String,
}

/// Printed without its secrets, should it ever be (a log, a test's failure).
impl std::fmt::Debug for Item {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Item").field("id", &self.id).field("name", &self.name).field("username", &self.username).field("uris", &self.uris).finish_non_exhaustive()
    }
}

/// Its secrets wiped when it goes.
impl Drop for Item {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.password.zeroize();
        self.totp.zeroize();
    }
}

/// This computer, as Bitwarden knows it: one identifier kept in `path`, so it
/// is not a new device each time.
pub fn device_id(path: &std::path::Path) -> String {
    if let Some(id) = std::fs::read_to_string(path).ok().map(|t| t.trim().to_string()).filter(|t| t.len() >= 32) {
        return id;
    }
    let id = uuid::Uuid::new_v4().to_string();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, &id);
    id
}

/// Reads the vault: every login, opened.
pub fn sync(server: &Server, unlocked: &Unlocked) -> Result<Vec<Item>, Problem> {
    let (status, json) = answer(
        agent()
            .get(&format!("{}/sync?excludeDomains=true", server.api))
            .header("Authorization", &format!("Bearer {}", unlocked.access))
            .header("Bitwarden-Client-Name", "desktop")
            .header("Bitwarden-Client-Version", &client_version(server))
            .header("Device-Type", device_type())
            .call(),
    )?;
    if status != 200 {
        return Err(Problem::Refused(format!("sync: {status}")));
    }
    // Each organisation's key, through your RSA key.
    let mut orgs: HashMap<String, Key> = HashMap::new();
    if let (Some(private), Some(list)) = (unlocked.private.as_ref(), field(&json, "profile").and_then(|p| field(p, "organizations")).and_then(Value::as_array)) {
        for org in list {
            if let (Some(id), Some(key)) = (field(org, "id").and_then(Value::as_str), field(org, "key").and_then(Value::as_str))
                && let Ok(bytes) = decrypt_rsa(key, private)
                && let Ok(key) = Key::from_bytes(&bytes)
            {
                orgs.insert(id.to_string(), key);
            }
        }
    }
    let mut items = Vec::new();
    for cipher in field(&json, "ciphers").and_then(Value::as_array).into_iter().flatten() {
        // Logins only, and not those in the trash.
        if field(cipher, "type").and_then(Value::as_u64) != Some(1) || field(cipher, "deletedDate").is_some() {
            continue;
        }
        let base = match field(cipher, "organizationId").and_then(Value::as_str) {
            Some(org) => match orgs.get(org) {
                Some(key) => key,
                None => continue,
            },
            None => &unlocked.user,
        };
        // An item may have its own key, opened with the user's or the organisation's.
        let own = field(cipher, "key").and_then(Value::as_str).and_then(|k| decrypt(k, base).ok()).and_then(|b| Key::from_bytes(&b).ok());
        let key = own.as_ref().unwrap_or(base);
        let login = field(cipher, "login");
        items.push(Item {
            id: field(cipher, "id").and_then(Value::as_str).unwrap_or_default().to_string(),
            name: text(field(cipher, "name"), key),
            username: text(login.and_then(|l| field(l, "username")), key),
            password: text(login.and_then(|l| field(l, "password")), key),
            totp: text(login.and_then(|l| field(l, "totp")), key),
            uris: login.and_then(|l| field(l, "uris")).and_then(Value::as_array).into_iter().flatten().map(|u| text(field(u, "uri"), key)).filter(|u| !u.is_empty()).collect(),
        });
    }
    Ok(items)
}

/// The sites of a login, as their hosts in lower case: its addresses that are
/// web pages, with or without "https://". An app's ("androidapp://…") is no
/// site, nor is a pattern ("^https?://…", for Bitwarden's matching by regex).
pub fn hosts_of(item: &Item) -> Vec<String> {
    item.uris.iter().filter_map(|uri| web_host(uri)).collect()
}

fn web_host(uri: &str) -> Option<String> {
    let uri = uri.trim();
    let address = match uri.split_once("://") {
        Some((scheme, _)) if !scheme.eq_ignore_ascii_case("https") && !scheme.eq_ignore_ascii_case("http") => return None,
        Some(_) => uri.to_string(),
        None => format!("https://{uri}"),
    };
    let host = sioul_core::sites::host_of(&address);
    (!host.is_empty()).then_some(host)
}

/// The domain a host is registered under (`sites::domain_of`: "ameli.fr" for
/// assure.ameli.fr); an address in numbers (192.168.1.1) or a computer's
/// name (one label) is its own.
fn domain(host: &str) -> String {
    let numbers = host.split('.').all(|label| !label.is_empty() && label.bytes().all(|b| b.is_ascii_digit()));
    if numbers || host.starts_with('[') || !host.contains('.') { host.to_string() } else { sioul_core::sites::domain_of(host) }
}

/// The registrable domain of an address or a host, as the search reads it:
/// "ameli.fr" for https://assure.ameli.fr/….
pub fn domain_of_address(address: &str) -> String {
    domain(&sioul_core::sites::host_of(address.trim()))
}

/// Case and accents aside, as everywhere in Sioul (`text::fold_char`).
fn folded(text: &str) -> String {
    text.chars().map(sioul_core::text::fold_char).collect()
}

/// Whether `word` begins a label of `host`, or a part of one between hyphens:
/// "ameli" in ameli.fr and assure.ameli.fr, not in camelia.com; "agricole" in
/// credit-agricole.fr; "ameli.f", typed on the way to "ameli.fr", in ameli.fr.
fn begins_label(host: &str, word: &str) -> bool {
    host.match_indices(word).any(|(at, _)| at == 0 || matches!(host.as_bytes()[at - 1], b'.' | b'-'))
}

/// Whether `word` begins a word of `text`: "ameli" in "Mon Ameli", not in "Camelia".
fn begins_word(text: &str, word: &str) -> bool {
    text.match_indices(word).any(|(at, _)| !text[..at].chars().next_back().is_some_and(char::is_alphanumeric))
}

/// Whether `word` is a whole word of `text`: "google" in "Google Play", not in "Googleplex".
fn whole_word(text: &str, word: &str) -> bool {
    text.match_indices(word).any(|(at, _)| !text[..at].chars().next_back().is_some_and(char::is_alphanumeric) && !text[at + word.len()..].chars().next().is_some_and(char::is_alphanumeric))
}

/// What a login is searched by, folded once as the vault opens: its name, its
/// user name, its sites and their domains. Never its password, its notes or
/// its one-time code's secret: no search reads them.
struct Searched {
    name: String,
    user: String,
    hosts: Vec<String>,
    domains: Vec<String>,
    /// Its sites as shown: `hosts` before folding.
    shown: Vec<String>,
}

impl Searched {
    fn of(item: &Item) -> Searched {
        let shown = hosts_of(item);
        let hosts: Vec<String> = shown.iter().map(|h| folded(h)).collect();
        Searched { name: folded(&item.name), user: folded(&item.username), domains: hosts.iter().map(|h| domain(h)).collect(), hosts, shown }
    }

    /// How near the page its sites are: 0 its host, 1 its domain, 2 neither;
    /// and which of its sites is.
    fn nearness(&self, page: &Page) -> (u8, Option<usize>) {
        if page.host.is_empty() {
            return (2, None);
        }
        if let Some(at) = self.hosts.iter().position(|h| *h == page.host) {
            return (0, Some(at));
        }
        match self.domains.iter().position(|d| *d == page.domain) {
            Some(at) => (1, Some(at)),
            None => (2, None),
        }
    }
}

/// The address the logins are wanted for: a site's page, an account's server.
struct Page {
    host: String,
    domain: String,
}

impl Page {
    fn of(url: &str) -> Page {
        let host = folded(&sioul_core::sites::host_of(url.trim()));
        Page { domain: domain(&host), host }
    }
}

/// How a login's site matched the site searched, the best first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum By {
    /// The same host.
    Host,
    /// The same registrable domain.
    Domain,
    /// A host whose labels begin with the words typed.
    Label,
    /// The login's name.
    Name,
}

/// The site searched, as typed: an address ("ameli.fr", a page's whole
/// address), or words ("ameli", "credit agricole").
enum SiteAsked {
    Address {
        host: String,
        domain: String,
        /// The domain's own name, for logins named after their site, as a
        /// whole word: "ameli" for ameli.fr; "x" for x.com, not "Xbox".
        name: String,
    },
    Words(Vec<String>),
}

impl SiteAsked {
    fn of(text: &str) -> Option<SiteAsked> {
        let text = folded(text.trim());
        if text.is_empty() {
            return None;
        }
        // An address: a scheme, or one word with a dot, a port or a path ("ameli.fr", "localhost:8080").
        if text.contains("://") || (text.contains(['.', ':', '/']) && !text.contains(char::is_whitespace)) {
            let host = sioul_core::sites::host_of(&if text.contains("://") { text.clone() } else { format!("https://{text}") });
            if !host.is_empty() {
                let domain = domain(&host);
                let numbers = domain.bytes().all(|b| b.is_ascii_digit() || b == b'.');
                let name = if numbers || domain.starts_with('[') { String::new() } else { domain.split('.').next().unwrap_or_default().to_string() };
                return Some(SiteAsked::Address { host, domain, name });
            }
        }
        Some(SiteAsked::Words(text.split_whitespace().map(str::to_string).collect()))
    }

    /// How a login matches, and which of its sites did; `None` when it does not.
    fn matches(&self, login: &Searched, page: &Page) -> Option<(By, Option<usize>)> {
        match self {
            SiteAsked::Address { host, domain, name } => {
                // A page's domain stands for the page: its own host is the same host.
                let same = |h: &String| h == host || (*host == page.domain && *h == page.host);
                if let Some(at) = login.hosts.iter().position(same) {
                    return Some((By::Host, Some(at)));
                }
                if let Some(at) = login.domains.iter().position(|d| d == domain) {
                    return Some((By::Domain, Some(at)));
                }
                if let Some(at) = login.hosts.iter().position(|h| begins_label(h, host)) {
                    return Some((By::Label, Some(at)));
                }
                (!name.is_empty() && whole_word(&login.name, name)).then_some((By::Name, None))
            }
            SiteAsked::Words(words) => {
                if let Some(at) = login.hosts.iter().position(|h| words.iter().all(|w| begins_label(h, w))) {
                    return Some((By::Label, Some(at)));
                }
                words.iter().all(|w| begins_word(&login.name, w)).then_some((By::Name, None))
            }
        }
    }
}

/// A match, before the best are kept: what it is ordered by (not the login
/// chosen last, how its site matched, how near the page, not the user name
/// typed whole), which login, how its site matched, which of its sites to show.
struct Hit {
    key: (bool, Option<By>, u8, bool),
    login: usize,
    by: Option<By>,
    site: usize,
}

/// A login found by a search.
pub struct Found<'a> {
    pub item: &'a Item,
    /// How its site matched: `None` when no site was searched.
    pub by: Option<By>,
    /// Its site to show: the one that matched, else the page's, else its first ("" without one).
    pub site: String,
    /// Made for the page's own domain (`find`'s `page`).
    pub own: bool,
}

/// What the chooser opens with for a page (`Logins::opening`).
#[derive(Debug, PartialEq, Eq)]
pub struct Opening {
    /// The page's registrable domain: the Site field's first text.
    pub domain: String,
    /// The domain of the login chosen there last, "" when none: the Site
    /// field's first text instead, the page's own domain when that login is
    /// made for it.
    pub chosen: String,
    /// The login filled at once: the page's only one, unless another was
    /// chosen there last.
    pub only: Option<String>,
}

/// The vault's logins, opened, with what each is searched by.
pub struct Logins {
    items: Vec<Item>,
    searched: Vec<Searched>,
}

impl Logins {
    pub fn new(items: Vec<Item>) -> Logins {
        let searched = items.iter().map(Searched::of).collect();
        Logins { items, searched }
    }

    pub fn items(&self) -> &[Item] {
        &self.items
    }

    /// What the chooser opens with for a page, and what "Fill the login" fills
    /// at once; `last` is the login chosen there last.
    pub fn opening(&self, page: &str, last: Option<&str>) -> Opening {
        let own = self.for_site(page);
        let last = last.and_then(|id| self.items.iter().find(|item| item.id == id));
        let only = match (own.as_slice(), last) {
            ([one], None) => Some(one.id.clone()),
            ([one], Some(last)) if last.id == one.id => Some(one.id.clone()),
            _ => None,
        };
        let domain = domain_of_address(page);
        let chosen = match last {
            Some(last) if own.iter().any(|item| item.id == last.id) => domain.clone(),
            Some(last) => hosts_of(last).first().map(|host| domain_of_address(host)).unwrap_or_default(),
            None => String::new(),
        };
        Opening { domain, chosen, only }
    }

    /// The logins for a page: those made for its domain, its own host first.
    pub fn for_site(&self, page: &str) -> Vec<&Item> {
        let page = Page::of(page);
        let mut found: Vec<(u8, &Item)> = self.items.iter().zip(&self.searched).map(|(item, s)| (s.nearness(&page).0, item)).filter(|(near, _)| *near < 2).collect();
        found.sort_by_key(|(near, _)| *near);
        found.into_iter().map(|(_, item)| item).collect()
    }

    /// The logins by site and by user name, each optional, both matching when
    /// both are given; nothing when neither is.
    /// - `site` matches the login's sites: the same host, then the same
    ///   registrable domain, then a host whose labels begin with the words
    ///   typed ("ameli": ameli.fr, assure.ameli.fr, not camelia.com), then the
    ///   login's name (an address's own name, "ameli" for ameli.fr). Never its
    ///   user name: "gmail" finds Gmail, not the logins with a Gmail address.
    /// - `user` matches the user name only, holding every word typed.
    ///
    /// Case and accents aside. The best `limit` of them, the best first:
    /// `first` (the login chosen last for the page), then by how the site
    /// matched, the page's own (its host, then its domain), the user name
    /// typed whole, the name; and how many matched in all. Only those kept are
    /// sorted: a large vault, all of it matching, costs one pass.
    pub fn find(&self, site: &str, user: &str, page: &str, first: Option<&str>, limit: usize) -> (Vec<Found<'_>>, usize) {
        let asked = SiteAsked::of(site);
        let user = folded(user.trim());
        let words: Vec<&str> = user.split_whitespace().collect();
        let whole = words.join(" ");
        if asked.is_none() && words.is_empty() {
            return (Vec::new(), 0);
        }
        let page = Page::of(page);
        let mut hits: Vec<Hit> = Vec::new();
        for (at, (item, login)) in self.items.iter().zip(&self.searched).enumerate() {
            let matched = match &asked {
                Some(asked) => match asked.matches(login, &page) {
                    Some(matched) => Some(matched),
                    None => continue,
                },
                None => None,
            };
            if !words.iter().all(|w| login.user.contains(w)) {
                continue;
            }
            let (near, at_page) = login.nearness(&page);
            let by = matched.map(|(by, _)| by);
            hits.push(Hit { key: (first != Some(item.id.as_str()), by, near, words.is_empty() || login.user != whole), login: at, by, site: matched.and_then(|(_, site)| site).or(at_page).unwrap_or(0) });
        }
        let total = hits.len();
        let order = |a: &Hit, b: &Hit| {
            let (x, y) = (&self.searched[a.login], &self.searched[b.login]);
            a.key.cmp(&b.key).then_with(|| x.name.cmp(&y.name)).then_with(|| x.user.cmp(&y.user)).then_with(|| a.login.cmp(&b.login))
        };
        if hits.len() > limit {
            if limit > 0 {
                hits.select_nth_unstable_by(limit - 1, order);
            }
            hits.truncate(limit);
        }
        hits.sort_unstable_by(order);
        let found = hits
            .into_iter()
            .map(|hit| Found { item: &self.items[hit.login], by: hit.by, site: self.searched[hit.login].shown.get(hit.site).cloned().unwrap_or_default(), own: hit.key.2 < 2 })
            .collect();
        (found, total)
    }
}

/// RFC 4648 base32, letters and digits 2–7, spaces and padding ignored.
fn base32(text: &str) -> Option<Vec<u8>> {
    let mut bits: u64 = 0;
    let mut count = 0;
    let mut out = Vec::new();
    for c in text.chars().filter(|c| !c.is_whitespace() && *c != '=' && *c != '-') {
        let value = match c.to_ascii_uppercase() {
            c @ 'A'..='Z' => c as u64 - 'A' as u64,
            c @ '2'..='7' => c as u64 - '2' as u64 + 26,
            _ => return None,
        };
        bits = (bits << 5) | value;
        count += 5;
        if count >= 8 {
            count -= 8;
            out.push((bits >> count) as u8);
        }
    }
    Some(out)
}

/// The one-time code now (RFC 6238; RFC 4226 for the truncation): from a base32
/// secret, an `otpauth://totp/…` address (its digits, period and SHA-1,
/// SHA-256 or SHA-512), or Steam's five characters.
pub fn totp(secret: &str, now: i64) -> Option<String> {
    let secret = secret.trim();
    let (key, digits, period, algorithm, steam) = if let Some(rest) = secret.strip_prefix("otpauth://") {
        let query = rest.split_once('?').map_or("", |(_, q)| q);
        let param = |name: &str| query.split('&').find_map(|p| p.split_once('=').filter(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.to_string()));
        let digits = param("digits").and_then(|d| d.parse().ok()).unwrap_or(6);
        let period = param("period").and_then(|p| p.parse().ok()).unwrap_or(30);
        (base32(&param("secret")?)?, digits, period, param("algorithm").unwrap_or_else(|| "SHA1".into()).to_uppercase(), false)
    } else if let Some(rest) = secret.strip_prefix("steam://") {
        (base32(rest)?, 5, 30, "SHA1".into(), true)
    } else {
        (base32(secret)?, 6, 30, "SHA1".into(), false)
    };
    if key.is_empty() || period == 0 {
        return None;
    }
    let counter = (now / period).to_be_bytes();
    let digest: Vec<u8> = match algorithm.as_str() {
        "SHA256" => {
            let mut mac = Hmac::<Sha256>::new_from_slice(&key).ok()?;
            mac.update(&counter);
            mac.finalize().into_bytes().to_vec()
        }
        "SHA512" => {
            let mut mac = Hmac::<sha2::Sha512>::new_from_slice(&key).ok()?;
            mac.update(&counter);
            mac.finalize().into_bytes().to_vec()
        }
        _ => {
            let mut mac = Hmac::<sha1::Sha1>::new_from_slice(&key).ok()?;
            mac.update(&counter);
            mac.finalize().into_bytes().to_vec()
        }
    };
    let offset = usize::from(digest.last()? & 0x0f);
    let code = u32::from_be_bytes(digest.get(offset..offset + 4)?.try_into().ok()?) & 0x7fff_ffff;
    if steam {
        const STEAM: &[u8] = b"23456789BCDFGHJKMNPQRTVWXY";
        let mut value = code as usize;
        return Some((0..5).map(|_| {
            let c = STEAM[value % STEAM.len()] as char;
            value /= STEAM.len();
            c
        }).collect());
    }
    let digits = u32::clamp(digits, 4, 10);
    Some(format!("{:0width$}", u64::from(code) % 10u64.pow(digits), width = digits as usize))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_as_bitwarden_makes_them() {
        // Bitwarden's own test vectors (bitwarden-crypto, master_key.rs and utils.rs).
        for email in ["test@bitwarden.com", "TEST@bitwarden.com", " test@bitwarden.com"] {
            let master = master_key("asdfasdf", email, Kdf::Pbkdf2 { iterations: 100_000 }).unwrap();
            assert_eq!(password_hash(&master, "asdfasdf"), "wmyadRMyBZOH7P/a/ucTCbSghKgdzDpPqUnu/DAVtSw=");
        }
        let master = master_key("asdfasdf", "test_salt", Kdf::Argon2id { iterations: 4, memory_mib: 32, parallelism: 2 }).unwrap();
        assert_eq!(password_hash(&master, "asdfasdf"), "PR6UjYmjmppTYcdyTiNbAhPJuQQOmynKbdEl1oyi/iQ=");
        let master = [31, 79, 104, 226, 150, 71, 177, 90, 194, 80, 172, 209, 17, 129, 132, 81, 138, 167, 69, 167, 254, 149, 2, 27, 39, 197, 64, 42, 22, 195, 86, 75];
        let stretched = Key::stretched(&master);
        assert_eq!(stretched.enc.as_slice(), [111, 31, 178, 45, 238, 152, 37, 114, 143, 215, 124, 83, 135, 173, 195, 23, 142, 134, 120, 249, 61, 132, 163, 182, 113, 197, 189, 204, 188, 21, 237, 96]);
        assert_eq!(stretched.mac.as_slice(), [221, 127, 206, 234, 101, 27, 202, 38, 86, 52, 34, 28, 78, 28, 185, 16, 48, 61, 127, 166, 209, 247, 194, 87, 232, 26, 48, 85, 193, 249, 179, 155]);
    }

    #[test]
    fn encstrings_open_and_refuse_a_changed_byte() {
        use aes::cipher::BlockEncryptMut;
        let key = Key::from_bytes(&[7u8; 64]).unwrap();
        let iv = [3u8; 16];
        let data = cbc::Encryptor::<aes::Aes256>::new_from_slices(&key.enc, &iv).unwrap().encrypt_padded_vec_mut::<Pkcs7>(b"hunter2");
        let mut mac = HmacSha256::new_from_slice(&key.mac).unwrap();
        mac.update(&iv);
        mac.update(&data);
        let mac = mac.finalize().into_bytes();
        let enc = format!("2.{}|{}|{}", STANDARD.encode(iv), STANDARD.encode(&data), STANDARD.encode(mac));
        assert_eq!(decrypt(&enc, &key).unwrap().as_slice(), b"hunter2");
        let mut bad = mac.to_vec();
        bad[0] ^= 1;
        let forged = format!("2.{}|{}|{}", STANDARD.encode(iv), STANDARD.encode(&data), STANDARD.encode(bad));
        assert_eq!(decrypt(&forged, &key).err(), Some(Problem::WrongPassword));
        // The same text without its check: refused by a key that has a MAC key,
        // opened by the oldest keys, which have none.
        let unchecked = format!("0.{}|{}", STANDARD.encode(iv), STANDARD.encode(&data));
        assert!(matches!(decrypt(&unchecked, &key), Err(Problem::Refused(_))));
        let oldest = Key::from_bytes(&key.enc).unwrap();
        assert_eq!(decrypt(&unchecked, &oldest).unwrap().as_slice(), b"hunter2");
    }

    #[test]
    fn a_server_sets_no_key_derivation_out_of_bounds() {
        // Cheaper than Bitwarden's clients accept, or more than its server allows.
        for kdf in [
            Kdf::Pbkdf2 { iterations: 4_999 },
            Kdf::Pbkdf2 { iterations: u32::MAX },
            Kdf::Argon2id { iterations: 1, memory_mib: 64, parallelism: 4 },
            Kdf::Argon2id { iterations: 3, memory_mib: 1, parallelism: 4 },
            Kdf::Argon2id { iterations: 3, memory_mib: u32::MAX, parallelism: 4 },
            Kdf::Argon2id { iterations: 3, memory_mib: 64, parallelism: 0 },
            Kdf::Argon2id { iterations: 3, memory_mib: 64, parallelism: 255 },
        ] {
            assert!(matches!(master_key("asdfasdf", "test@example.org", kdf), Err(Problem::Refused(_))), "{kdf:?}");
        }
    }

    #[test]
    fn cose_messages_open() {
        use chacha20poly1305::aead::{Aead, KeyInit, Payload};
        use ciborium::Value;
        // A COSE key as Bitwarden pads it (to 65 bytes at least, so never read
        // as a 64-byte key), then a padded UTF-8 message under it.
        let k = vec![9u8; 32];
        let mut clean = Vec::new();
        ciborium::into_writer(&Value::Map(vec![(Value::Integer(3.into()), Value::Integer(XCHACHA20_POLY1305.into())), (Value::Integer((-1).into()), Value::Bytes(k.clone()))]), &mut clean).unwrap();
        let pad = 65usize.saturating_sub(clean.len()).max(1);
        clean.extend(std::iter::repeat_n(pad as u8, pad));
        assert_eq!(clean.len(), 65);
        let key = Key::from_bytes(&clean).unwrap();
        let mut protected = Vec::new();
        ciborium::into_writer(&Value::Map(vec![(Value::Integer(1.into()), Value::Integer(XCHACHA20_POLY1305.into())), (Value::Integer(3.into()), Value::Text("application/x.bitwarden.utf8-padded".into()))]), &mut protected).unwrap();
        let mut aad = Vec::new();
        ciborium::into_writer(&Value::Array(vec![Value::Text("Encrypt0".into()), Value::Bytes(protected.clone()), Value::Bytes(Vec::new())]), &mut aad).unwrap();
        let nonce = [5u8; 24];
        let mut plain = b"secret".to_vec();
        let pad = 32 - plain.len();
        plain.extend(std::iter::repeat_n(pad as u8, pad));
        let sealed = chacha20poly1305::XChaCha20Poly1305::new_from_slice(&k).unwrap().encrypt(chacha20poly1305::XNonce::from_slice(&nonce), Payload { msg: &plain, aad: &aad }).unwrap();
        let mut message = Vec::new();
        ciborium::into_writer(&Value::Array(vec![Value::Bytes(protected), Value::Map(vec![(Value::Integer(5.into()), Value::Bytes(nonce.to_vec()))]), Value::Bytes(sealed)]), &mut message).unwrap();
        assert_eq!(decrypt(&format!("7.{}", STANDARD.encode(&message)), &key).unwrap().as_slice(), b"secret");
    }

    #[test]
    fn one_time_codes() {
        // RFC 6238, appendix B: the ASCII secret "12345678901234567890", 8 digits.
        let rfc = "otpauth://totp/x?secret=GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ&digits=8";
        assert_eq!(totp(rfc, 59).as_deref(), Some("94287082"));
        assert_eq!(totp(rfc, 1_111_111_109).as_deref(), Some("07081804"));
        assert_eq!(totp("GEZD GNBV GY3T QOJQ GEZD GNBV GY3T QOJQ", 59).as_deref(), Some("287082"));
        assert_eq!(totp("not base32 !", 59), None);
    }

    #[test]
    fn servers() {
        assert_eq!(Server::of("bitwarden.eu").api, "https://api.bitwarden.eu");
        assert_eq!(Server::of("Vault.Bitwarden.eu/").api, "https://api.bitwarden.eu", "the vault's address, as a browser shows it");
        assert_eq!(Server::of("vault.bitwarden.com").identity, "https://identity.bitwarden.com");
        assert_eq!(Server::of("vault.example.org").identity, "https://vault.example.org/identity");
    }
    #[test]
    fn versions_read() {
        assert!(is_version("2026.9.2") && is_version("2025.12.0") && is_version("2026.10"));
        assert!(!is_version("") && !is_version("2026") && !is_version("v2026.9.2") && !is_version("2026.9.2-beta"));
        assert!(is_version(CLIENT_VERSION));
    }

    /// Bitwarden's cloud takes Sioul's login request for a made-up account at
    /// example.org: refused for its password, not for its version. Run by
    /// hand: it asks Bitwarden's servers.
    #[test]
    #[ignore]
    fn the_cloud_takes_the_version() {
        let server = Server::of("");
        assert!(is_version(&client_version(&server)));
        let refused = unlock(&server, "nobody-sioul-test@example.org", "not a real password", &uuid::Uuid::new_v4().to_string(), None, None, None);
        assert!(matches!(refused, Err(Problem::WrongPassword)), "{:?}", refused.err());
    }

    /// A fixed stream of bytes, for the RSA keys of the tests.
    struct Stream(u64);

    impl rsa::rand_core::RngCore for Stream {
        fn next_u32(&mut self) -> u32 {
            self.next_u64() as u32
        }
        fn next_u64(&mut self) -> u64 {
            // SplitMix64.
            self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        }
        fn fill_bytes(&mut self, dest: &mut [u8]) {
            for chunk in dest.chunks_mut(8) {
                let bytes = self.next_u64().to_le_bytes();
                chunk.copy_from_slice(&bytes[..chunk.len()]);
            }
        }
        fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rsa::rand_core::Error> {
            self.fill_bytes(dest);
            Ok(())
        }
    }

    impl rsa::rand_core::CryptoRng for Stream {}

    /// A passkey's keys made as Bitwarden's web vault makes them when its
    /// encryption is turned on (an RSA pair; the private key sealed by the
    /// stretched secret; the user key sealed by the public key, OAEP-SHA1),
    /// then opened by the secret, as Bitwarden's server gives them back.
    #[test]
    fn a_passkey_opens_the_vault() {
        use aes::cipher::BlockEncryptMut;
        use rsa::pkcs8::EncodePrivateKey;
        let mut stream = Stream(7);
        let pair = RsaPrivateKey::new(&mut stream, 1024).unwrap();
        let secret = [42u8; 32];
        let sealing = Key::stretched(&secret);
        let iv = [9u8; 16];
        let der = pair.to_pkcs8_der().unwrap();
        let data = cbc::Encryptor::<aes::Aes256>::new_from_slices(&sealing.enc, &iv).unwrap().encrypt_padded_vec_mut::<Pkcs7>(der.as_bytes());
        let mut mac = HmacSha256::new_from_slice(&sealing.mac).unwrap();
        mac.update(&iv);
        mac.update(&data);
        let private = format!("2.{}|{}|{}", STANDARD.encode(iv), STANDARD.encode(&data), STANDARD.encode(mac.finalize().into_bytes()));
        let user: Vec<u8> = (0..64u8).collect();
        let sealed_user = rsa::RsaPublicKey::from(&pair).encrypt(&mut stream, Oaep::new::<sha1::Sha1>(), &user).unwrap();
        let option = serde_json::json!({ "EncryptedPrivateKey": private, "EncryptedUserKey": format!("4.{}", STANDARD.encode(sealed_user)), "CredentialId": "abc" });
        let opened = user_key_of_passkey(&secret, &option).unwrap();
        assert_eq!(opened.enc.as_slice(), &user[..32]);
        assert_eq!(opened.mac.as_slice(), &user[32..]);
        // Another key's secret is refused before anything is decrypted.
        assert_eq!(user_key_of_passkey(&[1u8; 32], &option).err(), Some(Problem::WrongPassword));
        // A passkey whose encryption is off: nothing to open.
        assert_eq!(user_key_of_passkey(&secret, &serde_json::json!({})).err(), Some(Problem::PasskeyNotForVault));
        // No secret from the key: refused before Bitwarden is asked.
        let none = unlock_with_passkey(&Server::of("vault.invalid"), "device", "token", "{}", "");
        assert_eq!(none.err(), Some(Problem::NoKeySecret));
    }

    #[test]
    fn the_key_script() {
        let options = r#"{"challenge":"abc","rpId":"vault.bitwarden.com","allowCredentials":[]}"#;
        let script = key_script(options, true);
        assert!(script.starts_with("// SPDX-License-Identifier"), "{script}");
        assert!(script.contains(r#"})("passkey", {"#) && script.contains(r#""challenge":"abc""#), "{script}");
        // The salt Bitwarden's apps use: SHA-256 of "passwordless-login", base64url.
        assert!(script.trim_end().ends_with(&format!("\"{}\");", URL_SAFE_NO_PAD.encode(passkey_salt()))), "{script}");
        assert!(key_script(options, false).contains(r#"})("factor", {"#));
        assert_eq!(key_page(&Server::of("")), "https://vault.bitwarden.com/webauthn-connector.html");
        assert_eq!(key_page(&Server::of("bitwarden.eu")), "https://vault.bitwarden.eu/webauthn-connector.html");
    }

    /// A made-up vault: every name, address and secret here is invented.
    fn fixture() -> Logins {
        // Items wipe their secrets when dropped: built field by field.
        let login = |id: &str, name: &str, user: &str, uris: &[&str], password: &str, totp: &str| {
            let mut item = Item::default();
            item.id = id.into();
            item.name = name.into();
            item.username = user.into();
            item.uris = uris.iter().map(|u| u.to_string()).collect();
            item.password = password.into();
            item.totp = totp.into();
            item
        };
        Logins::new(vec![
            login("ameli", "Ameli", "1 85 07 75 123 456 78", &["https://ameli.fr"], "x", ""),
            login("assure", "Compte assuré", "1 85 07 75 123 456 78", &["https://assure.ameli.fr/PortailAS/appmanager"], "x", ""),
            login("camelia", "Camelia fleurs", "moi@exemple.test", &["camelia.com"], "x", ""),
            login("google", "Google", "aurore.exemple@gmail.com", &["https://accounts.google.com/v3/signin"], "x", "otpauth://totp/Google?secret=JBSWY3DPEHPK3PXP"),
            login("forum", "Forum de photo", "aurore.exemple@gmail.com", &["https://forum.photo.test"], "x", ""),
            login("shop", "Boutique", "aurore.exemple@gmail.com", &["boutique.test"], "x", ""),
            login("impots", "Impôts", "1234567890123", &[], "x", ""),
            login("bank-a", "Banque, particuliers", "Élodie", &["https://particuliers.banque.test/connexion"], "x", ""),
            login("bank-b", "Banque, pro", "elodie.pro", &["https://pro.banque.test"], "x", ""),
            login("credit", "Crédit agricole", "12345678", &["https://www.credit-agricole.fr"], "x", ""),
            login("router", "Box", "admin", &["http://192.168.1.1", "androidapp://com.box.admin"], "x", ""),
            // Secrets that look like what is searched: never matched.
            login("secret", "Coffre", "quelqu-un", &["https://coffre.test"], "gmail ameli Élodie", "GMAILAMELI234567"),
        ])
    }

    /// Every login a search finds, best first.
    fn search<'a>(vault: &'a Logins, site: &str, user: &str, page: &str, first: Option<&str>) -> Vec<Found<'a>> {
        let (found, total) = vault.find(site, user, page, first, usize::MAX);
        assert_eq!(found.len(), total);
        found
    }

    fn ids(found: Vec<Found<'_>>) -> Vec<String> {
        found.into_iter().map(|f| f.item.id.clone()).collect()
    }

    #[test]
    fn by_host_then_domain() {
        let vault = fixture();
        // Opened for a page, the field holding its domain: the page's host first, then its domain.
        assert_eq!(ids(search(&vault, "ameli.fr", "", "https://assure.ameli.fr/PortailAS/x", None)), ["assure", "ameli"]);
        assert_eq!(ids(search(&vault, "ameli.fr", "", "https://ameli.fr", None)), ["ameli", "assure"]);
        let found = search(&vault, "assure.ameli.fr", "", "", None);
        assert_eq!(found.iter().map(|f| (f.item.id.as_str(), f.by)).collect::<Vec<_>>(), [("assure", Some(By::Host)), ("ameli", Some(By::Domain))]);
        assert_eq!(found[0].site, "assure.ameli.fr");
        // A whole address, pasted.
        assert_eq!(ids(search(&vault, "https://www.credit-agricole.fr/particulier/acceder.html", "", "", None)), ["credit"]);
        // The login chosen last for the page comes first.
        assert_eq!(ids(search(&vault, "ameli.fr", "", "https://ameli.fr", Some("assure"))), ["assure", "ameli"]);
        // The page's own logins, as "Fill the login" counts them.
        let own: Vec<&str> = vault.for_site("https://pro.banque.test/espace").iter().map(|i| i.id.as_str()).collect();
        assert_eq!(own, ["bank-b", "bank-a"]);
        // An address in numbers is its own domain; an app's address is no site.
        assert_eq!(ids(search(&vault, "192.168.1.1", "", "", None)), ["router"]);
        assert_eq!(ids(search(&vault, "192.168.1.1:8080/admin", "", "", None)), ["router"], "a port and a path");
        assert!(search(&vault, "192.168.0.1", "", "", None).is_empty(), "not the same domain as 192.168.1.1");
        assert!(search(&vault, "admin", "", "", None).is_empty(), "androidapp://com.box.admin is no site");
        assert_eq!(hosts_of(&vault.items()[10]), ["192.168.1.1"]);
    }

    #[test]
    fn what_a_page_opens_with() {
        let vault = fixture();
        let opening = |page: &str, last: Option<&str>| {
            let o = vault.opening(page, last);
            (o.domain, o.chosen, o.only)
        };
        let some = |id: &str| Some(id.to_string());
        // One login for the site: filled at once.
        assert_eq!(opening("https://www.credit-agricole.fr/particulier", None), ("credit-agricole.fr".into(), "".into(), some("credit")));
        assert_eq!(opening("https://www.credit-agricole.fr/", Some("credit")), ("credit-agricole.fr".into(), "credit-agricole.fr".into(), some("credit")));
        // Another login chosen there last (a sign-in with Google): the chooser opens on its domain.
        assert_eq!(opening("https://www.credit-agricole.fr/", Some("google")), ("credit-agricole.fr".into(), "google.com".into(), None));
        // A login gone from the vault since: as if none was chosen.
        assert_eq!(opening("https://www.credit-agricole.fr/", Some("gone")), ("credit-agricole.fr".into(), "".into(), some("credit")));
        // Two logins, or none: the chooser.
        assert_eq!(opening("https://pro.banque.test/espace", None).2, None);
        assert_eq!(opening("https://pro.banque.test/espace", Some("bank-a")), ("banque.test".into(), "banque.test".into(), None));
        assert_eq!(opening("https://unknown.test", None), ("unknown.test".into(), "".into(), None));
        // A mail server, the provider's login chosen for it before.
        assert_eq!(opening("https://imap.gmail.com", Some("google")), ("gmail.com".into(), "google.com".into(), None));
        // A login without a site chosen last: no domain to begin with.
        assert_eq!(opening("https://www.impots.gouv.fr", Some("impots")), ("impots.gouv.fr".into(), "".into(), None));
    }

    #[test]
    fn by_the_start_of_labels() {
        let vault = fixture();
        assert_eq!(ids(search(&vault, "ameli", "", "", None)), ["ameli", "assure"], "ameli.fr and assure.ameli.fr, not camelia.com");
        let found = search(&vault, "ameli", "", "", None);
        assert!(found.iter().all(|f| f.by == Some(By::Label)));
        assert_eq!(ids(search(&vault, "cam", "", "", None)), ["camelia"]);
        assert_eq!(ids(search(&vault, "agricole", "", "", None)), ["credit"], "a part between hyphens");
        assert_eq!(ids(search(&vault, "credit agricole", "", "", None)), ["credit"], "every word");
        // Typed on the way to an address.
        assert_eq!(ids(search(&vault, "ameli.", "", "", None)), ["ameli", "assure"]);
        assert_eq!(ids(search(&vault, "ameli.f", "", "", None)), ["ameli", "assure"]);
        assert!(search(&vault, "meli", "", "", None).is_empty());
    }

    #[test]
    fn by_name_when_no_site_matches() {
        let vault = fixture();
        let found = search(&vault, "impots", "", "", None);
        assert_eq!(found.iter().map(|f| (f.item.id.as_str(), f.by, f.site.as_str())).collect::<Vec<_>>(), [("impots", Some(By::Name), "")]);
        // An address's own name finds the logins named after it.
        assert_eq!(ids(search(&vault, "impots.gouv.fr", "", "https://www.impots.gouv.fr/accueil", None)), ["impots"]);
        // Sites before names: "banque" begins a label of both banks, and the name of neither matters then.
        let found = search(&vault, "banque", "", "", None);
        assert_eq!(found.iter().map(|f| (f.item.id.as_str(), f.by)).collect::<Vec<_>>(), [("bank-a", Some(By::Label)), ("bank-b", Some(By::Label))]);
        assert_eq!(ids(search(&vault, "fleurs", "", "", None)), ["camelia"]);
        assert!(search(&vault, "leurs", "", "", None).is_empty(), "words begin where the name's words begin");
        // An address's name is a whole word of the login's: "com" for com.box, not "Compte assuré".
        assert!(search(&vault, "com.box", "", "", None).is_empty());
        assert_eq!(ids(search(&vault, "box.example", "", "", None)), ["router"]);
    }

    #[test]
    fn by_user_name_alone() {
        let vault = fixture();
        assert_eq!(ids(search(&vault, "", "aurore.exemple@gmail.com", "", None)), ["shop", "forum", "google"], "by name: Boutique, Forum de photo, Google");
        assert_eq!(ids(search(&vault, "", "aurore", "", None)), ["shop", "forum", "google"]);
        assert_eq!(ids(search(&vault, "", "@gmail.com", "", None)), ["shop", "forum", "google"]);
        assert_eq!(ids(search(&vault, "", "elodie pro", "", None)), ["bank-b"], "every word");
        // The page's own first, then the user name typed whole: an account's server.
        assert_eq!(ids(search(&vault, "", "aurore.exemple@gmail.com", "https://forum.photo.test", None)), ["forum", "shop", "google"]);
        assert!(search(&vault, "", "", "https://ameli.fr", None).is_empty(), "nothing asked, nothing found");
        assert!(search(&vault, "  ", " ", "", None).is_empty());
    }

    #[test]
    fn both_fields_together() {
        let vault = fixture();
        assert_eq!(ids(search(&vault, "google", "aurore", "", None)), ["google"]);
        assert_eq!(ids(search(&vault, "banque.test", "elodie", "", None)), ["bank-a", "bank-b"]);
        assert_eq!(ids(search(&vault, "banque.test", "pro", "", None)), ["bank-b"]);
        assert!(search(&vault, "ameli", "aurore", "", None).is_empty());
        let found = search(&vault, "photo", "@gmail.com", "", None);
        assert_eq!(found.iter().map(|f| (f.item.id.as_str(), f.site.as_str())).collect::<Vec<_>>(), [("forum", "forum.photo.test")]);
    }

    #[test]
    fn a_site_is_not_a_user_name() {
        let vault = fixture();
        // "gmail" as a site: Gmail's own logins, not every login whose user name is a Gmail address.
        assert!(search(&vault, "gmail", "", "", None).is_empty());
        assert!(search(&vault, "gmail.com", "", "", None).is_empty());
        assert_eq!(ids(search(&vault, "google", "", "", None)), ["google"]);
        // A user name is no site either.
        assert!(search(&vault, "", "ameli", "", None).is_empty());
    }

    #[test]
    fn case_and_accents_aside() {
        let vault = fixture();
        assert_eq!(ids(search(&vault, "IMPÔTS", "", "", None)), ["impots"]);
        assert_eq!(ids(search(&vault, "Crédit", "", "", None)), ["credit"]);
        assert_eq!(ids(search(&vault, "AMELI.FR", "", "", None)), ["ameli", "assure"]);
        assert_eq!(ids(search(&vault, "", "ELODIE", "", None)), ["bank-a", "bank-b"]);
        assert_eq!(ids(search(&vault, "", "élodie.PRO", "", None)), ["bank-b"]);
        assert_eq!(ids(search(&vault, "compte assure", "", "", None)), ["assure"]);
    }

    #[test]
    fn secrets_never_match() {
        let vault = fixture();
        // A password reads "gmail ameli Élodie", its one-time secret "GMAILAMELI…": found by neither.
        for (site, user) in [("gmail", ""), ("ameli", ""), ("", "gmail"), ("", "elodie"), ("", "GMAILAMELI"), ("gmailameli", ""), ("ameli.fr", "elodie")] {
            assert!(!ids(search(&vault, site, user, "", None)).contains(&"secret".to_string()), "{site:?} {user:?}");
        }
        // Google's one-time secret.
        for (site, user) in [("JBSWY3DP", ""), ("", "JBSWY3DP"), ("", "jbswy3dpehpk3pxp"), ("totp", ""), ("", "otpauth")] {
            assert!(search(&vault, site, user, "", None).is_empty(), "{site:?} {user:?}");
        }
        // Every password is "x": only the user names holding an x.
        assert_eq!(ids(search(&vault, "", "x", "", None)), ["shop", "camelia", "forum", "google"]);
        assert_eq!(ids(search(&vault, "coffre", "", "", None)), ["secret"], "found by its site");
    }

    /// How long a search of a large vault takes (20,000 made-up logins), on the
    /// window's thread: printed, run by hand with `--ignored --nocapture`.
    #[test]
    #[ignore]
    fn a_large_vault_is_searched_quickly() {
        let items: Vec<Item> = (0..20_000)
            .map(|n| {
                let mut item = Item::default();
                item.id = n.to_string();
                item.name = format!("Login {n} société");
                item.username = format!("person{}@example{}.test", n % 97, n % 13);
                item.uris = vec![format!("https://www{}.site{}.example{}.test/login", n % 3, n, n % 7), format!("app{n}.test")];
                item
            })
            .collect();
        let started = std::time::Instant::now();
        let vault = Logins::new(items);
        println!("index: {:?}", started.elapsed());
        for (site, user) in [("site123", ""), ("example3.test", ""), ("", "person5"), ("site1", "example1"), ("societe", ""), ("", "@"), ("https://www1.site19999.example6.test/login", "")] {
            let started = std::time::Instant::now();
            let (found, total) = vault.find(site, user, "https://www1.site42.example0.test", None, 50);
            println!("{site:?} {user:?}: {} of {total} in {:?}", found.len(), started.elapsed());
        }
    }

    #[test]
    fn the_best_are_kept() {
        let vault = fixture();
        let (found, total) = vault.find("", "@gmail.com", "", None, 2);
        assert_eq!((ids(found), total), (vec!["shop".to_string(), "forum".to_string()], 3));
        let (found, total) = vault.find("", "@gmail.com", "", Some("google"), 1);
        assert_eq!((ids(found), total), (vec!["google".to_string()], 3), "the login chosen last kept first");
        let (found, total) = vault.find("", "@gmail.com", "", None, 0);
        assert!(found.is_empty() && total == 3);
    }

    #[test]
    fn the_account_of_a_session() {
        let claims = URL_SAFE_NO_PAD.encode(br#"{"email":" Someone@Example.org","premium":false}"#);
        let unlocked = Unlocked { access: format!("e30.{claims}.c2ln"), user: Key::from_bytes(&[0u8; 64]).unwrap(), private: None, remember: None };
        assert_eq!(account(&unlocked).as_deref(), Some("someone@example.org"));
    }

    /// Bitwarden's cloud begins a passkey login: options for the browser, for
    /// its vault's address, the PIN asked; a token to send back. Run by hand:
    /// it asks Bitwarden's servers (no account).
    #[test]
    #[ignore]
    fn the_cloud_asks_a_passkey() {
        let ask = passkey_ask(&Server::of("")).unwrap();
        let options: Value = serde_json::from_str(&ask.options).unwrap();
        assert_eq!(options["rpId"], "vault.bitwarden.com");
        assert_eq!(options["userVerification"], "required");
        assert!(options["challenge"].as_str().is_some_and(|c| c.len() >= 16));
        assert!(ask.token.len() > 32);
    }

}
