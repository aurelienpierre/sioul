// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Your security key behind the window (docs/client.md, "Your keys on a
//! security key"): the band of the writing window and of the Reader (plug the
//! key in, its PIN, a touch, what went wrong, in words), signing at Send
//! before the ten seconds of "Undo", opening a message on "Open with your
//! security key", the PIN held in memory for fifteen quiet minutes, the key's
//! setup in Accounts ▸ Encryption, and "Let GnuPG release it".
//!
//! Nothing here touches the key unless you asked: a message shown is never
//! opened with it, and no lookup runs by itself. A demo profile never reaches
//! the system's smart card service: it has a key of its own, made of
//! software, whose PIN is 123456.

use crate::backend::{QtThread, Shared, json, load_config, local_path, offline, say, tell, tr};
use cxx_qt_lib::QString;
use sioul_core::compose::Draft;
use sioul_core::pgp::{self, Keys, Place};
use sioul_core::securitykey::{self, CardAccess, CardError, CardInfo, KnownCard, NoReaders, Password, PinMemory, Purpose, Readers, Remedy, SoftCard, SoftReaders, Touch};
use sioul_sync::securitykey::{Looked, Released};
use sioul_sync::send::BuildError;
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// The PIN, held in memory for fifteen quiet minutes after its last use.
static PIN: Mutex<PinMemory> = Mutex::new(PinMemory::new());
/// Whether the watch over the held PIN runs.
static WATCHING: AtomicBool = AtomicBool::new(false);
/// The bands waiting for the key to be plugged in, by their context, until "Not now".
static WAITING: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());
/// What the setup read from the key, for "Look for it" and "Import a file…".
static SETUP: Mutex<Option<CardInfo>> = Mutex::new(None);
/// The window, told when the PIN is held or forgotten: Accounts offers "Forget the PIN now" then.
static WINDOW: Mutex<Option<QtThread>> = Mutex::new(None);

/// How often Sioul looks whether a key it waits for came, or whether the key whose PIN it holds went.
const LOOK_EVERY: Duration = Duration::from_millis(1500);
/// How long a band waits for its key before it stops looking.
const WAIT_AT_MOST: Duration = Duration::from_secs(5 * 60);
/// The band of the setup, in Accounts ▸ Encryption.
const SETUP_BAND: &str = "setup";

/// The demo's security key: software behind the same traits, its PIN 123456,
/// a touch asked to sign; its certificate names the profile's first address.
struct Demo {
    readers: SoftReaders,
    certificate: Vec<u8>,
}

fn demo() -> Option<&'static Demo> {
    static DEMO: OnceLock<Option<Demo>> = OnceLock::new();
    DEMO.get_or_init(|| {
        let config = load_config();
        let accounts: Vec<_> = config.accounts.iter().filter(|a| a.address.is_some()).collect();
        // One user ID per address, named when an account names it.
        let mut userids: Vec<String> = Vec::new();
        for account in &accounts {
            let address = account.address.clone().unwrap_or_default();
            if userids.iter().any(|u| u.ends_with(&format!("<{address}>"))) {
                continue;
            }
            let name = accounts.iter().filter(|a| a.address.as_deref() == Some(address.as_str())).find_map(|a| a.name.clone()).unwrap_or_default();
            userids.push(format!("{name} <{address}>").trim().to_string());
        }
        let userids: Vec<&str> = userids.iter().map(String::as_str).collect();
        let (mut card, certificate) = SoftCard::generate("0006:01234567", &userids, "123456").ok()?;
        card.info.holder = accounts.first().and_then(|a| a.name.clone()).unwrap_or_default();
        if let Some(slot) = card.info.sign.as_mut() {
            slot.touch = Touch::On;
        }
        card.touch_delay = Duration::from_secs(4);
        Some(Demo { readers: SoftReaders::with(vec![card]), certificate })
    })
    .as_ref()
}

/// Where security keys are: the system's smart card service; in a demo, the demo's own key, never a real one.
fn readers() -> &'static dyn Readers {
    static NONE: NoReaders = NoReaders;
    if offline() {
        return demo().map_or(&NONE as &dyn Readers, |d| &d.readers as &dyn Readers);
    }
    sioul_sync::securitykey::readers()
}

/// What the band of a writing window, of the Reader or of the setup shows, as JSON.
#[derive(serde::Serialize, Default)]
struct Band {
    /// "working", "plug", "pin", "touch", "problem", "found", "missing", "done".
    state: &'static str,
    /// What it says.
    line: String,
    /// Under the PIN's field: "2 tries left.", when fewer than the most seen.
    tries: String,
    /// Said in warm colours: a wrong PIN, a problem; never red.
    warm: bool,
    /// The problem's button: "retry" (ask the key again), "release" (Let
    /// GnuPG release it), "lookup" (look for the certificate again), or none.
    action: &'static str,
    /// The key's fingerprint, for `gpg --export` in the setup's hint.
    fingerprint: String,
}

fn tell_band(qt: &QtThread, context: &str, band: Band) {
    let (context, state) = (context.to_string(), json(&band));
    let _ = qt.queue(move |mut sioul| sioul.as_mut().security_key_changed(QString::from(&context), QString::from(&state)));
}

fn working(qt: &QtThread, context: &str, id: &str) {
    tell_band(qt, context, Band { state: "working", line: tr().text(id, None), ..Band::default() });
}

fn problem_line(qt: &QtThread, context: &str, line: String, action: &'static str) {
    tell_band(qt, context, Band { state: "problem", line, warm: true, action, ..Band::default() });
}

/// The security key that signs a draft at Send, as JSON {ident, label}; or
/// {expired: …} when its certificate expired; "" when the draft is signed
/// another way, or not at all.
pub(crate) fn for_draft(id: &str) -> String {
    let Some(draft) = Draft::by_id(id).filter(|d| d.sign) else { return String::new() };
    let Some(address) = load_config().account(&draft.account).and_then(|a| a.address.clone()) else { return String::new() };
    let keys = Keys::load();
    match keys.own_for(&address) {
        Some(pgp::Own { place: Place::Card { ident }, .. }) => {
            let label = keys.cards.iter().find(|c| c.ident == ident).map(|c| c.label(tr())).unwrap_or_default();
            json(&serde_json::json!({ "ident": ident, "label": label }))
        }
        Some(_) => String::new(),
        None => expired_line(&keys, &address).map(|line| json(&serde_json::json!({ "expired": line }))).unwrap_or_default(),
    }
}

/// What the writing window says when the security key that signs for
/// `address` has an expired certificate: the date, and how to renew it.
pub(crate) fn expired_line(keys: &Keys, address: &str) -> Option<String> {
    let (known, cert) = keys.card_for(address)?;
    let expires = securitykey::expiry_of(cert, &[&known.sign, &known.decrypt]);
    securitykey::expiry_line(tr(), expires, &cert.fingerprint().to_hex(), now_unix()).filter(|(_, warm)| *warm).map(|(line, _)| line)
}

fn now_unix() -> i64 {
    jiff::Timestamp::now().as_second()
}

/// The PIN to use: the one typed, else the one held (not for a key that asks
/// it for each signature, decision 5); with what the key says, read without
/// its PIN. None as the PIN: the band asks for it.
fn pin_for(ident: &str, typed: Option<&Password>, purpose: Purpose) -> Result<(CardInfo, Option<Password>), CardError> {
    let info = securitykey::cards(readers())?.into_iter().find(|c| c.ident.eq_ignore_ascii_case(ident)).ok_or(CardError::Absent)?;
    if info.pin_tries == 0 {
        return Err(CardError::Blocked);
    }
    if let Some(typed) = typed {
        return Ok((info, Some(typed.clone())));
    }
    if purpose == Purpose::Sign && info.pin_each_signature {
        return Ok((info, None));
    }
    let held = PIN.lock().ok().and_then(|mut memory| memory.get(ident, Instant::now()));
    Ok((info, held))
}

/// The band asks for the PIN, with the tries left when fewer than the most seen.
fn ask_pin(qt: &QtThread, context: &str, info: &CardInfo, purpose: Purpose) {
    let known = securitykey::known_cards().into_iter().find(|c| c.ident.eq_ignore_ascii_case(&info.ident)).unwrap_or_default();
    let tries = known.tries_line(info.pin_tries, tr()).unwrap_or_default();
    let line = if purpose == Purpose::Sign && info.pin_each_signature { "seckey-pin-each" } else { "seckey-pin" };
    tell_band(qt, context, Band { state: "pin", line: tr().text(line, None), warm: !tries.is_empty(), tries, ..Band::default() });
}

/// Tells the window the keys' state changed: the PIN held, or forgotten.
fn keys_changed() {
    if let Some(qt) = WINDOW.lock().ok().and_then(|w| w.clone()) {
        let _ = qt.queue(|mut sioul| sioul.as_mut().keys_changed());
    }
}

/// The PIN worked: held for its quiet time, watched; what the key said, kept.
fn remember(ident: &str, pin: Password, reader: Option<String>, info: &CardInfo) {
    if let Ok(mut memory) = PIN.lock() {
        memory.keep(ident, pin, &reader.unwrap_or_else(|| info.reader.clone()), Instant::now());
    }
    keys_changed();
    let mut known = securitykey::known_cards().into_iter().find(|c| c.ident.eq_ignore_ascii_case(ident));
    if let Some(known) = known.as_mut() {
        let before = known.clone();
        known.update(info);
        if *known != before {
            let updated = known.clone();
            let _ = securitykey::change_known(|cards| {
                if let Some(card) = cards.iter_mut().find(|c| c.ident == updated.ident) {
                    *card = updated;
                }
            });
        }
    }
    watch_pin();
}

/// While a PIN is held: forgotten when its quiet time ends, or when its key
/// is pulled out (its reader gone).
fn watch_pin() {
    if WATCHING.swap(true, Ordering::Relaxed) {
        return;
    }
    std::thread::spawn(|| {
        loop {
            std::thread::sleep(LOOK_EVERY);
            let present = readers().present();
            let Ok(mut memory) = PIN.lock() else { break };
            memory.expire(Instant::now());
            if let Ok(present) = present {
                memory.went_away(&present);
            }
            if memory.holder().is_none() {
                break;
            }
        }
        WATCHING.store(false, Ordering::Relaxed);
        keys_changed();
    });
}

/// "Forget the PIN now".
pub(crate) fn forget_pin() -> String {
    if let Ok(mut memory) = PIN.lock() {
        memory.forget();
    }
    tr().text("seckey-pin-forgotten", None)
}

/// Sioul closes: the PIN and the session keys go with it.
pub(crate) fn closing() {
    if let Ok(mut memory) = PIN.lock() {
        memory.forget();
    }
    pgp::SessionKeys::global().forget();
}

/// A PIN the key took before something else failed (a touch missed, the key
/// pulled out): held, so that "Try again" does not ask it again.
fn keep_taken(ident: &str, pin: &Password, access: &CardAccess, info: &CardInfo) {
    if access.pin_accepted() {
        remember(ident, pin.clone(), access.reader(), info);
    }
}

/// What a failure does to the band, and to a PIN held that the key refused.
/// A key not plugged in is waited for instead ([`wait_for`]).
fn failed(qt: &QtThread, context: &str, error: CardError, purpose: Purpose, typed: bool) {
    if matches!(error, CardError::WrongPin { .. } | CardError::Blocked) && !typed {
        // The PIN held is refused: never given again.
        if let Ok(mut memory) = PIN.lock() {
            memory.forget();
        }
    }
    let line = error.sentence(tr(), purpose);
    match error.remedy() {
        Remedy::Pin => tell_band(qt, context, Band { state: "pin", line, warm: true, ..Band::default() }),
        Remedy::Again => problem_line(qt, context, line, "retry"),
        Remedy::Release => problem_line(qt, context, line, "release"),
        Remedy::None => problem_line(qt, context, line, ""),
    }
}

/// Remembers the window, to tell it when the PIN is held or forgotten.
fn know_window(qt: &QtThread) {
    if let Ok(mut window) = WINDOW.lock() {
        *window = Some(qt.clone());
    }
}

/// Signs a draft with your security key at Send, then schedules it with its
/// ten seconds of "Undo"; `typed` is the PIN typed in the band, else "".
pub(crate) fn sign_and_send(qt: &QtThread, shared: &Arc<Shared>, id: &str, typed: String) {
    know_window(qt);
    let typed = (!typed.is_empty()).then(|| Password::from(typed));
    let (qt, shared, id) = (qt.clone(), Arc::clone(shared), id.to_string());
    std::thread::spawn(move || sign_now(&qt, &shared, &id, typed));
}

fn sign_now(qt: &QtThread, shared: &Arc<Shared>, id: &str, typed: Option<Password>) {
    working(qt, id, "seckey-reading");
    let Some(draft) = Draft::by_id(id) else { return problem_line(qt, id, tr().text("mail-draft-gone", None), "") };
    let config = load_config();
    let Some(account) = config.account(&draft.account).filter(|a| a.syncs()).cloned() else {
        return problem_line(qt, id, say("account-unknown", &[("id", draft.account.clone())]), "");
    };
    let keys = Keys::load();
    let ident = match keys.own_for(account.address.as_deref().unwrap_or_default()) {
        Some(pgp::Own { place: Place::Card { ident }, .. }) => ident,
        // Not (or no longer) signed with a security key: sent as any message is.
        _ => {
            match crate::mail::send(qt, shared, id) {
                Some(problem) => problem_line(qt, id, problem, ""),
                None => tell_band(qt, id, Band { state: "done", ..Band::default() }),
            }
            return;
        }
    };
    let (info, pin) = match pin_for(&ident, typed.as_ref(), Purpose::Sign) {
        Ok(found) => found,
        Err(CardError::Absent) => return wait_for(qt, shared, id, &ident, Purpose::Sign, typed),
        Err(error) => return failed(qt, id, error, Purpose::Sign, typed.is_some()),
    };
    let Some(pin) = pin else { return ask_pin(qt, id, &info, Purpose::Sign) };
    working(qt, id, "seckey-signing");
    let (touch_qt, context) = (qt.clone(), id.to_string());
    let touch = move || tell_band(&touch_qt, &context, Band { state: "touch", line: tr().text("seckey-touch", None), ..Band::default() });
    let access = CardAccess::new(readers(), &pin, &touch);
    match sioul_sync::send::build_draft(&account, &draft, tr(), Some(&access)) {
        Ok(outgoing) => {
            remember(&ident, pin.clone(), access.reader(), &info);
            crate::mail::send_signed(qt, shared, id, account, outgoing);
            tell_band(qt, id, Band { state: "done", ..Band::default() });
        }
        Err(BuildError::Card(CardError::Absent)) => wait_for(qt, shared, id, &ident, Purpose::Sign, typed),
        Err(BuildError::Card(error)) => {
            keep_taken(&ident, &pin, &access, &info);
            failed(qt, id, error, Purpose::Sign, typed.is_some());
        }
        Err(other) => problem_line(qt, id, other.into_sync(tr()).sentence(tr(), &account.id), ""),
    }
}

/// Opens a message encrypted to your security key ("Open with your security
/// key"); its session key then stays in memory until Sioul closes, so that it
/// opens again, with its attachments and its answer, without the key.
pub(crate) fn open_message(qt: &QtThread, shared: &Arc<Shared>, key: &str, typed: String) {
    know_window(qt);
    let typed = (!typed.is_empty()).then(|| Password::from(typed));
    let (qt, shared, key) = (qt.clone(), Arc::clone(shared), key.to_string());
    std::thread::spawn(move || open_now(&qt, &shared, &key, typed));
}

fn open_now(qt: &QtThread, shared: &Arc<Shared>, key: &str, typed: Option<Password>) {
    working(qt, key, "seckey-reading");
    let Some(raw) = crate::mail::locate(key).and_then(|(_, file)| std::fs::read(file).ok()) else {
        return problem_line(qt, key, tr().text("mail-message-gone", None), "");
    };
    // Which key it waits for; opened already from its session key, it is done.
    let ident = match crate::crypto::open(&raw) {
        Some((Some(_), _)) | None => return tell_band(qt, key, Band { state: "done", ..Band::default() }),
        Some((None, view)) => match view.security_key {
            Some(ident) => ident,
            None => return problem_line(qt, key, tr().text("pgp-not-opened", None), ""),
        },
    };
    let (info, pin) = match pin_for(&ident, typed.as_ref(), Purpose::Open) {
        Ok(found) => found,
        Err(CardError::Absent) => return wait_for(qt, shared, key, &ident, Purpose::Open, typed),
        Err(error) => return failed(qt, key, error, Purpose::Open, typed.is_some()),
    };
    let Some(pin) = pin else { return ask_pin(qt, key, &info, Purpose::Open) };
    working(qt, key, "seckey-opening");
    let (touch_qt, context) = (qt.clone(), key.to_string());
    let touch = move || tell_band(&touch_qt, &context, Band { state: "touch", line: tr().text("seckey-touch", None), ..Band::default() });
    let access = CardAccess::new(readers(), &pin, &touch);
    let passphrase = |fingerprint: &pgp::Fingerprint| sioul_sync::secret::pgp_passphrase(&fingerprint.to_hex());
    let unlock = pgp::Unlock::new(&passphrase, pgp::SessionKeys::global()).with_card(&access);
    match pgp::open(&raw, &Keys::load(), &unlock) {
        Some((Some(_), _)) => {
            remember(&ident, pin.clone(), access.reader(), &info);
            tell_band(qt, key, Band { state: "done", ..Band::default() });
        }
        _ => match access.failure() {
            Some(CardError::Absent) => wait_for(qt, shared, key, &ident, Purpose::Open, typed),
            Some(error) => {
                keep_taken(&ident, &pin, &access, &info);
                failed(qt, key, error, Purpose::Open, typed.is_some());
            }
            None => problem_line(qt, key, tr().text("pgp-not-opened", None), ""),
        },
    }
}

/// The key is not plugged in: the band says so, and Sioul looks, without
/// opening any key, until it comes, "Not now", or a few minutes pass.
fn wait_for(qt: &QtThread, shared: &Arc<Shared>, context: &str, ident: &str, purpose: Purpose, typed: Option<Password>) {
    if let Ok(mut waiting) = WAITING.lock() {
        waiting.insert(context.to_string());
    }
    tell_band(qt, context, Band { state: "plug", line: CardError::Absent.sentence(tr(), purpose), ..Band::default() });
    let (qt, shared, context, ident) = (qt.clone(), Arc::clone(shared), context.to_string(), ident.to_string());
    std::thread::spawn(move || {
        let since = Instant::now();
        let mut seen: Option<Vec<String>> = None;
        loop {
            std::thread::sleep(LOOK_EVERY);
            if !WAITING.lock().is_ok_and(|w| w.contains(&context)) {
                return;
            }
            if since.elapsed() > WAIT_AT_MOST {
                if let Ok(mut waiting) = WAITING.lock() {
                    waiting.remove(&context);
                }
                return problem_line(&qt, &context, CardError::Absent.sentence(tr(), purpose), "retry");
            }
            // A key is opened only when the readers change: another key plugged in is left alone.
            let present = readers().present().ok();
            if present == seen {
                continue;
            }
            seen = present;
            if securitykey::cards(readers()).is_ok_and(|cards| cards.iter().any(|c| c.ident.eq_ignore_ascii_case(&ident))) {
                if let Ok(mut waiting) = WAITING.lock() {
                    waiting.remove(&context);
                }
                return match purpose {
                    Purpose::Sign => sign_now(&qt, &shared, &context, typed),
                    _ => open_now(&qt, &shared, &context, typed),
                };
            }
        }
    });
}

/// "Not now": the band stops waiting for the key.
pub(crate) fn not_now(context: &str) {
    if let Ok(mut waiting) = WAITING.lock() {
        waiting.remove(context);
    }
}

/// "Let GnuPG release it", pressed: GnuPG's smart card daemon stopped.
/// Returns JSON {line, done}: done, the band tries again at once.
pub(crate) fn release() -> String {
    let released = if offline() { Released::Done } else { sioul_sync::securitykey::release_gnupg() };
    json(&serde_json::json!({ "line": released.sentence(tr()), "done": released == Released::Done }))
}

/// The cardholder's name as written on the key ("Ferrand<<Noa",
/// ISO/IEC 7501-1), in the usual order ("Noa Ferrand").
fn holder_name(written: &str) -> String {
    match written.split_once("<<") {
        Some((surname, given)) => format!("{} {}", given.replace('<', " ").trim(), surname.replace('<', " ").trim()).trim().to_string(),
        None => written.replace('<', " ").trim().to_string(),
    }
}

/// "YubiKey 12 345 678, Noa Ferrand. It signs (Ed25519, made on 3 March 2026) and decrypts (Curve25519)."
fn describe(info: &CardInfo) -> String {
    let holder = holder_name(&info.holder);
    let holder = if holder.is_empty() { String::new() } else { format!(", {holder}") };
    let made = |slot: &securitykey::SlotInfo| match slot.created {
        0 => slot.algorithm.clone(),
        at => say("seckey-made-on", &[("algorithm", slot.algorithm.clone()), ("date", securitykey::full_date(tr(), i64::from(at)))]),
    };
    let (sign, decrypt) = (info.sign.as_ref().map(made), info.decrypt.as_ref().map(made));
    let id = match (&sign, &decrypt) {
        (Some(_), Some(_)) => "seckey-found-both",
        (Some(_), None) => "seckey-found-sign",
        _ => "seckey-found-decrypt",
    };
    say(id, &[("label", info.label(tr())), ("holder", holder), ("sign", sign.unwrap_or_default()), ("decrypt", decrypt.unwrap_or_default())])
}

/// "Use a security key": reads the key plugged in, without its PIN.
pub(crate) fn read_for_setup(qt: &QtThread) {
    let qt = qt.clone();
    std::thread::spawn(move || {
        working(&qt, SETUP_BAND, "seckey-reading");
        match securitykey::cards(readers()) {
            Ok(found) => {
                let known = securitykey::known_cards();
                // A key not set up yet comes first, when several are plugged in.
                let Some(info) = found.iter().find(|c| !known.iter().any(|k| k.ident == c.ident)).or(found.first()).cloned() else { return };
                let line = describe(&info);
                let fingerprint = fingerprint_of(&info);
                if let Ok(mut setup) = SETUP.lock() {
                    *setup = Some(info);
                }
                tell_band(&qt, SETUP_BAND, Band { state: "found", line, fingerprint, ..Band::default() });
            }
            Err(error) => {
                let action = if error.remedy() == Remedy::Release { "release" } else { "retry" };
                problem_line(&qt, SETUP_BAND, error.sentence(tr(), Purpose::Read), action);
            }
        }
    });
}

/// The fingerprint `gpg --export` takes for the key: its signing key's, else its decryption key's.
fn fingerprint_of(info: &CardInfo) -> String {
    info.sign.as_ref().or(info.decrypt.as_ref()).map(|s| s.fingerprint.clone()).unwrap_or_default()
}

/// The key to look up a certificate for: the one the setup read, else a key
/// known already ("Look for a newer version"), from what Sioul kept of it.
fn card_for_lookup(ident: &str) -> Option<CardInfo> {
    let read = SETUP.lock().ok().and_then(|s| s.clone()).filter(|c| ident.is_empty() || c.ident.eq_ignore_ascii_case(ident));
    read.or_else(|| securitykey::known_cards().into_iter().find(|c| c.ident.eq_ignore_ascii_case(ident)).map(|known| as_info(&known)))
}

/// What Sioul kept of a key, as the key would say it (fingerprints, no public parts).
fn as_info(known: &KnownCard) -> CardInfo {
    let slot = |fingerprint: &str, touch: Touch| (!fingerprint.is_empty()).then(|| securitykey::SlotInfo { fingerprint: fingerprint.to_string(), created: 0, algorithm: String::new(), touch, public: None });
    CardInfo {
        ident: known.ident.clone(),
        manufacturer: known.manufacturer,
        serial: known.serial,
        holder: known.holder.clone(),
        url: String::new(),
        sign: slot(&known.sign, known.touch_sign),
        decrypt: slot(&known.decrypt, known.touch_decrypt),
        pin_tries: known.pin_tries_full,
        reset_tries: 0,
        admin_tries: 0,
        pin_each_signature: known.pin_each_signature,
        signatures: 0,
        reader: String::new(),
    }
}

/// Your mail addresses.
fn your_addresses() -> Vec<String> {
    load_config().accounts.iter().filter(|a| a.syncs()).filter_map(|a| a.address.clone()).map(|a| a.to_ascii_lowercase()).collect()
}

/// "Look for it", "Look for a newer version": the key's certificate looked
/// up, only now that you asked, checked against the key, and kept.
pub(crate) fn look_for_certificate(qt: &QtThread, ident: &str) {
    let (qt, ident) = (qt.clone(), ident.to_string());
    std::thread::spawn(move || {
        let Some(info) = card_for_lookup(&ident) else { return problem_line(&qt, SETUP_BAND, CardError::Absent.sentence(tr(), Purpose::Read), "") };
        working(&qt, SETUP_BAND, "seckey-looking");
        let looked = if offline() {
            // A demo profile stays off the network: its own key's certificate, as if found.
            match demo().map(|d| securitykey::certificate_in(&d.certificate, &info)) {
                Some(Ok((cert, check))) => Ok(Looked::Found { cert, check, source: "https://keys.openpgp.org/".into() }),
                _ => Ok(Looked::NotFound),
            }
        } else {
            sioul_sync::securitykey::find_certificate(&info, &your_addresses())
        };
        match looked {
            Ok(Looked::Found { cert, check, source }) => adopt(&qt, &info, &cert, &check, &source),
            Ok(Looked::Mismatch(why)) => problem_line(&qt, SETUP_BAND, why.sentence(tr()), ""),
            Ok(Looked::NotFound) => tell_band(&qt, SETUP_BAND, Band { state: "missing", line: tr().text("seckey-not-found", None), fingerprint: fingerprint_of(&info), ..Band::default() }),
            Err(error) => problem_line(&qt, SETUP_BAND, error.sentence(tr(), ""), "lookup"),
        }
    });
}

/// "Import a file…": the key's certificate from `gpg --export`, checked, kept.
pub(crate) fn import_certificate(qt: &QtThread, ident: &str, url: &str) {
    let Some(info) = card_for_lookup(ident) else { return problem_line(qt, SETUP_BAND, CardError::Absent.sentence(tr(), Purpose::Read), "") };
    let path = local_path(url);
    match std::fs::read(&path) {
        Ok(bytes) => match securitykey::certificate_in(&bytes, &info) {
            Ok((cert, check)) => adopt(qt, &info, &cert, &check, "file"),
            Err(why) => problem_line(qt, SETUP_BAND, why.sentence(tr()), ""),
        },
        Err(e) => problem_line(qt, SETUP_BAND, format!("{}: {e}", path.display()), ""),
    }
}

/// The certificate found is the key's: kept, the key made yours for the
/// addresses it names among yours, and said.
fn adopt(qt: &QtThread, info: &CardInfo, cert: &securitykey::Certificate, check: &securitykey::CertCheck, source: &str) {
    let yours = your_addresses();
    let named: Vec<String> = check.addresses.iter().filter(|a| yours.contains(a)).cloned().collect();
    // A certificate naming no address at all (keys.openpgp.org gives none
    // until you confirm one) still signs your mail; one naming others' does not.
    let addresses = if check.addresses.is_empty() { yours } else { named };
    match securitykey::adopt(info, cert, source, addresses.clone(), now_unix()) {
        Ok(_) => {
            let names = cert.userids().map(|u| String::from_utf8_lossy(u.userid().value()).to_string()).collect::<Vec<_>>().join(", ");
            let mut line = if check.addresses.is_empty() {
                tr().text("seckey-kept-no-address", None)
            } else if addresses.is_empty() {
                say("seckey-kept-not-yours", &[("names", names)])
            } else {
                say("seckey-kept", &[("names", names), ("addresses", addresses.join(", "))])
            };
            let mut warm = addresses.is_empty();
            if let Some((expiry, late)) = securitykey::expiry_line(tr(), check.expires, &check.cert, now_unix()) {
                line = format!("{line} {expiry}");
                warm |= late;
            }
            if let Ok(mut setup) = SETUP.lock() {
                *setup = None;
            }
            tell_band(qt, SETUP_BAND, Band { state: "done", line, warm, ..Band::default() });
            let _ = qt.queue(|mut sioul| sioul.as_mut().keys_changed());
        }
        Err(e) => problem_line(qt, SETUP_BAND, e, ""),
    }
}

/// "Stop using this security key": it leaves Sioul's list, its certificate
/// is kept aside; the PIN held, if it was its, is forgotten.
pub(crate) fn stop_using(qt: &QtThread, shared: &Shared, ident: &str) {
    if PIN.lock().is_ok_and(|m| m.holder().is_some_and(|h| h.eq_ignore_ascii_case(ident))) {
        let _ = forget_pin();
    }
    let line = match securitykey::forget(ident) {
        Ok(()) => tr().text("seckey-stopped", None),
        Err(e) => e,
    };
    tell(qt, shared, line);
    let _ = qt.queue(|mut sioul| sioul.as_mut().keys_changed());
}

/// One known security key, as Accounts ▸ Encryption lists it.
#[derive(serde::Serialize)]
struct KnownView {
    ident: String,
    /// "YubiKey 12 345 678, Jane Doe".
    title: String,
    /// "Signs for jane@example.org."; or that it signs for none.
    signs_for: String,
    /// "Its certificate came from keys.openpgp.org on 7 October 2026."
    source: String,
    /// Its expiry, and whether it is said in warm colours (expired, or within a month).
    expiry: String,
    late: bool,
    /// "Touch asked to sign and to open."
    touch: String,
    /// Its PIN is held now.
    pin_held: bool,
}

/// Your security keys, as JSON {cards: […]}, for Accounts ▸ Encryption.
pub(crate) fn known_view() -> String {
    let keys = Keys::load();
    let holder = PIN.lock().ok().and_then(|m| m.holder().map(str::to_string));
    let cards: Vec<KnownView> = keys
        .cards
        .iter()
        .map(|card| {
            let name = holder_name(&card.holder);
            let title = if name.is_empty() { card.label(tr()) } else { format!("{}, {name}", card.label(tr())) };
            let source = match card.source.as_str() {
                "file" => say("seckey-source-file", &[("date", securitykey::full_date(tr(), card.checked))]),
                url => say("seckey-source", &[("source", host_of(url)), ("date", securitykey::full_date(tr(), card.checked))]),
            };
            let (expiry, late) = keys
                .cert(&card.cert)
                .and_then(|cert| securitykey::expiry_line(tr(), securitykey::expiry_of(cert, &[&card.sign, &card.decrypt]), &card.cert, now_unix()))
                .unwrap_or_default();
            let touch = match (card.touch_sign.asked() && !card.sign.is_empty(), card.touch_decrypt.asked() && !card.decrypt.is_empty()) {
                (true, true) => "seckey-touch-both",
                (true, false) => "seckey-touch-sign",
                (false, true) => "seckey-touch-open",
                (false, false) => "seckey-touch-none",
            };
            KnownView {
                ident: card.ident.clone(),
                title,
                signs_for: if card.addresses.is_empty() { tr().text("seckey-signs-for-none", None) } else { say("seckey-signs-for", &[("addresses", card.addresses.join(", "))]) },
                source,
                expiry,
                late,
                touch: tr().text(touch, None),
                pin_held: holder.as_deref().is_some_and(|h| h.eq_ignore_ascii_case(&card.ident)),
            }
        })
        .collect();
    json(&serde_json::json!({ "cards": cards }))
}

/// `keys.openpgp.org` from `https://keys.openpgp.org/vks/v1/…`.
fn host_of(url: &str) -> String {
    url.split_once("://").map_or(url, |(_, rest)| rest).split(['/', '?', '#']).next().unwrap_or(url).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_holder_reads_in_the_usual_order() {
        assert_eq!(holder_name("Doe<<Jane<Marie"), "Jane Marie Doe");
        assert_eq!(holder_name("Jane Doe"), "Jane Doe");
        assert_eq!(holder_name(""), "");
        assert_eq!(host_of("https://keys.openpgp.org/vks/v1/by-fingerprint/ABC"), "keys.openpgp.org");
        assert_eq!(host_of("https://openpgpkey.example.org/.well-known/openpgpkey/example.org/hu/x?l=jane"), "openpgpkey.example.org");
    }
}
