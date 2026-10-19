// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! OpenPGP (RFC 9580) for mail, with Sequoia and its pure-Rust cryptography.
//!
//! - **Your keys** live in `~/.local/share/sioul/pgp/own/`. A key made here
//!   keeps its revocation certificate beside it, `<fingerprint>.rev`, yours
//!   alone as the key is ([`revocation_path`]); revoking is not built. A key's private
//!   part is in one of two places ([`Place`]): here, encrypted with a
//!   passphrase the system keyring keeps (`sioul-sync` asks it), so nothing is
//!   asked at each message; or on a security key, which signs and opens itself
//!   after its PIN ([`crate::securitykey`]), Sioul keeping only its public
//!   certificate. Keys come from an import (a file, GnuPG's export), are made
//!   here, or are a security key's.
//! - **Others' keys** live in `…/pgp/others/`: imported, found in their
//!   messages (Autocrypt), or looked up on request.
//! - **Signing and opening** take Sequoia's own `Signer` and `Decryptor`
//!   traits: a key here or a security key, the same way. A message a security
//!   key opened keeps its session key in memory ([`SessionKeys`]), so that it
//!   opens again without the key until Sioul closes.
//! - **What this module does not do**: talk to the network, or to the
//!   keyring. Passphrases and PINs come in as arguments.

use crate::config::data_dir;
use crate::securitykey::{self, CardAccess, CardDecryptor, KnownCard};
use openpgp::cert::prelude::*;
use openpgp::crypto::{self, Password, SessionKey, mpi};
use openpgp::packet::{PKESK, SKESK, key};
use openpgp::parse::Parse;
use openpgp::parse::stream::{DecryptionHelper, DecryptorBuilder, DetachedVerifierBuilder, MessageLayer, MessageStructure, VerificationHelper};
use openpgp::policy::StandardPolicy;
use openpgp::serialize::stream::{Armorer, Encryptor, LiteralWriter, Message, Signer};
use openpgp::serialize::{Serialize, SerializeInto};
use openpgp::types::{HashAlgorithm, SymmetricAlgorithm};
use openpgp::{Cert, KeyHandle};
/// A key's fingerprint, as the crates that do not use Sequoia themselves name it.
pub use openpgp::Fingerprint;
use sequoia_openpgp as openpgp;
use serde::Serialize as Serde;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

fn policy() -> StandardPolicy<'static> {
    StandardPolicy::new()
}

fn pgp_dir() -> PathBuf {
    data_dir().join("pgp")
}

fn own_dir() -> PathBuf {
    pgp_dir().join("own")
}

/// The keys Sioul knows: yours (their secret parts here, or on a security
/// key), others', and your security keys.
#[derive(Default, Clone)]
pub struct Keys {
    pub own: Vec<Cert>,
    pub others: Vec<Cert>,
    /// Your security keys read on this device, as `pgp/cards.toml` lists
    /// them: they alone sign here (review of 19 October 2026, F18).
    pub cards: Vec<KnownCard>,
    /// Your security keys your other devices made known and this one did not
    /// read (`pgp/cards/`), those stopped here aside (`pgp/stopped.toml`):
    /// they serve encryption to you, Autocrypt and the reader's words, never
    /// signing here.
    pub shared: Vec<KnownCard>,
    /// Your addresses: your accounts' (`config.toml`). With the addresses
    /// your security keys serve, they are yours whatever certificate claims
    /// them; never taken from a certificate's user IDs (review of 19 October
    /// 2026, F1).
    pub addresses: Vec<String>,
}

/// Where the private part of one of your keys is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Place {
    /// Here: its secret part in `pgp/own`, encrypted with a passphrase the system keyring keeps.
    Here,
    /// On a security key, by its identifier ("0006:12345678").
    Card {
        /// The security key, as [`securitykey::CardInfo::ident`].
        ident: String,
    },
}

/// Your key for an address: its certificate, and where its private part is.
#[derive(Debug, Clone)]
pub struct Own<'a> {
    pub cert: &'a Cert,
    pub place: Place,
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

/// Whether a key holds `address` among its user IDs bound to it, as
/// Sequoia's policy sees them: a user ID with no binding signature of the
/// key's own, revoked, or bound under an algorithm the policy refuses, names
/// nothing (review of 19 October 2026, F1: `userids()` alone counts every
/// user ID, "even those without a binding signature").
fn bound(cert: &Cert, address: &str) -> bool {
    let policy = policy();
    let Ok(valid) = cert.with_policy(&policy, None) else { return false };
    valid.userids().revoked(false).any(|u| email_is(u.userid(), address))
}

/// Whether a certificate may serve at all: valid under Sequoia's policy and
/// not revoked as a whole. Sequoia's `revoked(false)` on a key asks that
/// key's own revocation, never the certificate's: a certificate revoked as
/// compromised still has live, unrevoked subkeys (review of 19 October 2026,
/// N3). A revoked certificate never takes a message or a seal and never
/// signs; it still opens what was sent to it before.
fn usable(cert: &Cert) -> bool {
    let policy = policy();
    cert.with_policy(&policy, None).is_ok_and(|valid| !matches!(valid.revocation_status(), openpgp::types::RevocationStatus::Revoked(_)))
}

/// Whether a key serves `address`: bound to it (`bound`) and not revoked as
/// a whole (`usable`). Every choice of a key for an address goes through it
/// (`for_address`, `self_cert_for`, `own_for`).
fn names(cert: &Cert, address: &str) -> bool {
    usable(cert) && bound(cert, address)
}

/// For the window's words: whether `cert` holds `address` among the user
/// IDs bound to it, revoked as a whole or not.
pub fn names_address(cert: &Cert, address: &str) -> bool {
    bound(cert, address)
}

/// Whether `cert` is valid but revoked as a whole, for the window's words.
pub fn revoked_whole(cert: &Cert) -> bool {
    !usable(cert) && cert.with_policy(&policy(), None).is_ok()
}

fn email_is(userid: &openpgp::packet::UserID, address: &str) -> bool {
    userid.email().ok().flatten().is_some_and(|e| e.eq_ignore_ascii_case(address.trim()))
}

/// `cert` with only its user IDs for `address`, when one of them is bound
/// to it (`names`); none otherwise. What a certificate brings from
/// elsewhere (an Autocrypt header, a lookup) is kept for the address it
/// came for alone: its other user IDs, bound or not, never make it
/// another address's key (review of 19 October 2026, F2).
fn for_its_address(cert: Cert, address: &str) -> Option<Cert> {
    let kept = cert.retain_userids(|u| email_is(u.userid(), address));
    // Bound, revoked as a whole or not: a revocation that comes is kept, and stops the key kept.
    bound(&kept, address).then_some(kept)
}

/// The addresses bound to `cert` (`bound`), as written there, each once.
fn bound_addresses(cert: &Cert) -> Vec<String> {
    let policy = policy();
    let Ok(valid) = cert.with_policy(&policy, None) else { return Vec::new() };
    let mut out: Vec<String> = Vec::new();
    for address in valid.userids().revoked(false).filter_map(|u| u.userid().email().ok().flatten().map(|e| e.trim().to_string())) {
        if !out.iter().any(|kept| kept.eq_ignore_ascii_case(&address)) {
            out.push(address);
        }
    }
    out
}

/// What travels beside each of others' keys (`others/<fingerprint>.toml`):
/// the addresses it was learned for (an Autocrypt header's, a lookup's) or,
/// imported from a file you chose, every address bound to it. The only ones
/// it serves (review of 19 October 2026, N2).
#[derive(serde::Serialize, serde::Deserialize)]
struct Served {
    format: u32,
    #[serde(default)]
    addresses: Vec<String>,
    /// Of those, the ones judged from a key kept without this file (the
    /// migration, `judged`) rather than learned: a later rule may look at
    /// them again. An address learned since leaves this list.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    judged: Vec<String>,
}

/// The format of `others/`: 2 since its keys say what they serve; 1 (none
/// said), what Sioul kept before, or what an older Sioul writes through the
/// sharing.
const OTHERS_FORMAT: u32 = 2;

fn served_path(dir: &std::path::Path, fingerprint: &str) -> PathBuf {
    dir.join(format!("{fingerprint}.toml"))
}

/// What the file beside someone's key `cert`, kept in `dir`, says, when it
/// reads.
fn record_of(dir: &std::path::Path, cert: &Cert) -> Option<Served> {
    std::fs::read_to_string(served_path(dir, &cert.fingerprint().to_hex())).ok().and_then(|text| toml::from_str::<Served>(&text).ok())
}

/// What a key kept without that file serves (before 19 October 2026, or
/// written through the sharing by an older Sioul), judged as it is: the
/// address of every user ID it carries, bound or not, revoked or not, since
/// a Sioul before 1f9acc0 took a key on any of them (a header's key had to
/// name its sender in some user ID, bound by nothing if it liked). Its one
/// address, only when every user ID names that same address and it is
/// bound; else none (review of 19 October 2026, N2 and R1: Mallory's key
/// binding Jane's address, his own carried by nothing, came from his header
/// and named one bound address, hers). A key it serves none waits, kept,
/// until it comes again for an address.
fn judged(cert: &Cert) -> Vec<String> {
    let mut named: Vec<String> = Vec::new();
    for address in cert.userids().filter_map(|u| u.userid().email().ok().flatten().map(|e| e.trim().to_string())) {
        if !named.iter().any(|kept| kept.eq_ignore_ascii_case(&address)) {
            named.push(address);
        }
    }
    match named.as_slice() {
        [one] if bound(cert, one) => named,
        _ => Vec::new(),
    }
}

/// The addresses someone's key `cert`, kept in `dir`, serves: those its file
/// beside says, as they are bound to it; without that file, as it is
/// judged (`judged`), at each reading.
fn served_by(dir: &std::path::Path, cert: &Cert) -> Vec<String> {
    record_of(dir, cert).map_or_else(|| judged(cert), |served| served.addresses)
}

/// `cert` cut to the user IDs of `addresses`.
fn only_for(cert: Cert, addresses: &[String]) -> Cert {
    cert.retain_userids(|u| u.userid().email().ok().flatten().is_some_and(|e| addresses.iter().any(|a| a.trim().eq_ignore_ascii_case(e.trim()))))
}

/// Others' keys in `dir`, each cut to the addresses it serves (`served_by`);
/// one that serves none is left out of every use, its signatures said as a
/// key's Sioul does not know (kept on the disk, it comes back once learned
/// again for an address).
fn read_others(dir: &std::path::Path) -> Vec<Cert> {
    read_certs(dir)
        .into_iter()
        .filter_map(|cert| {
            let served = served_by(dir, &cert);
            let cut = only_for(cert, &served);
            cut.userids().next().is_some().then_some(cut)
        })
        .collect()
}

/// Writes what someone's key serves beside it.
fn write_served(dir: &std::path::Path, fingerprint: &str, served: &Served) -> Result<(), String> {
    let text = format!(
        "# Sioul: the addresses this key was learned for, the only ones it serves (docs/client.md, \"What others' keys serve\").\n{}",
        toml::to_string(served).map_err(|e| e.to_string())?
    );
    write_key(&served_path(dir, fingerprint), text.as_bytes())
}

/// Once on each device (`state/pgp-others.toml`): each key of `others/`
/// kept without what it serves (format 1) gets it, as it is judged now
/// (`judged`): its one address when every user ID it carries names it and
/// it is bound, else none; written as judged, not learned. Kept so, an older
/// Sioul's later write to it (every user ID of a header, merged) adds no
/// address it serves. The key's own file is never touched: one that serves
/// none stays, to serve again once it comes for an address (its owner's
/// next message, a lookup). How many keys it wrote it for.
pub fn keep_others_for_their_addresses() -> Result<usize, String> {
    keep_others_for_their_addresses_at(&pgp_dir(), &crate::config::state_dir().join("pgp-others.toml"))
}

/// `keep_others_for_their_addresses` in the folder `root` (`pgp/`), its mark at `mark`.
fn keep_others_for_their_addresses_at(root: &std::path::Path, mark: &std::path::Path) -> Result<usize, String> {
    #[derive(Default, serde::Serialize, serde::Deserialize)]
    struct Mark {
        #[serde(default)]
        format: u32,
    }
    let done = std::fs::read_to_string(mark).ok().and_then(|text| toml::from_str::<Mark>(&text).ok()).unwrap_or_default();
    if done.format >= OTHERS_FORMAT {
        return Ok(0);
    }
    let dir = root.join("others");
    let mut written = 0;
    for cert in read_certs(&dir) {
        let fingerprint = cert.fingerprint().to_hex();
        if served_path(&dir, &fingerprint).exists() {
            continue;
        }
        let addresses = judged(&cert);
        write_served(&dir, &fingerprint, &Served { format: OTHERS_FORMAT, judged: addresses.clone(), addresses })?;
        written += 1;
    }
    if let Some(parent) = mark.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let text = format!(
        "# Sioul: others' keys say beside them what they serve (pgp/others/<fingerprint>.toml) since this format; never shared.\n{}",
        toml::to_string(&Mark { format: OTHERS_FORMAT }).map_err(|e| e.to_string())?
    );
    crate::filelock::replace(mark, text.as_bytes()).map_err(|e| format!("{}: {e}", mark.display()))?;
    Ok(written)
}

impl Keys {
    pub fn load() -> Keys {
        let addresses = crate::config::Config::load(&crate::config::default_path()).map(|c| c.accounts.iter().filter_map(|a| a.address.clone()).collect()).unwrap_or_default();
        Keys { addresses, ..Keys::load_at(&pgp_dir()) }
    }

    /// The keys of the folder `root` (`pgp/`): yours, others', your security
    /// keys (`cards.toml`), and those your other devices made known
    /// (`cards/`, [`securitykey::shared_cards`]): a security key read on your
    /// computer, its certificate with it, so that a phone encrypts to you,
    /// hands your key by Autocrypt and says what waits for the key. A key
    /// this device read itself says what it is; its certificate is merged
    /// with the shared one (a renewal made on another computer).
    pub fn load_at(root: &std::path::Path) -> Keys {
        let mut keys = Keys { own: read_certs(&root.join("own")), others: read_others(&root.join("others")), cards: securitykey::known_cards_in(&root.join("cards.toml")), ..Keys::default() };
        let stopped = securitykey::stopped_in(root);
        for (card, cert) in securitykey::shared_cards_in(&root.join(securitykey::SHARED_CARDS)) {
            let read_here = keys.cards.iter().any(|known| known.ident.eq_ignore_ascii_case(&card.ident));
            // Stopped on this device: never taken back from what the others share (F4).
            if !read_here && stopped.iter().any(|ident| ident.eq_ignore_ascii_case(&card.ident)) {
                continue;
            }
            if !read_here {
                keys.shared.push(card);
            }
            match keys.own.iter_mut().find(|own| own.fingerprint() == cert.fingerprint()) {
                Some(own) => {
                    if let Ok(merged) = own.clone().merge_public(cert) {
                        *own = merged;
                    }
                }
                None => keys.own.push(cert),
            }
        }
        keys
    }

    /// Your key for an address, able to sign now. A security key read on
    /// this device that serves the address comes first, and alone: a key kept
    /// here for the same address only opens older mail. When that security
    /// key's certificate expired, nothing signs for the address until it is
    /// renewed. A security key another computer read signs nowhere but there
    /// (review of 19 October 2026, F18): to sign here, it is read here.
    pub fn own_for(&self, address: &str) -> Option<Own<'_>> {
        if let Some((card, cert)) = self.card_for(address) {
            return (can_sign_with(cert, &card.sign) && self.usable(cert)).then(|| Own { cert, place: Place::Card { ident: card.ident.clone() } });
        }
        let policy = policy();
        self.own
            .iter()
            .find(|c| self.serves(c, address) && c.keys().secret().with_policy(&policy, None).alive().revoked(false).for_signing().next().is_some())
            .map(|cert| Own { cert, place: Place::Here })
    }

    /// The security key read here that signs for `address`, and its
    /// certificate, even when it expired: to say so.
    pub fn card_for(&self, address: &str) -> Option<(&KnownCard, &Cert)> {
        self.cards.iter().filter(|card| card.serves(address)).find_map(|card| Some((card, self.cert(&card.cert)?)))
    }

    /// The security key of yours that serves `address`, read here or on
    /// another device of yours, and its certificate: encryption to you,
    /// Autocrypt, and what the reader says wait for it.
    pub fn serving_card(&self, address: &str) -> Option<(&KnownCard, &Cert)> {
        self.all_cards().filter(|card| card.serves(address)).find_map(|card| Some((card, self.cert(&card.cert)?)))
    }

    /// Every security key of yours known here: read here first, then those
    /// your other devices made known.
    pub fn all_cards(&self) -> impl Iterator<Item = &KnownCard> {
        self.cards.iter().chain(&self.shared)
    }

    /// Your certificate of this fingerprint.
    pub fn cert(&self, fingerprint: &str) -> Option<&Cert> {
        self.own.iter().find(|c| c.fingerprint().to_hex().eq_ignore_ascii_case(fingerprint))
    }

    /// A key to encrypt to `address`: someone's, or yours. For one of your
    /// own addresses, your own key alone: never a certificate someone sent
    /// claiming it (an Autocrypt header, a lookup), which would read what you
    /// send yourself; none when yours cannot take encryption now.
    ///
    /// Someone's keys are kept for the address each came for alone
    /// (`for_its_address`). Two of them for one address (a key made anew,
    /// the old one kept): the one whose newest live encryption key is the
    /// most recent, then the one whose primary key is, then by fingerprint;
    /// never the order the folder lists them in (review of 19 October 2026,
    /// F2). Others' before a key of yours that names the address.
    pub fn for_address(&self, address: &str) -> Option<&Cert> {
        if self.is_own_address(address) {
            return self.self_cert_for(address);
        }
        let policy = policy();
        let newest = |c: &Cert| {
            let encryption = c.keys().with_policy(&policy, None).alive().revoked(false).for_transport_encryption().map(|k| k.key().creation_time()).max();
            (encryption, c.primary_key().key().creation_time(), c.fingerprint().to_hex())
        };
        let best = |certs: &'_ [Cert]| -> Option<usize> {
            certs.iter().enumerate().filter(|(_, c)| self.serves(c, address)).filter_map(|(i, c)| { let n = newest(c); n.0.map(|_| (n, i)) }).max_by(|a, b| a.0.cmp(&b.0)).map(|(_, i)| i)
        };
        best(&self.others).map(|i| &self.others[i]).or_else(|| best(&self.own).map(|i| &self.own[i]))
    }

    /// Your own certificate to encrypt to, for `address`: your security key's
    /// when one serves the address (its certificate as your computer read
    /// it), else a key of yours naming it; none when it cannot take
    /// encryption now (expired, revoked).
    pub fn self_cert_for(&self, address: &str) -> Option<&Cert> {
        let policy = policy();
        let encrypts = |c: &&Cert| self.usable(c) && c.keys().with_policy(&policy, None).alive().revoked(false).for_transport_encryption().next().is_some();
        if let Some((_, cert)) = self.serving_card(address) {
            return Some(cert).filter(encrypts);
        }
        self.own.iter().filter(|c| self.serves(c, address)).find(encrypts)
    }

    /// Your own certificates that can take what is sealed for you alone
    /// (your security keys', as your computer read them, and your keys
    /// here): an encryption key not revoked, expired or not, since you open
    /// such a seal with it (F19); never a certificate revoked as a whole
    /// (N3). Encrypted to each, any of them opens it.
    pub fn own_certs(&self) -> Vec<&Cert> {
        let policy = policy();
        let mut out: Vec<&Cert> = Vec::new();
        for cert in &self.own {
            if self.usable(cert) && cert.keys().with_policy(&policy, None).supported().revoked(false).for_transport_encryption().next().is_some() && !out.iter().any(|c| c.fingerprint() == cert.fingerprint()) {
                out.push(cert);
            }
        }
        out
    }

    /// Whether `address` is yours: one of your accounts' addresses, or one a
    /// security key of yours serves. Never from a certificate's user IDs,
    /// which anyone can write (review of 19 October 2026, F1).
    pub fn is_own_address(&self, address: &str) -> bool {
        let address = address.trim();
        self.addresses.iter().any(|a| a.trim().eq_ignore_ascii_case(address)) || self.all_cards().any(|card| card.serves(address))
    }

    /// Whether `cert` may serve: valid, not revoked as a whole (`usable`),
    /// and not revoked by its designated revoker (a Revocation Key, GnuPG's
    /// `addrevoker`) whose own certificate is kept here, yours or someone's,
    /// the revocation checked against it (review of 19 October 2026, R2).
    /// Sequoia says such a revocation "could be", leaving that check to
    /// whoever holds the revoker's certificate; one that does not check is
    /// passed over, since anyone can attach one.
    pub fn usable(&self, cert: &Cert) -> bool {
        let policy = policy();
        let Ok(valid) = cert.with_policy(&policy, None) else { return false };
        match valid.revocation_status() {
            openpgp::types::RevocationStatus::Revoked(_) => false,
            openpgp::types::RevocationStatus::NotAsFarAsWeKnow => true,
            openpgp::types::RevocationStatus::CouldBe(revocations) => {
                let revokers: Vec<&Fingerprint> = cert.revocation_keys(&policy).map(|key| key.revoker().1).collect();
                let kept = self.own.iter().chain(&self.others).filter(|kept| revokers.contains(&&kept.fingerprint()));
                !kept.into_iter().any(|revoker| revocations.iter().any(|revocation| revocation.verify_primary_key_revocation(revoker.primary_key().key(), cert.primary_key().key()).is_ok()))
            }
        }
    }

    /// Whether `cert` serves `address`: bound to it and usable here (`names`,
    /// [`Keys::usable`]).
    fn serves(&self, cert: &Cert, address: &str) -> bool {
        names(cert, address) && self.usable(cert)
    }

    /// Whether `fingerprint` is one of your certificates: kept with yours,
    /// or your security keys' (`cards.toml`).
    pub fn is_yours(&self, fingerprint: &Fingerprint) -> bool {
        let hex = fingerprint.to_hex();
        self.own.iter().any(|c| c.fingerprint() == *fingerprint) || self.all_cards().any(|card| card.cert.eq_ignore_ascii_case(&hex))
    }

    fn all(&self) -> impl Iterator<Item = &Cert> {
        self.own.iter().chain(&self.others)
    }

    fn by_handle(&self, handle: &KeyHandle) -> Option<&Cert> {
        self.all().find(|c| c.keys().any(|k| k.key().key_handle().aliases(handle)))
    }
}

/// Whether the key `fingerprint` of `cert` can sign now: valid, alive, not
/// revoked, allowed to sign.
fn can_sign_with(cert: &Cert, fingerprint: &str) -> bool {
    let policy = policy();
    usable(cert) && cert.keys().with_policy(&policy, None).alive().revoked(false).for_signing().any(|k| k.key().fingerprint().to_hex().eq_ignore_ascii_case(fingerprint))
}

/// The key `fingerprint` of `cert`, its public part, primary or subkey.
pub fn key_of(cert: &Cert, fingerprint: &str) -> Option<securitykey::PublicKey> {
    cert.keys().find(|k| k.key().fingerprint().to_hex().eq_ignore_ascii_case(fingerprint)).map(|k| k.key().clone())
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
    /// The security key holding its private keys ("0006:12345678"); empty when they are here.
    pub card: String,
    /// Yours, made here: its revocation certificate is kept beside it (`revocation_path`).
    pub revocation: bool,
}

fn summary(cert: &Cert, own: bool) -> KeySummary {
    let policy = policy();
    let valid = cert.with_policy(&policy, None).ok();
    let date = |t: std::time::SystemTime| jiff::Timestamp::try_from(t).map(|t| t.strftime("%Y-%m-%d").to_string()).unwrap_or_default();
    KeySummary {
        fingerprint: cert.fingerprint().to_hex(),
        // The user IDs bound to it, as matching reads them (`names`).
        names: valid.as_ref().map(|v| v.userids().revoked(false).map(|u| String::from_utf8_lossy(u.userid().value()).to_string()).collect()).unwrap_or_default(),
        own,
        created: date(cert.primary_key().key().creation_time()),
        expires: valid.as_ref().and_then(|v| v.primary_key().key_expiration_time()).map(date).unwrap_or_default(),
        can_encrypt: usable(cert) && cert.keys().with_policy(&policy, None).alive().revoked(false).for_transport_encryption().next().is_some(),
        card: String::new(),
        revocation: false,
    }
}

/// Every key, yours first; a security key's certificate names its key.
pub fn summaries() -> Vec<KeySummary> {
    let keys = Keys::load();
    // Revoked by a designated revoker kept here (R2): said as unable to encrypt too.
    let summarised = |c: &Cert, own: bool| KeySummary { can_encrypt: summary(c, own).can_encrypt && keys.usable(c), ..summary(c, own) };
    let mut all: Vec<KeySummary> = keys.own.iter().map(|c| summarised(c, true)).chain(keys.others.iter().map(|c| summarised(c, false))).collect();
    for key in all.iter_mut().filter(|k| k.own) {
        if let Some(card) = keys.all_cards().find(|c| c.cert.eq_ignore_ascii_case(&key.fingerprint)) {
            key.card = card.ident.clone();
        }
        key.revocation = revocation_path(&key.fingerprint).is_file();
    }
    all
}

/// Keeps a key that came from elsewhere for `address` (a lookup: its
/// domain's Web Key Directory, keys.openpgp.org): its user IDs for that
/// address alone. One of yours brought again so is merged without anything
/// it names: only what your own primary key binds (a renewal's signatures,
/// a subkey), never another source's user IDs into your certificate
/// (review of 19 October 2026, F1).
fn store_from_elsewhere_at(root: &std::path::Path, cert: Cert, address: &str) -> Option<KeySummary> {
    let cert = for_its_address(cert.strip_secret_key_material(), address)?;
    let fingerprint = cert.fingerprint().to_hex();
    let yours = root.join("own").join(format!("{fingerprint}.asc"));
    if let Some(known) = std::fs::read(&yours).ok().and_then(|bytes| Cert::from_bytes(&bytes).ok()) {
        let names: Vec<Vec<u8>> = known.userids().map(|u| u.userid().value().to_vec()).collect();
        let merged = known.merge_public(cert).ok()?.retain_userids(|u| names.iter().any(|n| n.as_slice() == u.userid().value()));
        return store_in(root, only_bound_subkeys(merged), true).ok();
    }
    if yours_at(root, &fingerprint) {
        return None;
    }
    store_in(root, cert, false).ok()
}

/// `cert` without the subkeys its own primary key does not bind under the
/// policy (bound by nothing, or by another key): what a lookup brings of
/// yours adds only what your key signed (review of 19 October 2026, Info:
/// such subkeys were inert, but kept, and a security key's travelled in its
/// shared file).
fn only_bound_subkeys(cert: Cert) -> Cert {
    let policy = policy();
    let bound: Vec<Fingerprint> = cert.keys().subkeys().with_policy(&policy, None).map(|k| k.key().fingerprint()).collect();
    cert.retain_subkeys(|k| bound.contains(&k.key().fingerprint()))
}

/// Whether the certificate `fingerprint` (hexadecimal) is one of yours in
/// the folder `root` (`pgp/`): kept with your keys, a security key's read
/// here, or one your other devices made known (`cards/`), as a phone knows
/// your security key's certificate: never kept among others' keys, from
/// wherever it comes again (your own mail's Autocrypt header, a lookup).
fn yours_at(root: &std::path::Path, fingerprint: &str) -> bool {
    root.join("own").join(format!("{fingerprint}.asc")).exists()
        || securitykey::known_cards_in(&root.join("cards.toml")).iter().any(|card| card.cert.eq_ignore_ascii_case(fingerprint))
        || securitykey::shared_cards_in(&root.join(securitykey::SHARED_CARDS)).iter().any(|(card, _)| card.cert.eq_ignore_ascii_case(fingerprint))
}

/// Keeps a key you chose (imported, made here) in the folder `root`
/// (`pgp/`), merged with what was known of it. Secret parts go with yours,
/// and so does a certificate of yours.
fn store_at(root: &std::path::Path, cert: Cert) -> Result<KeySummary, String> {
    let known_as_own = root.join("own").join(format!("{}.asc", cert.fingerprint().to_hex())).exists();
    let own = cert.is_tsk() || known_as_own;
    store_in(root, cert, own)
}

fn store_in(root: &std::path::Path, cert: Cert, own: bool) -> Result<KeySummary, String> {
    if !own {
        let learned = bound_addresses(&cert);
        return store_others(root, cert, &learned);
    }
    let dir = root.join(if own { "own" } else { "others" });
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{}.asc", cert.fingerprint().to_hex()));
    let merged = match std::fs::read(&path).ok().and_then(|bytes| Cert::from_bytes(&bytes).ok()) {
        Some(known) => known.merge_public_and_secret(cert).map_err(|e| e.to_string())?,
        None => cert,
    };
    let armored = if merged.is_tsk() { merged.as_tsk().armored().to_vec() } else { merged.armored().to_vec() }.map_err(|e| e.to_string())?;
    write_key(&path, &armored)?;
    // A security key's certificate kept again (renewed, a subkey added): your other devices follow.
    let fingerprint = merged.fingerprint().to_hex();
    if own && securitykey::known_cards_in(&root.join("cards.toml")).iter().any(|card| card.cert.eq_ignore_ascii_case(&fingerprint)) {
        securitykey::share_cards_in(&root.join("cards.toml"), &dir, &root.join(securitykey::SHARED_CARDS), false)?;
    }
    Ok(summary(&merged, own))
}

/// Keeps someone's key for `learned`: the address it came for (a key from
/// elsewhere comes cut to it, `for_its_address`) or, imported from a file
/// you chose, every address bound to it. Merged with the one kept, it serves
/// what it served and these (its file beside, written first), cut to their
/// user IDs: an address a key kept before, or an older Sioul's write, named
/// besides is never kept nor served (review of 19 October 2026, N2), and the
/// user IDs of an address it was learned for before stay.
fn store_others(root: &std::path::Path, cert: Cert, learned: &[String]) -> Result<KeySummary, String> {
    let dir = root.join("others");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let fingerprint = cert.fingerprint().to_hex();
    let path = dir.join(format!("{fingerprint}.asc"));
    let kept = std::fs::read(&path).ok().and_then(|bytes| Cert::from_bytes(&bytes).ok());
    let mut served = match &kept {
        Some(kept) => record_of(&dir, kept).unwrap_or_else(|| {
            let addresses = judged(kept);
            Served { format: OTHERS_FORMAT, judged: addresses.clone(), addresses }
        }),
        None => Served { format: OTHERS_FORMAT, addresses: Vec::new(), judged: Vec::new() },
    };
    served.format = OTHERS_FORMAT;
    for address in learned {
        let address = address.trim();
        served.judged.retain(|judged| !judged.eq_ignore_ascii_case(address));
        if !served.addresses.iter().any(|a| a.trim().eq_ignore_ascii_case(address)) {
            served.addresses.push(address.to_string());
        }
    }
    let merged = match kept {
        Some(kept) => kept.merge_public(cert.strip_secret_key_material()).map_err(|e| e.to_string())?,
        None => cert.strip_secret_key_material(),
    };
    let merged = only_for(merged, &served.addresses);
    write_served(&dir, &fingerprint, &served)?;
    write_key(&path, &merged.armored().to_vec().map_err(|e| e.to_string())?)?;
    Ok(summary(&merged, false))
}

/// Keeps the certificate of your security key with your keys: public parts only.
pub fn keep_card_certificate(cert: &Cert) -> Result<KeySummary, String> {
    store_in(&pgp_dir(), cert.clone().strip_secret_key_material(), true)
}

/// Writes a key file beside, then moves it in place. On Unix it is readable by
/// you alone, whatever the umask: your secret keys, and whose keys you keep.
fn write_key(path: &std::path::Path, armored: &[u8]) -> Result<(), String> {
    let fail = |e: std::io::Error| format!("{}: {e}", path.display());
    // Named after the whole file: a key and what it serves (`<fingerprint>.asc`,
    // `.toml`) never share one; hidden, so that the sharing passes it over.
    let temporary = path.with_file_name(format!(".{}.tmp", path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()));
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

/// The certificates of `bytes` (armored or binary), read, not kept.
pub fn certificates_in(bytes: &[u8]) -> Vec<Cert> {
    certificates_of(bytes).unwrap_or_default()
}

/// The certificates among `bytes` (armored or binary, one or several), as a
/// certificate from outside is read everywhere here (a file, GnuPG, a
/// server, a message): read as they came, and again with their signatures'
/// numbers written shortest (`minimal_mpis`), merged by fingerprint, in the
/// order they came; so the repair only ever adds, never takes a certificate
/// or a part away. Sequoia's words when neither reading reads.
pub fn certificates_of(bytes: &[u8]) -> Result<Vec<Cert>, String> {
    let read = |bytes: &[u8]| CertParser::from_bytes(bytes).map(|parser| parser.filter_map(Result::ok).collect::<Vec<Cert>>()).map_err(|e| e.to_string());
    let as_they_came = read(bytes);
    let shortest = match minimal_mpis(bytes) {
        std::borrow::Cow::Borrowed(_) => return as_they_came,
        std::borrow::Cow::Owned(repaired) => read(&repaired),
    };
    let (mut certs, shortest) = match (as_they_came, shortest) {
        (Ok(certs), Ok(shortest)) => (certs, shortest),
        (Ok(certs), Err(_)) => return Ok(certs),
        (Err(_), shortest) => return shortest,
    };
    for cert in shortest {
        match certs.iter().position(|kept| kept.fingerprint() == cert.fingerprint()) {
            Some(index) => {
                // The same key read both ways: every component either reading found.
                if let Ok(merged) = certs[index].clone().merge_public_and_secret(cert) {
                    certs[index] = merged;
                }
            }
            None => certs.push(cert),
        }
    }
    Ok(certs)
}

/// `bytes` (armored or binary) with the numbers of each signature written in
/// their shortest form, as RFC 9580 (3.2) asks and Sequoia insists
/// (`certificates_of` reads with it). GnuPG 2.4.4, Ubuntu 24.04's, writes
/// about one key signature in 128 with a number one octet too long (a
/// leading zero octet counted in its length); Sequoia then read that
/// signature as none, and the subkey it binds, or the whole key, as missing.
/// A signature's numbers are not part of what it signs: written shorter they
/// are the same numbers, and Sequoia still checks the signature. Only ever
/// shorter: a length said too short is left as it came. Armor is read block
/// by block, every key block kept, in order. The bytes as they came when none
/// needed it, or when they do not read as packets of known length; armored
/// bytes come back binary once changed, which every reader here takes.
pub fn minimal_mpis(bytes: &[u8]) -> std::borrow::Cow<'_, [u8]> {
    use std::borrow::Cow;
    // A binary packet starts with its tag's high bit set: the packets as they come.
    if bytes.first().is_some_and(|b| b & 0x80 != 0) {
        return shortened(bytes).map_or(Cow::Borrowed(bytes), Cow::Owned);
    }
    // Armor: each key block on its own, dearmored, then shortened where it
    // needs it; every one kept, as Sequoia reads them all.
    let mut out = Vec::new();
    let mut changed = false;
    for block in armored_keys(bytes) {
        let mut reader = openpgp::armor::Reader::from_bytes(block, openpgp::armor::ReaderMode::Tolerant(None));
        let mut binary = Vec::new();
        if std::io::Read::read_to_end(&mut reader, &mut binary).is_err() || binary.is_empty() {
            return Cow::Borrowed(bytes);
        }
        match shortened(&binary) {
            Some(shorter) => {
                out.extend_from_slice(&shorter);
                changed = true;
            }
            None => out.extend_from_slice(&binary),
        }
    }
    if changed { Cow::Owned(out) } else { Cow::Borrowed(bytes) }
}

/// The armored key blocks of `text`, in order: each from its "-----BEGIN PGP
/// PUBLIC KEY BLOCK-----" (or PRIVATE) to the end of its "-----END PGP" line.
/// What lies around or between them is left out, a message's or a
/// signature's block among it.
fn armored_keys(text: &[u8]) -> Vec<&[u8]> {
    const BEGIN: &[u8] = b"-----BEGIN PGP ";
    const END: &[u8] = b"-----END PGP ";
    let find = |from: usize, needle: &[u8]| text.get(from..).and_then(|rest| rest.windows(needle.len()).position(|window| window == needle)).map(|i| from + i);
    let mut blocks = Vec::new();
    let mut at = 0;
    while let Some(begin) = find(at, BEGIN) {
        let Some(end) = find(begin + BEGIN.len(), END) else { break };
        let line_end = text[end..].iter().position(|b| *b == b'\n').map_or(text.len(), |i| end + i + 1);
        let kind = &text[begin + BEGIN.len()..];
        if kind.starts_with(b"PUBLIC KEY BLOCK-----") || kind.starts_with(b"PRIVATE KEY BLOCK-----") {
            blocks.push(&text[begin..line_end]);
        }
        at = line_end;
    }
    blocks
}

/// The packets of `binary` with each version 4 signature's numbers in their
/// shortest form; none when none needed it, or when `binary` does not read
/// as packets of known length (a partial length is data's, never a key's).
fn shortened(binary: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(binary.len());
    let mut changed = false;
    let mut at = 0;
    while at < binary.len() {
        let (tag, body) = packet_at(binary, at)?;
        let shorter = if tag == 2 && binary[body.clone()].first() == Some(&4) { signature_shortened(&binary[body.clone()]) } else { None };
        match shorter {
            // A signature, version 4 (RFC 9580, 5.2.3), whose numbers were written longer.
            Some(shorter) => {
                put_packet(&mut out, tag, &shorter);
                changed = true;
            }
            None => out.extend_from_slice(&binary[at..body.end]),
        }
        at = body.end;
    }
    changed.then_some(out)
}

/// The packet at `at` (RFC 9580, 4.2): its tag and where its body lies; none
/// for one cut short, or of partial length.
fn packet_at(bytes: &[u8], at: usize) -> Option<(u8, std::ops::Range<usize>)> {
    let ctb = *bytes.get(at)?;
    let number = |from: usize, octets: usize| bytes.get(from..from + octets).map(|b| b.iter().fold(0usize, |n, b| (n << 8) | usize::from(*b)));
    let (tag, start, length) = match ctb {
        // OpenPGP's format: the tag in six bits, then one, two or five octets of length.
        0xC0..=0xFF => match *bytes.get(at + 1)? {
            first @ 0..=191 => (ctb & 0x3F, at + 2, usize::from(first)),
            first @ 192..=223 => (ctb & 0x3F, at + 3, ((usize::from(first) - 192) << 8) + usize::from(*bytes.get(at + 2)?) + 192),
            255 => (ctb & 0x3F, at + 6, number(at + 2, 4)?),
            _ => return None,
        },
        // The legacy format: the tag in four bits, the length's size in two.
        0x80..=0xBF => match ctb & 3 {
            0 => ((ctb >> 2) & 0x0F, at + 2, number(at + 1, 1)?),
            1 => ((ctb >> 2) & 0x0F, at + 3, number(at + 1, 2)?),
            2 => ((ctb >> 2) & 0x0F, at + 5, number(at + 1, 4)?),
            // Up to the end.
            _ => ((ctb >> 2) & 0x0F, at + 1, bytes.len().checked_sub(at + 1)?),
        },
        _ => return None,
    };
    let end = start.checked_add(length)?;
    (end <= bytes.len()).then_some((tag, start..end))
}

/// A version 4 signature's body with its numbers in their shortest form (no
/// leading zero bits, RFC 9580, 3.2), and those of a signature embedded in
/// its unhashed area (a signing subkey's back-signature, where GnuPG writes
/// it); never anything in its hashed area, which the signature covers. None
/// when nothing needed it, when its algorithm's are not numbers (Ed25519's own
/// octets) or do not read whole, or when a length is said too short: only
/// ever shorter, as GnuPG's fault is.
fn signature_shortened(body: &[u8]) -> Option<Vec<u8>> {
    shortened_at(body, true)
}

/// `signature_shortened`, the signatures embedded in the unhashed area
/// looked into only when `descend`: one level, never more. An embedded
/// signature's own embedded ones stay as they came: Sequoia refuses a
/// signature embedded in an embedded one, and each level looked into would
/// take the stack of the thread reading a file from outside (a phone's
/// import runs on Qt's own, about 1 MiB), a crafted one nesting thousands.
fn shortened_at(body: &[u8], descend: bool) -> Option<Vec<u8>> {
    let numbers = match *body.get(2)? {
        // RSA.
        1 | 3 => 1,
        // ElGamal, DSA, ECDSA, EdDSA (the legacy one GnuPG makes).
        16 | 17 | 19 | 20 | 22 => 2,
        _ => return None,
    };
    let length_at = |at: usize| body.get(at..at + 2).map(|two| usize::from(u16::from_be_bytes([two[0], two[1]])));
    // Version, type, the key's and the hash's algorithms, then the hashed
    // area after its length: as it came.
    let unhashed = 4usize.checked_add(2 + length_at(4)?)?;
    let mut out = body.get(..unhashed)?.to_vec();
    // The unhashed area, its embedded signatures shortened (one level).
    let area = body.get(unhashed + 2..unhashed + 2 + length_at(unhashed)?)?;
    let (area_out, mut changed) = match descend.then(|| embedded_shortened(area)).flatten() {
        Some(shorter) => (shorter, true),
        None => (area.to_vec(), false),
    };
    out.extend_from_slice(&u16::try_from(area_out.len()).ok()?.to_be_bytes());
    out.extend_from_slice(&area_out);
    // The hash's first two octets, then the numbers.
    let mut at = unhashed + 2 + area.len();
    out.extend_from_slice(body.get(at..at + 2)?);
    at += 2;
    for _ in 0..numbers {
        let bits = length_at(at)?;
        let value = body.get(at + 2..at + 2 + bits.div_ceil(8))?;
        at += 2 + value.len();
        let value = &value[value.iter().position(|b| *b != 0).unwrap_or(value.len())..];
        let shortest = value.first().map_or(0, |first| value.len() * 8 - first.leading_zeros() as usize);
        if shortest > bits {
            return None;
        }
        changed |= shortest < bits;
        out.extend_from_slice(&u16::try_from(shortest).ok()?.to_be_bytes());
        out.extend_from_slice(value);
    }
    (changed && at == body.len()).then_some(out)
}

/// A signature's unhashed area (RFC 9580, 5.2.3.7) with each embedded
/// signature (subpacket 32) shortened, its own unhashed area left as it came
/// (`shortened_at`), its length said again; none when none needed it, or
/// when the area does not read as subpackets.
fn embedded_shortened(area: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(area.len());
    let mut changed = false;
    let mut at = 0;
    while at < area.len() {
        let (start, length) = match *area.get(at)? {
            first @ 0..=191 => (at + 1, usize::from(first)),
            first @ 192..=254 => (at + 2, ((usize::from(first) - 192) << 8) + usize::from(*area.get(at + 1)?) + 192),
            255 => (at + 5, usize::try_from(u32::from_be_bytes(area.get(at + 1..at + 5)?.try_into().ok()?)).ok()?),
        };
        // Its type (the critical bit aside), then its body.
        let subpacket = area.get(start..start.checked_add(length)?)?;
        let (&kind, inner) = subpacket.split_first()?;
        match (kind & 0x7F == 32 && inner.first() == Some(&4)).then(|| shortened_at(inner, false)).flatten() {
            Some(shorter) => {
                put_subpacket_length(&mut out, 1 + shorter.len());
                out.push(kind);
                out.extend_from_slice(&shorter);
                changed = true;
            }
            None => out.extend_from_slice(&area[at..start + length]),
        }
        at = start + length;
    }
    changed.then_some(out)
}

/// A subpacket's length (RFC 9580, 5.2.3.7), in one, two or five octets.
fn put_subpacket_length(out: &mut Vec<u8>, length: usize) {
    match length {
        0..=191 => out.push(length as u8),
        192..=16_319 => out.extend_from_slice(&[((length - 192) >> 8) as u8 + 192, (length - 192) as u8]),
        _ => {
            out.push(255);
            out.extend_from_slice(&(length as u32).to_be_bytes());
        }
    }
}

/// A packet in OpenPGP's format: its tag, its length, its body.
fn put_packet(out: &mut Vec<u8>, tag: u8, body: &[u8]) {
    out.push(0xC0 | tag);
    match body.len() {
        length @ 0..=191 => out.push(length as u8),
        length @ 192..=8383 => out.extend_from_slice(&[((length - 192) >> 8) as u8 + 192, (length - 192) as u8]),
        length => {
            out.push(255);
            out.extend_from_slice(&(length as u32).to_be_bytes());
        }
    }
    out.extend_from_slice(body);
}

/// Imports the keys of a file: armored or binary, public or secret.
/// Someone's key imported so serves every address bound to it: you chose
/// the file.
pub fn import(bytes: &[u8]) -> Result<Vec<KeySummary>, String> {
    import_at(&pgp_dir(), bytes)
}

/// [`import`] into the folder `root` (`pgp/`).
fn import_at(root: &std::path::Path, bytes: &[u8]) -> Result<Vec<KeySummary>, String> {
    let certs = certificates_of(bytes)?;
    if certs.is_empty() {
        return Err("no OpenPGP key".into());
    }
    certs.into_iter().map(|cert| store_at(root, cert)).collect()
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
/// years; its secret parts encrypted with `passphrase`. Its revocation
/// certificate, which Sequoia makes with it, is kept beside it
/// (`revocation_path`): the only way to revoke the key once it is lost.
pub fn generate(name: &str, address: &str, passphrase: &str) -> Result<KeySummary, String> {
    generate_at(&pgp_dir(), name, address, passphrase)
}

/// [`generate`] into the folder `root` (`pgp/`).
fn generate_at(root: &std::path::Path, name: &str, address: &str, passphrase: &str) -> Result<KeySummary, String> {
    let userid = if name.trim().is_empty() { format!("<{}>", address.trim()) } else { format!("{} <{}>", name.trim(), address.trim()) };
    let (cert, revocation) = CertBuilder::general_purpose(Some(userid))
        .set_cipher_suite(CipherSuite::Cv25519)
        .set_validity_period(std::time::Duration::from_secs(3 * 365 * 24 * 3600))
        .set_password(Some(Password::from(passphrase)))
        .generate()
        .map_err(|e| e.to_string())?;
    // The certificate first: a key is never kept without it.
    let own = root.join("own");
    std::fs::create_dir_all(&own).map_err(|e| e.to_string())?;
    let kept = own.join(format!("{}.rev", cert.fingerprint().to_hex()));
    write_key(&kept, revocation_text(&cert, revocation)?.as_bytes())?;
    match store_at(root, cert) {
        Ok(summary) => Ok(KeySummary { revocation: true, ..summary }),
        Err(e) => {
            let _ = std::fs::remove_file(&kept);
            Err(e)
        }
    }
}

/// Where the revocation certificate of your key made here is kept: beside
/// the key, `pgp/own/<fingerprint>.rev`, readable by you alone (`write_key`),
/// as GnuPG keeps its own in `openpgp-revocs.d`.
pub fn revocation_path(fingerprint: &str) -> PathBuf {
    own_dir().join(format!("{fingerprint}.rev"))
}

/// A revocation certificate as a file to keep: what it is, in a few lines,
/// then the signature armored, a colon before its first line so that it
/// cannot be imported by mistake (as GnuPG writes its own).
fn revocation_text(cert: &Cert, revocation: openpgp::packet::Signature) -> Result<String, String> {
    let mut armored = Vec::new();
    {
        let mut writer = openpgp::armor::Writer::with_headers(&mut armored, openpgp::armor::Kind::PublicKey, [("Comment", "This is a revocation certificate")]).map_err(|e| e.to_string())?;
        openpgp::Packet::from(revocation).serialize(&mut writer).map_err(|e| e.to_string())?;
        writer.finalize().map_err(|e| e.to_string())?;
    }
    let names: Vec<String> = cert.userids().map(|u| String::from_utf8_lossy(u.userid().value()).to_string()).collect();
    Ok(format!(
        "This is the revocation certificate of the OpenPGP key\n{}\n{}\n\n\
         Published, it tells everyone that this key must no longer be used, for\n\
         good: it cannot be taken back. Keep a copy where only you can reach it,\n\
         apart from the key, for the day the key is lost or stolen.\n\n\
         A colon stands before the five dashes below, so that the certificate\n\
         cannot be imported by mistake: take it away with a text editor before\n\
         you import the certificate and publish it.\n\n:{}",
        cert.fingerprint().to_hex(),
        names.join("\n"),
        String::from_utf8_lossy(&armored)
    ))
}

/// Copies the revocation certificate of your key `fingerprint` into
/// `folder`, as `<fingerprint>.rev` (a number added when that name is
/// taken), yours alone; none kept for that key, an error.
pub fn save_revocation(fingerprint: &str, folder: &std::path::Path) -> Result<PathBuf, String> {
    save_revocation_at(&pgp_dir(), fingerprint, folder)
}

/// [`save_revocation`] from the folder `root` (`pgp/`).
fn save_revocation_at(root: &std::path::Path, fingerprint: &str, folder: &std::path::Path) -> Result<PathBuf, String> {
    let text = std::fs::read(root.join("own").join(format!("{fingerprint}.rev"))).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    let path = (1..)
        .map(|n| folder.join(if n == 1 { format!("{fingerprint}.rev") } else { format!("{fingerprint}-{n}.rev") }))
        .find(|p| !p.exists())
        .unwrap_or_else(|| folder.join(format!("{fingerprint}.rev")));
    write_key(&path, &text)?;
    Ok(path)
}

/// A key's public part, armored, to give to others.
pub fn export_public(fingerprint: &str) -> Option<String> {
    let keys = Keys::load();
    let cert = keys.all().find(|c| c.fingerprint().to_hex() == fingerprint)?;
    cert.armored().to_vec().ok().map(|b| String::from_utf8_lossy(&b).to_string())
}

/// Removes a key; yours are kept aside in `pgp/removed`, never deleted, with
/// their revocation certificate.
pub fn remove(fingerprint: &str) -> Result<(), String> {
    remove_in(&pgp_dir(), fingerprint)
}

/// [`remove`] in the folder `root` (`pgp/`).
pub(crate) fn remove_in(root: &std::path::Path, fingerprint: &str) -> Result<(), String> {
    for (dir, own) in [(root.join("own"), true), (root.join("others"), false)] {
        let path = dir.join(format!("{fingerprint}.asc"));
        if !path.exists() {
            continue;
        }
        if own {
            let aside = root.join("removed");
            std::fs::create_dir_all(&aside).map_err(|e| e.to_string())?;
            std::fs::rename(&path, aside.join(format!("{fingerprint}.asc"))).map_err(|e| e.to_string())?;
            let revocation = dir.join(format!("{fingerprint}.rev"));
            if revocation.exists() {
                std::fs::rename(&revocation, aside.join(format!("{fingerprint}.rev"))).map_err(|e| e.to_string())?;
            }
        } else {
            std::fs::remove_file(&path).map_err(|e| e.to_string())?;
            let _ = std::fs::remove_file(served_path(&dir, fingerprint));
        }
    }
    Ok(())
}

/// A signer of yours, as Sequoia takes it: a key here ([`here_signer`]) or
/// on a security key ([`securitykey::CardSigner`]).
pub type Signing<'a> = dyn crypto::Signer + Send + Sync + 'a;

/// Your key here that signs, its secret part opened with `passphrase`: the
/// signer of a key kept on this device.
pub fn here_signer(cert: &Cert, passphrase: &str) -> Result<crypto::KeyPair, String> {
    let policy = policy();
    let key = cert.keys().secret().with_policy(&policy, None).alive().revoked(false).for_signing().next().ok_or("no signing key")?.key().clone();
    let key = if key.has_unencrypted_secret() { key } else { key.decrypt_secret(&Password::from(passphrase)).map_err(|_| "wrong passphrase".to_string())? };
    key.into_keypair().map_err(|e| e.to_string())
}

/// A signer lent for one message: Sequoia's stream takes its signer by value,
/// and a security key's signer borrows what it needs to reach the key.
struct Lent<'r, 'o>(&'r mut Signing<'o>);

impl crypto::Signer for Lent<'_, '_> {
    fn public(&self) -> &openpgp::packet::Key<key::PublicParts, key::UnspecifiedRole> {
        self.0.public()
    }

    fn acceptable_hashes(&self) -> &[HashAlgorithm] {
        self.0.acceptable_hashes()
    }

    fn sign(&mut self, hash_algo: HashAlgorithm, digest: &[u8]) -> openpgp::Result<mpi::Signature> {
        self.0.sign(hash_algo, digest)
    }
}

/// A detached signature of `data`, armored, and the hash it used ("pgp-sha512"),
/// for multipart/signed (RFC 3156 §5). `signer` is your key here
/// ([`here_signer`]) or on a security key ([`securitykey::CardSigner`]).
pub fn sign_detached(data: &[u8], signer: &mut Signing<'_>) -> Result<(String, String), String> {
    let mut out = Vec::new();
    {
        let message = Message::new(&mut out);
        let message = Armorer::new(message).kind(openpgp::armor::Kind::Signature).build().map_err(|e| e.to_string())?;
        let mut signing = Signer::new(message, Lent(signer)).map_err(|e| e.to_string())?.detached().build().map_err(|e| e.to_string())?;
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
pub fn encrypt(data: &[u8], recipients: &[&Cert], signer: Option<&mut Signing<'_>>) -> Result<String, String> {
    let policy = policy();
    // Never to a certificate revoked as a whole (N3), whoever chose it.
    let keys: Vec<_> = recipients.iter().filter(|c| usable(c)).flat_map(|c| c.keys().with_policy(&policy, None).supported().alive().revoked(false).for_transport_encryption()).collect();
    encrypt_to(data, keys, signer)
}

/// `data` encrypted for you alone, to `yours`: an encryption key of yours
/// that expired but was not revoked still takes it, since you open it with
/// that key, which Sequoia's policy allows (review of 19 October 2026, F19).
fn encrypt_for_yourself(data: &[u8], yours: &[&Cert]) -> Result<String, String> {
    let policy = policy();
    let keys: Vec<_> = yours.iter().filter(|c| usable(c)).flat_map(|c| c.keys().with_policy(&policy, None).supported().revoked(false).for_transport_encryption()).collect();
    encrypt_to(data, keys, None)
}

fn encrypt_to<'a>(data: &[u8], keys: Vec<ValidErasedKeyAmalgamation<'a, openpgp::packet::key::PublicParts>>, signer: Option<&mut Signing<'_>>) -> Result<String, String> {
    if keys.is_empty() {
        return Err("no encryption key".into());
    }
    let mut out = Vec::new();
    {
        let message = Message::new(&mut out);
        let message = Armorer::new(message).build().map_err(|e| e.to_string())?;
        let message = Encryptor::for_recipients(message, keys).build().map_err(|e| e.to_string())?;
        let message = match signer {
            Some(signer) => Signer::new(message, Lent(signer)).map_err(|e| e.to_string())?.build().map_err(|e| e.to_string())?,
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
    /// Why it is not good: "unknown key", "bad signature", "revoked key"…
    pub problem: String,
}

/// What opening a message found.
#[derive(Debug, Clone, Default)]
pub struct Opened {
    pub data: Vec<u8>,
    pub signatures: Vec<Checked>,
}

/// The session keys of messages a security key opened, by message: such a
/// message opens again, with its attachments and its answer, without the key,
/// until Sioul closes. In memory only, never on disk.
#[derive(Default)]
pub struct SessionKeys(Mutex<BTreeMap<[u8; 32], (Option<SymmetricAlgorithm>, SessionKey)>>);

impl SessionKeys {
    /// None yet.
    pub const fn new() -> SessionKeys {
        SessionKeys(Mutex::new(BTreeMap::new()))
    }

    /// The session's own: one for each running Sioul.
    pub fn global() -> &'static SessionKeys {
        static GLOBAL: SessionKeys = SessionKeys::new();
        &GLOBAL
    }

    fn get(&self, id: &[u8; 32]) -> Option<(Option<SymmetricAlgorithm>, SessionKey)> {
        self.0.lock().ok()?.get(id).cloned()
    }

    fn keep(&self, id: [u8; 32], algo: Option<SymmetricAlgorithm>, key: SessionKey) {
        if let Ok(mut kept) = self.0.lock() {
            kept.insert(id, (algo, key));
        }
    }

    /// Forgets them all: every message a security key opened needs it again.
    pub fn forget(&self) {
        if let Ok(mut kept) = self.0.lock() {
            kept.clear();
        }
    }
}

/// A message's name among the session keys: SHA-256 of its encrypted bytes.
fn message_id(message: &[u8]) -> Option<[u8; 32]> {
    let mut context = HashAlgorithm::SHA256.context().ok()?.for_digest();
    context.update(message);
    context.into_digest().ok()?.try_into().ok()
}

/// How your keys open what is encrypted to you.
pub struct Unlock<'a> {
    /// The passphrase of a key kept here, from the system keyring.
    pub passphrase: &'a dyn Fn(&Fingerprint) -> Option<String>,
    /// The session keys of messages a security key opened before.
    pub sessions: &'a SessionKeys,
    /// Your security key, when you asked to open with it now; without it, no
    /// security key is ever asked anything.
    pub card: Option<&'a CardAccess<'a>>,
    /// The security key a message waits for.
    wanted: RefCell<Option<String>>,
}

impl<'a> Unlock<'a> {
    /// Your keys here, with their passphrases, and the session keys kept.
    pub fn new(passphrase: &'a dyn Fn(&Fingerprint) -> Option<String>, sessions: &'a SessionKeys) -> Unlock<'a> {
        Unlock { passphrase, sessions, card: None, wanted: RefCell::new(None) }
    }

    /// And your security key, asked now.
    pub fn with_card(mut self, card: &'a CardAccess<'a>) -> Unlock<'a> {
        self.card = Some(card);
        self
    }

    /// The security key the last message waits for: it is encrypted to it, and
    /// it was not asked, or it did not open it.
    pub fn wanted(&self) -> Option<String> {
        self.wanted.borrow().clone()
    }
}

struct Helper<'a> {
    keys: &'a Keys,
    unlock: &'a Unlock<'a>,
    /// The message's name for its session key.
    id: Option<[u8; 32]>,
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
                    // Revoked by its designated revoker kept here (R2): Sequoia's
                    // verifier counts only a revocation it can check itself, and
                    // says this one "could be". Any other revocation it judged
                    // already, at the signature's time (a key superseded since
                    // still made good signatures before).
                    Ok(good) if matches!(good.ka.cert().revocation_status(&policy(), None), openpgp::types::RevocationStatus::CouldBe(_)) && !self.keys.usable(good.ka.cert()) => Checked { good: false, signer: String::new(), fingerprint: good.ka.cert().fingerprint().to_hex(), problem: "revoked key".into() },
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
        // Opened before with a security key: its session key, kept in memory.
        if let Some(id) = &self.id
            && let Some((algo, session)) = self.unlock.sessions.get(id)
            && decrypt(algo, &session)
        {
            return Ok(None);
        }
        let policy = policy();
        for pkesk in pkesks {
            for cert in &self.keys.own {
                let passphrase = (self.unlock.passphrase)(&cert.fingerprint()).unwrap_or_default();
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
        // Keys on a security key: asked only when you asked to open with it;
        // otherwise the message says which key it waits for.
        for pkesk in pkesks {
            let Some(recipient) = pkesk.recipient() else { continue };
            for card in self.keys.all_cards() {
                let Ok(fingerprint) = card.decrypt.parse::<Fingerprint>() else { continue };
                if !KeyHandle::from(fingerprint).aliases(&recipient) {
                    continue;
                }
                let Some((cert, public)) = self.keys.cert(&card.cert).and_then(|cert| Some((cert, key_of(cert, &card.decrypt)?))) else { continue };
                *self.unlock.wanted.borrow_mut() = Some(card.ident.clone());
                let Some(access) = self.unlock.card else { continue };
                let mut decryptor = CardDecryptor::new(access, &card.ident, public);
                if let Some((algo, session)) = pkesk.decrypt(&mut decryptor, sym_algo)
                    && decrypt(algo, &session)
                {
                    *self.unlock.wanted.borrow_mut() = None;
                    if let Some(id) = self.id {
                        self.unlock.sessions.keep(id, algo, session);
                    }
                    return Ok(Some(cert.clone()));
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
/// signatures inside. `unlock` gives the passphrase of a key of yours here,
/// the session keys kept, and your security key when you asked to use it.
pub fn decrypt(message: &[u8], keys: &Keys, unlock: &Unlock) -> Result<Opened, String> {
    use std::io::Read as _;
    let policy = policy();
    let helper = Helper { keys, unlock, id: message_id(message), checked: Vec::new() };
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
    let sessions = SessionKeys::new();
    let unlock = Unlock::new(&none, &sessions);
    let helper = Helper { keys, unlock: &unlock, id: None, checked: Vec::new() };
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
    learn_at(&pgp_dir(), bytes, address)
}

/// [`learn`] into the folder `root` (`pgp/`).
fn learn_at(root: &std::path::Path, bytes: &[u8], address: &str) -> Vec<KeySummary> {
    certificates_of(bytes).unwrap_or_default().into_iter().filter_map(|c| store_from_elsewhere_at(root, c, address)).collect()
}

/// What a protected message showed when opened, for the reader's line.
#[derive(Debug, Clone, Default, Serde, PartialEq, Eq)]
pub struct PgpView {
    pub encrypted: bool,
    /// Decrypted; false when no key of yours opens it.
    pub opened: bool,
    pub signatures: Vec<Checked>,
    /// Not opened, and encrypted to your security key of this identifier: the
    /// reader offers "Open with your security key".
    pub security_key: Option<String>,
    /// This device cannot use a security key yet (a phone): a message waiting
    /// for yours says to open it on a computer, and offers no button.
    pub key_elsewhere: bool,
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
/// None when nothing is protected. A message encrypted to your security key
/// opens only when `unlock` carries it, or from a session key kept; otherwise
/// its view names the key it waits for.
pub fn open(raw: &[u8], keys: &Keys, unlock: &Unlock) -> Option<(Option<Vec<u8>>, PgpView)> {
    let message = mail_parser::MessageParser::default().parse(raw)?;
    let root = message.parts.first()?;
    let (kind, protocol) = kind_of(root);
    let children: Vec<&mail_parser::MessagePart> = root.sub_parts().map(|ids| ids.iter().filter_map(|&id| message.parts.get(id as usize)).collect()).unwrap_or_default();
    let closed = || PgpView { encrypted: true, opened: false, signatures: Vec::new(), security_key: unlock.wanted(), key_elsewhere: false };
    if kind == "multipart/encrypted" && protocol == "application/pgp-encrypted" {
        let payload = children.iter().find(|p| kind_of(p).0 == "application/octet-stream").map(|p| p.contents().to_vec())?;
        return Some(match decrypt(&payload, keys, unlock) {
            Ok(opened) => {
                let mut virtual_message = outer_headers(raw, root.raw_body_offset() as usize);
                virtual_message.extend(canonical(&opened.data));
                let mut signatures = opened.signatures;
                // Signed, then encrypted as a multipart/signed (RFC 3156 §6.1): checked too.
                if let Some((_, inner)) = open(&virtual_message, keys, unlock) {
                    signatures.extend(inner.signatures);
                }
                (Some(virtual_message), PgpView { encrypted: true, opened: true, signatures, security_key: None, key_elsewhere: false })
            }
            Err(_) => (None, closed()),
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
        return Some((None, PgpView { encrypted: false, opened: true, signatures: checked, security_key: None, key_elsewhere: false }));
    }
    // Inline: an armored message in the text.
    let text = message.body_text(0)?.to_string();
    let start = text.find("-----BEGIN PGP MESSAGE-----")?;
    let end = text[start..].find("-----END PGP MESSAGE-----").map(|e| start + e + "-----END PGP MESSAGE-----".len())?;
    Some(match decrypt(text[start..end].as_bytes(), keys, unlock) {
        Ok(opened) => {
            let mut virtual_message = outer_headers(raw, root.raw_body_offset() as usize);
            virtual_message.extend_from_slice(b"Content-Type: text/plain; charset=utf-8\r\n\r\n");
            virtual_message.extend(canonical(&opened.data));
            (Some(virtual_message), PgpView { encrypted: true, opened: true, signatures: opened.signatures, security_key: None, key_elsewhere: false })
        }
        Err(_) => (None, closed()),
    })
}

/// Whether a message is encrypted with OpenPGP, as a whole: PGP/MIME (RFC
/// 3156, the shape [`seal_message`] writes), or an inline message whose whole
/// first text part is one armored message. Never a text that quotes one
/// among its words (review of 19 October 2026, F7: a decrypted copy quoting
/// an older armored block was taken for sealed). Nothing is opened.
pub fn is_encrypted(raw: &[u8]) -> bool {
    let Some(message) = mail_parser::MessageParser::default().parse(raw) else { return false };
    let Some(root) = message.parts.first() else { return false };
    let (kind, protocol) = kind_of(root);
    if kind == "multipart/encrypted" && protocol == "application/pgp-encrypted" {
        return true;
    }
    message.body_text(0).is_some_and(|text| {
        let text = text.trim();
        text.starts_with("-----BEGIN PGP MESSAGE-----") && text.ends_with("-----END PGP MESSAGE-----") && text.matches("-----BEGIN PGP MESSAGE-----").count() == 1
    })
}

/// The head of a message as [`open`] rebuilds an encrypted one: its outer
/// header lines (none of them `Content-` nor `MIME-Version`), the line
/// `MIME-Version: 1.0` it adds after them, then the opened entity's own
/// header lines, a `Content-` one among them. Where that line is, when the
/// head has that shape: what a copy decrypted by Sioul before 19 October
/// 2026 looks like (a message that came in clear may too).
fn opened_split(raw: &[u8]) -> Option<usize> {
    mime_split(raw, true)
}

/// Where the `MIME-Version: 1.0` line [`open`] adds after a message's outer
/// headers starts, when nothing of MIME comes before it; with `content`,
/// only when a content's header follows it (what `open` rebuilt from an
/// entity with headers of its own). Without, whatever follows: an entity
/// with an empty head, or none, rebuilt so (review of 19 October 2026, N7).
fn mime_split(raw: &[u8], content: bool) -> Option<usize> {
    let head = &raw[..header_end(raw)];
    let mut at = 0;
    let mut seen_mime = None;
    let mut content_after = false;
    for line in head.split_inclusive(|b| *b == b'\n') {
        let start = at;
        at += line.len();
        if line.starts_with(b" ") || line.starts_with(b"\t") || line.trim_ascii().is_empty() {
            continue;
        }
        let name = String::from_utf8_lossy(line.split(|b| *b == b':').next().unwrap_or_default()).trim().to_ascii_lowercase();
        match seen_mime {
            None if line == b"MIME-Version: 1.0\r\n" => seen_mime = Some(start),
            None if name.starts_with("content-") || name == "mime-version" => return None,
            None => {}
            Some(_) => content_after |= name.starts_with("content-"),
        }
    }
    seen_mime.filter(|_| content_after || !content)
}

/// Whether `raw` has the head of a message [`open`] rebuilt (`opened_split`):
/// what may be a copy decrypted by Sioul before 19 October 2026.
pub fn looks_opened(raw: &[u8]) -> bool {
    opened_split(raw).is_some()
}

/// Where a message's header ends: after its first empty line.
fn header_end(raw: &[u8]) -> usize {
    let crlf = raw.windows(4).position(|w| w == b"\r\n\r\n").map(|p| p + 4);
    let lf = raw.windows(2).position(|w| w == b"\n\n").map(|p| p + 2);
    match (crlf, lf) {
        (Some(a), Some(b)) => a.min(b),
        (a, b) => a.or(b).unwrap_or(raw.len()),
    }
}

/// A message as it reads (opened, in clear), sealed again for `to`, as an
/// encrypted message comes (PGP/MIME, RFC 3156): its own headers in clear but
/// those of its content (From, To, Date, Subject, Message-ID: what a list of
/// mail shows), its content and their headers encrypted inside. [`open`]
/// gives the same message back. For the copy kept beside a draft that
/// answers an encrypted message, never left in clear.
pub fn seal_message(clear: &[u8], to: &[&Cert]) -> Result<Vec<u8>, String> {
    let end = header_end(clear);
    let (head, body) = clear.split_at(end);
    let (mut outer, mut inner): (Vec<u8>, Vec<u8>) = (Vec::new(), Vec::new());
    // A message `open` rebuilt: outside, only the outer headers it came with
    // (those before the `MIME-Version: 1.0` it adds); inside, everything
    // after, as it is: the entity's own headers whatever they are (protected
    // headers, the subject its sender encrypted, never come out in clear:
    // review of 19 October 2026, F8), or no head at all (N7: its first line
    // would read as a header, and stay outside). Any other message: its own
    // headers outside, its content's (`Content-`) inside.
    let payload = if let Some(split) = mime_split(clear, false) {
        let mime = b"MIME-Version: 1.0\r\n".len();
        outer.extend_from_slice(&head[..split]);
        clear[split + mime..].to_vec()
    } else {
        let mut into_outer = true;
        let mut skip = false;
        for line in head.split_inclusive(|b| *b == b'\n') {
            if line == b"\r\n" || line == b"\n" {
                continue;
            }
            if !line.starts_with(b" ") && !line.starts_with(b"\t") {
                let name = String::from_utf8_lossy(line.split(|b| *b == b':').next().unwrap_or_default()).trim().to_ascii_lowercase();
                skip = name == "mime-version";
                into_outer = !name.starts_with("content-");
            }
            if skip {
                continue;
            }
            if into_outer { outer.extend_from_slice(line) } else { inner.extend_from_slice(line) }
        }
        if inner.is_empty() {
            inner.extend_from_slice(b"Content-Type: text/plain; charset=utf-8\r\n");
        }
        let mut payload = canonical(&inner);
        payload.extend_from_slice(b"\r\n");
        payload.extend_from_slice(body);
        payload
    };
    let armored = encrypt_for_yourself(&payload, to)?;
    let boundary = "sioul-sealed-copy";
    let mut out = canonical(&outer);
    out.extend_from_slice(
        format!(
            "MIME-Version: 1.0\r\nContent-Type: multipart/encrypted; protocol=\"application/pgp-encrypted\"; boundary=\"{boundary}\"\r\n\r\n--{boundary}\r\nContent-Type: application/pgp-encrypted\r\nContent-Description: PGP/MIME version identification\r\n\r\nVersion: 1\r\n\r\n--{boundary}\r\nContent-Type: application/octet-stream; name=\"encrypted.asc\"\r\nContent-Disposition: inline; filename=\"encrypted.asc\"\r\n\r\n{}\r\n--{boundary}--\r\n",
            canonical(armored.trim_end().as_bytes()).iter().map(|&b| b as char).collect::<String>()
        )
        .as_bytes(),
    );
    Ok(out)
}

/// A key for tests, made now (Curve25519: its primary key certifies and
/// signs, a subkey decrypts), named `userid`, its secret part unencrypted,
/// armored: for the crates that do not use Sequoia themselves. Never one of
/// yours.
pub fn stand_in_key(userid: &str) -> Result<Vec<u8>, String> {
    let (cert, _) = CertBuilder::new().set_cipher_suite(CipherSuite::Cv25519).add_userid(userid).add_transport_encryption_subkey().generate().map_err(|e| e.to_string())?;
    cert.as_tsk().armored().to_vec().map_err(|e| e.to_string())
}

/// The keys a message is encrypted to, as its packets name them (key IDs or
/// fingerprints, hexadecimal), in its order: whom it is for, nothing opened.
pub fn recipients_of(raw: &[u8]) -> Vec<String> {
    let Some(message) = mail_parser::MessageParser::default().parse(raw) else { return Vec::new() };
    let payload = message.parts.iter().find(|p| kind_of(p).0 == "application/octet-stream").map(|p| p.contents().to_vec()).unwrap_or_else(|| raw.to_vec());
    let mut out = Vec::new();
    let Ok(mut parsing) = openpgp::parse::PacketParser::from_bytes(&payload) else { return out };
    while let openpgp::parse::PacketParserResult::Some(packet) = parsing {
        if let openpgp::Packet::PKESK(pkesk) = &packet.packet {
            out.push(pkesk.recipient().map(|r| r.to_hex()).unwrap_or_default());
        }
        match packet.next() {
            Ok((_, next)) => parsing = next,
            Err(_) => break,
        }
    }
    out
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
    // One certificate, as `Cert::from_bytes` reads it, kept for `addr`
    // alone (Autocrypt Level 1 binds the key to it; its other user IDs, bound
    // or not, are decoration: review of 19 October 2026, F1 and F2).
    let [cert] = <[Cert; 1]>::try_from(certificates_of(&bytes).ok()?).ok()?;
    for_its_address(cert, from)
}

/// Keeps a key from an Autocrypt header: only public parts, only for its
/// sender's address, never one of yours (whoever sends your own key back
/// cannot add anything to it).
pub fn learn_autocrypt(header: &str, from: &str) -> Option<KeySummary> {
    learn_autocrypt_at(&pgp_dir(), header, from)
}

/// [`learn_autocrypt`] into the folder `root` (`pgp/`).
fn learn_autocrypt_at(root: &std::path::Path, header: &str, from: &str) -> Option<KeySummary> {
    let cert = autocrypt_key(header, from)?.strip_secret_key_material();
    if yours_at(root, &cert.fingerprint().to_hex()) {
        return None;
    }
    store_in(root, cert, false).ok()
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
        let keys_of_bob = Keys { own: vec![bob.clone()], others: vec![alice.clone().strip_secret_key_material()], ..Keys::default() };
        let mut signer = here_signer(&alice, "secret words").unwrap();
        let sealed = encrypt(b"Content-Type: text/plain\r\n\r\nHello Bob.\r\n", &[&bob, &alice], Some(&mut signer)).unwrap();
        assert!(sealed.starts_with("-----BEGIN PGP MESSAGE-----"));
        let passphrase = |_: &Fingerprint| Some("secret words".to_string());
        let sessions = SessionKeys::new();
        let opened = decrypt(sealed.as_bytes(), &keys_of_bob, &Unlock::new(&passphrase, &sessions)).unwrap();
        assert_eq!(opened.data, b"Content-Type: text/plain\r\n\r\nHello Bob.\r\n");
        assert_eq!(opened.signatures.len(), 1);
        assert!(opened.signatures[0].good && opened.signatures[0].signer.contains("alice@example.org"));
        let wrong = |_: &Fingerprint| Some("other words".to_string());
        assert!(decrypt(sealed.as_bytes(), &keys_of_bob, &Unlock::new(&wrong, &sessions)).is_err());
        assert!(here_signer(&alice, "other words").is_err());
        let (signature, micalg) = sign_detached(b"signed part", &mut signer).unwrap();
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

    /// A key made here keeps its revocation certificate beside it, yours
    /// alone; a copy can be saved; removed, the key takes it aside with it.
    #[test]
    fn a_key_made_here_keeps_its_revocation_certificate() {
        let root = temporary().join("pgp-made");
        let _ = std::fs::remove_dir_all(&root);
        let made = generate_at(&root, "Jane Exemple", "jane@example.org", "secret words").unwrap();
        assert!(made.own && made.revocation);
        let kept = root.join("own").join(format!("{}.rev", made.fingerprint));
        let text = std::fs::read_to_string(&kept).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&kept).unwrap().permissions().mode() & 0o777, 0o600, "protected as the key is");
        }
        assert!(text.contains(&made.fingerprint) && text.contains("Jane Exemple <jane@example.org>"), "{text}");
        // Never imported by mistake: a colon before the armor; read as no key.
        assert!(text.contains("\n:-----BEGIN PGP PUBLIC KEY BLOCK-----") && certificates_in(text.as_bytes()).is_empty());
        // The colon taken away, it revokes this very key.
        let armored = &text[text.find(":-----BEGIN").unwrap() + 1..];
        let pile = openpgp::PacketPile::from_bytes(armored.as_bytes()).unwrap();
        let revocation = pile.descendants().find_map(|p| if let openpgp::Packet::Signature(s) = p { Some(s.clone()) } else { None }).unwrap();
        let key = Cert::from_bytes(&std::fs::read(root.join("own").join(format!("{}.asc", made.fingerprint))).unwrap()).unwrap();
        assert!(matches!(key.revocation_status(&policy(), None), openpgp::types::RevocationStatus::NotAsFarAsWeKnow));
        let (revoked, _) = key.insert_packets(revocation).unwrap();
        assert!(matches!(revoked.revocation_status(&policy(), None), openpgp::types::RevocationStatus::Revoked(_)));
        // A copy saved elsewhere, as kept, yours alone; a second one beside it.
        let elsewhere = root.join("saved");
        let copy = save_revocation_at(&root, &made.fingerprint, &elsewhere).unwrap();
        assert_eq!((copy.file_name().unwrap().to_string_lossy().to_string(), std::fs::read_to_string(&copy).unwrap()), (format!("{}.rev", made.fingerprint), text));
        assert!(save_revocation_at(&root, &made.fingerprint, &elsewhere).unwrap().ends_with(format!("{}-2.rev", made.fingerprint)));
        assert!(save_revocation_at(&root, "0000", &elsewhere).is_err(), "none kept for a key not made here");
        let _ = std::fs::remove_dir_all(&root);
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

    /// A key as a security key holds it: the primary signs, a Curve25519 subkey decrypts.
    fn card_key(address: &str, made: Option<std::time::SystemTime>) -> Cert {
        let mut builder = CertBuilder::new()
            .add_userid(format!("Card Holder <{address}>"))
            .set_cipher_suite(CipherSuite::Cv25519)
            .set_primary_key_flags(openpgp::types::KeyFlags::empty().set_certification().set_signing())
            .add_transport_encryption_subkey();
        if let Some(made) = made {
            builder = builder.set_creation_time(made).set_validity_period(std::time::Duration::from_secs(365 * 24 * 3600));
        }
        builder.generate().unwrap().0
    }

    fn record(card: &securitykey::SoftCard, cert: &Cert, address: &str) -> KnownCard {
        KnownCard {
            ident: card.info.ident.clone(),
            sign: card.info.sign.as_ref().map(|s| s.fingerprint.clone()).unwrap_or_default(),
            decrypt: card.info.decrypt.as_ref().map(|s| s.fingerprint.clone()).unwrap_or_default(),
            cert: cert.fingerprint().to_hex(),
            addresses: vec![address.to_string()],
            ..KnownCard::default()
        }
    }

    #[test]
    fn a_security_key_signs_alone_for_its_addresses() {
        let on_card = card_key("me@example.org", None);
        let card = securitykey::SoftCard::new("0006:12345678", &on_card, "123456");
        let here = key("me@example.org");
        let mut keys = Keys { own: vec![on_card.clone().strip_secret_key_material(), here.clone()], ..Keys::default() };
        assert_eq!(keys.own_for("me@example.org").unwrap().place, Place::Here);
        keys.cards.push(record(&card, &on_card, "me@example.org"));
        let own = keys.own_for("ME@example.org").unwrap();
        assert_eq!(own.place, Place::Card { ident: "0006:12345678".into() });
        assert_eq!(own.cert.fingerprint(), on_card.fingerprint());
        // Autocrypt announces the security key's certificate.
        assert!(autocrypt_header(own.cert, "me@example.org").is_some());
        assert!(keys.own_for("other@example.org").is_none());
        // Expired on the card: nothing signs for the address, and that is known.
        let late = card_key("late@example.org", Some(std::time::SystemTime::now() - std::time::Duration::from_secs(3 * 365 * 24 * 3600)));
        let late_card = securitykey::SoftCard::new("0006:00000002", &late, "123456");
        let keys = Keys { own: vec![late.clone().strip_secret_key_material(), key("late@example.org")], cards: vec![record(&late_card, &late, "late@example.org")], ..Keys::default() };
        assert!(keys.own_for("late@example.org").is_none());
        assert!(keys.card_for("late@example.org").is_some());
    }

    /// The folders of a device's keys (`pgp/`), and its security keys made known to the others.
    fn read_here(root: &std::path::Path) {
        securitykey::share_cards_in(&root.join("cards.toml"), &root.join("own"), &root.join(securitykey::SHARED_CARDS), false).unwrap();
    }

    /// What the sharing carries from one device's `pgp/cards/` to another's.
    fn carry(from: &std::path::Path, to: &std::path::Path) {
        let (from, to) = (from.join(securitykey::SHARED_CARDS), to.join(securitykey::SHARED_CARDS));
        let _ = std::fs::remove_dir_all(&to);
        std::fs::create_dir_all(&to).unwrap();
        for entry in std::fs::read_dir(&from).unwrap().filter_map(Result::ok) {
            std::fs::copy(entry.path(), to.join(entry.file_name())).unwrap();
        }
    }

    /// Batch A (18 October 2026): your security key's public half travels
    /// (`pgp/cards/`), written by the computer that read the key, read by a
    /// phone that never did: it then encrypts to you, hands your key by
    /// Autocrypt and knows what waits for the key. Never a secret; a
    /// certificate replaced, or another key's, is refused; a certificate
    /// claiming your address from outside is never yours.
    #[test]
    fn your_security_keys_certificate_travels_and_only_it_is_yours() {
        let root = temporary().join("cards-travel");
        let _ = std::fs::remove_dir_all(&root);
        let (computer, phone) = (root.join("computer"), root.join("phone"));
        let on_card = card_key("me@example.org", None);
        let card = securitykey::SoftCard::new("0006:12345678", &on_card, "123456");
        store_in(&computer, on_card.clone().strip_secret_key_material(), true).unwrap();
        securitykey::change_known_at(&computer.join("cards.toml"), |cards| cards.push(record(&card, &on_card, "me@example.org"))).unwrap();
        read_here(&computer);
        let shared = std::fs::read_to_string(computer.join("cards").join("0006-12345678.toml")).unwrap();
        assert!(shared.contains("BEGIN PGP PUBLIC KEY BLOCK") && !shared.contains("PRIVATE KEY"), "public parts only");
        // The phone: nothing of its own, the card's file carried by the sharing.
        carry(&computer, &phone);
        let keys = Keys::load_at(&phone);
        assert_eq!((keys.cards.len(), keys.shared.len()), (0, 1), "known from the computer, not read here");
        assert!(keys.own_for("me@example.org").is_none(), "signing only where it was read (F18)");
        assert_eq!(keys.serving_card("me@example.org").map(|(card, _)| card.ident.as_str()), Some("0006:12345678"));
        let yours = keys.self_cert_for("me@example.org").expect("yours to encrypt to");
        assert_eq!(yours.fingerprint(), on_card.fingerprint());
        assert!(autocrypt_header(yours, "me@example.org").is_some(), "Autocrypt from the phone too");
        // What waits for the key is known there.
        let none = |_: &Fingerprint| None;
        let sessions = SessionKeys::new();
        let unlock = Unlock::new(&none, &sessions);
        let sealed = encrypt(b"Hello me.", &[yours], None).unwrap();
        assert!(decrypt(sealed.as_bytes(), &keys, &unlock).is_err());
        assert_eq!(unlock.wanted().as_deref(), Some("0006:12345678"));
        // And your key opens it, wherever it can be used.
        let readers = securitykey::SoftReaders::with(vec![securitykey::SoftCard::new("0006:12345678", &on_card, "123456").with_x25519(securitykey::x25519::x25519)]);
        let pin = Password::from("123456");
        let access = CardAccess::new(&readers, &pin, &|| {});
        assert_eq!(decrypt(sealed.as_bytes(), &keys, &Unlock::new(&none, &sessions).with_card(&access)).unwrap().data, b"Hello me.");
        // Your own mail read on the phone carries your security key's
        // certificate in its Autocrypt header, which the phone knows from the
        // sharing alone: never kept among others' keys there, nor from a lookup.
        assert!(learn_autocrypt_at(&phone, &autocrypt_header(yours, "me@example.org").unwrap(), "me@example.org").is_none());
        assert!(learn_at(&phone, &on_card.clone().strip_secret_key_material().armored().to_vec().unwrap(), "me@example.org").is_empty());
        assert!(!phone.join("others").join(format!("{}.asc", on_card.fingerprint().to_hex())).exists());
        // A certificate claiming your address from outside (an Autocrypt header) is never yours.
        let stranger = key("me@example.org");
        store_in(&phone, stranger.clone().strip_secret_key_material(), false).unwrap();
        let keys = Keys::load_at(&phone);
        assert_eq!(keys.for_address("me@example.org").map(Cert::fingerprint), Some(on_card.fingerprint()));
        assert!(keys.own.iter().all(|c| c.fingerprint() != stranger.fingerprint()));
        // A shared file whose certificate was replaced, or is another key's: refused.
        let mut replaced: securitykey::SharedCard = toml::from_str(&shared).unwrap();
        replaced.certificate = String::from_utf8(stranger.clone().strip_secret_key_material().armored().to_vec().unwrap()).unwrap();
        std::fs::write(phone.join("cards").join("0006-12345678.toml"), toml::to_string(&replaced).unwrap()).unwrap();
        let keys = Keys::load_at(&phone);
        assert!(keys.shared.is_empty() && keys.serving_card("me@example.org").is_none() && keys.own.iter().all(|c| c.fingerprint() != stranger.fingerprint()), "replaced: refused");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A computer whose key changed (renewed, as on 8 October 2026) updates
    /// what the others know; a computer still holding the older certificate
    /// never undoes it; an expired certificate of yours takes no encryption.
    #[test]
    fn a_renewal_reaches_every_device_and_is_never_undone() {
        let root = temporary().join("cards-renewal");
        let _ = std::fs::remove_dir_all(&root);
        let (one, two, phone) = (root.join("one"), root.join("two"), root.join("phone"));
        let made = std::time::SystemTime::now() - std::time::Duration::from_secs(30 * 24 * 3600);
        let on_card = card_key("me@example.org", Some(made));
        let card = securitykey::SoftCard::new("0006:12345678", &on_card, "123456");
        for computer in [&one, &two] {
            store_in(computer, on_card.clone().strip_secret_key_material(), true).unwrap();
            securitykey::change_known_at(&computer.join("cards.toml"), |cards| cards.push(record(&card, &on_card, "me@example.org"))).unwrap();
            read_here(computer);
        }
        let expires = |keys: &Keys| keys.serving_card("me@example.org").and_then(|(_, cert)| cert.with_policy(&policy(), None).ok()?.primary_key().key_expiration_time());
        carry(&one, &phone);
        let before = expires(&Keys::load_at(&phone)).unwrap();
        // Renewed on the first computer: kept with its keys, the shared file follows.
        let mut signer = on_card.primary_key().key().clone().parts_into_secret().unwrap().into_keypair().unwrap();
        let later = std::time::SystemTime::now() + std::time::Duration::from_secs(2 * 365 * 24 * 3600);
        let signatures = on_card.set_expiration_time(&policy(), None, &mut signer, Some(later)).unwrap();
        let renewed = on_card.clone().insert_packets(signatures).unwrap().0;
        store_in(&one, renewed.strip_secret_key_material(), true).unwrap();
        carry(&one, &phone);
        let after = expires(&Keys::load_at(&phone)).unwrap();
        assert!(after > before, "renewed on the phone too");
        // The second computer, still on the older certificate, writes after: the renewal stays.
        carry(&one, &two);
        securitykey::change_known_at(&two.join("cards.toml"), |cards| cards[0].holder = "Card Holder".into()).unwrap();
        read_here(&two);
        carry(&two, &phone);
        assert_eq!(expires(&Keys::load_at(&phone)), Some(after), "never undone");
        // An expired certificate of yours: no encryption to it, said by the window (`self_cert_for` none).
        let old = card_key("old@example.org", Some(std::time::SystemTime::now() - std::time::Duration::from_secs(400 * 24 * 3600)));
        let old_card = securitykey::SoftCard::new("0006:00000001", &old, "123456");
        let keys = Keys { own: vec![old.clone().strip_secret_key_material()], cards: vec![record(&old_card, &old, "old@example.org")], ..Keys::default() };
        assert!(keys.is_own_address("old@example.org") && keys.self_cert_for("old@example.org").is_none() && keys.for_address("old@example.org").is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// This device's marks, as `state/security-keys.toml` keeps them.
    fn marks(root: &std::path::Path) -> std::path::PathBuf {
        root.join("marks.toml")
    }

    /// An exchange's pass over the security keys read on `root`.
    fn exchange_pass(root: &std::path::Path) {
        securitykey::share_cards_marked_in(&root.join("cards.toml"), &root.join("own"), &root.join(securitykey::SHARED_CARDS), &marks(root)).unwrap();
    }

    /// Review of 19 October 2026, P4 (F4): two computers read the key; the
    /// first stops using it. The second computer's next exchange no longer
    /// makes its file again ("once if missing" ran at every exchange), and
    /// when the second computer shares it again later, the first never takes
    /// it back: it signed with the key again, and said it was read on another
    /// device. The second computer still signs with it; the phone encrypts to it.
    #[test]
    fn a_card_stopped_on_one_computer_never_comes_back_to_it() {
        let root = temporary().join("cards-stopped");
        let _ = std::fs::remove_dir_all(&root);
        let (one, two, phone) = (root.join("one"), root.join("two"), root.join("phone"));
        let on_card = card_key("me@example.org", None);
        let card = securitykey::SoftCard::new("0006:12345678", &on_card, "123456");
        for computer in [&one, &two] {
            store_in(computer, on_card.clone().strip_secret_key_material(), true).unwrap();
            securitykey::change_known_at(&computer.join("cards.toml"), |cards| cards.push(record(&card, &on_card, "me@example.org"))).unwrap();
            exchange_pass(computer);
        }
        carry(&one, &phone);
        let file = |root: &std::path::Path| root.join(securitykey::SHARED_CARDS).join("0006-12345678.toml");
        assert!(file(&two).exists() && file(&phone).exists());
        // The first computer stops using it: listed as stopped there, its file taken out, everywhere.
        assert!(securitykey::forget_in(&one, "0006:12345678").unwrap());
        assert!(Keys::load_at(&one).own_for("me@example.org").is_none());
        carry(&one, &two);
        carry(&one, &phone);
        // The second computer's next exchange makes nothing again.
        exchange_pass(&two);
        assert!(!file(&two).exists(), "never made again by the exchange's pass");
        // Later, what the second computer knows of the key changes: written again, it travels;
        // the first computer never takes it back.
        securitykey::change_known_at(&two.join("cards.toml"), |cards| cards[0].holder = "Ferrand<<Noa".into()).unwrap();
        read_here(&two);
        carry(&two, &one);
        let first = Keys::load_at(&one);
        assert!(first.own_for("me@example.org").is_none() && first.shared.is_empty() && first.serving_card("me@example.org").is_none(), "stopped there");
        // The second computer signs with it; the phone encrypts to it and signs nothing.
        assert_eq!(Keys::load_at(&two).own_for("me@example.org").map(|o| o.place), Some(Place::Card { ident: "0006:12345678".into() }));
        carry(&two, &phone);
        let on_phone = Keys::load_at(&phone);
        assert!(on_phone.self_cert_for("me@example.org").is_some() && on_phone.own_for("me@example.org").is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Review of 19 October 2026, F18: a computer that keeps a key of its own
    /// for an address goes on signing with it when another computer shares a
    /// security key serving that address; the shared key serves encryption to
    /// you and Autocrypt there.
    #[test]
    fn a_card_read_on_another_computer_signs_only_there() {
        let root = temporary().join("cards-f18");
        let _ = std::fs::remove_dir_all(&root);
        let (one, three) = (root.join("one"), root.join("three"));
        let on_card = card_key("me@example.org", None);
        let card = securitykey::SoftCard::new("0006:12345678", &on_card, "123456");
        store_in(&one, on_card.clone().strip_secret_key_material(), true).unwrap();
        securitykey::change_known_at(&one.join("cards.toml"), |cards| cards.push(record(&card, &on_card, "me@example.org"))).unwrap();
        exchange_pass(&one);
        let here = key("me@example.org");
        store_at(&three, here.clone()).unwrap();
        carry(&one, &three);
        let keys = Keys::load_at(&three);
        let signer = keys.own_for("me@example.org").expect("the key kept here signs");
        assert_eq!((signer.place, signer.cert.fingerprint()), (Place::Here, here.fingerprint()));
        assert_eq!(keys.self_cert_for("me@example.org").map(Cert::fingerprint), Some(on_card.fingerprint()));
        assert!(autocrypt_header(keys.self_cert_for("me@example.org").unwrap(), "me@example.org").is_some());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Review of 19 October 2026, P3 (F3): a shared certificate whose
    /// decryption key has no binding, or is revoked, is never the security
    /// key's, as Sequoia's policy reads it; one in order is.
    #[test]
    fn a_shared_certificate_counts_only_with_the_cards_keys_bound() {
        let on_card = card_key("me@example.org", None);
        let card = securitykey::SoftCard::new("0006:12345678", &on_card, "123456");
        let known = record(&card, &on_card, "me@example.org");
        let armored = |c: &Cert| String::from_utf8(c.clone().strip_secret_key_material().armored().to_vec().unwrap()).unwrap();
        assert!(securitykey::vouched(&known, &armored(&on_card)).is_some());
        let unbound = Cert::from_packets(on_card.clone().into_packets().filter(|p| !matches!(p, openpgp::Packet::Signature(sig) if sig.typ() == openpgp::types::SignatureType::SubkeyBinding))).unwrap();
        assert!(on_card.keys().subkeys().count() == 1 && unbound.keys().subkeys().count() == 1);
        assert!(securitykey::vouched(&known, &armored(&unbound)).is_none(), "no binding");
        let mut signer = on_card.primary_key().key().clone().parts_into_secret().unwrap().into_keypair().unwrap();
        let subkey = on_card.keys().subkeys().next().unwrap().key().clone();
        let revocation = SubkeyRevocationBuilder::new().set_reason_for_revocation(openpgp::types::ReasonForRevocation::KeyCompromised, b"lost").unwrap().build(&mut signer, &on_card, &subkey, None).unwrap();
        let revoked = on_card.clone().insert_packets(vec![openpgp::Packet::from(revocation)]).unwrap().0;
        assert!(securitykey::vouched(&known, &armored(&revoked)).is_none(), "revoked");
        assert_eq!(securitykey::revoked_encryption(&revoked), Some(subkey.fingerprint().to_hex()));
        assert_eq!(securitykey::revoked_encryption(&on_card), None);
    }

    /// Review of 19 October 2026, P5 (F5): renewed on the first computer, the
    /// second (not reached yet) writes its older certificate after: the phone
    /// had the expired one until the first computer's record changed. Now
    /// the first computer's next exchange writes the renewal again.
    #[test]
    fn a_renewal_lost_to_an_older_write_comes_back_at_the_next_exchange() {
        let root = temporary().join("cards-renewal-lost");
        let _ = std::fs::remove_dir_all(&root);
        let (one, two, phone) = (root.join("one"), root.join("two"), root.join("phone"));
        let made = std::time::SystemTime::now() - std::time::Duration::from_secs(400 * 24 * 3600);
        let on_card = card_key("me@example.org", Some(made));
        let card = securitykey::SoftCard::new("0006:12345678", &on_card, "123456");
        for computer in [&one, &two] {
            store_in(computer, on_card.clone().strip_secret_key_material(), true).unwrap();
            securitykey::change_known_at(&computer.join("cards.toml"), |cards| cards.push(record(&card, &on_card, "me@example.org"))).unwrap();
            exchange_pass(computer);
        }
        let mut signer = on_card.primary_key().key().clone().parts_into_secret().unwrap().into_keypair().unwrap();
        let later = std::time::SystemTime::now() + std::time::Duration::from_secs(2 * 365 * 24 * 3600);
        let mut packets = on_card.set_expiration_time(&policy(), None, &mut signer, Some(later)).unwrap();
        let subkey = on_card.keys().subkeys().next().unwrap().key().clone().parts_into_secret().unwrap();
        let primary = on_card.primary_key().key().clone();
        let binding = openpgp::packet::signature::SignatureBuilder::new(openpgp::types::SignatureType::SubkeyBinding)
            .set_key_flags(openpgp::types::KeyFlags::empty().set_transport_encryption().set_storage_encryption())
            .unwrap()
            .set_key_validity_period(later.duration_since(subkey.creation_time()).unwrap())
            .unwrap()
            .sign_subkey_binding(&mut signer, &primary, &subkey)
            .unwrap();
        packets.push(binding.into());
        let renewed = on_card.clone().insert_packets(packets).unwrap().0;
        store_in(&one, renewed.strip_secret_key_material(), true).unwrap();
        assert!(Keys::load_at(&one).self_cert_for("me@example.org").is_some(), "renewed on the first");
        // The second, not reached yet, writes its older certificate: it travels after the renewal.
        securitykey::change_known_at(&two.join("cards.toml"), |cards| cards[0].holder = "Ferrand<<Noa".into()).unwrap();
        read_here(&two);
        carry(&two, &one);
        carry(&two, &phone);
        assert!(Keys::load_at(&phone).self_cert_for("me@example.org").is_none(), "the race: expired on the phone");
        // The first computer's next exchange writes it again: the phone has the renewal.
        exchange_pass(&one);
        carry(&one, &phone);
        assert!(Keys::load_at(&phone).self_cert_for("me@example.org").is_some(), "renewed on the phone");
        // And it settles: nothing more written.
        let written = std::fs::read(one.join(securitykey::SHARED_CARDS).join("0006-12345678.toml")).unwrap();
        exchange_pass(&one);
        assert_eq!(std::fs::read(one.join(securitykey::SHARED_CARDS).join("0006-12345678.toml")).unwrap(), written);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_message_to_the_security_key_waits_for_it_then_opens_from_memory() {
        let on_card = card_key("me@example.org", None);
        let card = securitykey::SoftCard::new("0006:12345678", &on_card, "123456").with_x25519(securitykey::x25519::x25519);
        let keys = Keys { own: vec![on_card.clone().strip_secret_key_material()], cards: vec![record(&card, &on_card, "me@example.org")], ..Keys::default() };
        let readers = securitykey::SoftReaders::with(vec![card]);
        let sealed = encrypt(b"Hello me.", &[&on_card], None).unwrap();
        let none = |_: &Fingerprint| None;
        let sessions = SessionKeys::new();
        // Shown: nothing is asked of the key, the message says which key it waits for.
        let unlock = Unlock::new(&none, &sessions);
        assert!(decrypt(sealed.as_bytes(), &keys, &unlock).is_err());
        assert_eq!(unlock.wanted().as_deref(), Some("0006:12345678"));
        assert_eq!(readers.card("0006:12345678", |c| c.operations), Some(0));
        // A wrong PIN: said, nothing opened.
        let wrong = Password::from("654321");
        let access = CardAccess::new(&readers, &wrong, &|| {});
        let unlock = Unlock::new(&none, &sessions).with_card(&access);
        assert!(decrypt(sealed.as_bytes(), &keys, &unlock).is_err());
        assert_eq!(access.failure(), Some(securitykey::CardError::WrongPin { left: 2 }));
        // "Open with your security key": opened, its session key kept.
        let pin = Password::from("123456");
        let access = CardAccess::new(&readers, &pin, &|| {});
        let opened = decrypt(sealed.as_bytes(), &keys, &Unlock::new(&none, &sessions).with_card(&access)).unwrap();
        assert_eq!(opened.data, b"Hello me.");
        assert_eq!(readers.card("0006:12345678", |c| c.operations), Some(1));
        // Again, its attachments, its answer: from memory, the key left alone.
        let again = decrypt(sealed.as_bytes(), &keys, &Unlock::new(&none, &sessions)).unwrap();
        assert_eq!(again.data, b"Hello me.");
        assert_eq!(readers.card("0006:12345678", |c| c.operations), Some(1));
        // Another message still waits for the key; forgotten, this one too.
        let other = encrypt(b"Another.", &[&on_card], None).unwrap();
        assert!(decrypt(other.as_bytes(), &keys, &Unlock::new(&none, &sessions)).is_err());
        sessions.forget();
        assert!(decrypt(sealed.as_bytes(), &keys, &Unlock::new(&none, &sessions)).is_err());
    }

    #[test]
    fn an_expired_security_key_still_opens_older_mail() {
        let then = std::time::SystemTime::now() - std::time::Duration::from_secs(3 * 365 * 24 * 3600);
        let late = card_key("late@example.org", Some(then));
        // Sent while the key was valid.
        let policy = policy();
        let sent = then + std::time::Duration::from_secs(24 * 3600);
        let recipients: Vec<_> = late.keys().with_policy(&policy, sent).alive().for_transport_encryption().collect();
        assert!(!recipients.is_empty());
        let mut sealed = Vec::new();
        {
            let message = Message::new(&mut sealed);
            let message = Encryptor::for_recipients(message, recipients).build().unwrap();
            let mut literal = LiteralWriter::new(message).build().unwrap();
            literal.write_all(b"Older mail.").unwrap();
            literal.finalize().unwrap();
        }
        let card = securitykey::SoftCard::new("0006:00000002", &late, "123456").with_x25519(securitykey::x25519::x25519);
        let keys = Keys { own: vec![late.clone().strip_secret_key_material()], cards: vec![record(&card, &late, "late@example.org")], ..Keys::default() };
        let readers = securitykey::SoftReaders::with(vec![card]);
        let pin = Password::from("123456");
        let access = CardAccess::new(&readers, &pin, &|| {});
        let none = |_: &Fingerprint| None;
        let sessions = SessionKeys::new();
        let opened = decrypt(&sealed, &keys, &Unlock::new(&none, &sessions).with_card(&access)).unwrap();
        assert_eq!(opened.data, b"Older mail.");
        // But nobody can encrypt to it now.
        assert!(encrypt(b"New mail.", &[&late], None).is_err());
    }

    #[test]
    fn a_security_key_certificate_comes_back_to_your_keys() {
        let root = temporary().join("pgp-card");
        let _ = std::fs::remove_dir_all(&root);
        let on_card = card_key("me@example.org", None);
        let name = format!("{}.asc", on_card.fingerprint().to_hex());
        let kept = store_in(&root, on_card.clone().strip_secret_key_material(), true).unwrap();
        assert!(kept.own);
        let file = std::fs::read_to_string(root.join("own").join(&name)).unwrap();
        assert!(file.starts_with("-----BEGIN PGP PUBLIC KEY BLOCK-----"), "public only: {file}");
        // Brought again by an Autocrypt header: refused, never with others'
        // (whoever sends your own key back cannot add to it: F1); by a
        // lookup: merged with yours, nothing it names added.
        let header = autocrypt_header(&on_card, "me@example.org").unwrap();
        assert!(learn_autocrypt_at(&root, &header, "me@example.org").is_none());
        assert!(learn_at(&root, &on_card.armored().to_vec().unwrap(), "me@example.org").iter().all(|k| k.own));
        assert!(!root.join("others").join(&name).exists());
        // Someone else's goes with others'.
        let jane = key("jane@example.org").strip_secret_key_material();
        assert!(!store_at(&root, jane).unwrap().own);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Review of 19 October 2026, P8 (F8): a message whose sender used
    /// protected headers ("..." outside, the real subject inside), opened as
    /// Sioul opened such a message before, then sealed again: its real subject
    /// stays inside, and opens again with the rest.
    #[test]
    fn a_sealed_copy_keeps_the_protected_headers_inside() {
        let me = key("me@example.org");
        let inner = "Content-Type: multipart/mixed; boundary=\"b\"; protected-headers=\"v1\"\r\nSubject: Lease renewal for flat 4B\r\nFrom: Jane <jane@example.org>\r\nTo: Me <me@example.org>\r\nMessage-ID: <ph@example.org>\r\n\r\n--b\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nThe renewal.\r\n--b--\r\n";
        let outer = "From: Jane <jane@example.org>\r\nTo: Me <me@example.org>\r\nSubject: ...\r\nMessage-ID: <ph@example.org>\r\n";
        let received = format!(
            "{outer}MIME-Version: 1.0\r\nContent-Type: multipart/encrypted; protocol=\"application/pgp-encrypted\"; boundary=\"o\"\r\n\r\n--o\r\nContent-Type: application/pgp-encrypted\r\n\r\nVersion: 1\r\n\r\n--o\r\nContent-Type: application/octet-stream\r\n\r\n{}\r\n--o--\r\n",
            encrypt(inner.as_bytes(), &[&me], None).unwrap()
        );
        let keys = Keys { own: vec![me.clone()], ..Keys::default() };
        let sessions = SessionKeys::new();
        let passphrase = |_: &Fingerprint| Some("secret words".to_string());
        let copy = open(received.as_bytes(), &keys, &Unlock::new(&passphrase, &sessions)).unwrap().0.unwrap();
        assert!(looks_opened(&copy), "as Sioul wrote it before");
        let sealed = seal_message(&copy, &[&me]).unwrap();
        let head = String::from_utf8_lossy(&sealed[..header_end(&sealed)]).to_string();
        assert!(!head.contains("Lease renewal") && head.contains("Subject: ..."), "{head}");
        let again = open(&sealed, &keys, &Unlock::new(&passphrase, &sessions)).unwrap().0.unwrap();
        assert!(String::from_utf8_lossy(&again).contains("Subject: Lease renewal for flat 4B"));
        // An expired encryption key of yours, not revoked, still takes a seal for you alone (F19).
        let late = CertBuilder::general_purpose(Some("Me <late@example.org>")).set_cipher_suite(CipherSuite::Cv25519).set_creation_time(std::time::SystemTime::now() - std::time::Duration::from_secs(400 * 24 * 3600)).set_validity_period(std::time::Duration::from_secs(365 * 24 * 3600)).generate().unwrap().0;
        assert!(encrypt(b"x", &[&late], None).is_err(), "nobody encrypts to it");
        assert!(seal_message(&copy, &[&late]).is_ok());
        assert_eq!(Keys { own: vec![late], ..Keys::default() }.own_certs().len(), 1);
    }

    /// A header carrying every packet of `cert`, as anyone may write one.
    fn header_of(cert: &Cert, addr: &str) -> String {
        let mut bytes = Vec::new();
        cert.serialize(&mut bytes).unwrap();
        format!("addr={addr}; keydata={}", crate::lines::base64_encode(&bytes))
    }

    /// Review of 19 October 2026, P1 (F1): a sender whose domain vouches for
    /// him sends, in his Autocrypt header, your own primary key with two user
    /// IDs bound by nothing, his and Jane's. It was kept as yours: Jane's
    /// address became yours, and a message to her went to you alone, on every
    /// device through your security key's shared file. Now refused: your
    /// certificate gains nothing, your addresses are your accounts', Jane's
    /// key stays hers.
    #[test]
    fn an_autocrypt_header_never_makes_another_address_yours() {
        let root = temporary().join("pgp-p1");
        let _ = std::fs::remove_dir_all(&root);
        let me = card_key("me@example.org", None);
        store_in(&root, me.clone().strip_secret_key_material(), true).unwrap();
        let jane = key("jane@example.org");
        assert!(learn_autocrypt_at(&root, &autocrypt_header(&jane, "jane@example.org").unwrap(), "jane@example.org").is_some());
        let load = || Keys { addresses: vec!["me@example.org".into()], ..Keys::load_at(&root) };
        assert_eq!(load().for_address("jane@example.org").map(Cert::fingerprint), Some(jane.fingerprint()));
        let forged = Cert::from_packets(vec![openpgp::Packet::from(me.primary_key().key().clone()), openpgp::packet::UserID::from("Mallory <mallory@example.net>").into(), openpgp::packet::UserID::from("Jane <jane@example.org>").into()].into_iter()).unwrap();
        assert!(!names(&forged, "jane@example.org") && !names(&forged, "mallory@example.net"), "unbound user IDs name nothing");
        assert!(learn_autocrypt_at(&root, &header_of(&forged, "mallory@example.net"), "mallory@example.net").is_none());
        let keys = load();
        assert!(!keys.is_own_address("jane@example.org"));
        assert!(keys.is_own_address("me@example.org"));
        assert_eq!(keys.for_address("jane@example.org").map(Cert::fingerprint), Some(jane.fingerprint()));
        assert_eq!(keys.own.iter().find(|c| c.fingerprint() == me.fingerprint()).unwrap().userids().count(), 1, "nothing added to yours");
        // A message to Jane: her key opens it.
        let sealed = encrypt(b"Hello Jane.", &[keys.for_address("jane@example.org").unwrap()], None).unwrap();
        let sessions = SessionKeys::new();
        let passphrase = |_: &Fingerprint| Some("secret words".to_string());
        assert!(decrypt(sealed.as_bytes(), &Keys { own: vec![jane.clone()], ..Keys::default() }, &Unlock::new(&passphrase, &sessions)).is_ok());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Review of 19 October 2026, P2 (F2): a verified sender's own key with a
    /// user ID of Jane's he signed himself, in his own header: kept for his
    /// address alone, never Jane's key, whatever order the folder lists keys
    /// in. Two keys of Jane's: the newer. A lookup keeps the user ID asked for
    /// alone; one of yours that it brings gains no user ID.
    #[test]
    fn a_senders_key_naming_janes_address_never_becomes_janes() {
        let root = temporary().join("pgp-p2");
        let _ = std::fs::remove_dir_all(&root);
        let mallory = CertBuilder::new().add_userid("Mallory <mallory@example.net>").add_userid("Jane <jane@example.org>").set_cipher_suite(CipherSuite::Cv25519).add_transport_encryption_subkey().generate().unwrap().0;
        assert!(names(&mallory, "jane@example.org"), "bound by Mallory himself");
        let kept = learn_autocrypt_at(&root, &header_of(&mallory, "mallory@example.net"), "mallory@example.net").unwrap();
        assert_eq!(kept.names, vec!["Mallory <mallory@example.net>".to_string()]);
        let keys = Keys::load_at(&root);
        assert!(keys.for_address("jane@example.org").is_none(), "Jane's key unknown: none, never Mallory's");
        assert_eq!(keys.for_address("mallory@example.net").map(Cert::fingerprint), Some(mallory.fingerprint()));
        // Jane's key, an older one and her new one: the new one, whichever file comes first.
        let year = std::time::Duration::from_secs(365 * 24 * 3600);
        let old = CertBuilder::general_purpose(Some("Jane <jane@example.org>")).set_cipher_suite(CipherSuite::Cv25519).set_creation_time(std::time::SystemTime::now() - year).generate().unwrap().0;
        let new = key("jane@example.org");
        for jane in [&new, &old] {
            assert!(learn_autocrypt_at(&root, &autocrypt_header(jane, "jane@example.org").unwrap(), "jane@example.org").is_some());
        }
        assert_eq!(Keys::load_at(&root).for_address("jane@example.org").map(Cert::fingerprint), Some(new.fingerprint()));
        // A lookup for Bob keeps Bob's user ID alone.
        let bob = CertBuilder::new().add_userid("Bob <bob@example.org>").add_userid("Robert <robert@example.net>").set_cipher_suite(CipherSuite::Cv25519).add_transport_encryption_subkey().generate().unwrap().0;
        let found = learn_at(&root, &bob.armored().to_vec().unwrap(), "bob@example.org");
        assert_eq!(found.iter().map(|k| k.names.clone()).collect::<Vec<_>>(), vec![vec!["Bob <bob@example.org>".to_string()]]);
        assert!(Keys::load_at(&root).for_address("robert@example.net").is_none());
        // One of yours, a user ID of yours added elsewhere: a lookup for that
        // address merges what your primary binds, adds no user ID.
        let me = key("me@example.org");
        store_at(&root, me.clone()).unwrap();
        let mut signer = me.primary_key().key().clone().parts_into_secret().unwrap().decrypt_secret(&Password::from("secret words")).unwrap().into_keypair().unwrap();
        let other = openpgp::packet::UserID::from("Me <other@example.net>");
        let binding = other.bind(&mut signer, &me, openpgp::packet::signature::SignatureBuilder::new(openpgp::types::SignatureType::PositiveCertification)).unwrap();
        let more = me.clone().insert_packets(vec![openpgp::Packet::from(other), binding.into()]).unwrap().0.strip_secret_key_material();
        assert!(learn_at(&root, &more.armored().to_vec().unwrap(), "other@example.net").iter().all(|k| k.own));
        let mine = Keys::load_at(&root).own.into_iter().find(|c| c.fingerprint() == me.fingerprint()).unwrap();
        assert_eq!(mine.userids().map(|u| String::from_utf8_lossy(u.userid().value()).to_string()).collect::<Vec<_>>(), vec!["Someone <me@example.org>".to_string()]);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A key whose secret part is unencrypted, made `ago` before now, for
    /// the tests that sign with it (a binding, a revocation).
    fn open_key(userids: &[&str], ago: std::time::Duration) -> Cert {
        let mut builder = CertBuilder::new().set_cipher_suite(CipherSuite::Cv25519).add_transport_encryption_subkey().set_creation_time(std::time::SystemTime::now() - ago);
        for userid in userids {
            builder = builder.add_userid(*userid);
        }
        builder.generate().unwrap().0
    }

    fn signer_of(cert: &Cert) -> crypto::KeyPair {
        cert.primary_key().key().clone().parts_into_secret().unwrap().into_keypair().unwrap()
    }

    fn public(cert: &Cert) -> Vec<u8> {
        cert.clone().strip_secret_key_material().armored().to_vec().unwrap()
    }

    fn revoked(cert: &Cert) -> Cert {
        let revocation = CertRevocationBuilder::new().set_reason_for_revocation(openpgp::types::ReasonForRevocation::KeyCompromised, b"stolen").unwrap().build(&mut signer_of(cert), cert, None).unwrap();
        cert.clone().insert_packets(vec![openpgp::Packet::from(revocation)]).unwrap().0
    }

    /// Review of 19 October 2026, N2 (probe `probe2_f2_replay_and_residue`):
    /// Mallory's key with his and Jane's self-bound user IDs, as Sioul kept
    /// it before or as an older Sioul's sharing brings it, beside Jane's
    /// real key, a year old. Not known for which address it came, it serves
    /// none: never Jane's, though its key is newer. His next header heals
    /// his file to his own address, Accounts names him alone, and Jane's
    /// address never comes back to it.
    #[test]
    fn a_key_kept_before_serves_no_address_it_was_not_learned_for() {
        let root = temporary().join("pgp-n2");
        let _ = std::fs::remove_dir_all(&root);
        let jane = open_key(&["Jane <jane@example.org>"], std::time::Duration::from_secs(365 * 24 * 3600));
        learn_autocrypt_at(&root, &autocrypt_header(&jane, "jane@example.org").unwrap(), "jane@example.org").unwrap();
        let mallory = open_key(&["Mallory <mallory@example.net>", "Jane <jane@example.org>"], std::time::Duration::ZERO);
        let legacy = root.join("others").join(format!("{}.asc", mallory.fingerprint().to_hex()));
        std::fs::write(&legacy, public(&mallory)).unwrap();
        let keys = Keys::load_at(&root);
        assert_eq!(keys.for_address("jane@example.org").map(Cert::fingerprint), Some(jane.fingerprint()), "Jane's own key, never the newer one naming her");
        assert!(keys.for_address("mallory@example.net").is_none(), "which address it came for is not known: none until it comes again");
        assert!(keys.others.iter().all(|c| c.fingerprint() == jane.fingerprint() || !bound(c, "jane@example.org")), "no other key names her");
        let kept = learn_autocrypt_at(&root, &header_of(&mallory.clone().strip_secret_key_material(), "mallory@example.net"), "mallory@example.net").unwrap();
        assert_eq!(kept.names, vec!["Mallory <mallory@example.net>".to_string()]);
        assert_eq!(Cert::from_bytes(&std::fs::read(&legacy).unwrap()).unwrap().userids().count(), 1, "Jane's user ID gone from his file");
        let keys = Keys::load_at(&root);
        assert_eq!(keys.for_address("mallory@example.net").map(Cert::fingerprint), Some(mallory.fingerprint()));
        assert_eq!(keys.for_address("jane@example.org").map(Cert::fingerprint), Some(jane.fingerprint()));
        // Jane's key gone: her address has none, never his.
        remove_in(&root, &jane.fingerprint().to_hex()).unwrap();
        assert!(!root.join("others").join(format!("{}.toml", jane.fingerprint().to_hex())).exists(), "what it served goes with it");
        assert!(Keys::load_at(&root).for_address("jane@example.org").is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Review of 19 October 2026, R1 (probe
    /// `review_probe_n2_legacy_key_bound_to_one_address_it_did_not_come_for`):
    /// Mallory's key binding Jane's address and carrying his own in a user
    /// ID bound by nothing, as a Sioul before 1f9acc0 kept it from his own
    /// header (any user ID passed then). Its one bound address is not the
    /// one it came for: judged by every user ID it carries, it names two and
    /// serves none, with Jane's own key or without it, before and after the
    /// migration, which writes that down as judged; his next header, his own
    /// address bound, makes it serve his alone, learned.
    #[test]
    fn a_key_kept_before_bound_to_one_address_it_did_not_come_for_serves_none() {
        let root = temporary().join("pgp-r1");
        let _ = std::fs::remove_dir_all(&root);
        let mark = root.join("state").join("pgp-others.toml");
        let jane = open_key(&["Jane <jane@example.org>"], std::time::Duration::from_secs(365 * 24 * 3600));
        learn_autocrypt_at(&root, &autocrypt_header(&jane, "jane@example.org").unwrap(), "jane@example.org").unwrap();
        let bare = open_key(&["Jane <jane@example.org>"], std::time::Duration::ZERO);
        let mallory = bare.clone().insert_packets(vec![openpgp::Packet::from(openpgp::packet::UserID::from("Mallory <mallory@example.net>"))]).unwrap().0;
        assert!(bound(&mallory, "jane@example.org") && !bound(&mallory, "mallory@example.net"));
        assert!(autocrypt_key(&header_of(&mallory, "mallory@example.net"), "mallory@example.net").is_none(), "refused today");
        std::fs::write(root.join("others").join(format!("{}.asc", mallory.fingerprint().to_hex())), public(&mallory)).unwrap();
        let (jane_fp, mallory_fp) = (Some(jane.fingerprint()), Some(mallory.fingerprint()));
        assert_eq!(Keys::load_at(&root).for_address("jane@example.org").map(Cert::fingerprint), jane_fp, "never the newer key of his");
        assert_eq!(keep_others_for_their_addresses_at(&root, &mark).unwrap(), 1, "Jane's was learned: its file is there");
        let said = record_of(&root.join("others"), &mallory).unwrap();
        assert!(said.addresses.is_empty() && said.judged.is_empty());
        assert_eq!(Keys::load_at(&root).for_address("jane@example.org").map(Cert::fingerprint), jane_fp);
        remove_in(&root, &jane.fingerprint().to_hex()).unwrap();
        assert!(Keys::load_at(&root).for_address("jane@example.org").is_none(), "without her key, still never his");
        // His next header, his own address bound to it: his alone, learned.
        let own = openpgp::packet::UserID::from("Mallory <mallory@example.net>");
        let binding = own.bind(&mut signer_of(&bare), &bare, openpgp::packet::signature::SignatureBuilder::new(openpgp::types::SignatureType::PositiveCertification)).unwrap();
        let bound_too = bare.clone().insert_packets(vec![openpgp::Packet::from(own), binding.into()]).unwrap().0;
        assert!(learn_autocrypt_at(&root, &header_of(&bound_too, "mallory@example.net"), "mallory@example.net").is_some());
        let keys = Keys::load_at(&root);
        assert_eq!(keys.for_address("mallory@example.net").map(Cert::fingerprint), mallory_fp);
        assert!(keys.for_address("jane@example.org").is_none());
        let said = record_of(&root.join("others"), &mallory).unwrap();
        assert_eq!((said.addresses, said.judged), (vec!["mallory@example.net".to_string()], Vec::<String>::new()));
        // A key kept before naming one address in every user ID, bound: judged to serve it.
        let bob = open_key(&["Bob <bob@example.org>", "Robert <bob@example.org>"], std::time::Duration::ZERO);
        assert_eq!(judged(&bob), vec!["bob@example.org".to_string()]);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// N2's migration and what follows: once on each device, each key kept
    /// without what it serves gets it as judged now (Bob's one address;
    /// Carol's two, her own, none until she writes). An older Sioul's later
    /// write to Bob's key, a user ID of Jane's he bound merged in, adds no
    /// address it serves; its new key naming two addresses serves neither.
    /// Carol writing from work, then from home, makes her key serve one,
    /// then both; a key imported from a file serves every address bound to
    /// it.
    #[test]
    fn others_keys_say_what_they_serve_and_an_older_write_adds_none() {
        let root = temporary().join("pgp-n2-migration");
        let _ = std::fs::remove_dir_all(&root);
        let mark = root.join("state").join("pgp-others.toml");
        let others = root.join("others");
        std::fs::create_dir_all(&others).unwrap();
        let file = |cert: &Cert, end: &str| others.join(format!("{}.{end}", cert.fingerprint().to_hex()));
        let bob = open_key(&["Bob <bob@example.org>"], std::time::Duration::ZERO);
        let carol = open_key(&["Carol <carol@work.example>", "Carol <carol@home.example>"], std::time::Duration::ZERO);
        for cert in [&bob, &carol] {
            std::fs::write(file(cert, "asc"), public(cert)).unwrap();
        }
        assert_eq!(keep_others_for_their_addresses_at(&root, &mark).unwrap(), 2);
        assert_eq!(keep_others_for_their_addresses_at(&root, &mark).unwrap(), 0, "once");
        let said = |cert: &Cert| toml::from_str::<Served>(&std::fs::read_to_string(file(cert, "toml")).unwrap()).unwrap();
        assert_eq!((said(&bob).format, said(&bob).addresses), (2, vec!["bob@example.org".to_string()]));
        assert!(said(&carol).addresses.is_empty());
        // An older Sioul merges into Bob's file a user ID of Jane's he bound himself.
        let jane_id = openpgp::packet::UserID::from("Bob <jane@example.org>");
        let binding = jane_id.bind(&mut signer_of(&bob), &bob, openpgp::packet::signature::SignatureBuilder::new(openpgp::types::SignatureType::PositiveCertification)).unwrap();
        let more = bob.clone().insert_packets(vec![openpgp::Packet::from(jane_id), binding.into()]).unwrap().0;
        assert!(bound(&more, "jane@example.org"));
        std::fs::write(file(&bob, "asc"), public(&more)).unwrap();
        // And writes a new key naming two addresses, with nothing beside it.
        let dave = open_key(&["Dave <dave@example.org>", "Dave <jane@example.org>"], std::time::Duration::ZERO);
        std::fs::write(file(&dave, "asc"), public(&dave)).unwrap();
        let keys = Keys::load_at(&root);
        assert_eq!(keys.for_address("bob@example.org").map(Cert::fingerprint), Some(bob.fingerprint()));
        assert!(keys.for_address("jane@example.org").is_none(), "neither Bob's nor Dave's");
        assert!(keys.for_address("dave@example.org").is_none());
        assert!(keys.for_address("carol@work.example").is_none() && keys.for_address("carol@home.example").is_none());
        // Carol writes from work, then from home.
        assert!(learn_autocrypt_at(&root, &header_of(&carol.clone().strip_secret_key_material(), "carol@work.example"), "carol@work.example").is_some());
        let keys = Keys::load_at(&root);
        assert_eq!(keys.for_address("carol@work.example").map(Cert::fingerprint), Some(carol.fingerprint()));
        assert!(keys.for_address("carol@home.example").is_none());
        assert!(learn_autocrypt_at(&root, &header_of(&carol.clone().strip_secret_key_material(), "carol@home.example"), "carol@home.example").is_some());
        let keys = Keys::load_at(&root);
        assert!(keys.for_address("carol@work.example").is_some() && keys.for_address("carol@home.example").is_some(), "learned for both: both");
        // Imported from a file you chose: every address bound to it.
        let erin = open_key(&["Erin <erin@work.example>", "Erin <erin@home.example>"], std::time::Duration::ZERO);
        import_at(&root, &public(&erin)).unwrap();
        let keys = Keys::load_at(&root);
        assert!(keys.for_address("erin@work.example").is_some() && keys.for_address("erin@home.example").is_some());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Review of 19 October 2026, N3 (probe
    /// `probe2_revoked_user_id_and_certificate`): a certificate revoked as a
    /// whole never serves. From a header it is kept, so that its revocation
    /// stops the copy kept, and never chosen; a revocation learnt later by a
    /// lookup stops a key that served; of two keys of Jane's, the newer
    /// revoked as compromised, the older live one serves; yours revoked
    /// takes no seal and no copy of your own, and signs nothing; nothing is
    /// ever encrypted to it; what was encrypted to it before still opens.
    #[test]
    fn a_revoked_certificate_never_serves() {
        let jane = open_key(&["Jane <jane@example.org>"], std::time::Duration::ZERO);
        let root = temporary().join("pgp-n3");
        let _ = std::fs::remove_dir_all(&root);
        assert!(learn_autocrypt_at(&root, &header_of(&revoked(&jane).strip_secret_key_material(), "jane@example.org"), "jane@example.org").is_some());
        assert!(Keys::load_at(&root).for_address("jane@example.org").is_none());
        assert!(encrypt(b"x", &[&revoked(&jane)], None).is_err(), "nothing is encrypted to it");
        // Kept live, then its revocation from a lookup.
        let _ = std::fs::remove_dir_all(&root);
        learn_autocrypt_at(&root, &autocrypt_header(&jane, "jane@example.org").unwrap(), "jane@example.org").unwrap();
        assert!(Keys::load_at(&root).for_address("jane@example.org").is_some());
        assert!(!learn_at(&root, &public(&revoked(&jane)), "jane@example.org").is_empty(), "the revocation kept");
        assert!(Keys::load_at(&root).for_address("jane@example.org").is_none());
        // Her older key live, her newer one revoked as compromised.
        let _ = std::fs::remove_dir_all(&root);
        let old = open_key(&["Jane <jane@example.org>"], std::time::Duration::from_secs(365 * 24 * 3600));
        learn_autocrypt_at(&root, &autocrypt_header(&old, "jane@example.org").unwrap(), "jane@example.org").unwrap();
        learn_at(&root, &public(&revoked(&jane)), "jane@example.org");
        assert_eq!(Keys::load_at(&root).for_address("jane@example.org").map(Cert::fingerprint), Some(old.fingerprint()));
        // Yours, revoked: no seal, no copy of your own, no signature.
        let me = open_key(&["Me <me@example.org>"], std::time::Duration::ZERO);
        let yours = Keys { own: vec![revoked(&me)], addresses: vec!["me@example.org".into()], ..Keys::default() };
        assert!(yours.own_certs().is_empty() && yours.self_cert_for("me@example.org").is_none() && yours.own_for("me@example.org").is_none());
        assert!(revoked_whole(&yours.own[0]) && names_address(&yours.own[0], "me@example.org"), "the window can say it");
        // A message sent to it before its revocation still opens with it.
        let sealed = encrypt(b"Before.", &[&me], None).unwrap();
        let none = |_: &Fingerprint| None;
        let sessions = SessionKeys::new();
        assert_eq!(decrypt(sealed.as_bytes(), &yours, &Unlock::new(&none, &sessions)).unwrap().data, b"Before.");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Review of 19 October 2026, R2 (probe `review_probe_n3_designated_revoker`):
    /// Jane's certificate names her backup key as its designated revoker,
    /// which revokes it as compromised. Sequoia says it "could be" revoked,
    /// and leaves the check to whoever holds the revoker's certificate: kept
    /// here, the revocation checks, and her certificate serves nothing. Not
    /// kept here, nothing checks it: it stays as it was. Yours revoked so
    /// takes no seal and no copy of your own, and signs nothing.
    #[test]
    fn a_designated_revokers_revocation_is_honoured_once_it_checks() {
        use openpgp::types::{ReasonForRevocation, RevocationKey};
        let root = temporary().join("pgp-r2");
        let _ = std::fs::remove_dir_all(&root);
        let naming = |userid: &str, revoker: &Cert| {
            CertBuilder::new()
                .set_cipher_suite(CipherSuite::Cv25519)
                .add_userid(userid)
                .add_transport_encryption_subkey()
                .add_signing_subkey()
                .set_creation_time(std::time::SystemTime::now() - std::time::Duration::from_secs(3600))
                .set_revocation_keys(vec![RevocationKey::new(revoker.primary_key().key().pk_algo(), revoker.fingerprint(), false)])
                .generate()
                .unwrap()
                .0
        };
        let revoke = |cert: &Cert, by: &Cert| {
            let revocation = CertRevocationBuilder::new().set_reason_for_revocation(ReasonForRevocation::KeyCompromised, b"stolen").unwrap().build(&mut signer_of(by), cert, None).unwrap();
            cert.clone().insert_packets(vec![openpgp::Packet::from(revocation)]).unwrap().0
        };
        let backup = open_key(&["Jane's backup <jane-backup@example.org>"], std::time::Duration::ZERO);
        let jane = naming("Jane <jane@example.org>", &backup);
        let revoked = revoke(&jane, &backup);
        assert!(usable(&revoked), "Sequoia alone: it could be");
        learn_autocrypt_at(&root, &header_of(&revoked, "jane@example.org"), "jane@example.org").unwrap();
        assert!(Keys::load_at(&root).for_address("jane@example.org").is_some(), "its revoker not kept here: nothing checks it");
        learn_autocrypt_at(&root, &header_of(&backup, "jane-backup@example.org"), "jane-backup@example.org").unwrap();
        let keys = Keys::load_at(&root);
        assert!(keys.for_address("jane@example.org").is_none(), "her revoker's revocation checks");
        let stored = keys.others.iter().find(|c| c.fingerprint() == jane.fingerprint()).unwrap();
        assert!(!keys.usable(stored));
        // A revocation made by a key it does not name changes nothing.
        let mallory = open_key(&["Mallory <mallory@example.net>"], std::time::Duration::ZERO);
        let forged = revoke(&jane, &mallory);
        assert!(Keys { others: vec![forged.clone(), mallory.clone()], ..Keys::default() }.usable(&forged));
        // Yours, revoked by your own designated revoker kept here: no seal, no copy of your own, no signature.
        let mine = open_key(&["Me backup <me-backup@example.org>"], std::time::Duration::ZERO);
        let me = revoke(&naming("Me <me@example.org>", &mine), &mine);
        let yours = Keys { own: vec![me.clone(), mine.clone()], addresses: vec!["me@example.org".into()], ..Keys::default() };
        assert!(yours.own_certs().iter().all(|c| c.fingerprint() != me.fingerprint()));
        assert!(yours.self_cert_for("me@example.org").is_none() && yours.own_for("me@example.org").is_none());
        let alone = Keys { own: vec![me.clone()], addresses: vec!["me@example.org".into()], ..Keys::default() };
        assert!(alone.self_cert_for("me@example.org").is_some() && alone.own_for("me@example.org").is_some(), "its revoker not kept: nothing checks it");
        // A message Jane signed: its signature is not taken for good once her revoker's revocation checks.
        let noa = open_key(&["Noa <noa@example.org>"], std::time::Duration::ZERO);
        let mut jane_signs = here_signer(&jane, "").unwrap();
        let signed = encrypt(b"The lease.", &[&noa], Some(&mut jane_signs)).unwrap();
        let none = |_: &Fingerprint| None;
        let sessions = SessionKeys::new();
        let reading = Keys { own: vec![noa.clone()], others: vec![revoked.clone(), backup.clone()], ..Keys::default() };
        let checked = decrypt(signed.as_bytes(), &reading, &Unlock::new(&none, &sessions)).unwrap().signatures;
        assert_eq!((checked[0].good, checked[0].problem.as_str()), (false, "revoked key"));
        let without = Keys { own: vec![noa.clone()], others: vec![revoked.clone()], ..Keys::default() };
        assert!(decrypt(signed.as_bytes(), &without, &Unlock::new(&none, &sessions)).unwrap().signatures[0].good, "its revoker unknown here");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Review of 19 October 2026, Info (probe `probe2_lookups_…`): a lookup
    /// that brings your own certificate with a subkey bound by someone
    /// else's key adds nothing of it: only what your key signed is merged.
    #[test]
    fn a_lookup_bringing_yours_adds_only_what_your_key_signed() {
        let root = temporary().join("pgp-lookup-yours");
        let _ = std::fs::remove_dir_all(&root);
        let me = open_key(&["Me <me@example.org>"], std::time::Duration::ZERO);
        store_in(&root, me.clone(), true).unwrap();
        let mallory = open_key(&["Mallory <mallory@example.net>"], std::time::Duration::ZERO);
        let stray = open_key(&[], std::time::Duration::ZERO).keys().subkeys().next().unwrap().key().clone();
        let binding = openpgp::packet::signature::SignatureBuilder::new(openpgp::types::SignatureType::SubkeyBinding)
            .set_key_flags(openpgp::types::KeyFlags::empty().set_transport_encryption())
            .unwrap()
            .sign_subkey_binding(&mut signer_of(&mallory), me.primary_key().key(), &stray)
            .unwrap();
        let mut packets: Vec<openpgp::Packet> = me.clone().strip_secret_key_material().into_packets().collect();
        packets.push(openpgp::Packet::from(stray.clone()));
        packets.push(binding.into());
        let polluted = Cert::from_packets(packets.into_iter()).unwrap();
        assert!(!learn_at(&root, &polluted.armored().to_vec().unwrap(), "me@example.org").is_empty());
        let stored = Cert::from_bytes(&std::fs::read(root.join("own").join(format!("{}.asc", me.fingerprint().to_hex()))).unwrap()).unwrap();
        assert_eq!(stored.keys().subkeys().count(), me.keys().subkeys().count(), "the stray subkey left out");
        assert!(stored.is_tsk(), "still your secret key");
        let _ = std::fs::remove_dir_all(&root);
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

    /// A key made and exported by GnuPG 2.4.4 (Ubuntu 24.04), cut to its
    /// primary key, its user ID and two of its subkeys: the second subkey's
    /// binding signature has its second number written over 32 octets, the
    /// first a zero, its length said as 256 bits.
    const GNUPG_2_4_4: &[u8] = include_bytes!("../tests/fixtures/pgp/gnupg-2.4.4-non-minimal-mpi.asc");

    /// The CI's GnuPG wrote about one key signature in 128 so: Sequoia alone
    /// drops it, and a subkey (or the whole key, when it is the user ID's)
    /// reads as missing (securitykey's test failed once in a while on CI).
    /// Read here as its value, every signature still checked by Sequoia.
    #[test]
    fn a_signature_gnupg_wrote_long_is_read_as_its_value() {
        let strict = Cert::from_bytes(GNUPG_2_4_4).unwrap();
        assert_eq!(securitykey::parts(&strict).len(), 2, "Sequoia alone: the primary key and one subkey, the other's binding dropped");
        let read = certificates_in(GNUPG_2_4_4);
        assert_eq!(read.len(), 1);
        let parts = securitykey::parts(&read[0]);
        assert_eq!(parts.len(), 3, "{parts:?}");
        assert!(parts[1..].iter().all(|part| part.role == securitykey::Role::Authentication), "{parts:?}");
        assert_eq!(securitykey::parts_in(GNUPG_2_4_4), parts);
        // Every signature there checked: the subkeys bound by the primary key.
        let policy = policy();
        let valid = read[0].with_policy(&policy, None).unwrap();
        assert_eq!(valid.keys().subkeys().count(), 2);
        // Read once, it is written shortest: nothing left to change.
        let written = read[0].to_vec().unwrap();
        assert!(matches!(minimal_mpis(&written), std::borrow::Cow::Borrowed(_)));
        assert!(matches!(minimal_mpis(GNUPG_2_4_4), std::borrow::Cow::Owned(_)));
    }

    /// The same fault made by hand on Sequoia's own key: each signature's
    /// last number written one octet longer, a zero first. Read back, the
    /// bytes are the key's own again; a key already shortest, a file that
    /// is no key and one cut short come back as they came.
    #[test]
    fn numbers_written_long_come_back_shortest() {
        let alice = key("alice@example.org").strip_secret_key_material();
        let binary = alice.to_vec().unwrap();
        let armored = String::from_utf8(alice.armored().to_vec().unwrap()).unwrap();
        assert!(matches!(minimal_mpis(&binary), std::borrow::Cow::Borrowed(_)));
        assert!(matches!(minimal_mpis(armored.as_bytes()), std::borrow::Cow::Borrowed(_)));
        let long = written_long(&binary);
        assert!(Cert::from_bytes(&long).ok().is_none_or(|strict| securitykey::parts(&strict).is_empty()), "Sequoia alone refuses what is written long");
        assert_eq!(minimal_mpis(&long).as_ref(), binary.as_slice());
        assert_eq!(certificates_in(&long), [alice]);
        // Nothing to read, or cut short: as it came.
        assert!(matches!(minimal_mpis(b"no key here"), std::borrow::Cow::Borrowed(_)));
        assert_eq!(minimal_mpis(b"no key here").as_ref(), b"no key here");
        let cut = &long[..long.len() - 5];
        assert_eq!(minimal_mpis(cut).as_ref(), cut);
        assert!(matches!(minimal_mpis(&[]), std::borrow::Cow::Borrowed(_)));
    }

    /// Where a version 4 signature body's numbers begin: past its two areas
    /// and the hash's first two octets.
    fn numbers_at(body: &[u8]) -> usize {
        let mut at = 4;
        for _ in 0..2 {
            at += 2 + usize::from(u16::from_be_bytes([body[at], body[at + 1]]));
        }
        at + 2
    }

    /// `body` (a signature's) with its last number written anew: `new` gets
    /// its length in bits and the octets from it on, and gives the length to
    /// say and the octets to write.
    fn last_number_as(body: &[u8], new: impl Fn(usize, &[u8]) -> (usize, Vec<u8>)) -> Vec<u8> {
        let mut at = numbers_at(body);
        let mut last = at;
        while at < body.len() {
            last = at;
            at += 2 + usize::from(u16::from_be_bytes([body[at], body[at + 1]])).div_ceil(8);
        }
        let bits = usize::from(u16::from_be_bytes([body[last], body[last + 1]]));
        let (said, octets) = new(bits, &body[last + 2..]);
        let mut out = body[..last].to_vec();
        out.extend_from_slice(&u16::try_from(said).unwrap().to_be_bytes());
        out.extend_from_slice(&octets);
        out
    }

    /// GnuPG 2.4.4's fault, made by hand: a number one octet longer, a zero
    /// first, its length said with it.
    fn one_octet_long(bits: usize, value: &[u8]) -> (usize, Vec<u8>) {
        let octets = bits.div_ceil(8);
        (octets * 8 + 8, std::iter::once(0).chain(value[..octets].iter().copied()).collect())
    }

    /// The packets of `binary`, each signature's body as `each` writes it.
    fn signatures_as(binary: &[u8], each: impl Fn(&[u8]) -> Vec<u8>) -> Vec<u8> {
        let mut out = Vec::new();
        let mut at = 0;
        while at < binary.len() {
            let (tag, body) = packet_at(binary, at).unwrap();
            let packet = &binary[body.clone()];
            put_packet(&mut out, tag, &if tag == 2 { each(packet) } else { packet.to_vec() });
            at = body.end;
        }
        out
    }

    /// Each signature's last number written one octet longer.
    fn written_long(binary: &[u8]) -> Vec<u8> {
        signatures_as(binary, |body| last_number_as(body, one_octet_long))
    }

    /// Binary packets armored as a public key block.
    fn armored_block(binary: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut writer = openpgp::armor::Writer::new(&mut out, openpgp::armor::Kind::PublicKey).unwrap();
        writer.write_all(binary).unwrap();
        writer.finalize().unwrap();
        out
    }

    /// Several keys in one file, each its armored block, some needing the
    /// repair and some not, in every order, words between them: every key
    /// read, each whole, never fewer than Sequoia alone reads. Until 15
    /// October 2026 a first block that needed the repair was read alone,
    /// the next ones dropped, and a later one was not repaired.
    #[test]
    fn concatenated_armored_keys_are_each_read_and_repaired() {
        let gnupg = certificates_in(GNUPG_2_4_4).remove(0);
        let bob = key("bob@example.org").strip_secret_key_material();
        let carol = key("carol@example.org").strip_secret_key_material();
        let blocks: [(&Cert, Vec<u8>); 3] = [(&gnupg, GNUPG_2_4_4.to_vec()), (&bob, bob.armored().to_vec().unwrap()), (&carol, armored_block(&written_long(&carol.to_vec().unwrap())))];
        let orders: [&[usize]; 12] = [&[0, 1], &[1, 0], &[1, 2], &[2, 1], &[0, 2], &[2, 0], &[0, 1, 2], &[0, 2, 1], &[1, 0, 2], &[1, 2, 0], &[2, 0, 1], &[2, 1, 0]];
        for order in orders {
            let mut file = Vec::new();
            for &i in order {
                file.extend_from_slice(&blocks[i].1);
                file.extend_from_slice(b"\nSomething said between the keys.\n");
            }
            let read = certificates_of(&file).unwrap();
            let fingerprints: Vec<Fingerprint> = read.iter().map(Cert::fingerprint).collect();
            let wanted: Vec<Fingerprint> = order.iter().map(|&i| blocks[i].0.fingerprint()).collect();
            assert_eq!(fingerprints, wanted, "{order:?}");
            for (cert, &i) in read.iter().zip(order) {
                assert_eq!(securitykey::parts(cert), securitykey::parts(blocks[i].0), "{order:?}: key {i} whole");
            }
            let alone: Vec<Fingerprint> = CertParser::from_bytes(&file).unwrap().filter_map(Result::ok).map(|c| c.fingerprint()).collect();
            assert!(alone.iter().all(|f| fingerprints.contains(f)), "{order:?}: never fewer than Sequoia alone");
            assert_eq!(matches!(minimal_mpis(&file), std::borrow::Cow::Owned(_)), order.contains(&0) || order.contains(&2), "{order:?}");
        }
        // Bob's alone, nothing to repair: as it came.
        assert!(matches!(minimal_mpis(&blocks[1].1), std::borrow::Cow::Borrowed(_)));
    }

    /// A key made by GnuPG 2.4.9 with a subkey that signs: its binding carries
    /// the subkey's own signature back (the back-signature) in its unhashed area.
    const SIGNING_SUBKEY: &[u8] = include_bytes!("../tests/fixtures/pgp/gnupg-2.4.9-signing-subkey.asc");

    /// A signature `body` with each signature embedded in its unhashed area
    /// written as `each` says; with `to_hashed`, moved into its hashed area.
    fn embedded_as(body: &[u8], each: impl Fn(&[u8]) -> Vec<u8>, to_hashed: bool) -> Vec<u8> {
        let hashed_end = 6 + usize::from(u16::from_be_bytes([body[4], body[5]]));
        let area_len = usize::from(u16::from_be_bytes([body[hashed_end], body[hashed_end + 1]]));
        let area = &body[hashed_end + 2..hashed_end + 2 + area_len];
        let (mut kept, mut embedded) = (Vec::new(), Vec::new());
        let mut at = 0;
        while at < area.len() {
            // These subpackets are short: one octet of length.
            let length = usize::from(area[at]);
            assert!(length < 192);
            let (kind, inner) = (area[at + 1], &area[at + 2..at + 1 + length]);
            if kind & 0x7F == 32 {
                let written = each(inner);
                put_subpacket_length(&mut embedded, 1 + written.len());
                embedded.push(kind);
                embedded.extend_from_slice(&written);
            } else {
                kept.extend_from_slice(&area[at..at + 1 + length]);
            }
            at += 1 + length;
        }
        assert!(!embedded.is_empty(), "a back-signature there");
        let (hashed, unhashed) = if to_hashed { ([&body[6..hashed_end], &embedded[..]].concat(), kept) } else { (body[6..hashed_end].to_vec(), [&kept[..], &embedded[..]].concat()) };
        let mut out = body[..4].to_vec();
        out.extend_from_slice(&u16::try_from(hashed.len()).unwrap().to_be_bytes());
        out.extend_from_slice(&hashed);
        out.extend_from_slice(&u16::try_from(unhashed.len()).unwrap().to_be_bytes());
        out.extend_from_slice(&unhashed);
        out.extend_from_slice(&body[hashed_end + 2 + area_len..]);
        out
    }

    /// The binding's back-signature written long, as GnuPG 2.4.4 may write
    /// any of its signatures: Sequoia alone drops the binding, and the
    /// signing subkey with it; read here, the key is whole again. The same
    /// number written long in the hashed area, which the binding signs, is
    /// left as it came.
    #[test]
    fn a_back_signature_written_long_is_read_shortest_in_the_unhashed_area_only() {
        let canonical = Cert::from_bytes(SIGNING_SUBKEY).unwrap();
        let signs = |cert: &Cert| securitykey::parts(cert).iter().any(|part| part.role == securitykey::Role::Signing);
        assert!(signs(&canonical));
        let mut binary = Vec::new();
        std::io::Read::read_to_end(&mut openpgp::armor::Reader::from_bytes(SIGNING_SUBKEY, openpgp::armor::ReaderMode::Tolerant(None)), &mut binary).unwrap();
        let binding = |to_hashed: bool| move |body: &[u8]| if body[1] == 0x18 { embedded_as(body, |inner| last_number_as(inner, one_octet_long), to_hashed) } else { body.to_vec() };
        let long = signatures_as(&binary, binding(false));
        assert!(Cert::from_bytes(&long).ok().is_none_or(|strict| !signs(&strict)), "Sequoia alone drops the signing subkey");
        assert!(matches!(minimal_mpis(&long), std::borrow::Cow::Owned(_)));
        assert_eq!(Cert::from_bytes(minimal_mpis(&long).as_ref()).unwrap(), canonical);
        let read = certificates_in(&long);
        assert_eq!(read.len(), 1);
        assert!(signs(&read[0]));
        // In the hashed area instead: never touched.
        let hashed = signatures_as(&binary, binding(true));
        assert!(matches!(minimal_mpis(&hashed), std::borrow::Cow::Borrowed(_)), "the signed area as it came");
    }

    /// Only ever shorter: a number whose length is said one bit too short
    /// (a bit past it set, which Sequoia refuses and GnuPG never writes) is
    /// left as it came, and so is its whole signature, though another of its
    /// numbers be written long.
    #[test]
    fn a_length_said_too_short_is_left_as_it_came() {
        let dave = key("dave@example.org").strip_secret_key_material().to_vec().unwrap();
        // The first number said a bit short in as many octets; none when it cannot be.
        let short_first = |body: &[u8]| -> Option<Vec<u8>> {
            let first = numbers_at(body);
            let bits = usize::from(u16::from_be_bytes([body[first], body[first + 1]]));
            if bits < 2 || (bits - 1).div_ceil(8) != bits.div_ceil(8) {
                return None;
            }
            let mut out = body.to_vec();
            out[first..first + 2].copy_from_slice(&u16::try_from(bits - 1).unwrap().to_be_bytes());
            Some(out)
        };
        let mut checked = 0;
        let mut at = 0;
        while at < dave.len() {
            let (tag, body) = packet_at(&dave, at).unwrap();
            if let Some(short) = (tag == 2).then(|| short_first(&dave[body.clone()])).flatten() {
                checked += 1;
                assert_eq!(signature_shortened(&short), None, "said short: as it came");
                assert_eq!(signature_shortened(&last_number_as(&short, one_octet_long)), None, "beside a number written long: as it came");
            }
            at = body.end;
        }
        assert!(checked > 0);
        let short_key = signatures_as(&dave, |body| short_first(body).unwrap_or_else(|| body.to_vec()));
        assert!(matches!(minimal_mpis(&short_key), std::borrow::Cow::Borrowed(_)));
    }

    /// A signature body (EdDSA) whose unhashed area holds `inner`, embedded,
    /// if any, and whose two numbers are `numbers` (their lengths said).
    fn nesting(inner: Option<&[u8]>, numbers: &[u8]) -> Vec<u8> {
        let mut area = Vec::new();
        if let Some(inner) = inner {
            put_subpacket_length(&mut area, 1 + inner.len());
            area.push(32);
            area.extend_from_slice(inner);
        }
        let mut body = vec![4, 0x19, 22, 10, 0, 0];
        body.extend_from_slice(&u16::try_from(area.len()).unwrap().to_be_bytes());
        body.extend_from_slice(&area);
        body.extend_from_slice(&[0xAB, 0xCD]);
        body.extend_from_slice(numbers);
        body
    }

    /// Signatures embedded in embedded signatures, thousands deep (made by
    /// hand: no OpenPGP program writes them, Sequoia refuses them): looked
    /// into one level only, so the file comes back at once, as it came, on
    /// a thread with a small stack (a phone's import runs on Qt's, about
    /// 1 MiB), though the deepest's numbers are written long. One level
    /// down, a signature written long is still repaired (a back-signature);
    /// two levels down, never.
    #[test]
    fn signatures_nested_deep_are_looked_into_one_level_only() {
        // r and s, each the number 1: shortest, then one said in nine bits, a zero octet first.
        let shortest: &[u8] = &[0, 1, 1, 0, 1, 1];
        let long: &[u8] = &[0, 9, 0, 1, 0, 1, 1];
        let packet = |body: &[u8]| {
            let mut out = Vec::new();
            put_packet(&mut out, 2, body);
            out
        };
        let mut deep = nesting(None, long);
        let mut levels = 0;
        while deep.len() < 60_000 {
            deep = nesting(Some(&deep), shortest);
            levels += 1;
        }
        assert!(levels > 2_000, "{levels} levels");
        let file = packet(&nesting(Some(&deep), shortest));
        let read = std::thread::Builder::new().stack_size(256 * 1024).spawn(move || matches!(minimal_mpis(&file), std::borrow::Cow::Borrowed(_))).unwrap();
        assert!(read.join().unwrap(), "as it came, at once");
        // One level down: repaired; two levels down: as it came.
        let one = packet(&nesting(Some(&nesting(None, long)), shortest));
        assert!(matches!(minimal_mpis(&one), std::borrow::Cow::Owned(_)));
        let two = packet(&nesting(Some(&nesting(Some(&nesting(None, long)), shortest)), shortest));
        assert!(matches!(minimal_mpis(&two), std::borrow::Cow::Borrowed(_)));
    }
}
