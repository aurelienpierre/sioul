// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! A file chosen in a dialog (docs/android.md, "Files chosen in a dialog").
//! On a computer a dialog answers with a `file://` address, the file's
//! place. On Android it answers with a `content://` address, a document of
//! some provider (the phone's storage, Downloads, a cloud's app), never a
//! path: Android alone reads it, and Qt keeps the leave to. Every caller that
//! receives a dialog's answer goes through here:
//!
//! - **`take`**, a file to read now (a paper kept, a scan added, a mail
//!   attachment, a bank export, contacts, a key, a certificate): a `file://`
//!   address or a path is the file itself, as before; a `content://` one is
//!   copied through Android's own reader (`Chosen.java`, which reads as the
//!   share sheet does) into a folder of Sioul's cache made for it, under its
//!   own name. The copy goes once what asked for it has taken it (`Chosen`
//!   dropped), or moves where it belongs (`Chosen::keep_in`, a draft's
//!   attachment). A file larger than `MOST`, or one that would leave the
//!   device with less than `MARGIN`, is not copied, and that is said.
//! - **`linkable`**, a file linked where it is (letters "linked where they
//!   are"): a document of the phone's own storage is linked by its real
//!   path when Sioul may read it there; any other `content://` address names
//!   no file Sioul could read later, and the window offers to add a copy.
//! - **`save`**, a file written where a "Save" dialog said: a path as before;
//!   a `content://` document (Android made it, empty, where you chose) is
//!   written through the attachments' own writer (`Attachments.write`),
//!   which removes it again when the copy fails.
//! - **`at_start`**: what a stopped Sioul left in the cache's folder goes,
//!   once this is known to be the profile's Sioul.

use std::path::{Component, Path, PathBuf};

use serde_json::{Value, json};

use crate::backend::say;

/// The largest file copied from a dialog: 100 MB.
pub(crate) const MOST: u64 = 100 * 1024 * 1024;
/// The room left on the device after a copy, at the least: 200 MB, as for
/// a site's file (`SiteDownload.MARGIN`).
pub(crate) const MARGIN: u64 = 200 * 1024 * 1024;

/// The phone's own storage, as its documents provider names a document:
/// `primary:Documents/scan.pdf`.
const STORAGE: &str = "com.android.externalstorage.documents";
/// Downloads' provider, which names a file of the Download folder by its
/// path (`raw:/storage/emulated/0/Download/…`), or by a number.
const DOWNLOADS: &str = "com.android.providers.downloads.documents";

/// What a dialog's answer names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Address {
    /// A file of this device by its place: a `file://` address, or a path already.
    Path(PathBuf),
    /// A document of the phone's own storage, with the path it has there,
    /// which Sioul may read only with "All files access".
    Storage { uri: String, path: PathBuf },
    /// A document of another provider (a cloud's app, the pictures, a
    /// download by its number): only Android reads it.
    Content(String),
}

/// What `url`, a dialog's answer, names.
pub(crate) fn address(url: &str) -> Address {
    let url = url.trim();
    let Some(rest) = url.get(..10).filter(|start| start.eq_ignore_ascii_case("content://")).map(|_| &url[10..]) else {
        return Address::Path(crate::backend::local_path(url));
    };
    let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
    let path = path.split(['?', '#']).next().unwrap_or_default();
    let id = match path.split('/').collect::<Vec<_>>().as_slice() {
        ["document", id] | ["tree", _, "document", id] => Some(crate::projects::percent_decoded(id)),
        _ => None,
    };
    match id.and_then(|id| storage_path(authority, &id)) {
        Some(path) => Address::Storage { uri: url.to_string(), path },
        None => Address::Content(url.to_string()),
    }
}

/// The path a document of the phone's own storage has, from its provider
/// and its id: "primary" the phone's storage, "home" its Documents, another
/// volume a card's; Downloads' "raw:" a path as it is. None for any other,
/// and for a path that is not whole (relative, or with "." or "..").
fn storage_path(authority: &str, id: &str) -> Option<PathBuf> {
    let path = match authority {
        DOWNLOADS => id.strip_prefix("raw:")?.to_string(),
        STORAGE => {
            let (volume, rest) = id.split_once(':')?;
            let root = match volume {
                "primary" => "/storage/emulated/0".to_string(),
                "home" => "/storage/emulated/0/Documents".to_string(),
                card if !card.is_empty() && card.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') => format!("/storage/{card}"),
                _ => return None,
            };
            if rest.is_empty() { root } else { format!("{root}/{rest}") }
        }
        _ => return None,
    };
    let path = PathBuf::from(path);
    let whole = path.to_string_lossy().starts_with('/') && path.components().all(|c| !matches!(c, Component::ParentDir | Component::CurDir));
    whole.then_some(path)
}

/// The name a dialog's answer shows: a file's own; a document's as its
/// provider gives it (Android asked), else the end of its address.
pub(crate) fn shown_name(url: &str) -> String {
    match address(url) {
        Address::Path(path) | Address::Storage { path, .. } => path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
        Address::Content(uri) => {
            let answer = crate::steps::java("chosen-name", &json!({ "uri": uri }).to_string());
            answer["name"].as_str().map(str::trim).filter(|n| !n.is_empty()).map_or_else(|| last_part(&uri), str::to_string)
        }
    }
}

/// The end of a document's address, its escapes undone: what names it when
/// its provider does not.
fn last_part(uri: &str) -> String {
    let path = uri.split(['?', '#']).next().unwrap_or_default();
    let decoded = crate::projects::percent_decoded(path.rsplit('/').next().unwrap_or_default());
    decoded.rsplit('/').next().unwrap_or_default().to_string()
}

/// One plain name: no separator, not "." nor "..", not empty.
fn plain(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.contains(['/', '\\', '\0'])
}

// ---------------------------------------------------------------- copied to be read

/// Where dialogs' files are copied: `<cache>/sioul/chosen`, each in a folder of its own.
fn root() -> PathBuf {
    sioul_core::config::cache_dir().join("chosen")
}

/// A folder of its own in `root` for one file (0700).
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

/// A held folder taken away: its files overwritten first (a key, a bank's
/// export), then the folder.
fn gone(hold: &Path) {
    for entry in std::fs::read_dir(hold).into_iter().flatten().filter_map(Result::ok) {
        let _ = sioul_core::filelock::remove_wiping(&entry.path());
    }
    let _ = std::fs::remove_dir_all(hold);
}

/// A file chosen, to read at `path`: the file itself, or Sioul's copy of
/// it, which goes when this is dropped.
#[derive(Debug)]
pub(crate) struct Chosen {
    path: PathBuf,
    /// The folder held for the copy; none for a file of this device.
    hold: Option<PathBuf>,
}

impl Chosen {
    /// Where to read it.
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    /// Kept by what took it, in `folder` (a draft's attachments): Sioul's
    /// copy moved there under its own name, and its path there; a file of
    /// this device stays where it is, and its path is given.
    pub(crate) fn keep_in(self, folder: &Path) -> Result<PathBuf, String> {
        if self.hold.is_none() {
            return Ok(self.path.clone());
        }
        let name = self.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let names = |n: u32| sioul_core::downloads::numbered(&name, n, sioul_core::downloads::Numbering::Paren);
        std::fs::create_dir_all(folder)
            .and_then(|()| sioul_core::filelock::place_new(&self.path, folder, &names, true))
            .map_err(|e| say("chosen-unreadable", &[("name", name.clone()), ("detail", e.to_string())]))
    }
}

impl Drop for Chosen {
    fn drop(&mut self) {
        if let Some(hold) = self.hold.take() {
            gone(&hold);
        }
    }
}

/// The file a dialog's answer names, ready to read: the file itself, or a
/// copy made through Android's own reader; what went wrong in words, to be
/// said in the dialog.
pub(crate) fn take(url: &str) -> Result<Chosen, String> {
    take_in(&root(), url, &|uri, hold| {
        let asked = json!({ "uri": uri, "folder": hold.display().to_string(), "most": MOST, "margin": MARGIN });
        crate::steps::java("chosen-copy", &asked.to_string())
    })
}

/// `take`, its copies held in `root`, Android's reader being `copy`.
fn take_in(root: &Path, url: &str, copy: &dyn Fn(&str, &Path) -> Value) -> Result<Chosen, String> {
    let uri = match address(url) {
        Address::Path(path) => return Ok(Chosen { path, hold: None }),
        Address::Storage { uri, .. } | Address::Content(uri) => uri,
    };
    let hold = new_hold(root).map_err(|e| say("chosen-unreadable", &[("name", last_part(&uri)), ("detail", e.to_string())]))?;
    match copied(&copy(&uri, &hold), &hold, &uri) {
        Ok(path) => Ok(Chosen { path, hold: Some(hold) }),
        Err(e) => {
            gone(&hold);
            Err(e)
        }
    }
}

/// Android's answer to a copy: the copy's path, one plain name inside
/// `hold` and a file there; else what went wrong, in words.
fn copied(answer: &Value, hold: &Path, uri: &str) -> Result<PathBuf, String> {
    let name = answer["name"].as_str().map(str::trim).filter(|n| !n.is_empty()).map_or_else(|| last_part(uri), str::to_string);
    if let Some(path) = answer["path"].as_str() {
        let path = PathBuf::from(path);
        if plain(&name) && path == hold.join(&name) && path.is_file() {
            return Ok(path);
        }
        return Err(say("chosen-unreadable", &[("name", name), ("detail", "not copied where asked".to_string())]));
    }
    Err(match answer["problem"].as_str() {
        Some("large") => say("chosen-too-large", &[("name", name), ("mb", (MOST / 1024 / 1024).to_string())]),
        Some("room") => say("chosen-no-room", &[("name", name)]),
        Some(problem) => say("chosen-unreadable", &[("name", name), ("detail", answer["detail"].as_str().unwrap_or(problem).to_string())]),
        None => say("chosen-no-answer", &[("name", name)]),
    })
}

// ---------------------------------------------------------------- linked where it is

/// The path a file chosen to be linked where it is can be read at later:
/// a file of this device; a document of the phone's own storage that Sioul
/// may read there. None for any other document: only a copy can be kept.
pub(crate) fn linkable(url: &str) -> Option<PathBuf> {
    match address(url) {
        Address::Path(path) => Some(path),
        Address::Storage { path, .. } if std::fs::File::open(&path).is_ok() => Some(path),
        Address::Storage { .. } | Address::Content(_) => None,
    }
}

// ---------------------------------------------------------------- saved where you chose

/// `bytes` written where a "Save" dialog answered `url`: a path by `local`,
/// as before; a document Android made through the attachments' writer.
/// `name` names the file in words. Where it went, as it is said there; else
/// what went wrong, in words.
pub(crate) fn save(url: &str, name: &str, bytes: &[u8], local: impl FnOnce(&Path) -> Result<(), String>) -> Result<String, String> {
    save_in(&root(), url, name, bytes, local, &|path, uri| {
        crate::steps::java("chosen-save", &json!({ "path": path.display().to_string(), "uri": uri, "name": name }).to_string())
    })
}

/// `save`, the bytes held in `root` meanwhile, Android's writer being `write`.
fn save_in(root: &Path, url: &str, name: &str, bytes: &[u8], local: impl FnOnce(&Path) -> Result<(), String>, write: &dyn Fn(&Path, &str) -> Value) -> Result<String, String> {
    let uri = match address(url) {
        Address::Path(path) => return local(&path).map(|()| path.display().to_string()),
        Address::Storage { uri, .. } | Address::Content(uri) => uri,
    };
    let not_saved = |detail: String| say("chosen-not-saved", &[("name", name.to_string()), ("detail", detail)]);
    let hold = new_hold(root).map_err(|e| not_saved(e.to_string()))?;
    let file = hold.join(if plain(name) { name } else { "file" });
    let said = match std::fs::write(&file, bytes) {
        Ok(()) => {
            let answer = write(&file, &uri);
            if answer["saved"] == true {
                Ok(answer["shown"].as_str().map(str::trim).filter(|s| !s.is_empty()).unwrap_or(name).to_string())
            } else {
                Err(not_saved(answer["problem"].as_str().unwrap_or("no answer").to_string()))
            }
        }
        Err(e) => Err(not_saved(e.to_string())),
    };
    gone(&hold);
    said
}

// ---------------------------------------------------------------- at the start

/// What an earlier run left in the cache's folder (a Sioul stopped while it
/// copied): taken away off the window's start. Called once this is known to
/// be the profile's Sioul: no dialog's file can be under way yet.
pub(crate) fn at_start() {
    let root = root();
    let aside = set_aside(&root);
    std::thread::spawn(move || sweep(&root, aside.as_deref()));
}

/// The folder renamed aside at once, so that a copy made meanwhile is never
/// the sweep's: its new name; none when there was nothing to set aside.
fn set_aside(root: &Path) -> Option<PathBuf> {
    let aside = root.with_file_name(format!("chosen-gone-{}", std::process::id()));
    std::fs::rename(root, &aside).is_ok().then_some(aside)
}

/// The folder set aside taken away, each copy overwritten first, and those
/// an earlier start left while removing.
fn sweep(root: &Path, aside: Option<&Path>) {
    let Some(parent) = root.parent() else { return };
    let mut left: Vec<PathBuf> = std::fs::read_dir(parent)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().starts_with("chosen-gone-"))
        .map(|e| e.path())
        .collect();
    left.extend(aside.map(Path::to_path_buf));
    left.sort();
    left.dedup();
    for folder in left {
        for hold in std::fs::read_dir(&folder).into_iter().flatten().filter_map(Result::ok) {
            gone(&hold.path());
        }
        let _ = std::fs::remove_dir_all(&folder);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A test's own folder, removed when done.
    struct Own(PathBuf);

    impl Own {
        fn new(name: &str) -> Own {
            sioul_core::config::stand_ins_only();
            let base = sioul_core::config::cache_dir().join(format!("chosen-test-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&base);
            std::fs::create_dir_all(&base).unwrap();
            Own(base)
        }
    }

    impl Drop for Own {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_dialogs_answer_is_read_by_its_kind() {
        // A computer's: a file:// address, or a path, as before.
        assert_eq!(address("file:///home/me/Re%C3%A7u%20final.pdf"), Address::Path(PathBuf::from("/home/me/Reçu final.pdf")));
        assert_eq!(address("/home/me/plain.pdf"), Address::Path(PathBuf::from("/home/me/plain.pdf")));
        // The phone's own storage, by the path it has there: the phone, its Documents, a card.
        let storage = |uri: &str, path: &str| assert_eq!(address(uri), Address::Storage { uri: uri.to_string(), path: PathBuf::from(path) }, "{uri}");
        storage("content://com.android.externalstorage.documents/document/primary%3ADocuments%2FScans%2Favis%20d%E2%80%99imp%C3%B4t.pdf", "/storage/emulated/0/Documents/Scans/avis d’impôt.pdf");
        storage("content://com.android.externalstorage.documents/document/home%3Areleve.pdf", "/storage/emulated/0/Documents/releve.pdf");
        storage("content://com.android.externalstorage.documents/document/1A2B-3C4D%3AScans%2Fx.pdf", "/storage/1A2B-3C4D/Scans/x.pdf");
        storage("content://com.android.externalstorage.documents/tree/primary%3ADocuments/document/primary%3ADocuments%2Fx.pdf", "/storage/emulated/0/Documents/x.pdf");
        storage("content://com.android.providers.downloads.documents/document/raw%3A%2Fstorage%2Femulated%2F0%2FDownload%2Fbank.ofx", "/storage/emulated/0/Download/bank.ofx");
        storage("CONTENT://com.android.externalstorage.documents/document/primary%3Ax.pdf", "/storage/emulated/0/x.pdf");
        // Any other: only Android reads it.
        for uri in [
            "content://com.android.providers.downloads.documents/document/msf%3A1000000033",
            "content://com.android.providers.downloads.documents/document/42",
            "content://com.android.providers.media.documents/document/image%3A1234",
            "content://org.example.cloud.documents/document/%2Fscans%2Fx.pdf",
            "content://com.android.externalstorage.documents/document/no-colon",
            "content://com.android.externalstorage.documents/document/weird%20volume%3Ax.pdf",
            "content://com.android.externalstorage.documents/document/primary%3A..%2F..%2Fdata%2Fdata%2Fx",
            "content://com.android.providers.downloads.documents/document/raw%3Arelative%2Fx.pdf",
            "content://com.android.externalstorage.documents/root/primary",
        ] {
            assert_eq!(address(uri), Address::Content(uri.to_string()), "{uri}");
        }
        // Its name in words, where Android is not asked.
        assert_eq!(shown_name("file:///home/me/Re%C3%A7u%20final.pdf"), "Reçu final.pdf");
        assert_eq!(shown_name("content://com.android.externalstorage.documents/document/primary%3ADocuments%2Fscan.pdf"), "scan.pdf");
        assert_eq!(last_part("content://org.example.cloud.documents/document/%2Fscans%2Fx%20y.pdf?x=1"), "x y.pdf");
    }

    #[test]
    fn a_document_is_copied_under_its_own_name_and_goes_once_taken() {
        let own = Own::new("copy");
        let root = own.0.join("chosen");
        let uri = "content://org.example.cloud.documents/document/7";
        // Android's reader, stood in for: the copy in the folder asked, under the name it gives.
        let reader = |given: &str, hold: &Path| {
            assert_eq!(given, uri);
            std::fs::write(hold.join("Relevé octobre.pdf"), b"%PDF stand-in").unwrap();
            json!({ "path": hold.join("Relevé octobre.pdf").display().to_string(), "name": "Relevé octobre.pdf" })
        };
        let chosen = take_in(&root, uri, &reader).unwrap();
        assert_eq!(chosen.path().file_name().unwrap(), "Relevé octobre.pdf");
        assert_eq!(std::fs::read(chosen.path()).unwrap(), b"%PDF stand-in");
        let hold = chosen.path().parent().unwrap().to_path_buf();
        assert_eq!(hold.parent(), Some(root.as_path()), "a folder of its own in the cache's");
        drop(chosen);
        assert!(!hold.exists(), "the copy goes once taken");
        // A file of this device is never copied, and stays when taken.
        let mine = own.0.join("mine.pdf");
        std::fs::write(&mine, b"mine").unwrap();
        let never = |_: &str, _: &Path| -> Value { panic!("Android asked for a file of this device") };
        let chosen = take_in(&root, &crate::backend::file_url(&mine), &never).unwrap();
        assert_eq!(chosen.path(), mine);
        drop(chosen);
        assert!(mine.is_file());
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0, "nothing held");
    }

    #[test]
    fn a_copy_that_fails_leaves_nothing_and_says_why() {
        let own = Own::new("fails");
        let root = own.0.join("chosen");
        let uri = "content://com.android.providers.downloads.documents/document/msf%3A12";
        let said = |answer: Value| {
            let error = take_in(&root, uri, &|_, hold| {
                // Half a file left by the reader, as a stopped copy would.
                std::fs::write(hold.join("half.pdf.new"), b"%PDF").unwrap();
                answer.clone()
            })
            .unwrap_err();
            assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0, "nothing left for {answer}");
            error
        };
        let large = said(json!({ "problem": "large", "name": "film.mkv" }));
        assert!(large.contains("film.mkv") && large.contains("100"), "{large}");
        let room = said(json!({ "problem": "room", "name": "scan.pdf" }));
        assert!(room.contains("scan.pdf"), "{room}");
        let failed = said(json!({ "problem": "failed", "name": "scan.pdf", "detail": "Permission denied" }));
        assert!(failed.contains("scan.pdf") && failed.contains("Permission denied"), "{failed}");
        // Android silent (no phone): said, named by the end of the address.
        let silent = said(Value::Null);
        assert!(silent.contains("msf:12"), "{silent}");
        // An answer naming a file elsewhere, or a name that climbs: never taken.
        let elsewhere = said(json!({ "path": "/etc/hosts", "name": "hosts" }));
        assert!(elsewhere.contains("hosts"), "{elsewhere}");
        let climbing = said(json!({ "path": root.join("x/../../escape").display().to_string(), "name": "../escape" }));
        assert!(!climbing.is_empty());
        // The words differ by what happened.
        assert!(large != room && room != failed && failed != silent);
    }

    #[test]
    fn a_copy_taken_into_a_draft_moves_there_and_a_file_of_this_device_stays() {
        let own = Own::new("keep");
        let root = own.0.join("chosen");
        let drafts = own.0.join("handed/files/one");
        let chosen = take_in(&root, "content://org.example.cloud.documents/document/9", &|_, hold| {
            std::fs::write(hold.join("devis.pdf"), b"%PDF").unwrap();
            json!({ "path": hold.join("devis.pdf").display().to_string(), "name": "devis.pdf" })
        })
        .unwrap();
        let kept = chosen.keep_in(&drafts).unwrap();
        assert_eq!(kept, drafts.join("devis.pdf"));
        assert_eq!(std::fs::read(&kept).unwrap(), b"%PDF");
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0, "its hold gone");
        let mine = own.0.join("mine.pdf");
        std::fs::write(&mine, b"mine").unwrap();
        let chosen = take_in(&root, mine.to_str().unwrap(), &|_, _| Value::Null).unwrap();
        assert_eq!(chosen.keep_in(&drafts).unwrap(), mine);
        assert!(mine.is_file() && !drafts.join("mine.pdf").exists(), "never copied");
    }

    #[test]
    fn only_a_file_sioul_may_read_later_is_linked_where_it_is() {
        let own = Own::new("link");
        let mine = own.0.join("courrier.pdf");
        std::fs::write(&mine, b"%PDF").unwrap();
        assert_eq!(linkable(&crate::backend::file_url(&mine)), Some(mine.clone()));
        // A path, as before, even one gone: the letters say it is not there.
        assert_eq!(linkable("/nowhere/gone.pdf"), Some(PathBuf::from("/nowhere/gone.pdf")));
        // A document of a provider: only a copy.
        assert_eq!(linkable("content://org.example.cloud.documents/document/9"), None);
        assert_eq!(linkable("content://com.android.providers.downloads.documents/document/msf%3A12"), None);
        // The phone's storage, where Sioul cannot read it (here: no such folder).
        assert_eq!(linkable("content://com.android.externalstorage.documents/document/primary%3ADocuments%2Fnot-here.pdf"), None);
    }

    #[test]
    fn a_file_saved_where_android_made_it_goes_through_its_writer() {
        let own = Own::new("save");
        let root = own.0.join("chosen");
        let uri = "content://com.android.externalstorage.documents/document/primary%3ADownload%2Ftime.csv";
        // A path, as before: written there, said by its path.
        let target = own.0.join("time.csv");
        let place = save_in(&root, &crate::backend::file_url(&target), "time.csv", b"a;b", |p| std::fs::write(p, b"a;b").map_err(|e| e.to_string()), &|_, _| panic!("Android asked for a path")).unwrap();
        assert_eq!((place, std::fs::read(&target).unwrap()), (target.display().to_string(), b"a;b".to_vec()));
        // A document: the bytes handed to the writer from a file of their own, which goes after.
        let handed = std::cell::RefCell::new(None);
        let writer = |path: &Path, given: &str| {
            assert_eq!(given, uri);
            *handed.borrow_mut() = Some((path.to_path_buf(), std::fs::read(path).unwrap()));
            json!({ "saved": true, "shown": "time (1).csv" })
        };
        let place = save_in(&root, uri, "time.csv", b"a;b", |_| panic!("written as a path"), &writer).unwrap();
        assert_eq!(place, "time (1).csv");
        let (path, bytes) = handed.borrow_mut().take().unwrap();
        assert_eq!((path.file_name().unwrap().to_string_lossy().to_string(), bytes), ("time.csv".to_string(), b"a;b".to_vec()));
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0, "nothing held after");
        // The writer refusing: said in words, nothing held.
        let error = save_in(&root, uri, "time.csv", b"a;b", |_| Ok(()), &|_, _| json!({ "saved": false, "problem": "IOException" })).unwrap_err();
        assert!(error.contains("time.csv") && error.contains("IOException"), "{error}");
        let silent = save_in(&root, uri, "../x", b"a", |_| Ok(()), &|path, _| {
            assert_eq!(path.file_name().unwrap(), "file", "a name that climbs is never used");
            Value::Null
        })
        .unwrap_err();
        assert!(!silent.is_empty());
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
    }

    #[test]
    fn what_a_stopped_sioul_left_goes_at_the_next_start() {
        let own = Own::new("start");
        let root = own.0.join("chosen");
        // A copy left by a Sioul stopped while it copied, and a folder an earlier start left while removing.
        let hold = new_hold(&root).unwrap();
        std::fs::write(hold.join("key.asc"), b"secret").unwrap();
        let earlier = own.0.join("chosen-gone-1");
        std::fs::create_dir_all(earlier.join("x")).unwrap();
        std::fs::write(earlier.join("x/scan.pdf"), b"%PDF").unwrap();
        let aside = set_aside(&root);
        assert!(aside.is_some() && !root.exists(), "set aside at once");
        // A copy made right after is never the sweep's.
        let after = new_hold(&root).unwrap();
        sweep(&root, aside.as_deref());
        assert!(!earlier.exists() && !aside.unwrap().exists(), "both taken away");
        assert!(after.is_dir(), "a copy made since stays");
        // Nothing there: nothing to do.
        let empty = own.0.join("empty/chosen");
        assert_eq!(set_aside(&empty), None);
    }
}
