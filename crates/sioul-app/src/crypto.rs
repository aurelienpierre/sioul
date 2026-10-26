// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! OpenPGP behind the window: protected messages opened for the reader, the
//! keys Autocrypt brings, what a draft can be (signed, encrypted), and your
//! keys: made, imported, exported, removed. Passphrases stay in the keyring;
//! decrypted messages stay in memory.

use crate::backend::{json, load_config, say, tr};
use sioul_core::compose::{Draft, DraftKind};
use sioul_core::draftcopies;
use sioul_core::pgp::{self, Keys, PgpView};
use sioul_core::securitykey;
use sioul_core::porch::Triaged;
use sioul_core::trust::Trust;
use sioul_sync::{SyncError, keys, secret};
use std::path::{Path, PathBuf};

/// Whether a message might be protected: cheap, before any key is read.
fn looks_protected(raw: &[u8]) -> bool {
    let text = String::from_utf8_lossy(&raw[..raw.len().min(512 * 1024)]);
    text.contains("application/pgp-encrypted") || text.contains("application/pgp-signature") || text.contains("-----BEGIN PGP MESSAGE-----")
}

/// A protected message opened with your keys: what it reads once decrypted,
/// and what was found; None when it is not protected. Your security key is
/// never asked here: a message encrypted to it opens from its session key,
/// kept since you opened it with the key, or waits for "Open with your
/// security key".
pub(crate) fn open(raw: &[u8]) -> Option<(Option<Vec<u8>>, PgpView)> {
    if !looks_protected(raw) {
        return None;
    }
    let mut opened = pgp::open(raw, &Keys::load(), &pgp::Unlock::new(&|fingerprint| secret::pgp_passphrase(&fingerprint.to_hex()), pgp::SessionKeys::global()));
    // A phone, until it can use the key: what waits for it is said, with no button.
    if let Some((_, view)) = opened.as_mut() {
        view.key_elsewhere = !sioul_sync::securitykey::usable_here();
    }
    opened
}

/// The message as it reads: decrypted when it was encrypted to you.
pub(crate) fn readable(path: &Path) -> Option<Vec<u8>> {
    let raw = std::fs::read(path).ok()?;
    Some(open(&raw).and_then(|(opened, _)| opened).unwrap_or(raw))
}

/// Keeps the key a sender's Autocrypt header carries, only when the sender's
/// domain vouches for the message (the Porch's judgement): an unproven one
/// could plant a key for someone you write to, and read your answers.
pub(crate) fn learn_autocrypt(raw: &[u8], triaged: &Triaged) {
    if triaged.trust != Trust::Verified {
        return;
    }
    let Some(from) = triaged.card.from_address.as_deref() else { return };
    if let Some(header) = triaged.card.headers.first("Autocrypt") {
        let _ = raw;
        pgp::learn_autocrypt(header, from);
    }
}

/// What a draft can be, for the writing window's switches.
#[derive(serde::Serialize, Default)]
pub(crate) struct DraftProtection {
    pub can_sign: bool,
    pub can_encrypt: bool,
    /// Recipients without a known key: "Encrypt" waits for them.
    pub missing: Vec<String>,
    /// The security key that signs for the address has an expired
    /// certificate: when it expired, and how to renew it; else "".
    pub expired: String,
    /// This device cannot use the security key that signs for the address
    /// (a phone, until it can): why "Sign" is off; else "".
    pub sign_not_here: String,
    /// Encrypted, your own certificate cannot take it (expired): you would
    /// not read your sent copy; said before sending; else "".
    pub self_copy: String,
    /// The copy of the message answered or forwarded is encrypted (it came
    /// so, or the look sealed it): what becomes of it
    /// when "Encrypt" is unticked (a reply goes without its quote, a forward
    /// not at all); said only then; else "".
    pub sealed_quote: String,
    /// Bcc recipients, the message encrypted: every recipient can see which
    /// keys it was encrypted to, theirs too (review of 19 October 2026, F13:
    /// said before Send; a message of their own for each, later); else "".
    pub bcc_seen: String,
}

/// The message a draft answers or forwards, as this device can read it. Its
/// copy beside the draft stays as it came: encrypted when it came so.
pub(crate) enum Answered {
    /// None, or not encrypted.
    Clear,
    /// Encrypted, and it opens here now (a key here, or the session key kept
    /// since you opened it): what it reads, in memory only.
    Opens(Vec<u8>),
    /// Encrypted to your security key, which this device can use: opened at Send.
    AtSend,
    /// Encrypted to your security key, which this device cannot use yet (a
    /// phone): it opens on a device that can.
    Elsewhere,
    /// Encrypted, and no key of yours known here opens it (to a key you no
    /// longer have, to one kept on another device alone, or to others
    /// alone): never quoted nor forwarded from here (F16).
    NoKey,
}

/// The copy beside `draft`, read whole (`filelock::read_whole`: never the
/// zeros written over a copy replaced meanwhile); none when there is none.
pub(crate) fn copy_of(draft: &Draft) -> Option<Vec<u8>> {
    draft.original_here().and_then(|path| sioul_core::filelock::read_whole(&path).ok())
}

/// How the copy beside a draft reads here (`raw`: as [`copy_of`] read it).
pub(crate) fn answered(raw: Option<&[u8]>) -> Answered {
    let Some(raw) = raw.filter(|raw| pgp::is_encrypted(raw)) else { return Answered::Clear };
    match open(raw) {
        Some((Some(opened), _)) => Answered::Opens(opened),
        Some((None, view)) if view.security_key.is_some() && !view.key_elsewhere => Answered::AtSend,
        Some((None, view)) if view.security_key.is_some() => Answered::Elsewhere,
        _ => Answered::NoKey,
    }
}

pub(crate) fn draft_protection(draft: &Draft) -> DraftProtection {
    let config = load_config();
    let Some(address) = config.account(&draft.account).and_then(|a| a.address.clone()) else { return DraftProtection::default() };
    protection_with(&Keys::load(), &address, draft, sioul_sync::securitykey::usable_here())
}

/// [`draft_protection`] with `keys`, for `address`; `card_here`: whether this
/// device can use a security key (a phone cannot yet).
fn protection_with(keys: &Keys, address: &str, draft: &Draft, card_here: bool) -> DraftProtection {
    let address = address.to_string();
    let missing: Vec<String> = draft.recipients().into_iter().filter(|r| keys.for_address(r).is_none()).collect();
    let signs = keys.own_for(&address);
    let can_sign = signs.is_some();
    let expired = if can_sign { String::new() } else { crate::securitykey::expired_line(keys, &address).unwrap_or_default() };
    // Your security key signs for the address, and this device cannot use it
    // yet (a phone); or it was read on another computer of yours, and a
    // computer signs with one it read itself (review of 19 October 2026, F18).
    let card_signs = signs.as_ref().is_some_and(|own| matches!(own.place, pgp::Place::Card { .. }));
    let shared_serves = !can_sign && keys.card_for(&address).is_none() && keys.serving_card(&address).is_some();
    let sign_not_here = match (card_signs || shared_serves, card_here) {
        (false, _) => String::new(),
        (true, false) => tr().text("seckey-sign-not-here", None),
        (true, true) if shared_serves => tr().text(crate::securitykey::read_here_words("seckey-sign-read-here"), None),
        (true, true) => String::new(),
    };
    // Encrypted without a copy for you: why, said whatever it is (F14).
    let self_copy = if keys.self_cert_for(&address).is_none() { self_copy_line(keys, &address, card_here) } else { String::new() };
    // An encrypted message is never quoted nor forwarded in clear.
    let sealed = copy_of(draft).is_some_and(|raw| pgp::is_encrypted(&raw));
    let sealed_quote = match (sealed, draft.kind) {
        (false, _) | (_, DraftKind::New) => String::new(),
        (true, DraftKind::Forward) => tr().text("pgp-forward-needs-encryption", None),
        (true, _) => tr().text("pgp-quote-left-out", None),
    };
    let bcc_seen = if draft.bcc.iter().any(|b| !b.trim().is_empty()) { tr().text("pgp-bcc-seen", None) } else { String::new() };
    DraftProtection { can_sign: can_sign && (card_here || !card_signs), can_encrypt: !draft.recipients().is_empty() && missing.is_empty(), missing, expired, sign_not_here, self_copy, sealed_quote, bcc_seen }
}

/// Why an encrypted message would go without a copy for you, for
/// `address`: the part of your certificate that expired or was revoked
/// (its encryption subkey, its primary key), how to renew it (on a phone:
/// on a computer); or that no certificate of yours for it is known here
/// (a phone whose settings do not come through the sharing). Review of 19
/// October 2026, F14: it said the whole certificate had expired whatever
/// part had, and said nothing when none was known.
fn self_copy_line(keys: &Keys, address: &str, computer: bool) -> String {
    let now = jiff::Timestamp::now().as_second();
    let card = keys.serving_card(address).map(|(_, cert)| cert);
    let cert = card.or_else(|| keys.own.iter().find(|c| pgp::names_address(c, address)));
    let Some(cert) = cert else { return tr().text("pgp-self-copy-none", None) };
    // Revoked as a whole: nothing renews it (review of 19 October 2026, N3).
    if pgp::revoked_whole(cert) {
        return tr().text("pgp-self-copy-revoked", None);
    }
    let mut why = securitykey::expired_parts(tr(), &securitykey::parts(cert), now);
    if why.is_empty() {
        why.extend(securitykey::revoked_encryption(cert).map(|fingerprint| say("seckey-part-revoked-encrypt", &[("fingerprint", securitykey::short_fingerprint(&fingerprint))])));
    }
    if why.is_empty() {
        return tr().text("pgp-self-copy-none", None);
    }
    // What can renew it (N8): your security key's certificate, GnuPG through
    // Accounts on a computer; a key kept here, only the program that made it,
    // or a new key.
    let renew = match (card.is_some(), computer) {
        // Known here from another computer only: Accounts renews a key read here (review of 19 October 2026, Info).
        (true, true) if keys.card_for(address).is_none() => tr().text("pgp-self-copy-renew-read-here", None),
        (true, true) => tr().text("pgp-self-copy-renew", None),
        (true, false) => tr().text("pgp-self-copy-renew-phone", None),
        (false, _) => tr().text("pgp-self-copy-new-key", None),
    };
    say("pgp-self-copy-why", &[("why", why.join(" ")), ("renew", renew)])
}

/// Answering an encrypted message: the answer is encrypted when every key is
/// known. The copy kept with the draft stays as it came, encrypted: it opens
/// only at Send, in memory (`sioul_sync::send::build_draft`), never in clear
/// on a disk, nor in the sharing, which carries drafts to every device. Until
/// 19 October 2026 its decrypted text was written there (`seal_clear_drafts`
/// seals those copies again).
pub(crate) fn protect_answer(draft: &mut Draft) {
    if copy_of(draft).is_some_and(|raw| pgp::is_encrypted(&raw)) {
        draft.encrypt = draft_protection(draft).can_encrypt;
    }
}

/// The copies beside drafts looked at (`draftcopies`), their verdicts read
/// from the disk once: one look at a time in this process.
static JUDGED: std::sync::Mutex<Option<draftcopies::Judged>> = std::sync::Mutex::new(None);

/// Where the look's verdicts are kept between processes, never shared.
fn verdicts_path() -> PathBuf {
    sioul_core::config::state_dir().join("draftcopies.json")
}

/// The copies beside drafts that hold an encrypted message's text in clear,
/// their earlier versions kept here and the copies kept before sharing
/// began (`share/before-sharing-<day>/`), encrypted again to your own
/// certificates (`sioul_core::draftcopies`): at the window's start, before
/// each of its exchanges (never holding the exchanges' `BUSY`: the look takes
/// the exchange lock in short holds of its own), and at a phone's background
/// step; never on a dose's alarm, nor at "Taken" (review of 19 October
/// 2026, F9). Cheap once done: its verdicts are kept on the disk, a file is
/// read again only once it changed, and your keys are read only when a copy
/// is to be sealed. Said in counts only, never what a copy says.
pub(crate) fn seal_clear_drafts() {
    // Others' keys kept before say once what they serve (review of 19 October 2026, N2).
    match pgp::keep_others_for_their_addresses() {
        Ok(0) => {}
        Ok(n) => eprintln!("sioul: keys: {n} kept before now say which address they serve"),
        Err(e) => eprintln!("sioul: keys: what others' keys serve not written: {e}"),
    }
    let mut judged = JUDGED.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let judged = judged.get_or_insert_with(|| draftcopies::Judged::load(&verdicts_path()));
    seal_with(judged);
    if let Err(e) = judged.save(&verdicts_path()) {
        eprintln!("sioul: drafts: the look's verdicts not kept: {}", e.kind());
    }
}

/// The copies' earlier versions kept here (`history`), and those kept before
/// sharing began (`share/before-sharing-<day>/`), beside the sharing's `memory`.
fn kept_versions(memory: &Path) -> Vec<PathBuf> {
    let mut kept = sioul_sync::history::draft_copies(&sioul_sync::history::root(memory));
    kept.extend(sioul_sync::share::draft_copies_kept_before_sharing(memory));
    kept
}

fn seal_with(judged: &mut draftcopies::Judged) {
    let config = load_config();
    let roots: Vec<PathBuf> = config.accounts.iter().filter(|a| a.kind == sioul_core::config::AccountKind::Imap).map(|a| a.maildir_path()).collect();
    let cache = sioul_core::config::cache_dir().join("mail-index.tsv");
    let done = seal_in(&Draft::folder(), &crate::share::memory_path(), &draftcopies::Certs::lazy(Keys::load), &roots, &cache, judged);
    if done.any() {
        eprintln!("{}", said(&done));
    }
}

/// [`seal_clear_drafts`] in these folders: the drafts', the sharing's memory
/// (its history and what it kept before sharing began beside it), your mail
/// (`roots`, its index of Message-IDs kept in `cache`). Each copy takes its
/// place under the sharing's exchange lock, a short hold at a time
/// (`share::replacements_hold`): no exchange reads one while it is replaced.
fn seal_in(drafts: &Path, memory: &Path, certs: &draftcopies::Certs, roots: &[PathBuf], cache: &Path, judged: &mut draftcopies::Judged) -> draftcopies::Sealed {
    // Your mail by Message-ID, read only when a copy in clear asks it.
    let mut index: Option<sioul_core::mailindex::MailIndex> = None;
    // Every file of a Message-ID: one message in two folders, or another given its ID (F6).
    let mut find = |id: &str| index.get_or_insert_with(|| sioul_core::mailindex::MailIndex::build(roots, cache)).paths(id);
    let hold = sioul_sync::share::replacements_hold(memory);
    let beside = draftcopies::seal_beside(drafts, certs, &mut find, judged, &hold);
    // Their earlier versions kept here, and what was kept before sharing began.
    let done = beside.and(draftcopies::seal_versions(&kept_versions(memory), certs, &mut find, judged, &hold));
    // What waits for a certificate of yours, said in Accounts ▸ Encryption (F19).
    judged.set_waiting(done.waiting);
    done
}

/// What a look did, for the log: counts only, never what a copy says.
fn said(done: &draftcopies::Sealed) -> String {
    format!(
        "sioul: drafts: {} copies encrypted again, {} earlier versions, {} not written, {} waiting for a key, {} without their draft taken out",
        done.copies, done.versions, done.failed, done.waiting, done.orphans
    )
}

#[derive(serde::Serialize)]
struct KeysView {
    keys: Vec<pgp::KeySummary>,
    /// Your mail addresses without a key of yours: "Make a key" is offered for each.
    without_key: Vec<String>,
    /// Copies kept beside drafts (and their earlier versions) waiting for a
    /// certificate of yours to be encrypted again: one quiet line, nothing to
    /// press (F19); "" when none waits.
    copies_waiting: String,
    /// On a computer, your addresses a security key of yours serves that
    /// another computer read: a line each, and "Use a security key", rather
    /// than "Make a key for" (review of 19 October 2026, N4).
    read_here: Vec<String>,
}

/// Your addresses Accounts offers "Make a key for", and those it offers to
/// read your security key for here instead: an address a security key of
/// yours serves has its key, read here or on another device (N4: since a
/// device signs only with the security key it read, `card_for` alone left
/// the phone, and a computer that did not read it, offering a second key).
/// Read here is offered on a computer only (a phone cannot read one yet),
/// also where a key kept here signs for the address meanwhile: read here,
/// the security key signs, and Autocrypt hands out the certificate that
/// signs.
fn offers(keys: &Keys, addresses: &[String], card_here: bool) -> (Vec<String>, Vec<String>) {
    let (mut make, mut read) = (Vec::new(), Vec::new());
    for address in addresses {
        if keys.card_for(address).is_some() {
            continue;
        }
        if keys.serving_card(address).is_some() {
            if card_here {
                read.push(say(crate::securitykey::read_here_words("pgp-read-card-here"), &[("address", address.clone())]));
            }
            continue;
        }
        if keys.own_for(address).is_none() {
            make.push(address.clone());
        }
    }
    (make, read)
}

/// Your keys and others', and the addresses that could have one.
pub(crate) fn keys_view() -> String {
    let keys = Keys::load();
    let config = load_config();
    // An address your security key serves has its key, even expired, wherever it was read.
    let addresses: Vec<String> = config.accounts.iter().filter(|a| a.syncs()).filter_map(|a| a.address.clone()).collect();
    let (without_key, read_here) = offers(&keys, &addresses, sioul_sync::securitykey::usable_here());
    // A look running holds its verdicts: what the last one kept on the disk, rather than wait for it.
    let waiting = JUDGED.try_lock().ok().and_then(|j| j.as_ref().map(draftcopies::Judged::waiting)).unwrap_or_else(|| draftcopies::Judged::load(&verdicts_path()).waiting());
    let copies_waiting = if waiting == 0 { String::new() } else { tr().text("pgp-copies-waiting", Some(&tr().counted(waiting))) };
    json(&KeysView { keys: pgp::summaries(), without_key, copies_waiting, read_here })
}

/// Makes a key for one of your addresses; its passphrase, random, goes to the keyring.
pub(crate) fn generate(address: &str) -> Result<String, String> {
    let config = load_config();
    let name = config.accounts.iter().find(|a| a.address.as_deref() == Some(address)).and_then(|a| a.name.clone()).unwrap_or_default();
    let passphrase = match secret::test_passphrase() {
        Some(fixed) => fixed,
        None => pgp::new_passphrase()?,
    };
    let made = pgp::generate(&name, address, &passphrase)?;
    secret::save_pgp_passphrase(&made.fingerprint, &passphrase).map_err(|e| e.sentence(tr(), address))?;
    Ok(say("pgp-made", &[("address", address.to_string())]))
}

/// Imports the keys of a file (a `file://` address); a secret key's passphrase
/// is checked, then kept in the keyring.
pub(crate) fn import(url: &str, passphrase: &str) -> Result<String, String> {
    let path = crate::backend::local_path(url);
    let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    // Your keys before: one imported again with a mistyped passphrase stays yours.
    let had: Vec<String> = pgp::summaries().into_iter().filter(|k| k.own).map(|k| k.fingerprint).collect();
    let imported = pgp::import(&bytes)?;
    for key in imported.iter().filter(|k| k.own) {
        if !pgp::opens(&key.fingerprint, passphrase) {
            if !had.contains(&key.fingerprint) {
                let _ = pgp::remove(&key.fingerprint);
            }
            return Err(tr().text("pgp-wrong-passphrase", None));
        }
        secret::save_pgp_passphrase(&key.fingerprint, passphrase).map_err(|e| e.sentence(tr(), &key.fingerprint))?;
    }
    let mut args = tr().counted(imported.len());
    args.set("n", imported.len());
    Ok(tr().text("pgp-imported", Some(&args)))
}

/// Your public key into the downloads folder, to give to others.
pub(crate) fn export(fingerprint: &str, folder: &Path) -> Result<PathBuf, String> {
    let armored = pgp::export_public(fingerprint).ok_or_else(|| tr().text("pgp-no-key", None))?;
    std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    let short: String = fingerprint.chars().rev().take(16).collect::<Vec<_>>().into_iter().rev().collect();
    let path = folder.join(format!("{short}.asc"));
    std::fs::write(&path, armored).map_err(|e| e.to_string())?;
    Ok(path)
}

/// Looks up the keys a draft's recipients miss; returns what the status line says.
pub(crate) fn lookup(draft: &Draft) -> String {
    let missing = draft_protection(draft).missing;
    let mut found = 0;
    for address in &missing {
        match keys::lookup(address) {
            Ok(keys) if !keys.is_empty() => found += 1,
            Ok(_) => {}
            Err(SyncError::Network(e)) => return say("pgp-lookup-failed", &[("detail", e)]),
            Err(e) => return e.sentence(tr(), address),
        }
    }
    let still = missing.len() - found;
    if still == 0 { tr().text("pgp-lookup-all", None) } else { say("pgp-lookup-some", &[("missing", draft_protection(draft).missing.join(", "))]) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sioul_core::securitykey::{KnownCard, SoftCard};

    /// Your security key's certificate, as a phone or a computer knows it.
    fn yours(dated: Option<(std::time::SystemTime, std::time::Duration)>) -> Keys {
        let (card, public) = SoftCard::generate_dated("0006:12345678", &["Card Holder <me@example.org>"], "123456", dated).unwrap();
        let mine = pgp::certificates_in(&public);
        let known = KnownCard {
            ident: "0006:12345678".into(),
            sign: card.info.sign.as_ref().unwrap().fingerprint.clone(),
            decrypt: card.info.decrypt.as_ref().unwrap().fingerprint.clone(),
            cert: mine[0].fingerprint().to_hex(),
            addresses: vec!["me@example.org".into()],
            ..KnownCard::default()
        };
        Keys { own: mine, cards: vec![known], ..Keys::default() }
    }

    /// Batch B (18 October 2026): answering an encrypted message keeps its
    /// copy beside the draft as it came, encrypted: never its text in clear,
    /// which the sharing would carry to every device.
    #[test]
    fn an_answered_encrypted_message_stays_encrypted_beside_the_draft() {
        // Writes Sioul's files where it reads them: never the person's own (review of 36002ea).
        sioul_core::config::stand_ins_only();
        let tsk = pgp::stand_in_key("Me <me@example.org>").unwrap();
        let keys = Keys { own: pgp::certificates_in(&tsk), ..Keys::default() };
        let clear = b"From: Jane <jane@example.org>\r\nTo: Me <me@example.org>\r\nSubject: The lease\r\nDate: Thu, 01 Oct 2026 10:00:00 +0200\r\nMessage-ID: <lease-b@example.org>\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nThe secret clause is here.\r\n";
        let sealed = pgp::seal_message(clear, &[&keys.own[0]]).unwrap();
        let dir = std::env::temp_dir().join(format!("sioul-answer-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let original = dir.join("lease.eml");
        std::fs::write(&original, &sealed).unwrap();
        let mut draft = sioul_core::compose::answer(&original, sioul_core::compose::DraftKind::Reply, "me", &["me@example.org".to_string()]).unwrap();
        draft.keep_original().unwrap();
        // The key here opens it (as a session key kept since you opened it would).
        let passphrase = |_: &pgp::Fingerprint| None;
        assert!(pgp::open(&sealed, &keys, &pgp::Unlock::new(&passphrase, &pgp::SessionKeys::new())).is_some_and(|(opened, _)| opened.is_some()));
        protect_answer(&mut draft);
        let kept = std::fs::read(draft.original.as_ref().unwrap()).unwrap();
        draft.discard();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(!String::from_utf8_lossy(&kept).contains("secret clause"), "never in clear beside the draft");
        assert_eq!(kept, sealed, "kept as it came");
    }

    /// Batch A (18 October 2026): a phone knows your key and cannot sign with
    /// it yet: "Sign" is off, said; a computer signs. An expired certificate
    /// of yours: encrypting is said to leave you no copy to read, before Send.
    #[test]
    fn the_writing_window_says_what_your_key_can_do_here() {
        let mut draft = Draft::new("me");
        draft.to = vec!["me@example.org".into()];
        let keys = yours(None);
        let phone = protection_with(&keys, "me@example.org", &draft, false);
        assert!(!phone.can_sign && phone.sign_not_here == tr().text("seckey-sign-not-here", None) && phone.self_copy.is_empty() && phone.can_encrypt);
        let computer = protection_with(&keys, "me@example.org", &draft, true);
        assert!(computer.can_sign && computer.sign_not_here.is_empty() && computer.self_copy.is_empty());
        let year = std::time::Duration::from_secs(365 * 24 * 3600);
        let expired = yours(Some((std::time::SystemTime::now() - 2 * year, year)));
        let late = protection_with(&expired, "me@example.org", &draft, true);
        // Which part expired, and how to renew it there (F14): a computer's way, a phone's.
        let why = securitykey::expired_parts(tr(), &securitykey::parts(&expired.own[0]), jiff::Timestamp::now().as_second()).join(" ");
        assert!(!why.is_empty());
        assert_eq!(late.self_copy, say("pgp-self-copy-why", &[("why", why.clone()), ("renew", tr().text("pgp-self-copy-renew", None))]));
        assert_eq!(protection_with(&expired, "me@example.org", &draft, false).self_copy, say("pgp-self-copy-why", &[("why", why), ("renew", tr().text("pgp-self-copy-renew-phone", None))]));
        assert!(!late.can_sign && !late.can_encrypt, "to yourself, with no key that takes it: {:?}", late.missing);
        // No certificate of yours for the address known here (a phone whose settings do not come): said too.
        let mut to_jane = Draft::new("me");
        to_jane.to = vec!["jane@example.org".into()];
        assert_eq!(protection_with(&Keys::default(), "me@example.org", &to_jane, false).self_copy, tr().text("pgp-self-copy-none", None));
        // Your security key read on another computer of yours: a computer says
        // to read it there to sign (F18), a phone that it cannot yet.
        let shared = Keys { shared: keys.cards.clone(), cards: Vec::new(), ..keys.clone() };
        let elsewhere = protection_with(&shared, "me@example.org", &draft, true);
        assert!(!elsewhere.can_sign && elsewhere.sign_not_here == tr().text("seckey-sign-read-here", None) && elsewhere.self_copy.is_empty());
        assert_eq!(protection_with(&shared, "me@example.org", &draft, false).sign_not_here, tr().text("seckey-sign-not-here", None));
    }

    /// Review of 19 October 2026, N8: the self-copy line says what can renew
    /// the certificate: your security key's, GnuPG through Accounts (on a
    /// computer) or one of your computers (on a phone); a key kept here, only
    /// the program that made it, or a new key: never a button Accounts lacks.
    #[test]
    fn the_self_copy_line_says_what_can_renew_the_key() {
        let mut draft = Draft::new("me");
        draft.to = vec!["me@example.org".into()];
        let year = std::time::Duration::from_secs(365 * 24 * 3600);
        let card = yours(Some((std::time::SystemTime::now() - 2 * year, year)));
        let here = Keys { cards: Vec::new(), ..card.clone() };
        let why = securitykey::expired_parts(tr(), &securitykey::parts(&here.own[0]), jiff::Timestamp::now().as_second()).join(" ");
        for computer in [true, false] {
            assert_eq!(protection_with(&here, "me@example.org", &draft, computer).self_copy, say("pgp-self-copy-why", &[("why", why.clone()), ("renew", tr().text("pgp-self-copy-new-key", None))]));
        }
        assert!(protection_with(&card, "me@example.org", &draft, true).self_copy.ends_with(&tr().text("pgp-self-copy-renew", None)));
        // Known on this computer from another one only: read it here first.
        let shared = Keys { shared: card.cards.clone(), cards: Vec::new(), ..card.clone() };
        assert!(protection_with(&shared, "me@example.org", &draft, true).self_copy.ends_with(&tr().text("pgp-self-copy-renew-read-here", None)));
        assert!(protection_with(&shared, "me@example.org", &draft, false).self_copy.ends_with(&tr().text("pgp-self-copy-renew-phone", None)));
    }

    /// Review of 19 October 2026, N4: an address your security key serves,
    /// read on another device, is never offered "Make a key for": a computer
    /// offers to read the key there, a phone nothing; an address without a
    /// key of yours is offered one.
    #[test]
    fn a_shared_security_key_is_read_here_never_doubled_by_a_new_key() {
        let keys = yours(None);
        let shared = Keys { shared: keys.cards.clone(), cards: Vec::new(), ..keys.clone() };
        let addresses = vec!["me@example.org".to_string(), "other@example.org".to_string()];
        let other = vec!["other@example.org".to_string()];
        assert_eq!(offers(&shared, &addresses, true), (other.clone(), vec![say("pgp-read-card-here", &[("address", "me@example.org".to_string())])]));
        assert_eq!(offers(&shared, &addresses, false), (other.clone(), Vec::new()), "a phone cannot read it yet");
        assert_eq!(offers(&keys, &addresses, true), (other.clone(), Vec::new()), "read here: it serves");
        // A key of yours kept here for that address meanwhile: reading the security key here is still offered.
        let mut both = shared.clone();
        both.own.extend(pgp::certificates_in(&pgp::stand_in_key("Me <me@example.org>").unwrap()));
        assert_eq!(offers(&both, &addresses, true).1.len(), 1);
    }

    const LEASE: &[u8] = b"From: Jane <jane@example.org>\r\nTo: Card Holder <me@example.org>\r\nSubject: The lease\r\nDate: Thu, 01 Oct 2026 10:00:00 +0200\r\nMessage-ID: <lease-view@example.org>\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nThe secret clause is here.\r\n";

    /// Batch B (19 October 2026): unticking "Encrypt" says what becomes of
    /// an encrypted message answered (its quote left out) or forwarded (not
    /// sent); of one that came in clear, nothing. At Send, the copy encrypted
    /// to your security key waits for it, when the answer is encrypted.
    #[test]
    fn unticking_encrypt_says_what_becomes_of_an_encrypted_quote() {
        let keys = yours(None);
        let dir = std::env::temp_dir().join(format!("sioul-sealed-quote-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (sealed, clear) = (dir.join("sealed.eml"), dir.join("clear.eml"));
        std::fs::write(&sealed, pgp::seal_message(LEASE, &[&keys.own[0]]).unwrap()).unwrap();
        std::fs::write(&clear, LEASE).unwrap();
        let mut draft = Draft::new("me");
        draft.to = vec!["me@example.org".into()];
        draft.kind = DraftKind::Reply;
        draft.original = Some(sealed.clone());
        assert_eq!(protection_with(&keys, "me@example.org", &draft, true).sealed_quote, tr().text("pgp-quote-left-out", None));
        draft.kind = DraftKind::Forward;
        assert_eq!(protection_with(&keys, "me@example.org", &draft, true).sealed_quote, tr().text("pgp-forward-needs-encryption", None));
        draft.original = Some(clear.clone());
        assert!(protection_with(&keys, "me@example.org", &draft, true).sealed_quote.is_empty());
        draft.kind = DraftKind::Reply;
        draft.original = Some(sealed);
        draft.encrypt = true;
        assert_eq!(crate::securitykey::quote_card(&draft, &keys).as_deref(), Some("0006:12345678"));
        draft.encrypt = false;
        assert_eq!(crate::securitykey::quote_card(&draft, &keys), None, "not encrypted: not quoted, no key asked");
        draft.encrypt = true;
        draft.original = Some(clear);
        assert_eq!(crate::securitykey::quote_card(&draft, &keys), None, "came in clear");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Batch B: the look at the start and before each exchange encrypts the
    /// clear copies beside drafts, their earlier versions and what was kept
    /// before sharing began; nothing of their text is left in clear in those
    /// folders nor in the cache, and the log says counts only.
    #[test]
    fn the_look_leaves_no_clear_copy_and_says_counts_only() {
        let base = std::env::temp_dir().join(format!("sioul-seal-in-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (drafts, state, mail, cache) = (base.join("data/drafts"), base.join("state/share"), base.join("mail"), base.join("cache"));
        let memory = state.join("memory.json");
        let put = |path: PathBuf, bytes: &[u8]| {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, bytes).unwrap();
        };
        let keys = Keys { own: pgp::certificates_in(&pgp::stand_in_key("Card Holder <me@example.org>").unwrap()), ..Keys::default() };
        // The message came encrypted, in your mail (a Maildir); its copy, decrypted as Sioul wrote it before.
        let sealed = pgp::seal_message(LEASE, &[&keys.own[0]]).unwrap();
        put(mail.join("INBOX/cur/1790000000.U1-1.sioul:2,S"), &sealed);
        std::fs::create_dir_all(mail.join("INBOX/new")).unwrap();
        let none = |_: &pgp::Fingerprint| None;
        let decrypted = pgp::open(&sealed, &keys, &pgp::Unlock::new(&none, &pgp::SessionKeys::new())).unwrap().0.unwrap();
        assert!(pgp::looks_opened(&decrypted));
        put(drafts.join("b1.toml"), toml::to_string(&Draft { id: "b1".into(), ..Draft::default() }).unwrap().as_bytes());
        put(drafts.join("b1.eml"), &decrypted);
        put(state.join("history/drafts/data/drafts/b1.eml/2026-10-18T10-00-00.000Z"), &decrypted);
        put(state.join("before-sharing-2026-09-21/data/drafts/b1.eml"), &decrypted);
        let done = seal_in(&drafts, &memory, &draftcopies::Certs::of(keys.clone()), &[mail.clone()], &cache.join("mail-index.tsv"), &mut draftcopies::Judged::default());
        assert_eq!(done, draftcopies::Sealed { copies: 1, versions: 2, ..draftcopies::Sealed::default() });
        for folder in [&drafts, &state, &cache] {
            for path in sioul_core::mailindex::message_files(folder).into_iter().chain(walk(folder)) {
                assert!(!String::from_utf8_lossy(&std::fs::read(&path).unwrap_or_default()).contains("secret clause"), "{}", path.display());
            }
        }
        assert!(Draft::load(&drafts.join("b1.toml")).is_some(), "the draft itself untouched");
        let line = said(&done);
        assert!(!line.contains("lease") && !line.contains("secret") && !line.contains("b1"), "{line}");
        let _ = std::fs::remove_dir_all(&base);
    }

    /// Review of 19 October 2026, F9: the look never runs on a dose's path
    /// (`share::exchange_here`: a dose's alarm, "Taken", a waking), where
    /// Android gives seconds, nor reads your keys there; the window's exchange
    /// runs it before it takes `BUSY`, which an alarm waits for; the phone's
    /// background step runs it after its exchange. Read from the source.
    #[test]
    fn the_look_is_off_the_doses_path() {
        let source = |file: &str| std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(file)).unwrap();
        let body = |text: &str, start: &str| text.split(start).nth(1).and_then(|rest| rest.split("\n}\n").next()).unwrap_or_else(|| panic!("{start}")).to_string();
        let share = source("src/share.rs");
        let alarm = body(&share, "\npub(crate) fn exchange_here(");
        assert!(!alarm.contains("seal_clear_drafts(") && !alarm.contains("seal_clear_drafts_if_idle") && !alarm.contains("Keys::load"), "the alarm's path looks at the copies");
        let window = body(&share, "\nfn exchange_now(");
        let (look, busy) = (window.find("crate::crypto::seal_clear_drafts();").expect("the window's exchange looks"), window.find("BUSY.lock()").unwrap());
        assert!(look < busy, "the look before BUSY");
        let steps = source("src/steps.rs");
        let step = body(&steps, "\nfn step(reason: &str)");
        let (pull, look) = (step.find("exchange_here(fetch_first)").unwrap(), step.find("crate::crypto::seal_clear_drafts").expect("the background step looks"));
        assert!(pull < look, "after the step's exchange");
        for file in ["src/health.rs", "src/wake.rs", "src/timenote.rs", "src/alarms.rs"] {
            assert!(!source(file).contains("seal_clear_drafts("), "{file}");
        }
        // At the window's start, its thread spawned once the environment is set, which it reads (F10).
        let start = source("src/lib.rs");
        let (set, spawn) = (start.rfind("std::env::set_var(").unwrap(), start.find("std::thread::spawn(crypto::seal_clear_drafts)").expect("the start looks"));
        assert!(set < spawn, "the look's thread after the environment is set");
    }

    /// Every file below `dir`.
    fn walk(dir: &Path) -> Vec<PathBuf> {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(dir).into_iter().flatten().filter_map(Result::ok) {
            if entry.path().is_dir() {
                out.extend(walk(&entry.path()));
            } else {
                out.push(entry.path());
            }
        }
        out
    }

    /// Batch B: a draft whose message came encrypted, opened on a device
    /// where no key opens it: the window says so, lists none of what it
    /// forwards, and the copy stays as it came.
    #[test]
    fn a_draft_opened_without_the_key_says_so_and_stays_encrypted() {
        // Writes Sioul's files where it reads them: never the person's own (review of 36002ea).
        sioul_core::config::stand_ins_only();
        let elsewhere = pgp::certificates_in(&pgp::stand_in_key("Card Holder <me@example.org>").unwrap());
        let sealed = pgp::seal_message(LEASE, &[&elsewhere[0]]).unwrap();
        let mut draft = Draft::new("me");
        draft.kind = DraftKind::Forward;
        draft.to = vec!["bob@example.org".into()];
        std::fs::create_dir_all(Draft::folder()).unwrap();
        let copy = Draft::folder().join(format!("{}.eml", draft.id));
        std::fs::write(&copy, &sealed).unwrap();
        draft.original = Some(copy.clone());
        draft.save().unwrap();
        let view = |id: &str| serde_json::from_str::<serde_json::Value>(&crate::mail::draft(id).unwrap()).unwrap();
        let shown = view(&draft.id);
        let below = shown["below"].as_str().unwrap_or_default().to_string();
        assert!(below.ends_with(&tr().text("compose-forward-sealed-no-key", None)), "no key of yours opens it: {below}");
        assert!(shown["forwarded"].as_array().is_some_and(Vec::is_empty), "{shown}");
        draft.kind = DraftKind::Reply;
        draft.save().unwrap();
        let below = view(&draft.id)["below"].as_str().unwrap_or_default().to_string();
        assert!(below.ends_with(&tr().text("compose-below-sealed-no-key", None)), "no key of yours opens it: {below}");
        assert_eq!(std::fs::read(&copy).unwrap(), sealed, "kept as it came");
        draft.discard();
    }
}
