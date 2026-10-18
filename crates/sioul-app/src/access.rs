// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What Android withholds from Sioul on this phone that it needs for what is
//! on here (docs/android.md, "What Android withholds"): the texts'
//! permissions while the part Texts is on; notification access, while other
//! apps' notifications are held or the phone's messages go to your
//! computers; the contacts while calls are screened or the phone's messages
//! go to your computers. Each said in a line, with what Android wants.
//! Whether Android keeps restricted settings from Sioul installed from a
//! file (Android 13 and later: notification access; 15 and later: the texts
//! too) no app can read, so the lines say the steps that hold either way,
//! in their order: Android's question or page once (which, refused for
//! restricted settings, makes App info's ⋮ offer Allow restricted
//! settings), then Sioul's app info. Said in the join's result
//! (JoinPanel.qml), with the part Texts' switch where it is off, and on the
//! Porch until you put it away (AccessCard.qml); never counted, never red,
//! never again once put away.

use crate::backend::tr;
use serde_json::{Value, json};
use sioul_core::i18n::Translator;

/// Android's version from which an app installed from a file is kept from restricted settings (13): notification access.
const GUARDED: i64 = 33;
/// The same for the texts' permissions, kept so from Android 15 only: on 13 and 14 a refusal is a plain one.
const GUARDED_TEXTS: i64 = 35;

/// What Android says on this phone, as the lines read it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Said {
    /// Android's version (its API level).
    pub api: i64,
    /// The part Texts on here; the phone able to read texts (it has an SMS app); Android's three texts permissions all given.
    pub texts_on: bool,
    pub texts_could: bool,
    pub texts_given: bool,
    /// The texts' question asked here once at least, and its last answer at
    /// once with none of the three given (no question shown).
    pub asked: bool,
    pub at_once: bool,
    /// Notification access needed here (other apps' notifications held, or the phone's messages for your computers), and given.
    pub notifications_needed: bool,
    pub notifications_given: bool,
    /// The contacts needed here (calls screened, the phone's messages for your computers), and READ_CONTACTS given.
    pub contacts_needed: bool,
    pub contacts_given: bool,
}

/// One line for each thing withheld: {what ("texts", "notifications",
/// "contacts", and with `join` "texts-part"), said, info (Sioul's app info
/// offered beside), switch (the part Texts')}. Pure: tested below.
pub(crate) fn lines(s: &Said, tr: &Translator, join: bool) -> Vec<Value> {
    let mut out = Vec::new();
    if join && !s.texts_on && s.texts_could {
        out.push(json!({ "what": "texts-part", "said": tr.text("access-texts-off", None), "info": false, "switch": true }));
    }
    if s.texts_on && !s.texts_given {
        let (words, info) = if s.asked && s.api >= GUARDED_TEXTS {
            ("access-texts-guarded", true)
        } else if s.at_once {
            ("access-texts-at-once", true)
        } else {
            ("access-texts", false)
        };
        out.push(json!({ "what": "texts", "said": tr.text(words, None), "info": info, "switch": false }));
    }
    if s.notifications_needed && !s.notifications_given {
        let guarded = s.api >= GUARDED;
        out.push(json!({ "what": "notifications", "said": tr.text(if guarded { "access-notifications-guarded" } else { "access-notifications" }, None), "info": guarded, "switch": false }));
    }
    if s.contacts_needed && !s.contacts_given {
        out.push(json!({ "what": "contacts", "said": tr.text("access-contacts", None), "info": false, "switch": false }));
    }
    out
}

/// What Android says now on this phone; nothing elsewhere.
fn said_here() -> Option<Said> {
    if !cfg!(target_os = "android") {
        return None;
    }
    let state = crate::steps::java("texts-state", "{}");
    let phone_messages = crate::share::shares(sioul_core::phonemsgs::PART);
    Some(Said {
        api: state["api"].as_i64().unwrap_or(0),
        texts_on: crate::share::shares(sioul_core::texts::PART),
        texts_could: state["sms_app"].as_str().is_some_and(|app| !app.is_empty()),
        texts_given: state["read"] == true && state["receive"] == true && state["send"] == true,
        asked: state["asked"] == true,
        at_once: state["at_once"] == true,
        notifications_needed: sioul_core::appnotes::Choices::load(&sioul_core::appnotes::Choices::default_path()).hold || phone_messages,
        notifications_given: crate::appnotes::access_here() != Some(false),
        contacts_needed: crate::calls::screens_here() || phone_messages,
        contacts_given: crate::steps::java("contacts-allowed", "{}") != Value::Bool(false),
    })
}

/// The lines of this phone (with `join`, the part Texts' too); none elsewhere.
pub(crate) fn withheld(join: bool) -> Vec<Value> {
    said_here().map(|s| lines(&s, tr(), join)).unwrap_or_default()
}

/// What the window asks (`texts::call`'s "access…" verbs): "access" {join},
/// the lines; "access-ask" {what, join}: Android's question, or its page
/// for notification access; "access-info" {join}: Sioul's app info. The
/// lines again, as JSON {lines}. The Porch puts a line away with a view
/// flag of its own (`porch-access-left-<what>`), in the window; the part
/// Texts' switch is `texts::call`'s "part".
pub(crate) fn call(verb: &str, asked: &Value) -> Value {
    match verb {
        "access-ask" => match asked["what"].as_str().unwrap_or_default() {
            "texts" => {
                crate::steps::java("texts-ask", "{}");
            }
            "notifications" => {
                crate::appnotes::change("open-access", "{}");
            }
            "contacts" => crate::steps::ask_contacts(),
            _ => {}
        },
        "access-info" => {
            crate::steps::java("texts-info", "{}");
        }
        _ => {}
    }
    json!({ "lines": withheld(asked["join"] == true) })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What each state says, in the steps' order: nothing withheld, nothing
    /// said; the texts' line only while the part is on, the part's own line
    /// only in the join's result; once asked on Android 15 and later, the
    /// steps that hold whether restricted settings keep them or not, with
    /// Sioul's app info (notification access from Android 13); on Android 12
    /// to 14 a question refused for good, app info too; notification access
    /// and the contacts only where they are needed.
    #[test]
    fn what_android_withholds_is_said_with_what_it_wants() {
        let tr = Translator::new("en");
        let all = Said { api: 36, texts_on: true, texts_could: true, texts_given: true, asked: true, at_once: false, notifications_needed: true, notifications_given: true, contacts_needed: true, contacts_given: true };
        assert!(lines(&all, &tr, true).is_empty(), "all given: nothing said");
        let whats = |s: &Said, join: bool| lines(s, &tr, join).iter().map(|l| (l["what"].as_str().unwrap().to_string(), l["info"] == true)).collect::<Vec<_>>();
        // A new phone: Android 16, Sioul installed from a file, the texts asked and refused.
        let refused = Said { texts_given: false, notifications_given: false, contacts_given: false, ..all.clone() };
        assert_eq!(whats(&refused, false), [("texts".to_string(), true), ("notifications".into(), true), ("contacts".into(), false)]);
        let said = lines(&refused, &tr, false);
        let texts = said[0]["said"].as_str().unwrap();
        let (once, info, again) = (texts.find("press Allow in Android… once").unwrap(), texts.find("Allow restricted settings").unwrap(), texts.find("Allow in Android… again").unwrap());
        assert!(once < info && info < again, "in their order: {texts}");
        assert!(said[1]["said"].as_str().unwrap().contains("tap Sioul once"), "{said:?}");
        // Not asked yet: Allow in Android… alone.
        assert_eq!(whats(&Said { texts_given: false, asked: false, ..all.clone() }, false), [("texts".to_string(), false)]);
        // Android 12: no restricted settings; refused for good (at once), app info; refused after the question, asked again.
        assert_eq!(whats(&Said { api: 31, texts_given: false, at_once: true, ..all.clone() }, false), [("texts".to_string(), true)]);
        assert_eq!(whats(&Said { api: 31, texts_given: false, ..all.clone() }, false), [("texts".to_string(), false)]);
        assert_eq!(whats(&Said { api: 31, notifications_given: false, ..all.clone() }, false), [("notifications".to_string(), false)]);
        // Android 13 and 14: notification access guarded, the texts' permissions not (from 15 only): a plain refusal.
        assert_eq!(whats(&Said { api: 34, texts_given: false, notifications_given: false, ..all.clone() }, false), [("texts".to_string(), false), ("notifications".into(), true)]);
        assert_eq!(whats(&Said { api: 33, texts_given: false, at_once: true, ..all.clone() }, false), [("texts".to_string(), true)]);
        assert!(!lines(&Said { api: 34, texts_given: false, ..all.clone() }, &tr, false)[0]["said"].as_str().unwrap().contains("restricted settings"), "on 14, never the restricted settings' steps for the texts");
        // The part off: no permission line; in the join's result, the part's own line with its switch.
        let off = Said { texts_on: false, texts_given: false, ..all.clone() };
        assert!(whats(&off, false).is_empty());
        assert_eq!(lines(&off, &tr, true)[0]["switch"], true);
        assert!(whats(&Said { texts_could: false, ..off.clone() }, true).is_empty(), "a phone with no texts to read: nothing");
        // Only where they are needed.
        assert!(whats(&Said { notifications_needed: false, notifications_given: false, contacts_needed: false, contacts_given: false, ..all.clone() }, false).is_empty());
        for language in ["en", "fr"] {
            let tr = Translator::new(language);
            for line in lines(&refused, &tr, false).into_iter().chain(lines(&off, &tr, true)).chain(lines(&Said { api: 31, texts_given: false, at_once: true, ..all.clone() }, &tr, false)) {
                assert!(!line["said"].as_str().unwrap().starts_with("access-"), "{language}: {line}");
            }
        }
    }
}
