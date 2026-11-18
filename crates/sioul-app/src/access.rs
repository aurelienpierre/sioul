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

// ---------------------------------------------------------------- restricted settings

/// Android's restricted settings for Sioul installed from a file (design of
/// 18 November 2026, section 4): what the card reads. No app can see
/// whether the restriction was lifted, only whether each access works.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Restricted {
    /// Android's version (its API level).
    pub api: i64,
    /// Where Sioul was installed from (`InstallSourceInfo`, Android 13 and
    /// later): "file", "store", "other", "unspecified"; "" not said.
    pub source: String,
    /// Notification access needed here, and given.
    pub notifications_needed: bool,
    pub notifications_given: bool,
    /// The part Texts on here, its three permissions given; their question
    /// asked once, and its last answer a refusal at once (Android's notice
    /// shown in its place, on Android 15 and later).
    pub texts_on: bool,
    pub texts_given: bool,
    pub texts_asked: bool,
    pub texts_at_once: bool,
    /// What this phone did of the card's steps (`Steps`).
    pub steps: Steps,
}

/// The card's steps pressed on this phone (Unix seconds; 0 never), its own
/// (`restricted.json` in the state folder): Android's page where the greyed
/// switch shows the notice opened (Android 13 and 14), Sioul's App info
/// opened, and the accesses asked again by Sioul on its return after.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Steps {
    #[serde(default)]
    pub notice: i64,
    #[serde(default)]
    pub info: i64,
    #[serde(default)]
    pub back: i64,
}

impl Steps {
    fn path() -> std::path::PathBuf {
        sioul_core::config::state_dir().join("restricted.json")
    }

    pub(crate) fn load() -> Steps {
        std::fs::read_to_string(Steps::path()).ok().and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default()
    }

    fn save(&self) {
        let path = Steps::path();
        let text = serde_json::to_string(self).unwrap_or_default();
        if let Err(e) = sioul_core::filelock::replace(&path, text.as_bytes()) {
            eprintln!("sioul: restricted settings: {}: {e}", path.display());
        }
    }
}

impl Restricted {
    /// The texts are among what Android holds back: Android 15 and later, the part on.
    fn texts_held(&self) -> bool {
        self.api >= GUARDED_TEXTS && self.texts_on && !self.texts_given
    }

    fn notifications_held(&self) -> bool {
        self.notifications_needed && !self.notifications_given
    }
}

/// The card, for Sioul installed from a file on Android 13 and later while
/// it needs an access Android keeps behind restricted settings
/// (notification access; from 15, the texts' permissions): {intro, steps:
/// [{step, said, button, done}], works, cannot}; none for any other install,
/// nothing needed, or an older Android. Each step ticked when its outcome is
/// seen: Android's notice shown (from 15, the texts' question refused at
/// once; before, its page opened), Sioul's page opened, every access working.
pub(crate) fn restricted(r: &Restricted, tr: &Translator) -> Option<Value> {
    let texts = r.api >= GUARDED_TEXTS && r.texts_on;
    if r.api < GUARDED || r.source != "file" || !(r.notifications_needed || texts) {
        return None;
    }
    let works = !r.notifications_held() && !r.texts_held();
    // From Android 15, a texts permission asked makes Android show its notice; before, the greyed switch on its page.
    let by_texts = r.api >= GUARDED_TEXTS;
    let notice = works || if by_texts { r.texts_asked && (r.texts_at_once || r.texts_given) } else { r.steps.notice > 0 };
    let intro = if texts && r.notifications_needed { "restricted-intro-both" } else if texts { "restricted-intro-texts" } else { "restricted-intro-notifications" };
    let step = |id: &str, said: &str, button: &str, done: bool| json!({ "step": id, "said": tr.text(said, None), "button": tr.text(button, None), "done": done });
    Some(json!({
        "intro": tr.text(intro, None),
        "steps": [
            step("notice", if by_texts { "restricted-notice-texts" } else { "restricted-notice-page" }, "restricted-notice-button", notice),
            step("info", "restricted-info", "restricted-info-button", works || r.steps.info > 0),
            step("back", if works { "restricted-back-done" } else { "restricted-back" }, "restricted-back-button", works),
        ],
        "works": works,
        "cannot": tr.text("restricted-cannot-see", None),
    }))
}

/// What Android and this phone say now, for the card; none elsewhere.
fn restricted_here() -> Option<Restricted> {
    let said = said_here()?;
    let source = crate::steps::java("install-source", "{}");
    let state = crate::steps::java("texts-state", "{}");
    Some(Restricted {
        api: said.api,
        source: source["source"].as_str().unwrap_or_default().to_string(),
        notifications_needed: said.notifications_needed,
        notifications_given: said.notifications_given,
        texts_on: said.texts_on,
        texts_given: said.texts_given,
        texts_asked: state["asked"] == true,
        texts_at_once: state["at_once"] == true,
        steps: Steps::load(),
    })
}

/// The card of this phone (`restricted`); null elsewhere or when none.
pub(crate) fn restricted_card() -> Value {
    restricted_here().and_then(|r| restricted(&r, tr())).unwrap_or(Value::Null)
}

/// A step of the card pressed: "notice" (Android 15 and later: the texts'
/// question; before: the page of notification access, where the greyed
/// switch shows Android's notice), "info" (Sioul's App info), "back" (the
/// accesses asked again now). The card again.
pub(crate) fn restricted_step(step: &str) -> Value {
    let Some(r) = restricted_here() else { return Value::Null };
    let mut steps = r.steps;
    let now = jiff::Timestamp::now().as_second();
    match step {
        "notice" if r.api >= GUARDED_TEXTS => {
            crate::steps::java("texts-ask", "{}");
        }
        "notice" => {
            crate::appnotes::change("open-access", "{}");
            steps.notice = now;
        }
        "info" => {
            crate::steps::java("texts-info", "{}");
            steps.info = now;
        }
        "back" => ask_again(&r, &mut steps, now),
        _ => {}
    }
    steps.save();
    restricted_card()
}

/// Back in Sioul after its page was opened (the window shown again): the
/// accesses still held asked again, once for each opening of the page.
pub(crate) fn restricted_back() {
    let Some(r) = restricted_here() else { return };
    if r.source != "file" || r.steps.info <= r.steps.back {
        return;
    }
    let mut steps = r.steps;
    ask_again(&r, &mut steps, jiff::Timestamp::now().as_second());
    steps.save();
}

fn ask_again(r: &Restricted, steps: &mut Steps, now: i64) {
    if r.texts_held() {
        crate::steps::java("texts-ask", "{}");
    } else if r.notifications_held() {
        crate::appnotes::change("open-access", "{}");
    }
    steps.back = now;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The card of restricted settings: only for an install from a file, on
    /// Android 13 and later, while an access it holds back is needed; each
    /// step ticked when its outcome is seen; once every access works, all
    /// three ticked. From 15, the notice comes from the texts' question;
    /// before, from the page of notification access.
    #[test]
    fn the_restricted_settings_card_follows_what_it_can_see() {
        let tr = Translator::new("en");
        let base = Restricted { api: 36, source: "file".into(), notifications_needed: true, texts_on: true, ..Restricted::default() };
        let done = |r: &Restricted| restricted(r, &tr).map(|card| card["steps"].as_array().unwrap().iter().map(|s| s["done"] == true).collect::<Vec<_>>());
        assert_eq!(done(&base), Some(vec![false, false, false]));
        for other in ["store", "other", "unspecified", ""] {
            assert_eq!(restricted(&Restricted { source: other.into(), ..base.clone() }, &tr), None, "no card for {other:?}");
        }
        assert_eq!(restricted(&Restricted { api: 32, ..base.clone() }, &tr), None, "Android 12: nothing restricted");
        assert_eq!(restricted(&Restricted { notifications_needed: false, texts_on: false, ..base.clone() }, &tr), None, "nothing needed");
        // Android 16: the texts' question asked, refused at once: Android's notice shown.
        let noticed = Restricted { texts_asked: true, texts_at_once: true, ..base.clone() };
        assert_eq!(done(&noticed), Some(vec![true, false, false]));
        let opened = Restricted { steps: Steps { info: 10, ..Steps::default() }, ..noticed.clone() };
        assert_eq!(done(&opened), Some(vec![true, true, false]));
        let card = restricted(&opened, &tr).unwrap();
        assert!(card["steps"][0]["said"].as_str().unwrap().contains("texts") && card["intro"].as_str().unwrap().contains("texts"));
        // Back in Sioul: what works is what the card follows.
        let works = Restricted { notifications_given: true, texts_given: true, ..opened.clone() };
        assert_eq!(done(&works), Some(vec![true, true, true]));
        assert_eq!(restricted(&works, &tr).unwrap()["works"], true);
        // Notification access alone works, the texts not yet: not done.
        assert_eq!(done(&Restricted { notifications_given: true, ..opened.clone() }), Some(vec![true, true, false]));
        // Android 13 and 14: notification access alone, its notice from the page opened.
        let older = Restricted { api: 34, texts_on: true, ..base.clone() };
        let card = restricted(&older, &tr).unwrap();
        assert!(!card["intro"].as_str().unwrap().contains("texts"), "{card}");
        assert_eq!(done(&Restricted { steps: Steps { notice: 5, ..Steps::default() }, ..older.clone() }), Some(vec![true, false, false]));
        assert_eq!(restricted(&Restricted { notifications_needed: false, ..older }, &tr), None, "on 14 the texts are not restricted");
        for language in ["en", "fr"] {
            let tr = Translator::new(language);
            for r in [base.clone(), works.clone(), Restricted { api: 33, ..base.clone() }] {
                let card = restricted(&r, &tr).unwrap();
                let words = [card["intro"].clone(), card["cannot"].clone()].into_iter().chain(card["steps"].as_array().unwrap().iter().flat_map(|s| [s["said"].clone(), s["button"].clone()]));
                for said in words {
                    assert!(!said.as_str().unwrap().starts_with("restricted-"), "{language}: {said}");
                }
            }
        }
    }

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
