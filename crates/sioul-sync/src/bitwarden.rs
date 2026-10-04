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

/// The logins for a site: those whose address shares its domain, the same host first.
/// The sites of a login, as their hosts.
pub fn hosts_of(item: &Item) -> Vec<String> {
    item.uris.iter().map(|u| sioul_core::sites::host_of(&if u.contains("://") { u.clone() } else { format!("https://{u}") })).filter(|h| !h.is_empty()).collect()
}

/// The logins whose name, user name or sites hold every word of `query`,
/// case and French accents aside; by name.
pub fn search<'a>(items: &'a [Item], query: &str) -> Vec<&'a Item> {
    let fold = |text: &str| -> String { text.chars().map(sioul_core::text::fold_char).collect() };
    let words: Vec<String> = fold(query).split_whitespace().map(str::to_string).collect();
    if words.is_empty() {
        return Vec::new();
    }
    let mut found: Vec<(String, &Item)> = items
        .iter()
        .filter_map(|item| {
            let all = fold(&format!("{} {} {}", item.name, item.username, hosts_of(item).join(" ")));
            words.iter().all(|w| all.contains(w.as_str())).then(|| (fold(&item.name), item))
        })
        .collect();
    found.sort_by(|a, b| a.0.cmp(&b.0));
    found.into_iter().map(|(_, item)| item).collect()
}

pub fn for_site<'a>(items: &'a [Item], url: &str) -> Vec<&'a Item> {
    let host = sioul_core::sites::host_of(url);
    let domain = sioul_core::sites::domain_of(&host);
    let mut found: Vec<(&Item, bool)> = items
        .iter()
        .filter_map(|item| {
            let hosts = hosts_of(item);
            let same_host = hosts.iter().any(|h| *h == host);
            (same_host || hosts.iter().any(|h| sioul_core::sites::domain_of(h) == domain)).then_some((item, same_host))
        })
        .collect();
    found.sort_by_key(|(_, same)| !same);
    found.into_iter().map(|(item, _)| item).collect()
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
    fn logins_by_site() {
        // Items wipe their secrets when dropped: built field by field.
        let item = |name: &str, uri: &str| {
            let mut item = Item::default();
            item.name = name.into();
            item.uris = vec![uri.into()];
            item
        };
        let items = vec![item("Webmail", "https://account.example.net"), item("Mail", "mail.example.net"), item("Other", "https://example.org")];
        let found: Vec<&str> = for_site(&items, "https://mail.example.net/u/0/inbox").iter().map(|i| i.name.as_str()).collect();
        assert_eq!(found, vec!["Mail", "Webmail"]);
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

    #[test]
    fn logins_searched() {
        let item = |name: &str, user: &str, uri: &str| {
            let mut item = Item::default();
            item.name = name.into();
            item.username = user.into();
            item.uris = vec![uri.into()];
            item
        };
        let items = vec![item("Société Exemple", "moi", "https://particuliers.banque.example"), item("Webmail", "work@example.org", "https://account.example.net"), item("Webmail perso", "me@example.org", "example.net")];
        let names = |query: &str| search(&items, query).iter().map(|i| i.name.clone()).collect::<Vec<_>>();
        assert_eq!(names("societe"), vec!["Société Exemple"], "accents aside");
        assert_eq!(names("WEBMAIL"), vec!["Webmail", "Webmail perso"]);
        assert_eq!(names("webmail me@"), vec!["Webmail perso"], "every word");
        assert_eq!(names("banque.example"), vec!["Société Exemple"], "by site");
        assert!(names("  ").is_empty());
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
