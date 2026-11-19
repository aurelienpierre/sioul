// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! A file a site offers, on a computer (docs/sites.md, "Downloads").
//! Nothing is saved before you say where: the Sites page holds Qt
//! WebEngine's request unanswered (SiteDownloads.qml), and its card
//! (DownloadCard.qml) shows what `offer` says of the file: its name made one
//! plain file name, its size, the host it comes from, a word of caution for a
//! program, the five places and which of them can take it, the one chosen
//! last for this site first. Then:
//!
//! - **Save** (`start`): the place kept for the site, and a folder of its own
//!   in Sioul's cache (`hold`) where Chromium writes the file while it comes,
//!   so that nothing half-written ever shows in the place chosen.
//! - **Come whole** (`done`, off the window's thread): placed where you said,
//!   under a free name, never over a file (`filelock::place_new`): the
//!   system's Downloads folder; the folder chosen in its "Save as" window;
//!   the notes folder, where new notes go; Papers by their own way in (the
//!   wallet's folder, then the paper's form with the file attached:
//!   `papers::keep_checked`, the window's `paper_kept`); Letters by theirs
//!   (where scans arrive, read as a letter received: `letters::add_one`,
//!   then the letter brought into view once read). The status line says it,
//!   and one notice names it: Open, and Show in its folder for the system's
//!   folders; in Sioul for Notes, Papers and Letters. Never opened by itself.
//! - **Failed or cut** (`failed`): its held folder taken away, said why.
//! - **Cancel** (`said`): nothing saved; the page tells the site.
//!
//! The rules (the name's cleaning, programs, hosts, the memory per site) are
//! the core's (`sioul_core::downloads`).

use crate::backend::{QtThread, Shared, json, load_config, say, tr};
use serde::{Deserialize, Serialize};
use sioul_core::config::{self, Config, NotesFolder};
use sioul_core::downloads::{self as rules, Memory, Place};
use sioul_core::notice::{Act, Kind, Notice};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// How much of a site's words a line shows: a file's name, a host.
const SHOWN_CHARS: usize = 60;

/// What the Sites page says of a file a site offers (Qt's `WebEngineDownloadRequest`).
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
    /// The name the site gives.
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
    /// The name the field starts with: one plain file name (`clean_name`).
    name: String,
    /// The host it comes from, cut short; "" when its address names none.
    host: String,
    /// "1.2 MB, from secure.bank.example", or as much as is known.
    from: String,
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

/// The five places, each with its line and whether it can take a file named `name` now.
fn places_of(config: &Config, name: &str) -> Vec<PlaceView> {
    let place = |place: Place, detail: String, enabled: bool| PlaceView { id: place.id(), label: tr().text(&format!("download-place-{}", place.id()), None), detail, enabled };
    let notes = match config.notes_folder() {
        NotesFolder::Ready(_) => place(Place::Notes, say("download-notes-about", &[("folder", crate::work::notes_folder())]), true),
        _ => place(Place::Notes, tr().text("download-notes-none", None), false),
    };
    let papers = match config.places() {
        Some(_) => place(Place::Papers, tr().text("download-papers-about", None), true),
        None => place(Place::Papers, tr().text("download-papers-none", None), false),
    };
    let letters = if !scan_kind(name) {
        place(Place::Letters, tr().text("download-letters-kind", None), false)
    } else if config.letters_inbox().is_some() {
        place(Place::Letters, tr().text("download-letters-about", None), true)
    } else {
        place(Place::Letters, tr().text("download-letters-none", None), false)
    };
    vec![
        place(Place::Downloads, config::downloads_dir().display().to_string(), true),
        place(Place::Folder, tr().text("download-folder-about", None), true),
        notes,
        papers,
        letters,
    ]
}

/// The card's words and places for a file a site offers (JSON, `OfferView`).
pub(crate) fn offer(asked: &str) -> String {
    let asked: Offered = serde_json::from_str(asked).unwrap_or_default();
    json(&offer_view(&load_config(), &asked, &Memory::load(&rules::memory_path())))
}

fn offer_view(config: &Config, asked: &Offered, memory: &Memory) -> OfferView {
    let host = host_of(&asked.url, &asked.page);
    let name = rules::clean_name(&asked.name);
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
    let places = places_of(config, &name);
    let (first, folder) = memory.first(&rules::site_key(&asked.site, &host));
    let first = if places.iter().any(|p| p.id == first.id() && p.enabled) { first } else { Place::Downloads };
    let folder = if folder.is_empty() { config::downloads_dir().display().to_string() } else { folder };
    OfferView { name, host: rules::shortened(&host, SHOWN_CHARS), from, program, caution, places, first: first.id(), folder }
}

/// Where downloads wait while they come: Sioul's cache, yours alone.
fn hold_root() -> PathBuf {
    config::cache_dir().join("downloads")
}

/// A folder of its own in `root` for one file while it comes; those left
/// by a Sioul stopped meanwhile (a day old) taken away first.
fn new_hold(root: &Path) -> std::io::Result<PathBuf> {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.filter_map(Result::ok) {
            let old = entry.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|age| age.as_secs() > 24 * 3600);
            if old {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
    }
    let hold = root.join(format!("{}-{}-{}", jiff::Timestamp::now().as_millisecond(), std::process::id(), NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    std::fs::create_dir_all(&hold)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&hold, std::fs::Permissions::from_mode(0o700));
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

/// What the window says when you press Save.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Started {
    site: String,
    site_name: String,
    host: String,
    place: String,
    /// The name in the field.
    name: String,
    /// For "folder": the file the system's "Save as" window gave (a `file://` address or a path).
    chosen: String,
}

/// Save pressed (JSON in: `Started`; out: {hold, name, folder, problem}):
/// the place kept for the site, a folder held for the file in Sioul's cache,
/// the status line saying it comes. For a folder chosen in the system's
/// window, its folder and name from what it gave.
pub(crate) fn start(qt: &QtThread, shared: &Shared, asked: &str) -> String {
    let asked: Started = serde_json::from_str(asked).unwrap_or_default();
    let answer = |hold: &Path, name: &str, folder: &str, problem: String| serde_json::json!({ "hold": hold.display().to_string(), "name": name, "folder": folder, "problem": problem }).to_string();
    let Some(place) = Place::read(&asked.place) else { return answer(Path::new(""), "", "", tr().text("download-no-place", None)) };
    let (name, folder) = if place == Place::Folder {
        let chosen = crate::backend::local_path(asked.chosen.trim());
        let folder = chosen.parent().filter(|f| f.is_absolute()).map(|f| f.display().to_string()).unwrap_or_default();
        let name = chosen.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        if folder.is_empty() || name.is_empty() {
            return answer(Path::new(""), "", "", tr().text("download-folder-unset", None));
        }
        (rules::clean_name(&name), folder)
    } else {
        (rules::clean_name(&asked.name), String::new())
    };
    let path = rules::memory_path();
    let mut memory = Memory::load(&path);
    memory.chose(&rules::site_key(&asked.site, &asked.host), place, &folder);
    if let Err(e) = memory.save(&path) {
        eprintln!("sioul: downloads: {e}");
    }
    let hold = match new_hold(&hold_root()) {
        Ok(hold) => hold,
        Err(e) => return answer(Path::new(""), "", "", say("download-hold-failed", &[("detail", e.to_string())])),
    };
    let who = if asked.host.trim().is_empty() { asked.site_name.trim().to_string() } else { asked.host.trim().to_string() };
    let shown = rules::shortened(&name, SHOWN_CHARS);
    let line = if who.is_empty() { say("download-coming-unknown", &[("file", shown)]) } else { say("download-coming", &[("file", shown), ("host", rules::shortened(&who, SHOWN_CHARS))]) };
    crate::backend::tell(qt, shared, line);
    answer(&hold, &name, &folder, String::new())
}

/// What the window says when the file came whole, or did not.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Ended {
    hold: String,
    name: String,
    place: String,
    /// For "folder": the folder chosen.
    folder: String,
    mime: String,
    /// Qt's reason when it did not come whole (`WebEngineDownloadRequest.interruptReason`).
    reason: i32,
}

/// What follows a file placed in Sioul.
#[derive(Debug, PartialEq, Eq)]
enum Then {
    Nothing,
    /// The paper's form, the file attached: its path, a title and a kind guessed.
    Paper { file: String, title: String, kind: &'static str },
    /// The letter brought into view once read: its id as the reading will give it.
    Letter(String),
}

/// A file placed.
#[derive(Debug)]
struct Placed {
    path: PathBuf,
    /// Where it lives in Sioul (`sioul:note/…`, `sioul:paper-file/…`, `sioul:letter/…`); "" in a folder of the system's.
    address: String,
    then: Then,
}

/// The held file placed where you said, under a free name: the place's own
/// numbering ("statement (2).pdf", "statement-2.pdf", "statement 2.pdf").
/// `downloads`: the system's Downloads folder (a test's own, in tests).
fn place_held(place: Place, file: &Path, name: &str, folder: &str, downloads: &Path, config: &Config) -> Result<Placed, String> {
    let into = |folder: &Path, name: &str| {
        let names = |n: u32| rules::numbered(name, n, place.numbering());
        sioul_core::filelock::place_new(file, folder, &names, true).map_err(|e| say("download-place-failed", &[("file", rules::shortened(name, SHOWN_CHARS)), ("detail", e.to_string())]))
    };
    match place {
        Place::Downloads => into(downloads, name).map(|path| Placed { path, address: String::new(), then: Then::Nothing }),
        Place::Folder => {
            let folder = Path::new(folder.trim());
            if !folder.is_absolute() {
                return Err(tr().text("download-folder-unset", None));
            }
            into(folder, name).map(|path| Placed { path, address: String::new(), then: Then::Nothing })
        }
        Place::Notes => {
            let Some(root) = config.notes_root_path().filter(|r| r.is_dir()) else { return Err(tr().text("download-notes-none", None)) };
            let sub = config.notes_folder.clone().filter(|f| !f.trim().is_empty()).unwrap_or_else(crate::work::notes_folder);
            let sub = sioul_core::notes::normalize(&sub).unwrap_or_default();
            // Links in the notes break on these: as a new note's name, they become "-".
            let name = name.replace(['#', '^', '[', ']'], "-");
            let path = sioul_core::notes::with_lock(|| into(&root.join(&sub), &name))?;
            let relative = path.strip_prefix(&root).map(|p| p.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect::<Vec<_>>().join("/")).unwrap_or_default();
            sioul_core::changes::said(sioul_core::changes::Kind::Notes);
            Ok(Placed { path, address: sioul_core::notes::uri_of(&relative), then: Then::Nothing })
        }
        Place::Papers => {
            let (path, title, kind) = crate::papers::keep_checked(file, name, "")?;
            Ok(Placed { path: PathBuf::from(&path), address: sioul_core::links::paper_file_uri(&path), then: Then::Paper { file: path, title, kind } })
        }
        Place::Letters => {
            let path = crate::letters::add_one(file)?;
            let (id, _) = crate::letters::letter_id(&path);
            Ok(Placed { path, address: sioul_core::links::letter_uri(&id), then: Then::Letter(id) })
        }
    }
}

/// The notice of a file placed: about the file, or its folder for a
/// program; Open (never for a program) and Show in its folder in a folder
/// of the system's; Open in Sioul for Notes, Papers and Letters.
fn notice_of(place: Place, placed: &Placed, program: bool) -> Notice {
    let name = placed.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let shown = rules::shortened(&name, SHOWN_CHARS);
    let title = say(&format!("download-in-{}", place.id()), &[("file", shown)]);
    let folder = placed.path.parent().map(Path::to_path_buf).unwrap_or_default();
    let folder_url = format!("{}/", crate::backend::file_url(&folder).trim_end_matches('/'));
    let open = tr().text("ui-open", None);
    if place.in_sioul() {
        let body = match place {
            Place::Papers => tr().text("download-in-papers-body", None),
            Place::Letters => tr().text("download-in-letters-body", None),
            _ => folder.display().to_string(),
        };
        return Notice::new(placed.address.clone(), Kind::Minor, title, body).action(open, Act::Open { address: placed.address.clone(), hint: String::new() });
    }
    let show = Act::Open { address: folder_url.clone(), hint: String::new() };
    if program {
        return Notice::new(folder_url, Kind::Minor, title, tr().text("download-program-kept", None)).action(tr().text("file-show-folder", None), show);
    }
    let file_url = crate::backend::file_url(&placed.path);
    Notice::new(file_url.clone(), Kind::Minor, title, folder.display().to_string()).action(open, Act::Open { address: file_url, hint: String::new() }).action(tr().text("file-show-folder", None), show)
}

/// The file came whole (JSON in: `Ended`): placed where you said, off the
/// window's thread; its held folder taken away whatever happened; said in
/// the status line and by one notice; Papers' form or the letter then.
pub(crate) fn done(qt: &QtThread, shared: &Arc<Shared>, asked: &str) {
    let asked: Ended = serde_json::from_str(asked).unwrap_or_default();
    let (qt, shared) = (qt.clone(), Arc::clone(shared));
    std::thread::spawn(move || {
        let name = rules::clean_name(&asked.name);
        let Some(hold) = held_in(&hold_root(), &asked.hold) else {
            crate::backend::tell(&qt, &shared, say("download-failed-disk", &[("file", rules::shortened(&name, SHOWN_CHARS))]));
            return;
        };
        let Some(place) = Place::read(&asked.place) else {
            let _ = std::fs::remove_dir_all(&hold);
            return;
        };
        let placed = place_held(place, &hold.join(&name), &name, &asked.folder, &config::downloads_dir(), &load_config());
        let _ = std::fs::remove_dir_all(&hold);
        let placed = match placed {
            Ok(placed) => placed,
            Err(e) => {
                crate::backend::tell(&qt, &shared, e);
                return;
            }
        };
        let program = rules::is_program(&name, &asked.mime);
        let notice = notice_of(place, &placed, program);
        // The status line: where it is; for Papers and Letters, what follows too.
        let line = if matches!(place, Place::Papers | Place::Letters) { format!("{} {}", notice.title, notice.body) } else { notice.title.clone() };
        crate::backend::tell(&qt, &shared, line);
        let _ = crate::tell::tell(notice);
        match placed.then {
            Then::Nothing => {}
            Then::Paper { file, title, kind } => {
                let _ = qt.queue(move |mut sioul| sioul.as_mut().paper_kept(cxx_qt_lib::QString::from(&file), cxx_qt_lib::QString::from(&title), cxx_qt_lib::QString::from(kind)));
            }
            Then::Letter(id) => {
                let _ = qt.queue(move |mut sioul| sioul.as_mut().letter_filed(cxx_qt_lib::QString::from(&id)));
                // Read once it is twenty seconds old, as a scanner's file is (`letters::tick`), rather than at a later minute.
                std::thread::sleep(std::time::Duration::from_secs(21));
                crate::letters::tick(&qt, &shared);
            }
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
/// away, so that nothing half-written stays; said why.
pub(crate) fn failed(qt: &QtThread, shared: &Shared, asked: &str) {
    let asked: Ended = serde_json::from_str(asked).unwrap_or_default();
    if let Some(hold) = held_in(&hold_root(), &asked.hold) {
        let _ = std::fs::remove_dir_all(&hold);
    }
    crate::backend::tell(qt, shared, say(failed_why(asked.reason), &[("file", rules::shortened(&rules::clean_name(&asked.name), SHOWN_CHARS))]));
}

/// A word of the card's in the status line (JSON in: {name, host, site_name}):
/// "cancelled", nothing saved and the site told; "waiting", a file offered
/// while Sites is not in view, its question waiting there.
pub(crate) fn said(qt: &QtThread, shared: &Shared, what: &str, asked: &str) {
    let asked: Offered = serde_json::from_str(asked).unwrap_or_default();
    let host = host_of(&asked.url, &asked.page);
    let who = rules::shortened(if host.is_empty() { asked.site_name.trim() } else { &host }, SHOWN_CHARS);
    let file = rules::shortened(&rules::clean_name(&asked.name), SHOWN_CHARS);
    let line = match what {
        "cancelled" => say("download-cancelled", &[("file", file)]),
        "waiting" if who.is_empty() => tr().text("download-waiting-unknown", None),
        "waiting" => say("download-waiting", &[("host", who)]),
        _ => return,
    };
    crate::backend::tell(qt, shared, line);
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
        let placed = place_held(Place::Downloads, &file, "statement.pdf", "", &downloads, &config).unwrap();
        assert_eq!(placed.path, downloads.join("statement (2).pdf"));
        assert_eq!((placed.address.as_str(), &placed.then), ("", &Then::Nothing));
        assert_eq!(std::fs::read(downloads.join("statement.pdf")).unwrap(), b"older", "never replaced");
        assert!(!file.exists(), "moved, not copied");
        // A folder chosen in the system's window.
        let chosen = own.base.join("Chosen");
        let file = own.held("archive.zip", b"PK");
        assert_eq!(place_held(Place::Folder, &file, "archive.zip", &chosen.display().to_string(), &downloads, &config).unwrap().path, chosen.join("archive.zip"));
        let file = own.held("archive.zip", b"PK");
        assert!(place_held(Place::Folder, &file, "archive.zip", "relative/folder", &downloads, &config).is_err(), "a folder that is not absolute: refused");
        // Notes: in the folder new notes go into, an address of the notes, link-breaking characters made "-".
        let file = own.held("tableau [v2] #1.ods", b"ods");
        let placed = place_held(Place::Notes, &file, "tableau [v2] #1.ods", "", &downloads, &config).unwrap();
        assert_eq!(placed.path, own.base.join("notes/notes/tableau -v2- -1.ods"));
        assert_eq!(placed.address, "sioul:note/notes/tableau%20-v2-%20-1.ods");
        let file = own.held("tableau [v2] #1.ods", b"ods");
        assert_eq!(place_held(Place::Notes, &file, "tableau [v2] #1.ods", "", &downloads, &config).unwrap().path, own.base.join("notes/notes/tableau -v2- -1 2.ods"), "a note's own numbering");
        // Papers: by the wallet's own way in, the form's file, a title and a kind guessed.
        let file = own.held("avis_impot_2026.pdf", b"%PDF-2");
        let placed = place_held(Place::Papers, &file, "avis_impot_2026.pdf", "", &downloads, &config).unwrap();
        let kept = own.base.join("notes/papers/avis_impot_2026.pdf");
        assert_eq!(placed.path, kept);
        assert_eq!(placed.address, sioul_core::links::paper_file_uri(&kept.display().to_string()));
        assert!(matches!(&placed.then, Then::Paper { file, title, .. } if *file == kept.display().to_string() && title == "avis impot 2026"), "{:?}", placed.then);
        // Letters: where scans arrive, the id its reading will give it.
        let file = own.held("courrier.pdf", b"%PDF-3");
        let placed = place_held(Place::Letters, &file, "courrier.pdf", "", &downloads, &config).unwrap();
        let inbox = own.base.join("notes/letters/inbox/courrier.pdf");
        assert_eq!(placed.path, inbox);
        let id = crate::letters::letter_id(&inbox).0;
        assert_eq!((placed.address, placed.then), (sioul_core::links::letter_uri(&id), Then::Letter(id)));
        // Letters read scans alone.
        let file = own.held("archive.zip", b"PK");
        assert!(place_held(Place::Letters, &file, "archive.zip", "", &downloads, &config).is_err());
        assert_eq!(own.names("notes/letters/inbox"), ["courrier.pdf"], "nothing else, nothing hidden");
    }

    /// A place that cannot take the file leaves nothing in it, and the held
    /// file goes back where it was, for its folder to be taken away.
    #[test]
    fn a_place_that_fails_keeps_nothing() {
        let own = Own::new("downloads-fails", "");
        let config = load_config();
        let file = own.held("statement.pdf", b"%PDF");
        assert!(place_held(Place::Notes, &file, "statement.pdf", "", &own.base.join("Downloads"), &config).is_err(), "no notes folder");
        assert!(place_held(Place::Papers, &file, "statement.pdf", "", &own.base.join("Downloads"), &config).is_err(), "no place for papers");
        assert!(file.exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let closed = own.base.join("Closed");
            std::fs::create_dir_all(&closed).unwrap();
            std::fs::set_permissions(&closed, std::fs::Permissions::from_mode(0o500)).unwrap();
            if std::fs::write(closed.join("probe"), b"").is_err() {
                assert!(place_held(Place::Folder, &file, "statement.pdf", &closed.display().to_string(), &own.base.join("Downloads"), &config).is_err());
                assert!(std::fs::read_dir(&closed).unwrap().next().is_none(), "nothing half-written there");
                assert!(file.exists(), "the held file put back");
            }
            std::fs::set_permissions(&closed, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
    }

    /// The held folders: one per file, taken away only when one of Sioul's
    /// own; a day-old one left by a Sioul stopped meanwhile swept.
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
        assert_eq!(view.from, say("download-from-size", &[("size", sioul_core::view::size(tr(), 1_258_291)), ("host", "secure.bank.example".into())]));
        let ids: Vec<(&str, bool)> = view.places.iter().map(|p| (p.id, p.enabled)).collect();
        assert_eq!(ids, [("downloads", true), ("folder", true), ("notes", true), ("papers", true), ("letters", true)]);
        // The place chosen last for this site first; a site of its own its own.
        memory.chose("bank", Place::Papers, "");
        assert_eq!(offer_view(&config, &asked("statement.pdf", ""), &memory).first, "papers");
        // A program: its caution, and Letters, which read scans alone, cannot take it.
        let view = offer_view(&config, &asked("setup.exe", "application/x-msdownload"), &memory);
        assert!(view.program && !view.caution.is_empty());
        assert_eq!(view.places.iter().find(|p| p.id == "letters").map(|p| p.enabled), Some(false));
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

    /// The notice: Open and Show in its folder in a system's folder, the
    /// folder alone for a program; the thing in Sioul for Notes, Papers, Letters.
    #[test]
    fn the_notice_opens_the_file_never_a_program() {
        let path = PathBuf::from("/stand-in/Downloads/statement (2).pdf");
        let placed = Placed { path: path.clone(), address: String::new(), then: Then::Nothing };
        let notice = notice_of(Place::Downloads, &placed, false);
        let acts: Vec<&Act> = notice.actions.iter().map(|a| &a.act).collect();
        assert_eq!(notice.about, crate::backend::file_url(&path));
        assert_eq!(acts, [&Act::Open { address: crate::backend::file_url(&path), hint: String::new() }, &Act::Open { address: "file:///stand-in/Downloads/".into(), hint: String::new() }]);
        let program = Placed { path: PathBuf::from("/stand-in/Downloads/setup.exe"), address: String::new(), then: Then::Nothing };
        let notice = notice_of(Place::Downloads, &program, true);
        assert_eq!(notice.about, "file:///stand-in/Downloads/");
        assert_eq!(notice.actions.iter().map(|a| &a.act).collect::<Vec<_>>(), [&Act::Open { address: "file:///stand-in/Downloads/".into(), hint: String::new() }], "never the program");
        let note = Placed { path: PathBuf::from("/stand-in/Notes/notes/x.ods"), address: "sioul:note/notes/x.ods".into(), then: Then::Nothing };
        let notice = notice_of(Place::Notes, &note, false);
        assert_eq!((notice.about.as_str(), notice.actions.len()), ("sioul:note/notes/x.ods", 1));
        assert_eq!(notice.kind, Kind::Minor);
    }
}
