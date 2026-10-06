// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Writing from other applications (docs/client.md, "Writing from other
//! applications"; docs/android.md, "Sharing"). Files and text shared to
//! Sioul, one of your addresses chosen in Android's share sheet, a `mailto:`
//! link: each request (`sioul_core::handed`) becomes a draft whose window
//! opens. On a phone each address that can send is a direct-share target
//! (`publish`); on a computer `sioul-app mailto:…` is handed to the Sioul
//! already open (`hand_over`). Nothing is sent until you press Send.

use crate::backend::qobject::Sioul;
use crate::backend::{QtThread, load_config, say, tr};
use cxx_qt::Threading;
use cxx_qt_lib::QString;
use sioul_core::compose::Draft;
use sioul_core::config::{Account, Config};
use sioul_core::handed::{self, ArrivedFile, Handed, Incoming};
use std::collections::BTreeMap;
use std::pin::Pin;
use std::sync::Mutex;

/// The window, once it runs: what comes while it is open goes to it at once.
static WINDOW: Mutex<Option<QtThread>> = Mutex::new(None);

/// Drafts whose files are still being copied: how many, and those already
/// known not to come (refused), said with the rest when they arrive.
static COMING: Mutex<BTreeMap<String, (u32, Vec<ArrivedFile>)>> = Mutex::new(BTreeMap::new());

/// The window has started: what waits is taken, what comes later goes to it.
/// On the window's thread, from `start`.
pub(crate) fn started(mut sioul: Pin<&mut Sioul>) {
    if let Ok(mut window) = WINDOW.lock() {
        *window = Some(sioul.qt_thread());
    }
    take(sioul.as_mut());
    std::thread::spawn(|| {
        // Copies nobody holds any more, from a Sioul stopped while copying.
        let held: Vec<std::path::PathBuf> = Draft::all().into_iter().flat_map(|d| d.attachments).collect();
        Incoming::here().tidy(&held, std::time::SystemTime::now());
        publish(true);
    });
}

/// Something was handed (Android's ShareActivity, a second `sioul-app`):
/// taken now when the window runs, else when it starts. Any thread.
pub(crate) fn poke() {
    let window = WINDOW.lock().ok().and_then(|w| w.clone());
    if let Some(qt) = window {
        let _ = qt.queue(take);
    }
}

/// Whether draft `id` still waits for files being copied for it.
pub(crate) fn coming(id: &str) -> bool {
    COMING.lock().is_ok_and(|coming| coming.contains_key(id))
}

/// "Attaching 2 files…" while draft `id` waits for them, else "".
pub(crate) fn attaching(id: &str) -> String {
    let count = COMING.lock().ok().and_then(|coming| coming.get(id).map(|(files, _)| *files)).unwrap_or(0);
    if count == 0 { String::new() } else { say("compose-attaching", &[("count", count.to_string())]) }
}

/// The requests waiting (`drafts/incoming/`): files arrived for drafts made
/// before are attached, each new request made a draft whose window opens. On
/// the window's thread, so that it never crosses the writing window's own
/// saves: at the start, when the window comes back, at its minute, and when
/// something is handed.
pub(crate) fn take(mut sioul: Pin<&mut Sioul>) {
    let incoming = Incoming::here();
    let me = std::process::id();
    let mut changed = false;
    // Files come for drafts already open: all copied, or their copy stopped
    // with the Sioul that made it (another process).
    for mark in incoming.marks() {
        let arrived = match incoming.arrived(&mark.id) {
            Some(arrived) => arrived,
            None if mark.copier != me => incoming.salvage(&mark.id, mark.files),
            None => continue,
        };
        let refused = COMING.lock().ok().and_then(|mut coming| coming.remove(&mark.draft)).map(|(_, refused)| refused).unwrap_or_default();
        match Draft::by_id(&mark.draft) {
            Some(mut draft) => {
                let lost = handed::attach(&mut draft, &incoming.files_of(&mark.id), &arrived);
                let mut attached = arrived.files.len().saturating_sub(lost.len());
                let mut missing = refused;
                missing.extend(lost);
                if let Err(e) = draft.save() {
                    attached = 0;
                    missing.push(ArrivedFile { error: e, ..ArrivedFile::default() });
                }
                let (line, problem) = arrival_line(attached, &missing);
                sioul.as_mut().draft_files_arrived(QString::from(&mark.draft), QString::from(&line), problem);
                incoming.done(&mark.id);
            }
            // Sent or discarded meanwhile: its copies go too.
            None => incoming.drop_request(&mark.id),
        }
        changed = true;
    }
    for request in incoming.waiting() {
        changed = true;
        made(sioul.as_mut(), &incoming, &request, me);
    }
    if changed {
        crate::mail::show_mail(&sioul.qt_thread(), &sioul.shared());
    }
}

/// One request made a draft, its window opened.
fn made(mut sioul: Pin<&mut Sioul>, incoming: &Incoming, request: &Handed, me: u32) {
    let config = load_config();
    let Some(mut draft) = handed::draft(request, &config) else {
        // No address that sends yet: said, and what was copied for it goes.
        sioul.as_mut().set_status(QString::from(&tr().text("handed-no-account", None)));
        incoming.drop_request(&request.id);
        return;
    };
    let refused: Vec<ArrivedFile> = request.refused.iter().map(|name| ArrivedFile { name: name.clone(), ..ArrivedFile::default() }).collect();
    // The files: none asked, all there already, or their copy stopped with another Sioul.
    let arrived = if request.files == 0 {
        Some(Default::default())
    } else {
        incoming.arrived(&request.id).or_else(|| (request.copier != me).then(|| incoming.salvage(&request.id, request.files)))
    };
    let mut missing = refused.clone();
    let mut attached = 0;
    if let Some(arrived) = &arrived {
        let lost = handed::attach(&mut draft, &incoming.files_of(&request.id), arrived);
        attached = arrived.files.len().saturating_sub(lost.len());
        missing.extend(lost);
    }
    if let Err(e) = draft.save() {
        sioul.as_mut().set_status(QString::from(&e));
        // Read again at the next showing, unless it can never be saved: the copies stay meanwhile.
        return;
    }
    match arrived {
        Some(_) => incoming.done(&request.id),
        None => {
            if let Ok(mut coming) = COMING.lock() {
                coming.insert(draft.id.clone(), (request.files, refused));
            }
            if let Err(e) = incoming.mark(&request.id, &draft.id, request.copier, request.files) {
                eprintln!("Sioul: {e}");
            }
        }
    }
    sioul.as_mut().compose_requested(QString::from(&draft.id));
    // What did not come is said in the writing window, now that it is open.
    if !coming(&draft.id) && (!missing.is_empty() || attached > 0) {
        let (line, problem) = arrival_line(attached, &missing);
        sioul.as_mut().draft_files_arrived(QString::from(&draft.id), QString::from(&line), problem);
    }
}

/// What the writing window says once the files came: "The 2 files are
/// attached.", or those not attached and why.
fn arrival_line(attached: usize, missing: &[ArrivedFile]) -> (String, bool) {
    if missing.is_empty() {
        return (say("handed-attached", &[("count", attached.to_string())]), false);
    }
    let named: Vec<String> = missing
        .iter()
        .map(|file| {
            let name = if file.name.is_empty() { tr().text("handed-a-file", None) } else { file.name.clone() };
            let why = if file.stopped {
                tr().text("handed-stopped", None)
            } else if file.error.is_empty() {
                tr().text("handed-refused", None)
            } else {
                file.error.clone()
            };
            format!("{name} ({why})")
        })
        .collect();
    (say("handed-missing", &[("files", named.join(", ")), ("count", missing.len().to_string())]), true)
}

// ------------------------------------------------------------------ Android

/// One direct-share target: an address that can send, as Android shows it.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
struct Target {
    /// The account's id; the shortcut's is "mail:" and it.
    id: String,
    /// Under its icon in the share sheet: the address.
    short: String,
    /// At a long press on Sioul's icon: "Write from you@example.org".
    long: String,
}

/// The direct-share targets: each account that sends, the most used first
/// (`sent`: messages in its Sent folders here), then by priority, then in
/// the file's order.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn targets(config: &Config, sent: &BTreeMap<String, usize>) -> Vec<Target> {
    let mut accounts: Vec<(usize, &Account)> = config.accounts.iter().filter(|a| a.syncs()).enumerate().collect();
    accounts.sort_by_key(|(order, account)| (std::cmp::Reverse(sent.get(&account.id).copied().unwrap_or(0)), account.priority, *order));
    accounts
        .into_iter()
        .map(|(_, account)| {
            let address = account.address.clone().filter(|a| !a.trim().is_empty()).unwrap_or_else(|| account.id.clone());
            Target { id: account.id.clone(), long: say("share-write-from", &[("address", address.clone())]), short: address }
        })
        .collect()
}

/// How many messages each account's Sent folders hold here: what "most
/// used" means for the share sheet. Read from the folders' listings only.
#[cfg(target_os = "android")]
fn sent(config: &Config) -> BTreeMap<String, usize> {
    let count = |dir: std::path::PathBuf| std::fs::read_dir(dir).map(|entries| entries.count()).unwrap_or(0);
    config
        .accounts
        .iter()
        .filter(|a| a.syncs())
        .map(|account| {
            let root = account.maildir_path();
            let folders = sioul_sync::mailbox::folders(&account.id);
            let total: usize = folders.iter().filter(|f| f.role == sioul_core::folders::Role::Sent).map(|f| count(root.join(&f.local).join("cur")) + count(root.join(&f.local).join("new"))).sum();
            (account.id.clone(), total)
        })
        .collect()
}

#[cfg(target_os = "android")]
unsafe extern "C" {
    /// Sioul's addresses that send, as Android's direct-share targets
    /// (android/main.cpp, MailShortcuts.java): JSON {accounts, gone}.
    fn sioul_android_mail_shortcuts(json: *const std::ffi::c_char);
}

/// Android: the direct-share targets brought in step with the accounts, when
/// they changed (added, renamed, removed, their order). `recount` counts the
/// Sent folders again (at the start, when the window comes back); otherwise
/// the last counts rank them. Only while the window is shown: Android limits
/// how often an app in the background may change them. Nothing elsewhere.
pub(crate) fn publish(recount: bool) {
    #[cfg(target_os = "android")]
    {
        static SENT: Mutex<BTreeMap<String, usize>> = Mutex::new(BTreeMap::new());
        static GIVEN: Mutex<String> = Mutex::new(String::new());
        let config = load_config();
        let sent = if recount {
            let counted = sent(&config);
            if let Ok(mut kept) = SENT.lock() {
                *kept = counted.clone();
            }
            counted
        } else {
            SENT.lock().map(|kept| kept.clone()).unwrap_or_default()
        };
        let json = serde_json::json!({ "accounts": targets(&config, &sent), "gone": tr().text("share-address-gone", None) }).to_string();
        let Ok(mut given) = GIVEN.lock() else { return };
        if *given == json {
            return;
        }
        let Ok(text) = std::ffi::CString::new(json.clone()) else { return };
        // SAFETY: a zero-terminated text, valid for the call; any thread.
        unsafe { sioul_android_mail_shortcuts(text.as_ptr()) };
        *given = json;
    }
    let _ = recount;
}

/// Android's ShareActivity: a request written, or the files copied for one
/// (android/main.cpp). Any thread.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "C" fn sioul_handed() {
    let _ = std::panic::catch_unwind(poke);
}

// ------------------------------------------------------------------ a computer

/// The program's arguments that are `mailto:` addresses (the desktop file's
/// `%u`: a mail link clicked while Sioul is the mail program).
pub(crate) fn addresses() -> Vec<String> {
    std::env::args_os().skip(1).filter_map(|a| a.into_string().ok()).filter(|a| sioul_core::mailto::is_mailto(a)).collect()
}

/// `addresses` handed to the window: taken at its start, or at once.
pub(crate) fn hand(addresses: &[String]) {
    let incoming = Incoming::here();
    for address in addresses {
        let request = Handed { mailto: address.clone(), ..Handed::new() };
        if let Err(e) = incoming.hand(&request) {
            eprintln!("Sioul: {e}");
        }
    }
    poke();
}

/// Where the Sioul of this profile listens for another one started after it:
/// in the session's runtime folder (Flatpak's own for its applications), its
/// name made from the state folder's, so that a test or a demo profile never
/// reaches yours; else in the state folder itself. None where it would not
/// fit a socket's name.
#[cfg(all(unix, not(target_os = "android")))]
fn socket() -> Option<std::path::PathBuf> {
    let state = sioul_core::config::state_dir();
    // FNV-1a: the same name from one build to the next.
    let hash = state.as_os_str().as_encoded_bytes().iter().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3));
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").filter(|v| !v.is_empty()).map(std::path::PathBuf::from);
    let path = match (runtime, std::env::var("FLATPAK_ID").ok().filter(|id| !id.is_empty())) {
        (Some(runtime), Some(app)) => runtime.join("app").join(app).join(format!("sioul-{hash:016x}.sock")),
        (Some(runtime), None) => runtime.join(format!("sioul-{hash:016x}.sock")),
        (None, _) => state.join("window.sock"),
    };
    // A socket's name holds 107 bytes on Linux, 103 on macOS.
    (path.as_os_str().len() < 100).then_some(path)
}

/// Another Sioul of this profile is open: `addresses` handed to it (with
/// none, its window brought forward), and true: this one has nothing more to
/// do. False when none answers: this one opens, and listens (`listen`).
#[cfg(all(unix, not(target_os = "android")))]
pub(crate) fn hand_over(addresses: &[String]) -> bool {
    !alone() && socket().is_some_and(|path| hand_over_at(&path, addresses))
}

/// The window's pictures and the demo (SIOUL_GRAB, SIOUL_DEMO) run beside
/// each other on one made-up profile: each stays its own, neither handing
/// over nor listening.
#[cfg(all(unix, not(target_os = "android")))]
fn alone() -> bool {
    std::env::var_os("SIOUL_GRAB").is_some() || std::env::var_os("SIOUL_DEMO").is_some()
}

#[cfg(not(all(unix, not(target_os = "android"))))]
pub(crate) fn hand_over(_addresses: &[String]) -> bool {
    false
}

#[cfg(all(unix, not(target_os = "android")))]
fn hand_over_at(path: &std::path::Path, addresses: &[String]) -> bool {
    use std::io::{BufRead, BufReader, Write};
    let Ok(mut stream) = std::os::unix::net::UnixStream::connect(path) else { return false };
    let wait = Some(std::time::Duration::from_secs(5));
    let _ = stream.set_read_timeout(wait);
    let _ = stream.set_write_timeout(wait);
    let asked = serde_json::json!({ "open": addresses }).to_string();
    if stream.write_all(format!("{asked}\n").as_bytes()).is_err() {
        return false;
    }
    let mut answer = String::new();
    // An open Sioul that does not answer (frozen): this one opens all the same.
    BufReader::new(stream).read_line(&mut answer).is_ok() && answer.trim() == "ok"
}

/// The first Sioul of this profile listens for those started after it: their
/// `mailto:` addresses become drafts here, a plain start brings this window
/// forward. A socket left by a Sioul that ended is replaced.
#[cfg(all(unix, not(target_os = "android")))]
pub(crate) fn listen() {
    if alone() {
        return;
    }
    let Some(path) = socket() else { return };
    let heard = |addresses: Vec<String>| {
        if addresses.is_empty() {
            let window = WINDOW.lock().ok().and_then(|w| w.clone());
            if let Some(qt) = window {
                // A reminder's "Open" with nothing to open: the window brought forward.
                let _ = qt.queue(|mut sioul| sioul.as_mut().reminder_opened(QString::default(), QString::default(), QString::default()));
            }
        } else {
            hand(&addresses);
        }
    };
    match listen_at(&path, heard) {
        Ok(()) => {
            if let Ok(mut listening) = LISTENING.lock() {
                *listening = Some(path);
            }
        }
        Err(e) => eprintln!("Sioul: {}: {e}; another Sioul started now would open beside this one.", path.display()),
    }
}

#[cfg(not(all(unix, not(target_os = "android"))))]
pub(crate) fn listen() {}

/// The socket this Sioul listens on, if it does.
#[cfg(all(unix, not(target_os = "android")))]
static LISTENING: Mutex<Option<std::path::PathBuf>> = Mutex::new(None);

/// Sioul ends: its socket goes with it, so that the next start does not knock there.
pub(crate) fn stop_listening() {
    #[cfg(all(unix, not(target_os = "android")))]
    if let Some(path) = LISTENING.lock().ok().and_then(|mut listening| listening.take()) {
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(all(unix, not(target_os = "android")))]
fn listen_at(path: &std::path::Path, heard: impl Fn(Vec<String>) + Send + 'static) -> std::io::Result<()> {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::os::unix::net::{UnixListener, UnixStream};
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let listener = match UnixListener::bind(path) {
        Ok(listener) => listener,
        // Left by a Sioul that ended: nobody answers there.
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse && UnixStream::connect(path).is_err() => {
            std::fs::remove_file(path)?;
            UnixListener::bind(path)?
        }
        Err(e) => return Err(e),
    };
    std::thread::spawn(move || {
        for stream in listener.incoming().filter_map(Result::ok) {
            let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(2)));
            let mut line = String::new();
            // One line of JSON, never more than 64 KB.
            if BufReader::new((&stream).take(64 * 1024)).read_line(&mut line).is_err() {
                continue;
            }
            let asked: serde_json::Value = serde_json::from_str(&line).unwrap_or_default();
            let addresses: Vec<String> = asked["open"]
                .as_array()
                .map(|all| all.iter().filter_map(|a| a.as_str()).filter(|a| sioul_core::mailto::is_mailto(a)).map(str::to_string).collect())
                .unwrap_or_default();
            if asked.get("open").is_some() {
                heard(addresses);
                let _ = (&stream).write_all(b"ok\n");
            }
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sioul_core::config::{Priority, Security};

    #[test]
    fn the_share_sheet_lists_the_most_used_addresses_first() {
        let mut config = Config::default();
        let work = Account::imap("work", "me@work.example", "imap.work.example", 993, Security::Tls, None);
        let mut home = Account::imap("home", "me@home.example", "imap.home.example", 993, Security::Tls, None);
        home.priority = Priority::Above;
        let mut bare = Account::imap("old", "", "imap.old.example", 993, Security::Tls, None);
        bare.address = None;
        let calendars = Account::dav("calendars", "me@dav.example", "dav.example", None, None);
        config.accounts = vec![work, home, bare, calendars];
        // Nothing sent from here yet: by priority, then the file's order; no address, its id.
        let order = |sent: &[(&str, usize)]| targets(&config, &sent.iter().map(|(id, n)| (id.to_string(), *n)).collect()).into_iter().map(|t| t.short).collect::<Vec<_>>();
        assert_eq!(order(&[]), ["me@home.example", "me@work.example", "old"]);
        // The most used first.
        assert_eq!(order(&[("work", 12), ("home", 3)]), ["me@work.example", "me@home.example", "old"]);
        assert_eq!(order(&[("old", 1)]), ["old", "me@home.example", "me@work.example"]);
        let first = &targets(&config, &BTreeMap::new())[0];
        assert_eq!(first.id, "home");
        assert!(first.long.contains("me@home.example"), "{}", first.long);
    }

    #[test]
    fn what_did_not_come_is_said() {
        let (line, problem) = arrival_line(2, &[]);
        assert!(!problem && line.contains('2'), "{line}");
        let missing = [
            ArrivedFile { name: "photo.jpg".into(), error: "Permission denied".into(), ..ArrivedFile::default() },
            ArrivedFile { name: "id_rsa".into(), ..ArrivedFile::default() },
            ArrivedFile { stopped: true, ..ArrivedFile::default() },
        ];
        let (line, problem) = arrival_line(1, &missing);
        assert!(problem && line.contains("photo.jpg (Permission denied)") && line.contains("id_rsa ("), "{line}");
        assert!(!line.contains("handed-"), "a message id left untranslated: {line}");
    }

    #[cfg(all(unix, not(target_os = "android")))]
    #[test]
    fn a_second_sioul_hands_its_link_to_the_first() {
        let dir = std::env::temp_dir().join(format!("sioul-hand-over-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("s.sock");
        // None open: this one opens.
        assert!(!hand_over_at(&path, &["mailto:a@example.org".into()]));
        // A socket left by a Sioul that ended is replaced.
        drop(std::os::unix::net::UnixListener::bind(&path).unwrap());
        let (send, heard) = std::sync::mpsc::channel();
        listen_at(&path, move |addresses| send.send(addresses).unwrap()).unwrap();
        assert!(hand_over_at(&path, &["mailto:a@example.org?subject=Hi".into()]));
        assert_eq!(heard.recv_timeout(std::time::Duration::from_secs(5)).unwrap(), ["mailto:a@example.org?subject=Hi"]);
        // A plain start: nothing to write, the window brought forward.
        assert!(hand_over_at(&path, &[]));
        assert!(heard.recv_timeout(std::time::Duration::from_secs(5)).unwrap().is_empty());
        // Only mailto: addresses are taken from another process.
        assert!(hand_over_at(&path, &["file:///etc/passwd".into(), "mailto:b@example.org".into()]));
        assert_eq!(heard.recv_timeout(std::time::Duration::from_secs(5)).unwrap(), ["mailto:b@example.org"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
