// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! A file a site offers, on a computer (docs/sites.md, "Downloads").
//! Qt WebEngine fetches a file whether or not it is answered, so the Sites
//! page accepts each request at once into a folder of its own in Sioul's
//! cache (`hold`: private, 0700), where it waits for your answer and goes
//! nowhere else; its card (DownloadCard.qml) shows what `offer` says of the
//! file: its name made one plain file name (with its type's extension when
//! it had none), its size, the host it comes from, a word of caution for a
//! program, the five places and which of them can take it, the one chosen
//! last for this site first. Then:
//!
//! - **Save** (`start`): the place checked and kept for the site; the status
//!   line says the file comes, if it still does.
//! - **Come whole and answered** (`done`, off the window's thread): checked
//!   by the antivirus when this computer has one (as an attachment is; the
//!   status line says "Checking…" only when one is there to ask), then
//!   placed where you said, under a free name, never over a file
//!   (`filelock::place_new`): the system's Downloads folder; the folder
//!   chosen in its "Save as" window; the notes folder, where new notes go;
//!   Papers by their own way in (the wallet's folder, a paper made at once
//!   with a title and a type guessed: `papers::file_download`); Letters by
//!   theirs (where scans arrive, read as a letter received:
//!   `letters::add_one`). The status line says it, and one notice names it,
//!   both adding that it was not checked, and why, when the antivirus found
//!   it too big or none answered; the notice is about an opaque id
//!   (`sioul:download/<id>`, never the file's path): Open, and Show in its
//!   folder for the system's folders; in Sioul for Notes, Papers and
//!   Letters. Never opened by itself. A place that refuses the file (full,
//!   closed, not its type) keeps it held: the card asks again, saying why
//!   (`download_refused`).
//! - **Failed or cut** (`failed`): its held folder taken away; said in the
//!   status line and by a notice.
//! - **Cancel** (`drop`): what came is deleted.
//! - **At start** (`at_start`), the whole held folder goes (no download can be
//!   under way then), and hidden files a day old that a stopped placing left
//!   in the papers' folder and where scans arrive.
//!
//! The rules (the name's cleaning, programs, hosts, the memory per site) are
//! the core's (`sioul_core::downloads`).

use crate::attachments::{Decision, Does, Found, System};
use crate::backend::{QtThread, Shared, json, load_config, say, tr};
use serde::{Deserialize, Serialize};
use sioul_core::config::{self, Config, NotesFolder};
use sioul_core::downloads::{self as rules, Memory, Place};
use sioul_core::notice::{Act, Kind, Notice};
use sioul_sync::antivirus::{self, Presence, Verdict};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

/// How much of a site's words a line shows: a file's name, a host.
const SHOWN_CHARS: usize = 60;

/// What the Sites page says of a file a site offers (Qt's `WebEngineDownloadRequest`),
/// read once, when it is offered.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Offered {
    /// The site's id in Sioul; "" for none (the vault's page).
    site: String,
    /// Its name, said when the file's address names no host.
    site_name: String,
    /// The file's address.
    url: String,
    /// The address of the page that offers it: the host of a "data:" file.
    page: String,
    /// The name the site gives, or the one in the card's field.
    name: String,
    /// The type it says.
    mime: String,
    /// Its size in bytes; 0 or less when the site does not say.
    total: f64,
}

/// One place, as the card shows it.
#[derive(Serialize, Debug)]
struct PlaceView {
    id: &'static str,
    label: String,
    /// One line under it: where, or what follows, or why it cannot take this file.
    detail: String,
    enabled: bool,
}

/// What the card shows (DownloadCard.qml).
#[derive(Serialize, Debug)]
struct OfferView {
    /// The name the field starts with: one plain file name (`rules::file_name`).
    name: String,
    /// The host it comes from, cut short; "" when its address names none.
    host: String,
    /// "1.2 MB, from secure.bank.example", or as much as is known.
    from: String,
    /// Where it waits meanwhile, and what Cancel does.
    waits: String,
    program: bool,
    /// A line of caution for a program; "" otherwise.
    caution: String,
    places: Vec<PlaceView>,
    /// The place offered first: the one chosen last for this site, when it can take the file.
    first: &'static str,
    /// The folder the system's "Save as" window opens in: the one chosen last, else Downloads.
    folder: String,
}

/// The host a file comes from: its address's, else its page's.
fn host_of(url: &str, page: &str) -> String {
    let host = rules::host_of(url);
    if host.is_empty() { rules::host_of(page) } else { host }
}

/// The kinds Letters read as scans (`letters::SCANS`).
fn scan_kind(name: &str) -> bool {
    let extension = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    ["pdf", "png", "jpg", "jpeg", "tif", "tiff", "webp"].contains(&extension.as_str())
}

/// Why a place cannot take a file of this name, type and size (`bytes`, 0
/// when not known yet): None when it can. Notes take what Notes show, a
/// text up to `notes::TEXT_NOTE_BYTES`, never a name Sioul reads as its own
/// list; Papers never a program nor such a name; Letters scans alone; and
/// each needs its folder on this device.
fn refusal(config: &Config, place: Place, name: &str, mime: &str, bytes: u64) -> Option<String> {
    let program = rules::is_program(name, mime);
    match place {
        Place::Downloads | Place::Folder => None,
        Place::Notes => match config.notes_folder() {
            NotesFolder::Ready(_) => match sioul_core::notes::kind_of_file(name) {
                _ if rules::is_sioul_own(name) => Some(tr().text("download-own-name", None)),
                Some(sioul_core::notes::NoteKind::Text) if bytes > sioul_core::notes::TEXT_NOTE_BYTES => Some(tr().text("download-notes-large", None)),
                Some(_) if !program => None,
                _ => Some(tr().text("download-notes-kind", None)),
            },
            _ => Some(tr().text("download-notes-none", None)),
        },
        Place::Papers if config.places().is_none() => Some(tr().text("download-papers-none", None)),
        Place::Papers if program => Some(tr().text("download-papers-program", None)),
        Place::Papers if rules::is_sioul_own(name) => Some(tr().text("download-own-name", None)),
        Place::Papers => None,
        Place::Letters if !scan_kind(name) || program => Some(tr().text("download-letters-kind", None)),
        Place::Letters if config.letters_inbox().is_none() => Some(tr().text("download-letters-none", None)),
        Place::Letters => None,
    }
}

/// The five places, each with its line and whether it can take a file named `name` now.
fn places_of(config: &Config, name: &str, mime: &str, bytes: u64) -> Vec<PlaceView> {
    Place::ALL
        .into_iter()
        .map(|place| {
            let refused = refusal(config, place, name, mime, bytes);
            let detail = refused.clone().unwrap_or_else(|| match place {
                Place::Downloads => config::downloads_dir().display().to_string(),
                Place::Folder => tr().text("download-folder-about", None),
                Place::Notes => say("download-notes-about", &[("folder", crate::work::notes_folder())]),
                Place::Papers => tr().text("download-papers-about", None),
                Place::Letters => tr().text("download-letters-about", None),
            });
            PlaceView { id: place.id(), label: tr().text(&format!("download-place-{}", place.id()), None), detail, enabled: refused.is_none() }
        })
        .collect()
}

/// The card's words and places for a file a site offers (JSON, `OfferView`);
/// asked again with the name in the field when it changes.
pub(crate) fn offer(asked: &str) -> String {
    let asked: Offered = serde_json::from_str(asked).unwrap_or_default();
    json(&offer_view(&load_config(), &asked, &Memory::load(&rules::memory_path())))
}

fn offer_view(config: &Config, asked: &Offered, memory: &Memory) -> OfferView {
    let host = host_of(&asked.url, &asked.page);
    let name = rules::file_name(&asked.name, &asked.mime);
    let program = rules::is_program(&name, &asked.mime) || rules::is_program(&asked.name, &asked.mime);
    let who = if host.is_empty() { rules::shortened(asked.site_name.trim(), SHOWN_CHARS) } else { rules::shortened(&host, SHOWN_CHARS) };
    // A size the site says, in your language's units.
    let size = (asked.total >= 1.0).then(|| sioul_core::view::size(tr(), asked.total as usize));
    let from = match (size, who.is_empty()) {
        (Some(size), false) => say("download-from-size", &[("size", size), ("host", who.clone())]),
        (Some(size), true) => say("download-size", &[("size", size)]),
        (None, false) => say("download-from", &[("host", who.clone())]),
        (None, true) => String::new(),
    };
    let caution = if !program {
        String::new()
    } else if who.is_empty() {
        tr().text("download-program-unknown", None)
    } else {
        say("download-program", &[("host", who.clone())])
    };
    let bytes = if asked.total >= 1.0 { asked.total as u64 } else { 0 };
    // A program's own name or type: what `refusal` reads, whatever the field says.
    let mime = if program && !rules::is_program(&name, &asked.mime) { "application/x-msdownload" } else { asked.mime.as_str() };
    let places = places_of(config, &name, mime, bytes);
    let (first, folder) = memory.first(&rules::site_key(&asked.site, &host));
    let first = if places.iter().any(|p| p.id == first.id() && p.enabled) { first } else { Place::Downloads };
    let folder = if folder.is_empty() { config::downloads_dir().display().to_string() } else { folder };
    OfferView { name, host: rules::shortened(&host, SHOWN_CHARS), from, waits: tr().text("download-waits", None), program, caution, places, first: first.id(), folder }
}

/// Where downloads wait for their answer: Sioul's cache, yours alone.
fn hold_root() -> PathBuf {
    config::cache_dir().join("downloads")
}

/// A folder of its own in `root` for one file (0700): the request is
/// accepted into it at once. Nothing else is taken away here.
fn new_hold(root: &Path) -> std::io::Result<PathBuf> {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let hold = root.join(format!("{}-{}-{}", jiff::Timestamp::now().as_millisecond(), std::process::id(), NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    std::fs::create_dir_all(&hold)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&hold, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(hold)
}

/// A held folder named by the window, only when it is one of `root`'s own:
/// nothing else is ever taken away.
fn held_in(root: &Path, hold: &str) -> Option<PathBuf> {
    let hold = PathBuf::from(hold.trim());
    let name = hold.file_name()?.to_string_lossy().to_string();
    (hold.parent() == Some(root) && !name.is_empty() && !name.starts_with('.') && hold.is_dir()).then_some(hold)
}

/// A file offered (JSON in: `Offered`; out: {hold, held, problem}): a folder
/// held for it in Sioul's cache, and the plain name it comes under there;
/// the window accepts the request into it at once.
pub(crate) fn hold(asked: &str) -> String {
    let asked: Offered = serde_json::from_str(asked).unwrap_or_default();
    let held = rules::file_name(&asked.name, &asked.mime);
    match new_hold(&hold_root()) {
        Ok(hold) => serde_json::json!({ "hold": hold.display().to_string(), "held": held, "problem": "" }).to_string(),
        Err(e) => serde_json::json!({ "hold": "", "held": held, "problem": say("download-hold-failed", &[("detail", e.to_string())]) }).to_string(),
    }
}

/// Bytes free on the disk of a held folder (JSON number); -1 when the system does not say.
pub(crate) fn room(hold: &str) -> f64 {
    held_in(&hold_root(), hold).and_then(|hold| sioul_sync::disk::space(&hold)).map_or(-1.0, |(free, _)| free as f64)
}

/// Whether `folder` is in one of Sioul's own folders (its data, state,
/// configuration, cache), where a site's file could pass for Sioul's own.
fn sioul_s_own_folder(folder: &Path) -> bool {
    let inside = |root: PathBuf| folder.starts_with(&root) || std::fs::canonicalize(folder).ok().zip(std::fs::canonicalize(&root).ok()).is_some_and(|(f, r)| f.starts_with(r));
    [config::data_dir(), config::state_dir(), config::config_dir(), config::cache_dir()].into_iter().any(inside)
}

/// What the window says when you press Save.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Started {
    site: String,
    site_name: String,
    /// The file's address and its page's, as offered: the site's host, for the memory.
    url: String,
    page: String,
    place: String,
    /// The name in the field.
    name: String,
    mime: String,
    /// For "folder": the file the system's "Save as" window gave (a `file://` address or a path).
    chosen: String,
    /// Still coming: the status line says it comes.
    coming: bool,
}

/// Save pressed (JSON in: `Started`; out: {name, folder, problem}): the
/// place checked for this file and kept for the site; for a folder chosen in
/// the system's window, its folder and name from what it gave.
pub(crate) fn start(qt: &QtThread, shared: &Shared, asked: &str) -> String {
    let asked: Started = serde_json::from_str(asked).unwrap_or_default();
    let answer = |name: &str, folder: &str, problem: String| serde_json::json!({ "name": name, "folder": folder, "problem": problem }).to_string();
    let Some(place) = Place::read(&asked.place) else { return answer("", "", tr().text("download-no-place", None)) };
    let (name, folder) = if place == Place::Folder {
        let chosen = crate::backend::local_path(asked.chosen.trim());
        let folder = chosen.parent().filter(|f| f.is_absolute()).map(|f| f.display().to_string()).unwrap_or_default();
        let name = chosen.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        if folder.is_empty() || name.is_empty() {
            return answer("", "", tr().text("download-folder-unset", None));
        }
        if sioul_s_own_folder(Path::new(&folder)) {
            return answer("", "", tr().text("download-folder-own", None));
        }
        (rules::clean_name(&name), folder)
    } else {
        (rules::clean_name(&asked.name), String::new())
    };
    if let Some(problem) = refusal(&load_config(), place, &name, &asked.mime, 0) {
        return answer("", "", problem);
    }
    let host = host_of(&asked.url, &asked.page);
    let path = rules::memory_path();
    let mut memory = Memory::load(&path);
    memory.chose(&rules::site_key(&asked.site, &host), place, &folder);
    if let Err(e) = memory.save(&path) {
        eprintln!("sioul: downloads: {e}");
    }
    if asked.coming {
        let who = if host.is_empty() { asked.site_name.trim().to_string() } else { host };
        let shown = rules::shortened(&name, SHOWN_CHARS);
        let line = if who.is_empty() { say("download-coming-unknown", &[("file", shown)]) } else { say("download-coming", &[("file", shown), ("host", rules::shortened(&who, SHOWN_CHARS))]) };
        crate::backend::tell(qt, shared, line);
    }
    answer(&name, &folder, String::new())
}

/// What the window says when the file came whole and is answered, or did not come.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Ended {
    hold: String,
    /// Its name in the held folder.
    held: String,
    /// The name it gets where it goes.
    name: String,
    place: String,
    /// For "folder": the folder chosen.
    folder: String,
    mime: String,
    /// Qt's reason when it did not come whole (`WebEngineDownloadRequest.interruptReason`).
    reason: i32,
}

/// A file placed.
#[derive(Debug)]
struct Placed {
    path: PathBuf,
    /// Where it lives in Sioul (`sioul:note/…`, `sioul:paper/…`, `sioul:letter/…`); "" in a folder of the system's.
    address: String,
    /// The paper's or the letter's id.
    id: String,
}

/// The held file placed where you said, under a free name: the place's own
/// numbering ("statement (2).pdf", "statement-2.pdf", "statement 2.pdf").
/// `downloads`: the system's Downloads folder (a test's own, in tests).
fn place_held(place: Place, file: &Path, name: &str, mime: &str, folder: &str, downloads: &Path, config: &Config) -> Result<Placed, String> {
    let bytes = std::fs::metadata(file).map(|m| m.len()).unwrap_or(0);
    if let Some(problem) = refusal(config, place, name, mime, bytes) {
        return Err(problem);
    }
    let failed = |e: std::io::Error| say("download-place-failed", &[("file", rules::shortened(name, SHOWN_CHARS)), ("detail", e.to_string())]);
    let into = |folder: &Path, name: &str| {
        let names = |n: u32| rules::numbered(name, n, place.numbering());
        sioul_core::filelock::place_new(file, folder, &names, true).map_err(failed)
    };
    let plain = |path: PathBuf| Placed { path, address: String::new(), id: String::new() };
    match place {
        Place::Downloads => into(downloads, name).map(plain),
        Place::Folder => {
            let folder = Path::new(folder.trim());
            if !folder.is_absolute() {
                return Err(tr().text("download-folder-unset", None));
            }
            if sioul_s_own_folder(folder) {
                return Err(tr().text("download-folder-own", None));
            }
            into(folder, name).map(plain)
        }
        Place::Notes => {
            let Some(root) = config.notes_root_path().filter(|r| r.is_dir()) else { return Err(tr().text("download-notes-none", None)) };
            let sub = config.notes_folder.clone().filter(|f| !f.trim().is_empty()).unwrap_or_else(crate::work::notes_folder);
            let sub = sioul_core::notes::normalize(&sub).unwrap_or_default();
            // Links in the notes break on these: as a new note's name, they become "-".
            let name = name.replace(['#', '^', '[', ']'], "-");
            let names = |n: u32| rules::numbered(&name, n, place.numbering());
            // The bytes written beside first, the notes' lock taken for the naming alone.
            let pending = sioul_core::filelock::Pending::prepare(file, &root.join(&sub), true).map_err(failed)?;
            let path = sioul_core::notes::with_lock(|| pending.name(&names).map_err(failed))?;
            let relative = path.strip_prefix(&root).map(|p| p.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect::<Vec<_>>().join("/")).unwrap_or_default();
            sioul_core::changes::said(sioul_core::changes::Kind::Notes);
            Ok(Placed { path, address: sioul_core::notes::uri_of(&relative), id: String::new() })
        }
        Place::Papers => {
            let (path, id) = crate::papers::file_download(file, name)?;
            Ok(Placed { path: PathBuf::from(&path), address: sioul_core::links::paper_uri(&id), id })
        }
        Place::Letters => {
            let path = crate::letters::add_one(file)?;
            let (id, _) = crate::letters::letter_id(&path);
            Ok(Placed { path, address: sioul_core::links::letter_uri(&id), id })
        }
    }
}

/// What each download's notice opens, by its opaque id (`sioul:download/<id>`),
/// this session: the file's address, its folder's, or the thing in Sioul.
static OPENED: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);

/// The notice's opaque address for a download: its held folder's name.
fn notice_address(hold: &Path) -> String {
    format!("sioul:download/{}", hold.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default())
}

/// What a download's notice opens (`sioul:download/<id>`): an address the
/// window opens as any other; "" when this session did not tell it.
pub(crate) fn opened(address: &str) -> String {
    OPENED.lock().unwrap_or_else(PoisonError::into_inner).as_ref().and_then(|o| o.get(address).cloned()).unwrap_or_default()
}

/// The notice of a file placed, about `about` (an opaque id): Open (never a
/// program, nor a file without an extension, which a desktop opens by what
/// it finds inside) and Show in its folder in a folder of the system's;
/// Open in Sioul for Notes, Papers and Letters. Returns it and what a click
/// on it opens.
fn notice_where(about: &str, place: Place, placed: &Placed, program: bool) -> (Notice, String) {
    let name = placed.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let shown = rules::shortened(&name, SHOWN_CHARS);
    let title = say(&format!("download-in-{}", place.id()), &[("file", shown)]);
    let folder = placed.path.parent().map(Path::to_path_buf).unwrap_or_default();
    let folder_url = format!("{}/", crate::backend::file_url(&folder).trim_end_matches('/'));
    let open = tr().text("ui-open", None);
    if place.in_sioul() {
        let body = match place {
            Place::Papers => tr().text("download-in-papers-body", None),
            Place::Letters if crate::letters::reader_hint().is_empty() => tr().text("download-in-letters-body", None),
            Place::Letters => tr().text("download-in-letters-later", None),
            _ => folder.display().to_string(),
        };
        return (Notice::new(about, Kind::Minor, title, body).action(open, Act::Open { address: placed.address.clone(), hint: String::new() }), placed.address.clone());
    }
    let show = Act::Open { address: folder_url.clone(), hint: String::new() };
    if program || !rules::has_extension(&name) {
        let body = if program { tr().text("download-program-kept", None) } else { folder.display().to_string() };
        return (Notice::new(about, Kind::Minor, title, body).action(tr().text("file-show-folder", None), show), folder_url);
    }
    let file_url = crate::backend::file_url(&placed.path);
    (Notice::new(about, Kind::Minor, title, folder.display().to_string()).action(open, Act::Open { address: file_url.clone(), hint: String::new() }).action(tr().text("file-show-folder", None), show), file_url)
}

/// What the window does with a file a site offered once the antivirus
/// answered, from the table every way in reads (`attachments::decide`): placed
/// where you said, saying that it was not checked when it was not (you chose
/// the place on the card, and Sioul never opens it), or refused.
fn decided(verdict: &Verdict, shown: &str) -> Decision {
    let system = System::here();
    let presence = if system == System::Phone { Presence::Absent } else { antivirus::presence() };
    crate::attachments::decide(&Found::Scanned(verdict.clone()), system, &presence, crate::attachments::Act::Place, shown, "")
}

/// What the antivirus could not say of a file placed, in one sentence: none
/// when it found the file clean.
fn unchecked(verdict: &Verdict) -> Option<String> {
    let decision = decided(verdict, "");
    (decision.does == Does::Go && !decision.note.is_empty()).then(|| decision.said(decision.note))
}

/// The notice of a file placed (`notice_where`), what a click on it opens,
/// and the status line: where it is; for Papers and Letters, what follows
/// too; then, in both, that it was not checked and why, when the antivirus
/// found it too big or none answered (`verdict`).
fn notice_of(about: &str, place: Place, placed: &Placed, program: bool, verdict: &Verdict) -> (Notice, String, String) {
    let (mut notice, opens) = notice_where(about, place, placed, program);
    let mut line = if matches!(place, Place::Papers | Place::Letters) { format!("{} {}", notice.title, notice.body) } else { notice.title.clone() };
    if let Some(said) = unchecked(verdict) {
        line = format!("{line} {said}");
        notice.body = format!("{}\n{said}", notice.body);
    }
    (notice, opens, line)
}

/// A notice told, and what it opens kept for its click.
fn tell_notice(notice: Notice, opens: String) {
    OPENED.lock().unwrap_or_else(PoisonError::into_inner).get_or_insert_with(HashMap::new).insert(notice.about.clone(), opens);
    let _ = crate::tell::tell(notice);
}

/// The file came whole and is answered (JSON in: `Ended`): checked by the
/// antivirus when there is one, then placed where you said, off the window's
/// thread; its held folder taken away once placed. Said in the status line
/// and by one notice, both saying that it was not checked, and why, when it
/// was not; then the window hears `download_filed` (Papers' correction,
/// Letters' page, when Sites is still in view). A place that refuses it
/// keeps it held, and the window hears `download_refused`.
pub(crate) fn done(qt: &QtThread, shared: &Arc<Shared>, asked: &str) {
    let asked: Ended = serde_json::from_str(asked).unwrap_or_default();
    let (qt, shared) = (qt.clone(), Arc::clone(shared));
    std::thread::spawn(move || {
        let name = rules::clean_name(&asked.name);
        let shown = rules::shortened(&name, SHOWN_CHARS);
        let held = rules::clean_name(&asked.held);
        // Whatever happens, the window hears once that this file is over (`download_filed`) or held again (`download_refused`).
        let over = |hold: String, place: &'static str, id: String| {
            let _ = qt.queue(move |mut sioul| sioul.as_mut().download_filed(cxx_qt_lib::QString::from(&hold), cxx_qt_lib::QString::from(place), cxx_qt_lib::QString::from(&id)));
        };
        let (Some(hold), Some(place), true) = (held_in(&hold_root(), &asked.hold), Place::read(&asked.place), held == asked.held) else {
            crate::backend::tell(&qt, &shared, say("download-failed-disk", &[("file", shown)]));
            over(asked.hold.clone(), "", String::new());
            return;
        };
        let file = hold.join(&held);
        let about = notice_address(&hold);
        // As an attachment is: one call to the antivirus; "Checking…" said
        // only when one is there to ask (none on a phone, nor without ClamAV).
        if antivirus::present() {
            crate::backend::tell(&qt, &shared, say("scan-running", &[("name", shown.clone())]));
        }
        let verdict = antivirus::scan(&file);
        let decision = decided(&verdict, &shown);
        if decision.does == Does::Refuse {
            let _ = std::fs::remove_dir_all(&hold);
            let line = decision.said(decision.line);
            crate::backend::tell(&qt, &shared, line.clone());
            tell_notice(Notice::new(&about, Kind::Minor, say("download-not-kept", &[("file", shown)]), line), String::new());
            over(hold.display().to_string(), "", String::new());
            return;
        }
        let placed = place_held(place, &file, &name, &asked.mime, &asked.folder, &config::downloads_dir(), &load_config());
        let placed = match placed {
            Ok(placed) => placed,
            Err(problem) => {
                // Kept held: the card asks again, saying why.
                let hold = hold.display().to_string();
                let _ = qt.queue(move |mut sioul| sioul.as_mut().download_refused(cxx_qt_lib::QString::from(&hold), cxx_qt_lib::QString::from(&problem)));
                return;
            }
        };
        let _ = std::fs::remove_dir_all(&hold);
        crate::backend::mark_from_outside(&placed.path);
        let program = rules::is_program(&name, &asked.mime);
        let (notice, opens, line) = notice_of(&about, place, &placed, program, &verdict);
        crate::backend::tell(&qt, &shared, line);
        tell_notice(notice, opens);
        over(hold.display().to_string(), place.id(), placed.id.clone());
        if place == Place::Letters {
            // Read once it is twenty seconds old, as a scanner's file is (`letters::tick`), rather than at a later minute.
            std::thread::sleep(std::time::Duration::from_secs(21));
            crate::letters::tick(&qt, &shared);
        }
    });
}

/// Why a file did not come whole, in words: Qt's reason (`WebEngineDownloadRequest.interruptReason`).
fn failed_why(reason: i32) -> &'static str {
    match reason {
        3 => "download-failed-space",
        1..=19 => "download-failed-disk",
        20..=29 => "download-failed-network",
        30..=39 => "download-failed-server",
        _ => "download-failed-cut",
    }
}

/// The file failed or was cut (JSON in: `Ended`): its held folder taken
/// away, so that nothing half-written stays; said why, in the status line
/// and by a notice, as a file that came is.
pub(crate) fn failed(qt: &QtThread, shared: &Shared, asked: &str) {
    let asked: Ended = serde_json::from_str(asked).unwrap_or_default();
    let hold = held_in(&hold_root(), &asked.hold);
    if let Some(hold) = &hold {
        let _ = std::fs::remove_dir_all(hold);
    }
    let shown = rules::shortened(&rules::clean_name(&asked.name), SHOWN_CHARS);
    let line = say(failed_why(asked.reason), &[("file", shown.clone())]);
    crate::backend::tell(qt, shared, line.clone());
    if let Some(hold) = hold {
        tell_notice(Notice::new(notice_address(&hold), Kind::Minor, say("download-not-kept", &[("file", shown)]), line), String::new());
    }
}

/// Cancel, or a file let go (JSON in: {hold}): what came of it deleted.
pub(crate) fn drop_held(asked: &str) {
    let asked: Ended = serde_json::from_str(asked).unwrap_or_default();
    if let Some(hold) = held_in(&hold_root(), &asked.hold) {
        let _ = std::fs::remove_dir_all(&hold);
    }
}

/// A word of the card's in the status line (JSON in: `Offered`): "cancelled"
/// (what came deleted); "waiting" (a file offered while Sites is not in
/// view, its question waiting there); "too-many" (a site's fourth file while
/// three wait); "too-big" (a file waiting unanswered past 1 GiB, or the disk
/// nearly full); "refused" (every file from this site refused until it is
/// closed or reloaded).
pub(crate) fn said(qt: &QtThread, shared: &Shared, what: &str, asked: &str) {
    let asked: Offered = serde_json::from_str(asked).unwrap_or_default();
    let host = host_of(&asked.url, &asked.page);
    let who = rules::shortened(if asked.site_name.trim().is_empty() { &host } else { asked.site_name.trim() }, SHOWN_CHARS);
    let file = rules::shortened(&rules::file_name(&asked.name, &asked.mime), SHOWN_CHARS);
    let line = match what {
        "cancelled" => say("download-cancelled", &[("file", file)]),
        "waiting" if who.is_empty() => tr().text("download-waiting-unknown", None),
        "waiting" => say("download-waiting", &[("host", who)]),
        "too-many" => say("download-too-many", &[("host", who)]),
        "too-big" => say("download-too-big", &[("file", file)]),
        "refused" => say("download-refused-site", &[("host", who)]),
        _ => return,
    };
    crate::backend::tell(qt, shared, line);
}

/// At start, once this Sioul is the profile's (no download can be under way):
/// the held folder taken away whole (renamed aside at once, removed off the
/// window's thread), and the hidden files a day old that a stopped placing
/// left in the papers' folder and where scans arrive.
#[cfg(not(target_os = "android"))]
pub(crate) fn at_start() {
    let root = hold_root();
    let aside = root.with_file_name(format!("downloads-gone-{}", std::process::id()));
    let moved = std::fs::rename(&root, &aside).is_ok();
    let cache = config::cache_dir();
    std::thread::spawn(move || {
        if moved {
            let _ = std::fs::remove_dir_all(&aside);
        }
        // Left by an earlier start stopped while removing.
        for entry in std::fs::read_dir(&cache).into_iter().flatten().filter_map(Result::ok) {
            if entry.file_name().to_string_lossy().starts_with("downloads-gone-") {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
        let config = load_config();
        if let Some(places) = config.places() {
            sioul_core::filelock::sweep_parts(&places.papers.folder);
        }
        if let Some(inbox) = config.letters_inbox() {
            sioul_core::filelock::sweep_parts(&inbox);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A test's own folders and configuration, on its thread (`backend::TEST_CONFIG`).
    struct Own {
        base: PathBuf,
    }

    impl Own {
        fn new(name: &str, config: &str) -> Own {
            sioul_core::config::stand_ins_only();
            let base = sioul_core::config::data_dir().join(format!("{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&base);
            for folder in ["notes", "held", "Downloads", "Papers", "Letters"] {
                std::fs::create_dir_all(base.join(folder)).unwrap();
            }
            let path = base.join("config.toml");
            std::fs::write(&path, config.replace("BASE", &base.display().to_string())).unwrap();
            crate::backend::TEST_CONFIG.with(|own| *own.borrow_mut() = Some(path));
            Own { base }
        }

        fn held(&self, name: &str, bytes: &[u8]) -> PathBuf {
            let file = self.base.join("held").join(name);
            std::fs::write(&file, bytes).unwrap();
            file
        }

        fn names(&self, folder: &str) -> Vec<String> {
            let mut names: Vec<String> = std::fs::read_dir(self.base.join(folder)).map(|r| r.filter_map(Result::ok).map(|e| e.file_name().to_string_lossy().to_string()).collect()).unwrap_or_default();
            names.sort();
            names
        }
    }

    impl Drop for Own {
        fn drop(&mut self) {
            crate::backend::TEST_CONFIG.with(|own| *own.borrow_mut() = None);
            let _ = std::fs::remove_dir_all(&self.base);
        }
    }

    const WITH_NOTES: &str = "case_store = \"BASE/notes\"\n";

    /// Each place does what the design's table says, a name already there
    /// numbered in the place's own way, never written over.
    #[test]
    fn each_place_takes_the_file_its_own_way() {
        let own = Own::new("downloads-places", WITH_NOTES);
        let config = load_config();
        let downloads = own.base.join("Downloads");
        std::fs::write(downloads.join("statement.pdf"), b"older").unwrap();
        // Downloads: a free name, the held file moved there.
        let file = own.held("statement.pdf", b"%PDF-1");
        let placed = place_held(Place::Downloads, &file, "statement.pdf", "", "", &downloads, &config).unwrap();
        assert_eq!(placed.path, downloads.join("statement (2).pdf"));
        assert_eq!((placed.address.as_str(), placed.id.as_str()), ("", ""));
        assert_eq!(std::fs::read(downloads.join("statement.pdf")).unwrap(), b"older", "never replaced");
        assert!(!file.exists(), "moved, not copied");
        // A folder chosen in the system's window.
        // A folder chosen outside Sioul's own (the test's folders are in its data folder).
        let chosen = std::env::temp_dir().join(format!("sioul-downloads-chosen-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&chosen);
        let file = own.held("archive.zip", b"PK");
        assert_eq!(place_held(Place::Folder, &file, "archive.zip", "", &chosen.display().to_string(), &downloads, &config).unwrap().path, chosen.join("archive.zip"));
        let file = own.held("archive.zip", b"PK");
        assert!(place_held(Place::Folder, &file, "archive.zip", "", "relative/folder", &downloads, &config).is_err(), "a folder that is not absolute: refused");
        // Notes: in the folder new notes go into, an address of the notes, link-breaking characters made "-".
        let file = own.held("tableau [v2] #1.ods", b"ods");
        let placed = place_held(Place::Notes, &file, "tableau [v2] #1.ods", "", "", &downloads, &config).unwrap();
        assert_eq!(placed.path, own.base.join("notes/notes/tableau -v2- -1.ods"));
        assert_eq!(placed.address, "sioul:note/notes/tableau%20-v2-%20-1.ods");
        let file = own.held("tableau [v2] #1.ods", b"ods");
        assert_eq!(place_held(Place::Notes, &file, "tableau [v2] #1.ods", "", "", &downloads, &config).unwrap().path, own.base.join("notes/notes/tableau -v2- -1 2.ods"), "a note's own numbering");
        // Papers: by the wallet's own way in, a paper made at once, yours, its title and type guessed.
        let file = own.held("avis_impot_2026.pdf", b"%PDF-2");
        let placed = place_held(Place::Papers, &file, "avis_impot_2026.pdf", "", "", &downloads, &config).unwrap();
        let kept = own.base.join("notes/papers/avis_impot_2026.pdf");
        assert_eq!(placed.path, kept);
        assert!(!placed.id.is_empty());
        assert_eq!(placed.address, sioul_core::links::paper_uri(&placed.id));
        let wallet = sioul_core::papers::Wallet::load_at(&config.places().unwrap()).unwrap();
        let paper = wallet.get(&placed.id).expect("a paper at once");
        assert_eq!((paper.title.as_str(), wallet.file_path(paper)), ("avis impot 2026", Some(kept)));
        // Letters: where scans arrive, the id its reading will give it.
        let file = own.held("courrier.pdf", b"%PDF-3");
        let placed = place_held(Place::Letters, &file, "courrier.pdf", "", "", &downloads, &config).unwrap();
        let inbox = own.base.join("notes/letters/inbox/courrier.pdf");
        assert_eq!(placed.path, inbox);
        let id = crate::letters::letter_id(&inbox).0;
        assert_eq!((placed.address, placed.id.clone()), (sioul_core::links::letter_uri(&id), id));
        // Letters read scans alone; Papers and Letters never take a program; Notes only what Notes show.
        let file = own.held("archive.zip", b"PK");
        assert!(place_held(Place::Letters, &file, "archive.zip", "", "", &downloads, &config).is_err());
        assert_eq!(place_held(Place::Notes, &file, "archive.zip", "", "", &downloads, &config).unwrap_err(), tr().text("download-notes-kind", None));
        let file = own.held("setup.pdf", b"MZ");
        assert_eq!(place_held(Place::Papers, &file, "setup.pdf", "application/x-msdownload", "", &downloads, &config).unwrap_err(), tr().text("download-papers-program", None));
        assert_eq!(place_held(Place::Papers, &file, "sioul-papers.toml", "", "", &downloads, &config).unwrap_err(), tr().text("download-own-name", None));
        // A text larger than 2 MiB is not a note: known once it came.
        let file = own.held("big.txt", &vec![b'a'; 2 * 1024 * 1024 + 1]);
        assert_eq!(place_held(Place::Notes, &file, "big.txt", "", "", &downloads, &config).unwrap_err(), tr().text("download-notes-large", None));
        assert!(file.exists(), "refused, the file stays held for the card to ask again");
        assert_eq!(own.names("notes/letters/inbox"), ["courrier.pdf"], "nothing else, nothing hidden");
        // A folder in Sioul's own folders is refused.
        let own_folder = sioul_core::config::state_dir().join("lists");
        assert!(sioul_s_own_folder(&own_folder) && !sioul_s_own_folder(&chosen));
        assert_eq!(place_held(Place::Folder, &file, "big.txt", "", &own_folder.display().to_string(), &downloads, &config).unwrap_err(), tr().text("download-folder-own", None));
        let _ = std::fs::remove_dir_all(&chosen);
    }

    /// A place that cannot take the file leaves nothing in it, and the held
    /// file goes back where it was, for its folder to be taken away.
    #[test]
    fn a_place_that_fails_keeps_nothing() {
        let own = Own::new("downloads-fails", "");
        let config = load_config();
        let file = own.held("statement.pdf", b"%PDF");
        assert!(place_held(Place::Notes, &file, "statement.pdf", "", "", &own.base.join("Downloads"), &config).is_err(), "no notes folder");
        assert!(place_held(Place::Papers, &file, "statement.pdf", "", "", &own.base.join("Downloads"), &config).is_err(), "no place for papers");
        assert!(file.exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let closed = std::env::temp_dir().join(format!("sioul-downloads-closed-{}", std::process::id()));
            std::fs::create_dir_all(&closed).unwrap();
            std::fs::set_permissions(&closed, std::fs::Permissions::from_mode(0o500)).unwrap();
            if std::fs::write(closed.join("probe"), b"").is_err() {
                assert!(place_held(Place::Folder, &file, "statement.pdf", "", &closed.display().to_string(), &own.base.join("Downloads"), &config).is_err());
                assert!(std::fs::read_dir(&closed).unwrap().next().is_none(), "nothing half-written there");
                assert!(file.exists(), "the held file put back");
            }
            std::fs::set_permissions(&closed, std::fs::Permissions::from_mode(0o700)).unwrap();
            let _ = std::fs::remove_dir_all(&closed);
        }
    }

    /// The held folders: one per file, yours alone, taken away only when one
    /// of Sioul's own (never swept while Sioul runs: only at start).
    #[test]
    fn held_folders_are_sioul_s_own() {
        let own = Own::new("downloads-held", "");
        let root = own.base.join("holds");
        let first = new_hold(&root).unwrap();
        let second = new_hold(&root).unwrap();
        assert_ne!(first, second);
        assert_eq!(first.parent(), Some(root.as_path()));
        assert_eq!(held_in(&root, &first.display().to_string()), Some(first.clone()));
        for elsewhere in [own.base.join("notes").display().to_string(), format!("{}/..", first.display()), root.display().to_string(), String::new(), "/".into()] {
            assert_eq!(held_in(&root, &elsewhere), None, "{elsewhere}");
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&first).unwrap().permissions().mode() & 0o777, 0o700, "yours alone");
        }
        assert_eq!(failed_why(3), "download-failed-space");
        assert_eq!(failed_why(22), "download-failed-network");
        assert_eq!(failed_why(36), "download-failed-server");
        assert_eq!(failed_why(1), "download-failed-disk");
        assert_eq!(failed_why(40), "download-failed-cut");
        assert_eq!(failed_why(0), "download-failed-cut");
    }

    /// The card: the name made plain, the host, the size, a program's
    /// caution, which places can take the file, the one chosen last for the
    /// site first when it can.
    #[test]
    fn the_card_says_what_comes_and_where_it_may_go() {
        let own = Own::new("downloads-offer", WITH_NOTES);
        let config = load_config();
        let asked = |name: &str, mime: &str| Offered { site: "bank".into(), site_name: "My bank".into(), url: "blob:https://secure.bank.example/1f".into(), page: String::new(), name: name.into(), mime: mime.into(), total: 1_258_291.0 };
        let mut memory = Memory::default();
        let view = offer_view(&config, &asked("../Relevé 2026-10.pdf", "application/pdf"), &memory);
        assert_eq!((view.name.as_str(), view.host.as_str(), view.program, view.caution.as_str(), view.first), ("_Relevé 2026-10.pdf", "secure.bank.example", false, "", "downloads"));
        assert_eq!(view.waits, tr().text("download-waits", None), "where it waits, and what Cancel does");
        assert_eq!(view.from, say("download-from-size", &[("size", sioul_core::view::size(tr(), 1_258_291)), ("host", "secure.bank.example".into())]));
        let ids: Vec<(&str, bool)> = view.places.iter().map(|p| (p.id, p.enabled)).collect();
        assert_eq!(ids, [("downloads", true), ("folder", true), ("notes", true), ("papers", true), ("letters", true)]);
        // The place chosen last for this site first; a site of its own its own.
        memory.chose("bank", Place::Papers, "");
        assert_eq!(offer_view(&config, &asked("statement.pdf", ""), &memory).first, "papers");
        // A program: its caution; Notes, Papers and Letters cannot take it, each saying why.
        let view = offer_view(&config, &asked("setup.exe", "application/x-msdownload"), &memory);
        assert!(view.program && !view.caution.is_empty());
        let shut: Vec<(&str, bool, &str)> = view.places.iter().map(|p| (p.id, p.enabled, p.detail.as_str())).filter(|p| !p.1).collect();
        assert_eq!(shut, [("notes", false, tr().text("download-notes-kind", None).as_str()), ("papers", false, tr().text("download-papers-program", None).as_str()), ("letters", false, tr().text("download-letters-kind", None).as_str())]);
        // A program by its type alone, whatever its name says.
        let view = offer_view(&config, &asked("statement.pdf", "application/x-msdownload"), &memory);
        assert!(view.places.iter().filter(|p| ["papers", "letters"].contains(&p.id)).all(|p| !p.enabled));
        // A name without extension gets its type's; a type Notes does not show cannot go there.
        let view = offer_view(&config, &asked("statement", "application/pdf"), &memory);
        assert_eq!(view.name, "statement.pdf");
        let view = offer_view(&config, &asked("data.csv", "text/csv"), &memory);
        assert_eq!(view.places.iter().find(|p| p.id == "notes").map(|p| (p.enabled, p.detail.clone())), Some((false, tr().text("download-notes-kind", None))));
        // A text the site says is larger than a note: not Notes.
        let mut large = asked("log.txt", "text/plain");
        large.total = 3.0 * 1024.0 * 1024.0;
        assert_eq!(offer_view(&config, &large, &memory).places.iter().find(|p| p.id == "notes").map(|p| p.enabled), Some(false));
        // Last chosen Letters for this site, and a file Letters cannot take: Downloads first.
        memory.chose("bank", Place::Letters, "");
        assert_eq!(offer_view(&config, &asked("archive.zip", ""), &memory).first, "downloads");
        // No host: the site's name says where it comes from; no size: none said.
        let mut nameless = asked("", "");
        nameless.url = "data:text/plain,hi".into();
        nameless.total = -1.0;
        let view = offer_view(&config, &nameless, &memory);
        assert_eq!((view.name.as_str(), view.host.as_str()), (rules::NAMELESS, ""));
        assert_eq!(view.from, say("download-from", &[("host", "My bank".into())]), "no size said");
        drop(own);
        // Without a notes folder: neither Notes, nor Papers, nor Letters.
        let _own = Own::new("downloads-offer-bare", "");
        let view = offer_view(&load_config(), &asked("statement.pdf", ""), &Memory::default());
        let ids: Vec<(&str, bool)> = view.places.iter().map(|p| (p.id, p.enabled)).collect();
        assert_eq!(ids, [("downloads", true), ("folder", true), ("notes", false), ("papers", false), ("letters", false)]);
    }

    /// The notice: about an opaque id, never the file's path; Open and Show
    /// in its folder in a system's folder, the folder alone for a program or
    /// a name without extension; the thing in Sioul for Notes, Papers, Letters.
    #[test]
    fn the_notice_opens_the_file_never_a_program() {
        let about = notice_address(Path::new("/stand-in/cache/downloads/1700000000000-12-3"));
        assert_eq!(about, "sioul:download/1700000000000-12-3");
        let path = PathBuf::from("/stand-in/Downloads/statement (2).pdf");
        let placed = Placed { path: path.clone(), address: String::new(), id: String::new() };
        let (notice, opens, _) = notice_of(&about, Place::Downloads, &placed, false, &Verdict::Clean);
        let acts: Vec<&Act> = notice.actions.iter().map(|a| &a.act).collect();
        assert_eq!((notice.about.as_str(), opens), (about.as_str(), crate::backend::file_url(&path)));
        assert_eq!(acts, [&Act::Open { address: crate::backend::file_url(&path), hint: String::new() }, &Act::Open { address: "file:///stand-in/Downloads/".into(), hint: String::new() }]);
        for kept in ["/stand-in/Downloads/setup.exe", "/stand-in/Downloads/statement"] {
            let placed = Placed { path: PathBuf::from(kept), address: String::new(), id: String::new() };
            let (notice, opens, _) = notice_of(&about, Place::Downloads, &placed, kept.ends_with(".exe"), &Verdict::Clean);
            assert_eq!((notice.about.as_str(), opens.as_str()), (about.as_str(), "file:///stand-in/Downloads/"));
            assert_eq!(notice.actions.iter().map(|a| &a.act).collect::<Vec<_>>(), [&Act::Open { address: "file:///stand-in/Downloads/".into(), hint: String::new() }], "never the file: {kept}");
        }
        let note = Placed { path: PathBuf::from("/stand-in/Notes/notes/x.ods"), address: "sioul:note/notes/x.ods".into(), id: String::new() };
        let (notice, opens, _) = notice_of(&about, Place::Notes, &note, false, &Verdict::Clean);
        assert_eq!((notice.about.as_str(), opens.as_str(), notice.actions.len()), (about.as_str(), "sioul:note/notes/x.ods", 1));
        assert_eq!(notice.kind, Kind::Minor);
        tell_notice(notice, opens);
        assert_eq!(opened(&about), "sioul:note/notes/x.ods", "a click opens the note");
        assert_eq!(opened("sioul:download/other"), "");
    }

    /// What the antivirus said goes with the file placed: nothing more when
    /// it found the file clean; when it was too big, or none answered, that
    /// it was not checked and why, in the notice's body and the status line,
    /// after where the file is (and, for Papers and Letters, what follows).
    #[test]
    fn a_file_not_checked_is_said_so() {
        let about = "sioul:download/1700000000000-12-4";
        let file = Placed { path: PathBuf::from("/stand-in/Downloads/statement.pdf"), address: String::new(), id: String::new() };
        let paper = Placed { path: PathBuf::from("/stand-in/Notes/papers/statement.pdf"), address: "sioul:paper/p1".into(), id: "p1".into() };
        let in_downloads = say("download-in-downloads", &[("file", "statement.pdf".into())]);
        let in_papers = say("download-in-papers", &[("file", "statement.pdf".into())]);
        let follows = tr().text("download-in-papers-body", None);
        // Clean: as before, where it is and nothing else.
        let (notice, _, line) = notice_of(about, Place::Downloads, &file, false, &Verdict::Clean);
        let folder = notice_where(about, Place::Downloads, &file, false).0.body;
        assert_eq!((notice.title.as_str(), notice.body.as_str(), line.as_str()), (in_downloads.as_str(), folder.as_str(), in_downloads.as_str()));
        let (notice, _, line) = notice_of(about, Place::Papers, &paper, false, &Verdict::Clean);
        assert_eq!((notice.body.as_str(), line), (follows.as_str(), format!("{in_papers} {follows}")));
        // Too big, and no antivirus: one sentence more each, its own.
        let too_big = tr().text("download-unchecked-too-big", None);
        let none = tr().text("download-unchecked-none", None);
        assert!(!too_big.is_empty() && !none.is_empty() && too_big != none && too_big != "download-unchecked-too-big" && none != "download-unchecked-none");
        for (verdict, said) in [(Verdict::TooBig, &too_big), (Verdict::Unavailable("no antivirus (test)".into()), &none)] {
            let (notice, _, line) = notice_of(about, Place::Downloads, &file, false, &verdict);
            assert_eq!((notice.title.as_str(), notice.body.clone(), line), (in_downloads.as_str(), format!("{folder}\n{said}"), format!("{in_downloads} {said}")), "{verdict:?}");
            let (notice, opens, line) = notice_of(about, Place::Papers, &paper, false, &verdict);
            assert_eq!((notice.body.clone(), line), (format!("{follows}\n{said}"), format!("{in_papers} {follows} {said}")), "{verdict:?}");
            assert_eq!(opens, "sioul:paper/p1", "it opens as a file checked does");
        }
    }
}
