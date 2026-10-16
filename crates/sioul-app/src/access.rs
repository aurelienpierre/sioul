// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What Android withholds from Sioul on this phone that it needs for what is
//! on here (docs/android.md, "What Android withholds"): the texts'
//! permissions while the part Texts is on; notification access, which holding
//! other apps' notifications and the phone's messages for your computers
//! need; the contacts while calls are screened or the phone's messages go to
//! your computers. Each said once in a line, with what Android wants:
//! "Allow in Android…", or, when Android keeps restricted settings from an
//! app installed from a file (Android 13 and later: notification access; 15
//! and later: the texts too), App info ▸ ⋮ ▸ Allow restricted settings first,
//! confirmed with the phone's PIN. Said in the join's result (JoinPanel.qml)
//! and on the Porch until you put it away (AccessCard.qml); never counted,
//! never red, never again once put away.

use crate::backend::tr;
use serde_json::{Value, json};
use sioul_core::i18n::Translator;

/// What Android says on this phone, as the lines read it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Said {
    /// The part Texts on here, and Android's three texts permissions all given.
    pub texts_on: bool,
    pub texts_given: bool,
    /// Notification access given.
    pub notifications_given: bool,
    /// The contacts needed here (calls screened, the phone's messages for
    /// your computers), and READ_CONTACTS given.
    pub contacts_needed: bool,
    pub contacts_given: bool,
    /// Android keeps restricted settings from Sioul (its app op says so).
    pub restricted: bool,
    /// The texts' last question answered at once, nothing given: none was shown.
    pub at_once: bool,
}

/// One line for each thing withheld: {what ("texts", "notifications",
/// "contacts"), said, restricted (App info first)}. Pure: tested below.
pub(crate) fn lines(s: &Said, tr: &Translator) -> Vec<Value> {
    let mut out = Vec::new();
    if s.texts_on && !s.texts_given {
        let (words, restricted) = if s.restricted {
            ("access-texts-restricted", true)
        } else if s.at_once {
            ("access-texts-at-once", true)
        } else {
            ("access-texts", false)
        };
        out.push(json!({ "what": "texts", "said": tr.text(words, None), "restricted": restricted }));
    }
    if !s.notifications_given {
        let words = if s.restricted { "access-notifications-restricted" } else { "access-notifications" };
        out.push(json!({ "what": "notifications", "said": tr.text(words, None), "restricted": s.restricted }));
    }
    if s.contacts_needed && !s.contacts_given {
        out.push(json!({ "what": "contacts", "said": tr.text("access-contacts", None), "restricted": false }));
    }
    out
}

/// What Android says now on this phone; nothing elsewhere.
fn said_here() -> Option<Said> {
    if !cfg!(target_os = "android") {
        return None;
    }
    let state = crate::steps::java("texts-state", "{}");
    Some(Said {
        texts_on: crate::share::shares(sioul_core::texts::PART),
        texts_given: state["read"] == true && state["receive"] == true && state["send"] == true,
        notifications_given: crate::appnotes::access_here() != Some(false),
        contacts_needed: crate::calls::screens_here() || crate::share::shares(sioul_core::phonemsgs::PART),
        contacts_given: crate::steps::java("contacts-allowed", "{}") != Value::Bool(false),
        restricted: state["restricted"] == true,
        at_once: state["at_once"] == true,
    })
}

/// The lines of this phone; none elsewhere.
pub(crate) fn withheld() -> Vec<Value> {
    said_here().map(|s| lines(&s, tr())).unwrap_or_default()
}

/// What the window asks (`texts::call`'s "access…" verbs): "access", the
/// lines; "access-ask" {what}: Android's question, or its page for
/// notification access; "access-info": App info. The lines again, as JSON
/// {lines}. The Porch puts a line away with a view flag of its own
/// (`porch-access-left-<what>`), in the window.
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
    json!({ "lines": withheld() })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What each state says: nothing withheld, nothing said; the texts' line
    /// only while the part is on; App info first when Android keeps
    /// restricted settings, or answered the question at once; the contacts
    /// only where they are needed.
    #[test]
    fn what_android_withholds_is_said_with_what_it_wants() {
        let tr = Translator::new("en");
        let all = Said { texts_on: true, texts_given: true, notifications_given: true, contacts_needed: true, contacts_given: true, restricted: false, at_once: false };
        assert!(lines(&all, &tr).is_empty(), "all given: nothing said");
        let whats = |s: &Said| lines(s, &tr).iter().map(|l| (l["what"].as_str().unwrap().to_string(), l["restricted"] == true)).collect::<Vec<_>>();
        // A new phone: Android 16, Sioul installed from a file.
        let restricted = Said { texts_given: false, notifications_given: false, contacts_given: false, restricted: true, ..all.clone() };
        assert_eq!(whats(&restricted), [("texts".to_string(), true), ("notifications".into(), true), ("contacts".into(), false)]);
        let said = lines(&restricted, &tr);
        assert!(said[0]["said"].as_str().unwrap().contains("Allow restricted settings"), "{said:?}");
        // Refused at once without restricted settings ("Don't allow" twice before): App info too.
        assert_eq!(whats(&Said { texts_given: false, at_once: true, ..all.clone() }), [("texts".to_string(), true)]);
        // Refused after the question: Allow in Android… again.
        assert_eq!(whats(&Said { texts_given: false, ..all.clone() }), [("texts".to_string(), false)]);
        // The part off: the texts not needed here.
        assert_eq!(whats(&Said { texts_on: false, texts_given: false, ..all.clone() }), Vec::<(String, bool)>::new());
        // The contacts only where calls are screened or the messages go to your computers.
        assert!(whats(&Said { contacts_needed: false, contacts_given: false, ..all.clone() }).is_empty());
        for language in ["en", "fr"] {
            let tr = Translator::new(language);
            for line in lines(&restricted, &tr) {
                assert!(!line["said"].as_str().unwrap().starts_with("access-"), "{language}: {line}");
            }
        }
    }
}
