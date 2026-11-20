// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What the window does with a file from outside once its antivirus has
//! answered, on every system, and mail attachments on a phone
//! (docs/client.md, "Antivirus").
//!
//! One table (`decide`) says, for what was found (a verdict, your yes to go
//! on unchecked, a phone), the system (`System`: Windows, ClamAV's Linux and
//! macOS, a phone), what that system said before (`Presence`) and what you
//! asked (open, save, keep as a paper, place a site's file), whether the
//! window goes on, asks once, or refuses, and with which words. Mail
//! attachments, papers kept from mail, a site's files and the letters they
//! become all read it (`backend::checked_attachment`, `downloads::done`), so
//! that they cannot differ. A file nobody read is never said clean; one
//! refused never opens and its copy goes; none is refused for want of an
//! antivirus: the question says it was not checked and may be dangerous, and
//! opens or saves it on your yes. Programs never open from Sioul: that is
//! decided before, whatever the answer (`kept_closed`). The words that say
//! how attachments are checked name the system's antivirus (`label`).
//!
//! A phone has no antivirus Sioul can call, so none is asked and none is
//! claimed: the words say the file was not checked. The attachment opens in
//! the app you choose, which Android lets read that one file and nothing
//! else of Sioul's, and "Save…" asks Android where (android/package/src/com/
//! aurelienpierre/sioul/Attachments.java, AttachmentSave.java). Sioul's own
//! refusals stand as on a computer: mail set aside keeps its attachments
//! closed, and programs, scripts and installers, Android's among them, are
//! never opened.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use sioul_core::config;
use sioul_sync::antivirus::{Presence, Verdict};

use crate::backend::{QtThread, say, set_status, tr};

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

/// The system whose antivirus is asked and named, read at run time from
/// where Sioul runs (`here`), so that a test on any system reads each.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum System {
    /// Windows: its own antivirus, Microsoft Defender or the one installed.
    Windows,
    /// Linux and macOS: ClamAV, when the system has it.
    ClamAv,
    /// A phone: no antivirus Sioul can call.
    Phone,
}

impl System {
    /// Where Sioul runs.
    pub(crate) fn here() -> System {
        if phone() {
            System::Phone
        } else if cfg!(windows) {
            System::Windows
        } else {
            System::ClamAv
        }
    }
}

/// What you asked of a file from outside.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Act {
    /// Open an attachment.
    Open,
    /// Save an attachment with your files.
    Save,
    /// Keep an attachment in the papers wallet.
    Paper,
    /// Place a file a site offers where you said (Downloads, a folder, Notes, Papers, Letters).
    Place,
}

/// What was learnt of a file before it goes anywhere.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Found {
    /// The antivirus's verdict; "clean" too for the same bytes found clean
    /// earlier in this session.
    Scanned(Verdict),
    /// You chose to go on, told that it was not checked.
    Chosen,
    /// A phone: no antivirus asked.
    Phone,
}

/// What the window does with the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Does {
    /// It opens, is saved, kept or placed.
    Go,
    /// One question first: it was not checked and may be dangerous. Yes, it
    /// goes unchecked (`Found::Chosen`); no, nothing opens.
    Ask,
    /// Never opened; its copy is deleted.
    Refuse,
}

/// What the window does, and with which words (message ids; `args` fills them).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Decision {
    pub(crate) does: Does,
    /// The status line: an opening's (Go, "" when the act says its own: a
    /// save, a paper, a phone's), the refusal (Refuse), or while it asks (Ask).
    pub(crate) line: &'static str,
    /// Ask: the question.
    pub(crate) question: &'static str,
    /// Ask: the button that goes on unchecked.
    pub(crate) action: &'static str,
    /// Said after the line of a file saved, kept or placed without a check
    /// ("" when it was checked, or a phone's own words say it).
    pub(crate) note: &'static str,
    /// What the words name: the file, the antivirus, the threat or what went
    /// wrong, how to have files checked.
    pub(crate) args: Vec<(&'static str, String)>,
}

impl Decision {
    /// One of its messages, said with its names.
    pub(crate) fn said(&self, id: &str) -> String {
        if id.is_empty() { String::new() } else { say(id, &self.args) }
    }
}

/// The antivirus the words name: the one the system names, else Windows'
/// own or ClamAV, in your language.
pub(crate) fn antivirus_name(system: System, presence: &Presence) -> String {
    match (system, presence) {
        (_, Presence::Here(Some(name))) => name.clone(),
        (System::Windows, _) => tr().text("antivirus-windows", None),
        _ => "ClamAV".to_string(),
    }
}

/// What the window does with a file from outside, and with which words: the
/// one table every way in reads (the module's header). `hint` says how to
/// have files checked here (`antivirus::install_hint`).
pub(crate) fn decide(found: &Found, system: System, presence: &Presence, act: Act, name: &str, hint: &str) -> Decision {
    // A name from a message never shows as a command to copy (the question shows backticks as commands).
    let shown = name.replace('`', "\u{02cb}");
    let mut args = vec![("name", shown), ("antivirus", antivirus_name(system, presence)), ("hint", hint.to_string())];
    let action = if act == Act::Open { "scan-open-anyway" } else { "scan-save-anyway" };
    let go = |line: &'static str, note: &'static str, args| Decision { does: Does::Go, line, question: "", action: "", note, args };
    let ask = |question: &'static str, line: &'static str, args| Decision { does: Does::Ask, line, question, action, note: "", args };
    let opened = |checked: &'static str, unchecked: &'static str| if act == Act::Open { (checked, "") } else { ("", unchecked) };
    match found {
        // A refusal stands everywhere: never opened, the copy deleted.
        Found::Scanned(Verdict::Infected(what)) => {
            args.push(("threat", what.clone()));
            args.push(("detail", what.clone()));
            let line = if system == System::Windows || what.is_empty() { "scan-refused" } else { "scan-infected" };
            Decision { does: Does::Refuse, line, question: "", action: "", note: "", args }
        }
        Found::Scanned(Verdict::Clean) => {
            let (line, _) = opened("scan-clean", "");
            go(line, "", args)
        }
        // Your yes: it goes, said unchecked.
        Found::Chosen => {
            let (line, note) = opened("scan-opens-unchecked", "scan-not-checked");
            go(line, note, args)
        }
        // A phone never asks: its own words say it was not checked; a site's file says it too.
        Found::Phone => go("", if act == Act::Place { "download-unchecked-none" } else { "" }, args),
        Found::Scanned(Verdict::TooBig) if act == Act::Place || system == System::Phone => go("", "download-unchecked-too-big", args),
        Found::Scanned(Verdict::Unavailable(_)) if act == Act::Place || system == System::Phone => go("", "download-unchecked-none", args),
        Found::Scanned(Verdict::TooBig) => ask("scan-ask-too-big", "scan-too-big", args),
        Found::Scanned(Verdict::Unavailable(detail)) => {
            args.push(("detail", detail.clone()));
            let question = match (system, presence) {
                (System::Windows, Presence::Absent) => "scan-ask-windows-none",
                (_, Presence::Absent) => "scan-ask",
                _ => "scan-ask-unanswered",
            };
            ask(question, "scan-unavailable-short", args)
        }
    }
}

/// The words above a message's attachments that say how they are checked
/// here: the system's antivirus by its name when the system gives one; that
/// none is there when the system says so; that Windows does not say, when
/// it does not; a phone's own.
pub(crate) fn label(system: System, presence: &Presence) -> String {
    let named = |id: &str| say(id, &[("antivirus", antivirus_name(system, presence))]);
    match (system, presence) {
        (System::Phone, _) => tr().text("ui-attachments-phone", None),
        (System::Windows, Presence::Here(Some(_))) => named("ui-attachments-scan-windows"),
        (System::Windows, Presence::Here(None)) => tr().text("ui-attachments-scan-windows-unnamed", None),
        (System::Windows, Presence::Unknown) => tr().text("ui-attachments-scan-windows-unknown", None),
        (System::Windows, Presence::Absent) => tr().text("ui-attachments-scan-windows-none", None),
        (System::ClamAv, Presence::Absent) => tr().text("ui-attachments-scan-none", None),
        (System::ClamAv, _) => tr().text("ui-attachments-scan", None),
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
        let _ = sioul_core::filelock::remove_wiping(file);
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
    let _ = sioul_core::filelock::remove_wiping(file);
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
        let _ = sioul_core::filelock::remove_wiping(path);
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
// SAFETY: no other symbol of the program has this name.
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

    /// Every answer on every system, for every act: what the window does.
    /// A refusal never goes; a file nobody read never goes on a computer
    /// without the question, nor is said clean; none is refused for want of
    /// an antivirus; a phone never asks; a site's file is placed and said
    /// unchecked, as the card's Save asked.
    #[test]
    fn one_table_for_every_way_in() {
        let verdicts = [Verdict::Clean, Verdict::Infected("Eicar-Test-Signature".into()), Verdict::Infected("AMSI 32768".into()), Verdict::TooBig, Verdict::Unavailable("no antivirus registered with AMSI".into())];
        let presences = [Presence::Here(Some("Microsoft Defender".into())), Presence::Here(None), Presence::Absent, Presence::Unknown];
        for system in [System::Windows, System::ClamAv, System::Phone] {
            for presence in &presences {
                for act in [Act::Open, Act::Save, Act::Paper, Act::Place] {
                    for verdict in &verdicts {
                        let decision = decide(&Found::Scanned(verdict.clone()), system, presence, act, "facture.pdf", "");
                        let expected = match verdict {
                            Verdict::Infected(_) => Does::Refuse,
                            Verdict::Clean => Does::Go,
                            Verdict::TooBig | Verdict::Unavailable(_) if act == Act::Place || system == System::Phone => Does::Go,
                            Verdict::TooBig | Verdict::Unavailable(_) => Does::Ask,
                        };
                        assert_eq!(decision.does, expected, "{system:?} {presence:?} {act:?} {verdict:?}");
                        // Not read: never said clean; said unchecked wherever it goes.
                        if !matches!(verdict, Verdict::Clean) {
                            assert_ne!(decision.line, "scan-clean", "{system:?} {act:?} {verdict:?}");
                        }
                        if decision.does == Does::Go && !matches!(verdict, Verdict::Clean) {
                            assert!(!decision.note.is_empty(), "{system:?} {act:?} {verdict:?}: placed without saying it was not checked");
                        }
                        if decision.does == Does::Ask {
                            assert!(!decision.question.is_empty() && !decision.action.is_empty());
                        }
                    }
                    // Your yes: it goes, said unchecked, never clean.
                    let chosen = decide(&Found::Chosen, system, presence, act, "facture.pdf", "");
                    assert_eq!(chosen.does, Does::Go);
                    assert_ne!(chosen.line, "scan-clean");
                    assert!(chosen.line == "scan-opens-unchecked" || chosen.note == "scan-not-checked", "{act:?}: {chosen:?}");
                    // A phone's attachment: goes, its own words say it was not checked.
                    assert_eq!(decide(&Found::Phone, system, presence, act, "facture.pdf", "").does, Does::Go);
                }
            }
        }
    }

    /// The question names what was missing: none found (with how to have
    /// one), the antivirus that gave no answer, or Windows saying none is on;
    /// Windows' words never name ClamAV, Linux's never Windows.
    #[test]
    fn the_question_names_the_systems_antivirus() {
        let unavailable = Found::Scanned(Verdict::Unavailable("no AMSI on this Windows".into()));
        let english = sioul_core::i18n::Translator::new("en");
        let words = |system, presence: &Presence| {
            let decision = decide(&unavailable, system, presence, Act::Open, "facture`rm`.pdf", "Windows Security ▸ Virus & threat protection");
            let args = decision.args.iter().fold(sioul_core::i18n::args(), |mut args, (k, v)| {
                args.set(k.to_string(), v.clone());
                args
            });
            (decision.question, english.text(decision.question, Some(&args)))
        };
        let defender = Presence::Here(Some("Microsoft Defender".into()));
        let (id, said) = words(System::Windows, &defender);
        assert_eq!(id, "scan-ask-unanswered");
        assert!(said.contains("Microsoft Defender") && said.contains("not checked") && said.contains("dangerous") && !said.contains("ClamAV"), "{said}");
        // A name from a message never reads as a command.
        assert!(!said.contains('`'), "{said}");
        // Unnamed, Windows' own, in the session's language.
        for presence in [Presence::Here(None), Presence::Unknown] {
            let (id, said) = words(System::Windows, &presence);
            assert_eq!(id, "scan-ask-unanswered");
            assert!(said.contains(&tr().text("antivirus-windows", None)) && !said.contains("ClamAV"), "{said}");
        }
        let (id, said) = words(System::Windows, &Presence::Absent);
        assert_eq!(id, "scan-ask-windows-none");
        assert!(said.contains("Windows Security") && said.contains("not checked") && said.contains("dangerous"), "{said}");
        let (id, said) = words(System::ClamAv, &Presence::Absent);
        assert_eq!(id, "scan-ask");
        assert!(said.contains("No antivirus was found") && said.contains("dangerous"), "{said}");
        let (id, said) = words(System::ClamAv, &Presence::Here(Some("ClamAV".into())));
        assert_eq!(id, "scan-ask-unanswered");
        assert!(said.contains("ClamAV gave no answer") && !said.contains("Windows"), "{said}");
        // A refusal on Windows names who refused and how; ClamAV's names the threat.
        let refused = decide(&Found::Scanned(Verdict::Infected("AMSI 32768".into())), System::Windows, &defender, Act::Open, "facture.pdf", "");
        assert_eq!(refused.line, "scan-refused");
        let said = refused.said(refused.line);
        assert!(said.contains("Microsoft Defender") && said.contains("AMSI 32768"), "{said}");
        let infected = decide(&Found::Scanned(Verdict::Infected("Eicar-Test-Signature".into())), System::ClamAv, &Presence::Here(Some("ClamAV".into())), Act::Save, "facture.pdf", "");
        assert_eq!(infected.line, "scan-infected");
        assert!(infected.said(infected.line).contains("Eicar-Test-Signature"));
    }

    /// The words above the attachments name the system's antivirus, chosen
    /// at run time from what the system is; a phone's are as they were.
    #[test]
    fn the_label_names_the_systems_antivirus() {
        let defender = label(System::Windows, &Presence::Here(Some("Microsoft Defender".into())));
        assert!(defender.contains("Microsoft Defender") && !defender.contains("ClamAV"), "{defender}");
        for presence in [Presence::Here(None), Presence::Unknown, Presence::Absent] {
            let said = label(System::Windows, &presence);
            assert!(said.contains("Windows") && !said.contains("ClamAV"), "{said}");
        }
        assert_eq!(label(System::Windows, &Presence::Unknown), tr().text("ui-attachments-scan-windows-unknown", None));
        assert_eq!(label(System::Windows, &Presence::Absent), tr().text("ui-attachments-scan-windows-none", None));
        assert_eq!(label(System::Windows, &Presence::Here(None)), tr().text("ui-attachments-scan-windows-unnamed", None));
        let clamav = label(System::ClamAv, &Presence::Here(Some("ClamAV".into())));
        assert!(clamav.contains("ClamAV") && !clamav.contains("Windows"), "{clamav}");
        assert_eq!(label(System::ClamAv, &Presence::Absent), tr().text("ui-attachments-scan-none", None));
        for presence in [Presence::Here(None), Presence::Absent] {
            assert_eq!(label(System::Phone, &presence), tr().text("ui-attachments-phone", None));
        }
    }

    /// Every message the table and the words above the attachments can give
    /// is there in each language, with the names it fills.
    #[test]
    fn every_word_of_the_table_is_written() {
        let mut ids: Vec<&str> = vec![
            "ui-attachments-scan", "ui-attachments-scan-none", "ui-attachments-scan-windows", "ui-attachments-scan-windows-unnamed",
            "ui-attachments-scan-windows-unknown", "ui-attachments-scan-windows-none", "ui-attachments-phone", "antivirus-windows",
        ];
        let verdicts = [Verdict::Clean, Verdict::Infected("x".into()), Verdict::Infected(String::new()), Verdict::TooBig, Verdict::Unavailable("x".into())];
        for system in [System::Windows, System::ClamAv, System::Phone] {
            for presence in [Presence::Here(Some("A".into())), Presence::Here(None), Presence::Absent, Presence::Unknown] {
                for act in [Act::Open, Act::Save, Act::Paper, Act::Place] {
                    let founds = verdicts.iter().cloned().map(Found::Scanned).chain([Found::Chosen, Found::Phone]);
                    for found in founds {
                        let decision = decide(&found, system, &presence, act, "a.pdf", "");
                        ids.extend([decision.line, decision.question, decision.action, decision.note].into_iter().filter(|id| !id.is_empty()));
                    }
                }
            }
        }
        ids.sort_unstable();
        ids.dedup();
        for language in ["en", "fr"] {
            let words = sioul_core::i18n::Translator::new(language);
            for id in &ids {
                let said = words.text(id, None);
                assert!(!said.is_empty() && said != *id, "{language}: {id}");
            }
        }
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
