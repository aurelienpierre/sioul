// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The screen before the window (StoreGateWindow.qml; docs/store.md, "At start"):
//! shown only when what Sioul keeps on this device is not open yet, or when
//! plain files wait to be sealed. It says "Sioul waits for your keyring to
//! open." and goes on by itself once the keyring gives the key; meanwhile
//! **Use a way back…** opens it with a passphrase, a recovery code or a
//! security key the person kept (`sioul_sync::storekey`), or starts again
//! from the servers and the other devices, what is sealed set aside, never
//! removed. The move of a profile's plain files shows its progress, and goes
//! on where it stopped after an interruption. At the first sealing the ways
//! back are offered once, calmly, each with what it protects against, none
//! needed; with none kept, what a lost keyring entry would cost is said.
//!
//! It runs as a process of its own, which the window starts and waits for
//! (`lib.rs`). At its end it hands the window the key the store opened with,
//! on a pipe of the window's whose end it is given (`SIOUL_GATE_HAND_OVER`,
//! never its standard output, which a terminal or a log may show): a way back
//! opens Sioul even when the keyring stays locked, refuses a new item or is
//! not there at all, and nothing of the key touches the disk or a terminal
//! (review-finish G5, third check). "Start again" is offered only when the keyring answers
//! that it holds no key, never while it is merely locked, and only once the
//! person confirmed what it sets aside (G4).
//!
//! The same ways back, in Settings: the card "What Sioul keeps here"
//! (StoreCard.qml), through the backend's functions of the same names, which
//! call `ways_json`, `keep_passphrase`, `new_code`, `keep_cards`, `forget`
//! here.

use cxx_qt::Threading;
use cxx_qt_lib::QString;
use sioul_sync::storekey::{self, Begin, KeyError};
use std::pin::Pin;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

#[cxx_qt::bridge]
pub mod qobject {
    // SAFETY: cxx checks these declarations against the headers they include, when it builds; the types are cxx-qt-lib's.
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        /// The screen before the window: its stage ("waiting", "lost",
        /// "moving", "offer", "ready"), its sentence, the move's progress,
        /// and a way back's answer.
        #[qobject]
        #[qml_element]
        #[qproperty(QString, stage)]
        #[qproperty(QString, said)]
        #[qproperty(i32, done)]
        #[qproperty(i32, total)]
        #[qproperty(QString, answer)]
        #[qproperty(bool, busy)]
        type StoreGate = super::StoreGateRust;

        /// A sentence in the person's language.
        #[qinvokable]
        fn text(self: &StoreGate, key: &QString) -> QString;
        /// A sentence with a count.
        #[qinvokable]
        fn text_counted(self: &StoreGate, key: &QString, n: i32) -> QString;
        /// Begins what the start left to do: waiting for the keyring, or the move.
        #[qinvokable]
        fn begin(self: Pin<&mut StoreGate>);
        /// A way back: the passphrase kept.
        #[qinvokable]
        fn use_passphrase(self: Pin<&mut StoreGate>, passphrase: &QString);
        /// A way back: the recovery code.
        #[qinvokable]
        fn use_code(self: Pin<&mut StoreGate>, code: &QString);
        /// A way back: the security key, with its PIN.
        #[qinvokable]
        fn use_card(self: Pin<&mut StoreGate>, pin: &QString);
        /// Without the key (the keyring holds none, said again here): what is sealed set aside, a new store made.
        #[qinvokable]
        fn start_again(self: Pin<&mut StoreGate>);
        /// Which passphrase the passphrase way wants: when it was kept, and whether it was the sharing's then; "" without one.
        #[qinvokable]
        fn passphrase_said(self: &StoreGate) -> QString;
        /// The ways back kept, as JSON (`ways_json`).
        #[qinvokable]
        fn store_ways(self: &StoreGate) -> QString;
        /// A passphrase way kept; "" or what went wrong.
        #[qinvokable]
        fn keep_passphrase_way(self: &StoreGate, passphrase: &QString, sharing: bool) -> QString;
        /// A new recovery code, to show once ("" when it could not be made).
        #[qinvokable]
        fn make_recovery_code(self: &StoreGate) -> QString;
        /// The security keys' ways kept; what was done, said.
        #[qinvokable]
        fn keep_card_way(self: &StoreGate) -> QString;
        /// A way taken out.
        #[qinvokable]
        fn forget_way(self: &StoreGate, kind: &QString) -> QString;
    }

    impl cxx_qt::Threading for StoreGate {}
}

/// The screen's state, in Rust.
#[derive(Default)]
pub struct StoreGateRust {
    stage: QString,
    said: QString,
    done: i32,
    total: i32,
    answer: QString,
    busy: bool,
}

/// What `begin` found, handed to the screen (`set`), taken once.
static BEGIN: Mutex<Option<Begin>> = Mutex::new(None);
/// Whether the store opened: the window opens then, and only then.
static OPENED: AtomicBool = AtomicBool::new(false);
/// Whether the move ran past its first file here (one at a time).
static PROCEEDING: AtomicBool = AtomicBool::new(false);

/// The environment's name for the descriptor the window gives the screen to
/// hand its word on: one end of a pipe of the window's, never the standard
/// output, which a terminal or a log may show.
pub(crate) const HAND_OVER_VAR: &str = "SIOUL_GATE_HAND_OVER";

/// Where the screen's process hands its word to the window (its parent): the
/// pipe's end the window gave it, kept under a descriptor no program it
/// starts inherits (`keep_hand_over`); -1 when there is none, and then the
/// key goes nowhere.
static HAND_OVER: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(-1);

/// The descriptor `var` names, when it is one end of a pipe (a FIFO): never a
/// terminal, a file or a socket a log could keep (third check).
#[cfg(target_os = "linux")]
fn hand_over_fd(var: Option<&str>) -> Option<i32> {
    let fd: i32 = var?.trim().parse().ok().filter(|fd| *fd > 2)?;
    // SAFETY: fstat on a descriptor number fills the zeroed struct or fails; nothing else is touched.
    let mut stat: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstat(fd, &mut stat) } != 0 {
        return None;
    }
    (stat.st_mode & libc::S_IFMT == libc::S_IFIFO).then_some(fd)
}

/// The screen's process, as it starts: the pipe's end the window gave it
/// (`SIOUL_GATE_HAND_OVER`), when it is one, kept under a descriptor no
/// program it starts inherits. Anything else (run by hand in a terminal, the
/// variable naming the standard output, a file) is left alone, and no key is
/// ever written.
pub(crate) fn keep_hand_over() {
    #[cfg(target_os = "linux")]
    if let Some(fd) = hand_over_fd(std::env::var(HAND_OVER_VAR).ok().as_deref()) {
        // SAFETY: plain descriptor calls on a descriptor of this process's own.
        let kept = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 10) };
        if kept >= 0 {
            // SAFETY: as above; the original goes, its copy stays.
            unsafe { libc::close(fd) };
            HAND_OVER.store(kept, Ordering::Release);
        }
    }
}

/// `bytes` written whole on the hand-over pipe; an error, and nothing
/// written anywhere, when there is none.
fn hand(bytes: &[u8]) -> std::io::Result<()> {
    let fd = HAND_OVER.load(Ordering::Acquire);
    if fd < 0 {
        return Err(std::io::Error::new(std::io::ErrorKind::NotConnected, "no pipe from the window: nothing handed"));
    }
    #[cfg(target_os = "linux")]
    {
        let mut at = 0;
        while at < bytes.len() {
            // SAFETY: `fd` is this process's own descriptor, kept open until it ends; the slice is alive for the call.
            let n = unsafe { libc::write(fd, bytes[at..].as_ptr().cast(), bytes.len() - at) };
            if n < 0 {
                let e = std::io::Error::last_os_error();
                if e.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(e);
            }
            at += n as usize;
        }
        Ok(())
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = bytes;
        Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "no pipe from the window here"))
    }
}

/// The screen's process, at its end, the store open: the key it opened with,
/// and what the window says once (files left in clear, set aside and where),
/// written on the window's pipe alone; nothing on the disk, nothing on the
/// standard output. Without the pipe (run by hand, a system without it), the
/// window asks the keyring itself.
pub(crate) fn leave_word() {
    if !opened() {
        return;
    }
    let folder = crate::ASIDE_FOLDER.lock().map(|f| f.clone()).unwrap_or_default();
    let word = word(storekey::remembered().as_ref(), crate::SET_ASIDE.load(Ordering::Relaxed), &folder, crate::LEFT_PLAIN.load(Ordering::Relaxed));
    if let Err(e) = hand(word.as_bytes()) {
        eprintln!("Sioul: the screen before the window handed nothing to the window: {e}");
    }
}

/// The word the screen hands the window: one line per thing, the key in hex.
fn word(key: Option<&sioul_core::sealed::RootKey>, aside: usize, aside_folder: &str, left: usize) -> zeroize::Zeroizing<String> {
    let mut word = zeroize::Zeroizing::new(String::new());
    if let Some(key) = key {
        word.push_str("key ");
        for b in key.bytes() {
            word.push_str(&format!("{b:02x}"));
        }
        word.push('\n');
    }
    word.push_str(&format!("aside {aside}\nleft {left}\n"));
    if !aside_folder.is_empty() && !aside_folder.contains('\n') {
        word.push_str(&format!("aside_folder {aside_folder}\n"));
    }
    word
}

/// The window's process: the screen's word read from its pipe, the counts
/// taken in (said once by the window); the key it handed, if any.
pub(crate) fn told_from_gate(word: &str) -> Option<sioul_core::sealed::RootKey> {
    let mut key = None;
    for line in word.lines() {
        let Some((name, value)) = line.split_once(' ') else { continue };
        let count = value.trim().parse::<usize>().unwrap_or(0);
        match name {
            "key" if value.len() == 64 => {
                let mut bytes = zeroize::Zeroizing::new([0u8; 32]);
                if (0..32).all(|i| u8::from_str_radix(&value[2 * i..2 * i + 2], 16).map(|b| bytes[i] = b).is_ok()) {
                    key = Some(sioul_core::sealed::RootKey::from_bytes(*bytes));
                }
            }
            "aside" => crate::SET_ASIDE.store(count, Ordering::Relaxed),
            "aside_folder" => {
                if let Ok(mut folder) = crate::ASIDE_FOLDER.lock() {
                    *folder = value.trim().to_string();
                }
            }
            "left" => crate::LEFT_PLAIN.store(count, Ordering::Relaxed),
            _ => {}
        }
    }
    key
}

/// What the start left to do, for the screen.
pub(crate) fn set(begin: Begin) {
    *BEGIN.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(begin);
}

/// Whether what Sioul keeps is open now: the window may start.
pub(crate) fn opened() -> bool {
    OPENED.load(Ordering::Acquire)
}

fn tr() -> &'static sioul_core::i18n::Translator {
    crate::backend::tr()
}

fn say(key: &str) -> String {
    tr().text(key, None)
}

/// A key error in the person's words.
fn said_of(e: &KeyError) -> String {
    match e {
        KeyError::Locked => say("store-key-locked"),
        KeyError::Missing => say("store-gate-missing"),
        KeyError::NoService => say("store-key-no-service"),
        KeyError::WrongPassphrase => say("store-wrong-passphrase"),
        KeyError::WrongCode => say("store-wrong-code"),
        KeyError::NoCard(_) => say("store-no-card"),
        KeyError::NoStore => say("store-not-created"),
        KeyError::Other(why) => why.clone(),
    }
}

type Qt = cxx_qt::CxxQtThread<qobject::StoreGate>;

fn tell(qt: &Qt, stage: Option<&'static str>, said: String) {
    let _ = qt.queue(move |mut gate| {
        if let Some(stage) = stage {
            gate.as_mut().set_stage(QString::from(stage));
        }
        gate.as_mut().set_said(QString::from(&said));
    });
}

fn answered(qt: &Qt, answer: String) {
    let _ = qt.queue(move |mut gate| {
        gate.as_mut().set_answer(QString::from(&answer));
        gate.as_mut().set_busy(false);
    });
}

/// The store opened with `key` (the keyring's or a way back's), then the
/// plain files moved in, the progress shown; once.
fn proceed(qt: &Qt, key: &sioul_core::sealed::RootKey, from_way: bool, made: bool) {
    if PROCEEDING.swap(true, Ordering::AcqRel) {
        return;
    }
    let folders = sioul_core::sealed::Folders::of_profile();
    let store = match storekey::opened_with(&folders, key, "window") {
        Ok(store) => store,
        Err(e) => {
            PROCEEDING.store(false, Ordering::Release);
            answered(qt, said_of(&e));
            return;
        }
    };
    if from_way && let Err(e) = storekey::keep_in_keyring(&folders, key) {
        eprintln!("Sioul: the key a way back gave could not be kept in the keyring again: {e}");
    }
    move_in(qt, &store, made);
}

/// The plain files moved in, the progress shown; then the window, or the ways back offered at a first sealing.
fn move_in(qt: &Qt, store: &sioul_core::sealed::Store, made: bool) {
    tell(qt, Some("moving"), String::new());
    let progress = |done: usize, total: usize| {
        let (done, total) = (i32::try_from(done).unwrap_or(i32::MAX), i32::try_from(total).unwrap_or(i32::MAX));
        let _ = qt.queue(move |mut gate| {
            gate.as_mut().set_done(done);
            gate.as_mut().set_total(total);
        });
    };
    match storekey::move_now(store, &progress) {
        Ok(moved) => {
            crate::SET_ASIDE.store(moved.set_aside, Ordering::Relaxed);
            crate::LEFT_PLAIN.store(moved.left.len(), Ordering::Relaxed);
            if let (Some(folder), Ok(mut kept)) = (moved.aside_folder.clone(), crate::ASIDE_FOLDER.lock()) {
                *kept = folder;
            }
            eprintln!("Sioul: what Sioul keeps on this device is sealed ({} files moved in, {} left in clear)", moved.files, moved.left.len());
            OPENED.store(true, Ordering::Release);
            // The ways back offered once at a first sealing; the sandbox's picture runs (`SIOUL_GRAB`) go straight on.
            let offer = made && std::env::var_os("SIOUL_GRAB").is_none();
            tell(qt, Some(if offer { "offer" } else { "ready" }), say("store-gate-sealed"));
        }
        // Stopped: what was sealed stays sealed, what was not stays in clear and
        // reads as before; the window opens all the same, and the next start goes on.
        Err(e) => {
            eprintln!("Sioul: the sealing stopped: {e}");
            crate::LEFT_PLAIN.store(usize::MAX, Ordering::Relaxed);
            OPENED.store(true, Ordering::Release);
            tell(qt, Some("ready"), say("store-gate-move-stopped"));
        }
    }
}

impl qobject::StoreGate {
    fn text(&self, key: &QString) -> QString {
        QString::from(&say(&key.to_string()))
    }

    fn text_counted(&self, key: &QString, n: i32) -> QString {
        QString::from(&tr().text(&key.to_string(), Some(&tr().counted(usize::try_from(n).unwrap_or(0)))))
    }

    fn begin(self: Pin<&mut Self>) {
        let Some(begin) = BEGIN.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take() else { return };
        let qt = self.qt_thread();
        match begin {
            Begin::Move { made } => {
                std::thread::spawn(move || {
                    if let Ok(store) = sioul_core::sealed::opened() {
                        PROCEEDING.store(true, Ordering::Release);
                        move_in(&qt, &store, made);
                    }
                });
            }
            Begin::NeedsKey(e) => {
                let lost = matches!(e, KeyError::Missing);
                tell(&qt, Some(if lost { "lost" } else { "waiting" }), said_of(&e));
                if lost {
                    return;
                }
                // The keyring asked once (it may ask to be opened), then looked at again each second, quietly.
                std::thread::spawn(move || {
                    let folders = sioul_core::sealed::Folders::of_profile();
                    let mut asked = false;
                    while !PROCEEDING.load(Ordering::Acquire) {
                        let got = if asked { storekey::root_key(&folders) } else { storekey::root_key_asking_now(&folders) };
                        asked = true;
                        match got {
                            Ok(key) => return proceed(&qt, &key, false, false),
                            Err(KeyError::Missing) => return tell(&qt, Some("lost"), say("store-gate-missing")),
                            Err(_) => std::thread::sleep(std::time::Duration::from_secs(1)),
                        }
                    }
                });
            }
            Begin::Ready | Begin::Plain(_) => {
                OPENED.store(true, Ordering::Release);
                tell(&qt, Some("ready"), String::new());
            }
        }
    }

    fn use_passphrase(self: Pin<&mut Self>, passphrase: &QString) {
        let passphrase = passphrase.to_string();
        self.way(move |folders| storekey::recover_with_passphrase(folders, &passphrase));
    }

    fn use_code(self: Pin<&mut Self>, code: &QString) {
        let code = code.to_string();
        self.way(move |folders| storekey::recover_with_code(folders, &code));
    }

    fn use_card(self: Pin<&mut Self>, pin: &QString) {
        let pin = pin.to_string();
        self.way(move |folders| storekey::recover_with_card(folders, &pin, sioul_sync::securitykey::readers()));
    }

    /// A way back tried off the window's thread; the store opened with what it gave.
    fn way(mut self: Pin<&mut Self>, recover: impl FnOnce(&sioul_core::sealed::Folders) -> Result<sioul_core::sealed::RootKey, KeyError> + Send + 'static) {
        self.as_mut().set_busy(true);
        self.as_mut().set_answer(QString::default());
        let qt = self.qt_thread();
        std::thread::spawn(move || {
            let folders = sioul_core::sealed::Folders::of_profile();
            match recover(&folders) {
                Ok(key) => proceed(&qt, &key, true, false),
                Err(e) => answered(&qt, said_of(&e)),
            }
        });
    }

    fn start_again(mut self: Pin<&mut Self>) {
        self.as_mut().set_busy(true);
        let qt = self.qt_thread();
        std::thread::spawn(move || {
            let folders = sioul_core::sealed::Folders::of_profile();
            // Asked again now: never while the keyring is merely locked, which would open it by itself.
            match storekey::root_key(&folders) {
                Err(KeyError::Missing) => {}
                Ok(key) => return proceed(&qt, &key, false, false),
                Err(e) => {
                    tell(&qt, Some("waiting"), said_of(&e));
                    return answered(&qt, say("store-gate-start-again-not-now"));
                }
            }
            if PROCEEDING.swap(true, Ordering::AcqRel) {
                return;
            }
            match storekey::start_again(&folders, sioul_core::sealed::Options::for_profile("window")) {
                Ok(store) => {
                    let store = sioul_core::sealed::install(store);
                    std::mem::forget(sioul_core::sealed::files::mount(store.clone()));
                    move_in(&qt, &store, true);
                }
                Err(e) => {
                    PROCEEDING.store(false, Ordering::Release);
                    answered(&qt, said_of(&e));
                }
            }
        });
    }

    fn store_ways(&self) -> QString {
        QString::from(&ways_json())
    }

    fn passphrase_said(&self) -> QString {
        QString::from(&passphrase_said())
    }

    fn keep_passphrase_way(&self, passphrase: &QString, sharing: bool) -> QString {
        QString::from(&keep_passphrase(&passphrase.to_string(), sharing))
    }

    fn make_recovery_code(&self) -> QString {
        QString::from(&new_code())
    }

    fn keep_card_way(&self) -> QString {
        QString::from(&keep_cards())
    }

    fn forget_way(&self, kind: &QString) -> QString {
        QString::from(&forget(&kind.to_string()))
    }
}

/// Which passphrase the passphrase way wants, said under its field: when it
/// was kept, and that it was the sharing's then (a passphrase way stays the
/// one it was kept with, whatever the sharing's becomes: review-finish G10).
pub(crate) fn passphrase_said() -> String {
    let folders = sioul_core::sealed::Folders::of_profile();
    let Some(way) = storekey::ways(&folders).into_iter().find(|w| w.kind == "passphrase") else { return String::new() };
    let date = sioul_core::securitykey::full_date(tr(), way.made_ms / 1000);
    let key = if way.said == "sharing" { "store-gate-passphrase-sharing-then" } else { "store-gate-passphrase-kept-on" };
    crate::backend::say(key, &[("date", date)])
}

/// Where a sealed file's copy in clear waits for another program to open it:
/// the session's runtime folder, in memory and gone at the session's end,
/// when there is one; else Sioul's cache (`attachments/opened/`, in clear by
/// rule). Each profile its own; emptied at every start (`forget_opened`),
/// so that no copy in clear of a sealed paper stays past its use (review-finish G8).
fn opened_folder() -> std::path::PathBuf {
    let state = sioul_core::config::state_dir();
    // FNV-1a, as the window's socket's name: the same from one build to the next.
    let hash = state.as_os_str().as_encoded_bytes().iter().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3));
    match std::env::var_os("XDG_RUNTIME_DIR").filter(|v| !v.is_empty()).map(std::path::PathBuf::from).filter(|dir| crate::files::is_dir(dir)) {
        Some(runtime) => runtime.join(format!("sioul-opened-{hash:016x}")),
        None => sioul_core::config::cache_dir().join("attachments").join("opened"),
    }
}

/// The copies opened at an earlier start taken away, overwritten first where
/// they lay on a disk (the window's start, before anything opens).
pub(crate) fn forget_opened() {
    let folder = opened_folder();
    if crate::files::exists(&folder) {
        let _ = sioul_core::filelock::remove_dir_wiping(&folder);
    }
}

/// A sealed file's copy in clear for another program to open (`opened_folder`,
/// made again each time); none for a file of the system, which opens as it is.
pub(crate) fn openable(path: &std::path::Path) -> Option<std::path::PathBuf> {
    use std::hash::{Hash, Hasher};
    if !crate::files::is_sealed(path) || !crate::files::is_file(path) {
        return None;
    }
    let bytes = crate::files::read(path).ok()?;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut hasher);
    let top = opened_folder();
    let folder = top.join(format!("{:016x}", hasher.finish()));
    crate::files::DirBuilder::new().recursive(true).mode(0o700).create(&folder).ok()?;
    let copy = folder.join(path.file_name()?);
    crate::files::write(&copy, bytes).ok()?;
    Some(copy)
}

/// The ways back kept, what the profile holds elsewhere, and n: `{"sealed":
/// true, "ways": ["passphrase", "code", "card"], "sharing": true, "versions": 2}`.
pub(crate) fn ways_json() -> String {
    let folders = sioul_core::sealed::Folders::of_profile();
    let sealed = sioul_core::sealed::opened().is_ok();
    let ways: Vec<String> = storekey::ways(&folders).into_iter().map(|w| w.kind).collect();
    // A security key reset or given a new encryption key: its way opens nothing now, said once with an offer to seal again (review-finish G10).
    let stale = if ways.iter().any(|w| w == "card") { storekey::stale_cards(&folders, &sioul_core::pgp::Keys::load()) } else { Vec::new() };
    let sharing = crate::share::here().folder_path().is_some();
    let versions = sioul_core::sealed::opened().map_or(sioul_core::sealed::VERSIONS, |s| s.versions_kept());
    // Sioul's lists still in the notes folder (not moved to its own folder yet): in clear there (review-finish F8).
    let lists_clear = sealed
        && crate::backend::load_config().records_root().is_some_and(|records| !sioul_core::sealed::files::inside_sioul(&records));
    serde_json::json!({ "sealed": sealed, "ways": ways, "stale": stale, "passphrase": passphrase_said(), "sharing": sharing, "versions": versions, "lists_clear": lists_clear }).to_string()
}

fn with_key(f: impl FnOnce(&sioul_core::sealed::Folders, &sioul_core::sealed::RootKey) -> Result<String, KeyError>) -> String {
    let folders = sioul_core::sealed::Folders::of_profile();
    match storekey::remembered() {
        Some(key) => f(&folders, &key).unwrap_or_else(|e| said_of(&e)),
        None => say("store-ways-not-open"),
    }
}

/// A passphrase way kept: "" when done, else why not.
pub(crate) fn keep_passphrase(passphrase: &str, sharing: bool) -> String {
    if passphrase.chars().count() < storekey::PASSPHRASE_CHARS {
        return tr().text("store-way-passphrase-short", Some(&tr().counted(storekey::PASSPHRASE_CHARS)));
    }
    with_key(|folders, key| storekey::keep_passphrase_way(folders, key, passphrase, if sharing { "sharing" } else { "" }).map(|()| String::new()))
}

/// A new recovery code, shown once; "" when none could be made (said on stderr).
pub(crate) fn new_code() -> String {
    let folders = sioul_core::sealed::Folders::of_profile();
    let Some(key) = storekey::remembered() else { return String::new() };
    storekey::make_recovery_code(&folders, &key).unwrap_or_else(|e| {
        eprintln!("Sioul: a recovery code could not be made: {e}");
        String::new()
    })
}

/// The security keys' ways kept: what was done, in a sentence.
pub(crate) fn keep_cards() -> String {
    with_key(|folders, key| {
        let keys = sioul_core::pgp::Keys::load();
        storekey::keep_card_ways(folders, key, &keys).map(|n| if n == 0 { say("store-way-card-none") } else { say("store-way-done") })
    })
}

/// A way taken out: "" when done.
pub(crate) fn forget(kind: &str) -> String {
    let folders = sioul_core::sealed::Folders::of_profile();
    storekey::forget_way(&folders, kind).map_or_else(|e| said_of(&e), |()| String::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sioul_core::sealed::{Folders, Options, RootKey, Store};

    /// Each way back gives the store's key, which the screen hands the window
    /// on its pipe (`word`, `told_from_gate`): the window opens the store with
    /// it, the keyring never asked (these tests keep nothing there: `no-keyring`), so a
    /// way back opens Sioul whether the keyring is locked, absent, or refuses
    /// to keep the key again (review-finish G5). The security key's way is
    /// opened by a stand-in card in sioul-sync's own test.
    #[test]
    fn each_way_back_s_key_reaches_the_window_without_the_keyring() {
        let base = std::env::temp_dir().join(format!("sioul-gate-ways-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let folders = Folders::under(&base);
        let key = RootKey::from_bytes([8u8; 32]);
        drop(Store::create(&folders, &key, Options::default()).unwrap());
        storekey::keep_passphrase_way(&folders, &key, "four long words of mine", "").unwrap();
        let code = storekey::make_recovery_code(&folders, &key).unwrap();
        let ways = [("passphrase", storekey::recover_with_passphrase(&folders, "four long words of mine")), ("code", storekey::recover_with_code(&folders, &code))];
        for (name, given) in ways {
            let given = given.unwrap();
            let said = word(Some(&given), 0, "", 1);
            assert!(said.starts_with("key ") && !said.contains("four long words") && !said.contains(&code), "{name}: the key alone, in hex");
            let handed = parse(&said).unwrap_or_else(|| panic!("{name}: no key handed"));
            assert_eq!(handed.bytes(), key.bytes(), "{name}");
            assert!(Store::open(&folders, &handed, Options::default()).is_ok(), "{name}: the window opens the store with it");
        }
        // A word without a key (a profile left in clear) or a damaged one hands none.
        assert!(parse(&word(None, 0, "", 0)).is_none() && parse("key 12zz\n").is_none());
        let _ = std::fs::remove_dir_all(&base);
    }

    /// The screen run by hand in a terminal, or with its hand-over naming a
    /// terminal, a file or the standard output, writes the key nowhere: only
    /// one end of a pipe is taken (`/dev/null` stands for a terminal, a
    /// character device as it is), and without one `hand` writes nothing at
    /// all, never falling back on the standard output (third check).
    #[cfg(target_os = "linux")]
    #[test]
    fn the_screen_never_prints_the_key_where_a_terminal_or_a_log_could_show_it() {
        use std::os::fd::AsRawFd;
        let tty_like = std::fs::OpenOptions::new().write(true).open("/dev/null").unwrap();
        let base = std::env::temp_dir().join(format!("sioul-gate-handover-{}", std::process::id()));
        let file = std::fs::File::create(&base).unwrap();
        let mut fds = [0i32; 2];
        // SAFETY: a pipe made for the test, both ends closed below.
        assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0);
        let refused = [None, Some("".to_string()), Some("1".to_string()), Some("2".to_string()), Some(tty_like.as_raw_fd().to_string()), Some(file.as_raw_fd().to_string()), Some("not a number".to_string())];
        for var in &refused {
            assert_eq!(hand_over_fd(var.as_deref()), None, "{var:?}: never a hand-over");
        }
        assert_eq!(hand_over_fd(Some(&fds[1].to_string())), Some(fds[1]), "one end of a pipe: taken");
        // No pipe kept: nothing written anywhere, the standard output included.
        assert_eq!(HAND_OVER.load(Ordering::Acquire), -1);
        assert!(hand(b"key 00\n").is_err());
        // SAFETY: the test's own descriptors.
        unsafe {
            libc::close(fds[0]);
            libc::close(fds[1]);
        }
        drop(file);
        let _ = std::fs::remove_file(&base);
    }

    /// `told_from_gate`'s key alone, its counts left as they are for the other tests.
    fn parse(word: &str) -> Option<RootKey> {
        let keyed: String = word.lines().filter(|l| l.starts_with("key ")).map(|l| format!("{l}\n")).collect();
        told_from_gate(&keyed)
    }
}
