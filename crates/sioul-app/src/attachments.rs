// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Mail attachments on a phone (docs/client.md, "Antivirus"). A phone has no
//! antivirus Sioul can call, so none is asked and none is claimed: the words
//! say the file was not checked. The attachment opens in the app you choose,
//! which Android lets read that one file and nothing else of Sioul's, and
//! "Save…" asks Android where (android/package/src/com/aurelienpierre/sioul/
//! Attachments.java, AttachmentSave.java). Sioul's own refusals stand as on a
//! computer: mail set aside keeps its attachments closed, and programs,
//! scripts and installers, Android's among them, are never opened.
//!
//! A computer is unchanged (`backend::checked_attachment`): its antivirus is
//! asked, or you are, when it has none.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use sioul_core::config;

use crate::backend::{QtThread, say, set_status};

/// Whether attachments go the phone's way: on Android; and in a demo's
/// pictures of a phone's screen (`SIOUL_GRAB_PHONE` with `SIOUL_DEMO`), so
/// that they show its words. There nothing reaches another app: Android is
/// not there to answer.
pub(crate) fn phone() -> bool {
    cfg!(target_os = "android") || (crate::backend::offline() && std::env::var_os("SIOUL_GRAB_PHONE").is_some())
}

/// What stands between an attachment and where it goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Check {
    /// A computer: its antivirus is asked.
    Scan,
    /// A computer, its antivirus not asked again: the same bytes were found
    /// clean earlier in this session, or you chose to go on without one.
    Skip,
    /// A phone: no antivirus can be called, and none is claimed.
    Unchecked,
}

/// The check an attachment gets, from where Sioul runs.
pub(crate) fn check(phone: bool, known_clean: bool, unchecked: bool) -> Check {
    if phone {
        Check::Unchecked
    } else if known_clean || unchecked {
        Check::Skip
    } else {
        Check::Scan
    }
}

/// Whether an attachment is kept from opening (it may still be saved): a
/// program, a script, an installer, a shortcut, a disk image
/// (`links::is_program`, as on a computer); on a phone, Android's
/// installers too (`links::is_phone_program`).
pub(crate) fn kept_closed(name: &str, phone: bool) -> bool {
    sioul_core::links::is_program(name) || (phone && sioul_core::links::is_phone_program(name))
}

/// The type an app is asked to open, from the name's extension, as Sioul
/// names the attachments it sends (`compose::mime_of`): never the type the
/// message claims. Java asks Android's own table for those this one does not
/// know, and never hands an installer.
pub(crate) fn type_of(name: &str) -> &'static str {
    sioul_core::compose::mime_of(name)
}

/// The folder attachments are written in to be opened:
/// `<cache>/sioul/attachments`, which Android's provider may lend from
/// (android/package/res/xml/qtprovider_paths.xml).
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub(crate) fn folder() -> PathBuf {
    config::cache_dir().join("attachments")
}

/// What Java's answer to "open" says, as the status line.
pub(crate) fn opened_line(answer: &serde_json::Value, name: &str) -> String {
    if answer["opened"] == true {
        return say("attachment-phone-opened", &[("name", name.to_string())]);
    }
    match answer["problem"].as_str() {
        Some("no-app") => say("attachment-phone-no-app", &[("name", name.to_string())]),
        problem => {
            let detail = answer["detail"].as_str().or(problem).unwrap_or("no answer");
            say("attachment-phone-not-opened", &[("name", name.to_string()), ("detail", detail.to_string())])
        }
    }
}

/// What AttachmentSave said once Android's question was answered, as the status line.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub(crate) fn saved_line(answer: &serde_json::Value) -> String {
    let shown = answer["shown"].as_str().filter(|s| !s.is_empty()).or_else(|| answer["name"].as_str()).unwrap_or("").to_string();
    if answer["saved"] == true {
        say("attachment-phone-saved", &[("name", shown)])
    } else if let Some(problem) = answer["problem"].as_str() {
        say("attachment-phone-save-failed", &[("name", shown), ("detail", problem.to_string())])
    } else {
        say("attachment-phone-not-saved", &[("name", shown)])
    }
}

/// The attachment written at `file` handed to the app you choose (Android
/// asks which, when none is set for its type); the line to show. A copy no
/// app took leaves the cache at once.
pub(crate) fn open(file: &Path, name: &str) -> String {
    let asked = serde_json::json!({ "path": file.display().to_string(), "type": type_of(name) });
    let answer = crate::steps::java("attachment-open", &asked.to_string());
    if answer["opened"] != true {
        let _ = std::fs::remove_file(file);
    }
    opened_line(&answer, name)
}

/// The window waiting for Android's question where to save to be answered.
static SAVING: Mutex<Option<QtThread>> = Mutex::new(None);

/// Android asked where to save the attachment written at `file`; the line to
/// show meanwhile. AttachmentSave copies it there, takes it out of the cache
/// and says what became of it (`sioul_attachment_saved`).
pub(crate) fn save(qt: &QtThread, file: &Path, name: &str) -> String {
    if let Ok(mut waiting) = SAVING.lock() {
        *waiting = Some(qt.clone());
    }
    let asked = serde_json::json!({ "path": file.display().to_string(), "name": name, "type": type_of(name) });
    let answer = crate::steps::java("attachment-save", &asked.to_string());
    if answer["asked"] == true {
        return say("attachment-phone-where", &[("name", name.to_string())]);
    }
    let _ = std::fs::remove_file(file);
    let detail = answer["detail"].as_str().or_else(|| answer["problem"].as_str()).unwrap_or("no answer");
    say("attachment-phone-save-failed", &[("name", name.to_string()), ("detail", detail.to_string())])
}

/// AttachmentSave's word (android/main.cpp), once the copy is made or not:
/// {path, name, saved, shown} or {path, name, saved: false, problem}; the
/// status line says it. The copy in the cache goes, saved or not: Java took
/// it out, and here it is taken out again if it stayed. On a thread of Java's.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn saved(text: &str) {
    let answer: serde_json::Value = serde_json::from_str(text).unwrap_or(serde_json::Value::Null);
    if let Some(path) = answer["path"].as_str().map(Path::new)
        && path.starts_with(folder())
    {
        let _ = std::fs::remove_file(path);
    }
    let waiting = SAVING.lock().ok().and_then(|w| w.clone());
    if let Some(qt) = waiting {
        set_status(&qt, saved_line(&answer));
    }
}

/// AttachmentSave.nativeSaved's side here (android/main.cpp).
///
/// # Safety
/// `json` is null, or a zero-terminated UTF-8 text valid for the call.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sioul_attachment_saved(json: *const std::ffi::c_char) {
    // SAFETY: as the caller promises.
    let text = if json.is_null() { String::new() } else { unsafe { std::ffi::CStr::from_ptr(json) }.to_string_lossy().into_owned() };
    let _ = std::panic::catch_unwind(|| saved(&text));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_phone_asks_no_antivirus_and_a_computer_is_unchanged() {
        // A phone: never a scan, whatever was found or chosen before.
        for (known, chosen) in [(false, false), (true, false), (false, true), (true, true)] {
            assert_eq!(check(true, known, chosen), Check::Unchecked);
        }
        // A computer: the antivirus, unless the same bytes passed it, or you chose to go on without one.
        assert_eq!(check(false, false, false), Check::Scan);
        assert_eq!(check(false, true, false), Check::Skip);
        assert_eq!(check(false, false, true), Check::Skip);
    }

    #[test]
    fn programs_and_installers_stay_closed_on_a_phone_too() {
        for name in ["setup.exe", "run.sh", "invoice.pdf.js", "Tool.app", "disk.iso", "launch.desktop"] {
            assert!(kept_closed(name, false) && kept_closed(name, true), "{name}");
        }
        // Android's installers, on a phone; a computer's list is as it was.
        for name in ["app.apk", "Bundle.APKS", "game.xapk", "split.apkm", "app.apk."] {
            assert!(kept_closed(name, true), "{name}");
            assert!(!kept_closed(name, false), "{name}");
        }
        for name in ["invoice.pdf", "photo.jpeg", "notes.txt", "apk-guide.pdf", "report.docx"] {
            assert!(!kept_closed(name, true) && !kept_closed(name, false), "{name}");
        }
    }

    #[test]
    fn the_type_comes_from_the_name_never_from_the_message() {
        assert_eq!(type_of("Facture.PDF"), "application/pdf");
        assert_eq!(type_of("photo.jpeg"), "image/jpeg");
        assert_eq!(type_of("scan.png"), "image/png");
        assert_eq!(type_of("no extension"), "application/octet-stream");
        // An installer is never named so, whatever its name: it is kept closed before.
        assert_eq!(type_of("app.apk"), "application/octet-stream");
    }

    #[test]
    fn the_words_say_what_happened_and_what_was_not_checked() {
        let name = "Facture mars.pdf";
        let line = |id: &str| say(id, &[("name", name.to_string())]);
        assert_eq!(opened_line(&serde_json::json!({ "opened": true }), name), line("attachment-phone-opened"));
        assert_eq!(opened_line(&serde_json::json!({ "problem": "no-app" }), name), line("attachment-phone-no-app"));
        let failed = say("attachment-phone-not-opened", &[("name", name.to_string()), ("detail", "SecurityException".to_string())]);
        assert_eq!(opened_line(&serde_json::json!({ "problem": "failed", "detail": "SecurityException" }), name), failed);
        // No answer from Java (a computer's demo): said, never claimed opened.
        assert!(opened_line(&serde_json::Value::Null, name).contains("no answer"));

        assert_eq!(saved_line(&serde_json::json!({ "name": name, "saved": true, "shown": "Facture mars (1).pdf" })), say("attachment-phone-saved", &[("name", "Facture mars (1).pdf".to_string())]));
        assert_eq!(saved_line(&serde_json::json!({ "name": name, "saved": false })), line("attachment-phone-not-saved"));
        let not_saved = say("attachment-phone-save-failed", &[("name", name.to_string()), ("detail", "IOException".to_string())]);
        assert_eq!(saved_line(&serde_json::json!({ "name": name, "saved": false, "problem": "IOException" })), not_saved);
        // Each says it was not checked, in English, and none claims a scan.
        let english = sioul_core::i18n::Translator::new("en");
        for id in ["attachment-phone-opened", "attachment-phone-saved", "ui-attachments-phone"] {
            let words = english.text(id, None);
            assert!(words.contains("not checked"), "{id}: {words}");
        }
    }

    #[test]
    fn android_may_lend_the_attachments_folder_alone() {
        // Where Rust writes attachments, under the cache Android gives (android/main.cpp: XDG_CACHE_HOME = getCacheDir()).
        let cache = config::cache_dir();
        let under = folder().strip_prefix(cache.parent().expect("the cache's parent")).expect("under the cache").to_string_lossy().replace('\\', "/");
        assert_eq!(under, "sioul/attachments");
        // The provider's paths name that folder, and no wider one of the cache or the device.
        let paths = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../android/package/res/xml/qtprovider_paths.xml")).expect("qtprovider_paths.xml");
        let caches: Vec<&str> = paths.lines().map(str::trim).filter(|l| l.starts_with("<cache-path")).collect();
        assert_eq!(caches, [format!("<cache-path name=\"attachments\" path=\"{under}/\"/>")]);
        assert!(!paths.contains("<root-path"));
    }
}
