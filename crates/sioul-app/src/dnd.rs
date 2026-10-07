// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Do-not-disturb during the two pauses, "Paused" and "Free time"
//! (docs/pauses.md, "Do-not-disturb"; the research behind it:
//! docs/research/emergency-pause.md, 7 and P7–P10), and for Sioul's own
//! do-not-disturb on every device (`Which::Global`: its switch, the focus
//! timer, sleep). Sioul holds its own notifications itself; here, the
//! system's do-not-disturb, with exceptions set beforehand, where the system
//! lets an application set it, and a sentence saying what was silenced and
//! what could not be.
//!
//! - **Android**: a mode of Sioul's own for each ("Pause", "Free time",
//!   "Do not disturb (Sioul)"),
//!   letting through starred contacts and repeat callers (or nobody), alarms,
//!   and Sioul's dose reminders (android/package/src/com/aurelienpierre/sioul/
//!   PauseMode.java). Needs Android's "Do Not Disturb access".
//! - **Plasma**: its notification server's inhibition, held while the pause
//!   lasts (sioul-sync's dnd.rs); it ends with Sioul's process.
//! - **GNOME**: only the person's own switch exists; Sioul turns it on and off
//!   again only with their yes, given in the pause's settings.
//! - **macOS**: the person's own shortcuts. **Windows**: nothing an
//!   application may do.
//!
//! Never turns off a do-not-disturb the person set: only what Sioul turned on
//! is turned off. Asked again unchanged, nothing is redone (the pauses ask
//! each minute); what was turned on is kept in the state folder (dnd.toml), so
//! that a crash is healed at the next start.

use crate::backend::tr;
use serde::{Deserialize, Serialize};
use sioul_core::i18n::{self, Translator};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Which pause.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Which {
    /// "Free time": a chosen pause, for the good weather or the energy there is now.
    FreeTime,
    /// "Paused": the pause for overwhelming moments.
    Paused,
    /// Sioul's do-not-disturb, on every device: its switch, the focus timer, sleep.
    Global,
}

impl Which {
    /// Its key, as Java knows it (Android only).
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    pub(crate) fn key(self) -> &'static str {
        match self {
            Which::FreeTime => "free-time",
            Which::Paused => "pause",
            Which::Global => "dnd",
        }
    }

    /// The system's name for its mode: "Pause", « En pause »; "Free time",
    /// « Temps libre »; "Do not disturb (Sioul)", « Ne pas déranger (Sioul) ».
    fn name(self, tr: &Translator) -> String {
        match self {
            Which::FreeTime => tr.text("dnd-name-free-time", None),
            Which::Paused => tr.text("dnd-name-pause", None),
            Which::Global => tr.text("dnd-name-global", None),
        }
    }
}

/// What a pause asks of the system, as set beforehand in its settings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Ask {
    pub which: Which,
    /// Android: calls and messages from starred contacts, and a second call
    /// from the same number within 15 minutes, get through; false: nobody.
    pub people: bool,
    /// Sioul's dose reminders get through. Alarms always do (and the sound of
    /// what the person plays).
    pub doses: bool,
    /// GNOME: the person's yes, given in the pause's settings, for Sioul to
    /// switch on GNOME's own Do Not Disturb, and off again after. Ignored elsewhere.
    pub desktop: bool,
}

/// What was done, in words for the screen.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Report {
    /// `enter`: the system's do-not-disturb is on for this pause, on this
    /// device. `can`: this device can do it now.
    pub on: bool,
    /// One or a few literal sentences, translated: what was silenced and what
    /// could not be, shown as they are.
    pub line: String,
    /// What the pause's settings can offer here: "access" (Android's page
    /// for Sioul's do-not-disturb access), "starred" (the Contacts app's
    /// starred list), "plasma" (Plasma's notification settings at Sioul's
    /// page), each a button for `open`; "desktop", GNOME's consent, a switch
    /// labelled `consent`.
    pub offers: Vec<&'static str>,
    /// The label of GNOME's consent switch; "" elsewhere.
    pub consent: String,
}

impl Report {
    /// For the window: {on, line, consent, offers: [{key, label}]}.
    pub(crate) fn json(&self) -> serde_json::Value {
        let offers: Vec<serde_json::Value> = self.offers.iter().map(|key| serde_json::json!({ "key": key, "label": offer_label(key) })).collect();
        serde_json::json!({ "on": self.on, "line": self.line, "consent": self.consent, "offers": offers })
    }
}

/// What the pause's settings can show here, with no change made.
pub(crate) fn can() -> Report {
    with(|layer| layer.can(tr()))
}

/// The system's do-not-disturb on for `ask`'s pause, or brought up to date
/// when its settings changed. Asked again unchanged, nothing is redone.
pub(crate) fn enter(ask: &Ask) -> Report {
    with(|layer| layer.enter(ask, tr()))
}

/// The do-not-disturb Sioul turned on for this pause, off again; nothing
/// else. Asked again, or for a pause not on, nothing is done (but the first
/// time in a run, which heals what a crash left).
pub(crate) fn leave(which: Which) -> Report {
    with(|layer| layer.leave(which, tr()))
}

/// Android: "Pause" pressed on the quick-settings tile or the home screen's
/// shortcut since last asked (PauseOpener.java). False elsewhere.
pub(crate) fn take_pause_pressed() -> bool {
    #[cfg(target_os = "android")]
    return phone::call("pressed", "").as_bool().unwrap_or(false);
    #[cfg(not(target_os = "android"))]
    false
}

/// A label for one of `Report::offers`.
pub(crate) fn offer_label(key: &str) -> String {
    match key {
        "access" => tr().text("dnd-offer-access", None),
        "starred" => tr().text("dnd-offer-starred", None),
        "plasma" => tr().text("dnd-offer-plasma", None),
        "desktop" => tr().text("dnd-gnome-consent", None),
        _ => String::new(),
    }
}

/// One of `Report::offers`, opened: "access", "starred", "plasma".
pub(crate) fn open(key: &str) {
    match key {
        "access" => open_access(),
        "starred" => open_starred(),
        "plasma" => open_plasma(),
        _ => {}
    }
}

/// Android's page where Sioul is given "Do Not Disturb access" (Modes
/// access from Android 15). Nothing elsewhere.
pub(crate) fn open_access() {
    #[cfg(target_os = "android")]
    phone::call("open", "access");
}

/// The Contacts app's starred list, which the person edits there: Sioul never
/// writes stars. Nothing elsewhere.
pub(crate) fn open_starred() {
    #[cfg(target_os = "android")]
    phone::call("open", "starred");
}

/// Plasma's notification settings, at Sioul's page ("Show in do not disturb
/// mode"). Nothing elsewhere.
pub(crate) fn open_plasma() {
    #[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
    desktop::open_plasma_settings();
}

// ---------------------------------------------------------------- what is said

/// A fact said on the pause's line or in its settings; `sentence` words it.
// Each system says its own facts: the others' are made only on their
// systems, and in the tests.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
enum Said {
    PhoneCan,
    PhoneNeedsAccess,
    PhoneNoAccess,
    PhoneTooOld,
    /// Emergency services' call back: their number in the person's country, where one is known.
    PhoneCallback(Option<String>),
    /// Silenced, with these exceptions as the mode is set (read back from Android).
    PhoneOn(Senders),
    /// Silenced as the mode of this name is set in Android's settings, otherwise than Sioul set it.
    PhoneAsSet(String),
    /// The mode is set otherwise in Android's settings than the pause asks.
    PhoneSetThere,
    PhoneThrough { alarms: Option<bool>, doses: Doses },
    PhoneAlready,
    PhoneFailed(String),
    PhoneNoAnswer,
    /// The mode of this name turned off in Android's settings (by the person): left off.
    PhoneDisabled(String),
    /// Do-not-disturb turned off on the phone during the pause (by the person): left off.
    PhoneTurnedOff,
    PhoneOff,
    PhoneStill,
    PlasmaCan,
    PlasmaOn,
    /// Whether Plasma shows Sioul's dose reminders during its do-not-disturb.
    PlasmaDoses(bool),
    PlasmaFailed(String),
    PlasmaOff,
    GnomeCan,
    GnomeCannot,
    GnomeOn,
    GnomeAlready,
    GnomeSandboxed,
    GnomeFailed(String),
    GnomeOff,
    GnomeLeft,
    /// A desktop with no way for an application: its server's name, "" unknown.
    DesktopCannot(String),
    MacCan,
    MacCannot,
    MacOn,
    MacFailed(String, String),
    MacOff,
    MacNoOff,
    WindowsCannot,
    /// Sioul's own notifications, held by the pauses, said when the system's are not silenced.
    OwnHeld { doses: bool },
}

/// Who gets through the phone's mode, as Sioul sets it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Senders {
    Starred,
    Nobody,
}

/// What becomes of a dose reminder during the pause, on the phone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Doses {
    Through,
    /// Not asked through: in the shade, silent.
    Shade,
    /// Asked through, but its channel was changed in Android's settings.
    Blocked,
}

/// The macOS shortcuts the person makes, run at the start and the end.
const MAC_ON: &str = "Sioul pause on";
const MAC_OFF: &str = "Sioul pause off";

fn sentence(said: &Said, tr: &Translator) -> String {
    let with = |key: &str, pairs: &[(&'static str, &str)]| {
        let mut args = i18n::args();
        for (name, value) in pairs {
            args.set(*name, value.to_string());
        }
        tr.text(key, Some(&args))
    };
    let shortcuts = [("on", MAC_ON), ("off", MAC_OFF)];
    match said {
        Said::PhoneCan => tr.text("dnd-phone-can", None),
        Said::PhoneNeedsAccess => tr.text("dnd-phone-needs-access", None),
        Said::PhoneNoAccess => tr.text("dnd-phone-no-access", None),
        Said::PhoneTooOld => tr.text("dnd-phone-too-old", None),
        Said::PhoneCallback(Some(number)) => with("dnd-phone-callback", &[("number", number.as_str())]),
        Said::PhoneCallback(None) => tr.text("dnd-phone-callback-unknown", None),
        Said::PhoneOn(Senders::Starred) => tr.text("dnd-phone-starred", None),
        Said::PhoneOn(Senders::Nobody) => tr.text("dnd-phone-nobody", None),
        Said::PhoneAsSet(name) => with("dnd-phone-as-set", &[("name", name.as_str())]),
        Said::PhoneSetThere => tr.text("dnd-phone-set-there", None),
        Said::PhoneThrough { alarms, doses } => {
            let alarms = match alarms {
                Some(false) => tr.text("dnd-phone-alarms-silenced", None),
                _ if *doses == Doses::Through => return tr.text("dnd-phone-alarms-doses", None),
                _ => tr.text("dnd-phone-alarms", None),
            };
            let doses = match doses {
                Doses::Through => tr.text("dnd-phone-doses", None),
                Doses::Shade => tr.text("dnd-phone-doses-shade", None),
                Doses::Blocked => tr.text("dnd-phone-doses-blocked", None),
            };
            format!("{alarms} {doses}")
        }
        Said::PhoneAlready => tr.text("dnd-phone-already", None),
        Said::PhoneFailed(why) => with("dnd-phone-failed", &[("why", why.as_str())]),
        Said::PhoneNoAnswer => tr.text("dnd-phone-no-answer", None),
        Said::PhoneDisabled(name) => with("dnd-phone-disabled", &[("name", name.as_str())]),
        Said::PhoneTurnedOff => tr.text("dnd-phone-turned-off", None),
        Said::PhoneOff => tr.text("dnd-phone-off", None),
        Said::PhoneStill => tr.text("dnd-phone-still", None),
        Said::PlasmaCan => tr.text("dnd-plasma-can", None),
        Said::PlasmaOn => tr.text("dnd-plasma-on", None),
        Said::PlasmaDoses(true) => tr.text("dnd-plasma-doses", None),
        Said::PlasmaDoses(false) => tr.text("dnd-plasma-doses-hidden", None),
        Said::PlasmaFailed(why) => with("dnd-plasma-failed", &[("why", why.as_str())]),
        Said::PlasmaOff => tr.text("dnd-plasma-off", None),
        Said::GnomeCan => tr.text("dnd-gnome-can", None),
        Said::GnomeCannot => tr.text("dnd-gnome-cannot", None),
        Said::GnomeOn => tr.text("dnd-gnome-on", None),
        Said::GnomeAlready => tr.text("dnd-gnome-already", None),
        Said::GnomeSandboxed => tr.text("dnd-gnome-sandboxed", None),
        Said::GnomeFailed(why) => with("dnd-gnome-failed", &[("why", why.as_str())]),
        Said::GnomeOff => tr.text("dnd-gnome-off", None),
        Said::GnomeLeft => tr.text("dnd-gnome-left", None),
        Said::DesktopCannot(name) if name.is_empty() => tr.text("dnd-desktop-cannot-unknown", None),
        Said::DesktopCannot(name) => with("dnd-desktop-cannot", &[("name", name.as_str())]),
        Said::MacCan => with("dnd-mac-can", &shortcuts),
        Said::MacCannot => with("dnd-mac-cannot", &shortcuts),
        Said::MacOn => with("dnd-mac-on", &shortcuts),
        Said::MacFailed(name, why) => with("dnd-mac-failed", &[("name", name.as_str()), ("why", why.as_str())]),
        Said::MacOff => with("dnd-mac-off", &shortcuts),
        Said::MacNoOff => with("dnd-mac-no-off", &shortcuts),
        Said::WindowsCannot => tr.text("dnd-windows-cannot", None),
        Said::OwnHeld { doses: true } => tr.text("dnd-own-held-doses", None),
        Said::OwnHeld { doses: false } => tr.text("dnd-own-held", None),
    }
}

/// What a platform did or can do, as facts.
#[derive(Debug, Default, PartialEq, Eq)]
struct Done {
    on: bool,
    said: Vec<Said>,
    offers: Vec<&'static str>,
    /// GNOME's consent switch is offered.
    consent: bool,
    /// Asking again may change the outcome (access given meanwhile, a server
    /// that refused): the pauses' next minute tries again.
    retry: bool,
}

impl Done {
    fn said(on: bool, said: Vec<Said>) -> Done {
        Done { on, said, ..Done::default() }
    }

    fn report(self, tr: &Translator) -> Report {
        let line = self.said.iter().map(|s| sentence(s, tr)).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ");
        Report { on: self.on, line, offers: self.offers, consent: if self.consent { tr.text("dnd-gnome-consent", None) } else { String::new() } }
    }
}

// ---------------------------------------------------------------- the logic, for every platform

/// One system's side, behind a trait: the logic below is tested with a stand-in.
trait Platform {
    /// What this device allows, changing nothing.
    fn can(&mut self, tr: &Translator) -> Done;
    /// `ask`'s pause silenced, or brought up to date; idempotent given `kept`.
    fn on(&mut self, ask: &Ask, kept: &mut Kept, tr: &Translator) -> Done;
    /// `which`'s pause no longer silenced, and nothing else; `others`: the
    /// pauses still on here.
    fn off(&mut self, which: Which, others: &[Ask], kept: &mut Kept) -> Done;
    /// Whether what `on` did for `which` still holds in this process.
    fn holds(&self, _which: Which) -> bool {
        true
    }
}

/// What Sioul turned on, kept in the state folder for the next run.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Kept {
    /// GNOME: Sioul switched its Do Not Disturb on, from off; off again when
    /// the last pause leaves.
    #[serde(default)]
    #[cfg_attr(any(target_os = "android", target_os = "macos", windows), allow(dead_code))]
    gnome: bool,
    /// macOS: the start shortcut ran; the end one runs when the last pause leaves.
    #[serde(default)]
    #[cfg_attr(not(any(target_os = "macos", test)), allow(dead_code))]
    mac: bool,
    /// The pauses entered on this device, as asked.
    #[serde(default)]
    entered: Vec<Ask>,
}

/// The logic: what is on, what was said, what the next run must know.
struct Layer<P: Platform> {
    platform: P,
    kept: Kept,
    /// Where `kept` is written; none in tests.
    path: Option<PathBuf>,
    /// Each pause entered in this run, as asked, with its report, and
    /// whether asking again could change it.
    said: Vec<(Ask, Report, bool)>,
    /// The pauses known off in this run: leaving again does nothing.
    off: Vec<Which>,
}

impl<P: Platform> Layer<P> {
    fn new(platform: P, path: Option<PathBuf>) -> Self {
        let kept = path.as_deref().map(read_kept).unwrap_or_default();
        Layer { platform, kept, path, said: Vec::new(), off: Vec::new() }
    }

    fn can(&mut self, tr: &Translator) -> Report {
        self.platform.can(tr).report(tr)
    }

    fn enter(&mut self, ask: &Ask, tr: &Translator) -> Report {
        if let Some((_, report, retry)) = self.said.iter().find(|(asked, _, _)| asked == ask)
            && !*retry
            && self.platform.holds(ask.which)
        {
            return report.clone();
        }
        let mut done = self.platform.on(ask, &mut self.kept, tr);
        self.kept.entered.retain(|a| a.which != ask.which);
        self.kept.entered.push(ask.clone());
        self.save();
        self.off.retain(|w| *w != ask.which);
        if !done.on {
            done.said.push(Said::OwnHeld { doses: ask.doses });
        }
        let retry = done.retry;
        let report = done.report(tr);
        self.said.retain(|(a, _, _)| a.which != ask.which);
        self.said.push((ask.clone(), report.clone(), retry));
        report
    }

    fn leave(&mut self, which: Which, tr: &Translator) -> Report {
        let entered = self.kept.entered.iter().any(|a| a.which == which);
        if !entered && self.off.contains(&which) {
            return Report::default();
        }
        self.kept.entered.retain(|a| a.which != which);
        let others = self.kept.entered.clone();
        let done = self.platform.off(which, &others, &mut self.kept);
        self.save();
        self.said.retain(|(a, _, _)| a.which != which);
        self.off.push(which);
        Done { on: false, ..done }.report(tr)
    }

    fn save(&self) {
        let Some(path) = &self.path else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let text = toml::to_string(&self.kept).unwrap_or_default();
        // Written whole beside it, then put in place: never half a file.
        let fresh = path.with_extension("toml.new");
        if std::fs::write(&fresh, text).is_ok() {
            let _ = std::fs::rename(&fresh, path);
        }
    }
}

fn read_kept(path: &Path) -> Kept {
    std::fs::read_to_string(path).ok().and_then(|text| toml::from_str(&text).ok()).unwrap_or_default()
}

/// Where what Sioul turned on is kept.
fn kept_path() -> PathBuf {
    sioul_core::config::state_dir().join("dnd.toml")
}

/// This system's platform.
#[cfg(target_os = "android")]
type System = phone::Phone;
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
type System = desktop::Desktop<desktop::Session>;
#[cfg(target_os = "macos")]
type System = mac::Mac<mac::Shortcuts>;
#[cfg(windows)]
type System = WindowsPlatform;
#[cfg(not(any(unix, windows)))]
type System = Nothing;

static LAYER: Mutex<Option<Layer<System>>> = Mutex::new(None);

fn with<R>(act: impl FnOnce(&mut Layer<System>) -> R) -> R {
    let mut layer = LAYER.lock().unwrap_or_else(|e| e.into_inner());
    act(layer.get_or_insert_with(|| Layer::new(System::default(), Some(kept_path()))))
}

// ---------------------------------------------------------------- Android

/// Android's answers (PauseMode.java), as facts. Compiled everywhere: tested on a computer.
#[cfg_attr(not(any(target_os = "android", test)), allow(dead_code))]
mod answers {
    use super::*;
    use serde_json::Value;

    /// ZenPolicy's PEOPLE_TYPE_STARRED, PEOPLE_TYPE_NONE; STATE_ALLOW, STATE_DISALLOW.
    const STARRED: i64 = 3;
    const NONE: i64 = 4;
    const ALLOW: i64 = 1;
    const DISALLOW: i64 = 2;

    /// `can`: {api, access, …}; `callback`, the number emergency services
    /// call back from in the person's country, where one is known.
    pub(super) fn can(answer: &Value, callback: Option<String>) -> Done {
        if answer.is_null() {
            return Done::said(false, vec![Said::PhoneNoAnswer]);
        }
        if answer["too_old"] == true {
            return Done::said(false, vec![Said::PhoneTooOld]);
        }
        if answer["access"] != true {
            return Done { said: vec![Said::PhoneNeedsAccess], offers: vec!["access", "starred"], ..Done::default() };
        }
        Done { on: true, said: vec![Said::PhoneCan, Said::PhoneCallback(callback)], offers: vec!["starred"], ..Done::default() }
    }

    /// `enter`: {access, too_old, error, rule: {name, calls, messages,
    /// repeat, alarms}, doses, already, disabled, turned_off}; `name`, the
    /// mode's name as Sioul gives it.
    pub(super) fn entered(ask: &Ask, answer: &Value, name: &str) -> Done {
        if answer.is_null() {
            return Done { said: vec![Said::PhoneNoAnswer], retry: true, ..Done::default() };
        }
        if answer["too_old"] == true {
            return Done::said(false, vec![Said::PhoneTooOld]);
        }
        if answer["access"] != true {
            return Done { said: vec![Said::PhoneNoAccess], offers: vec!["access"], retry: true, ..Done::default() };
        }
        if let Some(error) = answer["error"].as_str().filter(|e| !e.is_empty()) {
            return Done { said: vec![Said::PhoneFailed(error.to_string())], retry: true, ..Done::default() };
        }
        let rule = &answer["rule"];
        let named = rule["name"].as_str().filter(|n| !n.is_empty()).unwrap_or(name).to_string();
        if answer["disabled"] == true {
            return Done::said(false, vec![Said::PhoneDisabled(named)]);
        }
        if answer["turned_off"] == true {
            return Done::said(false, vec![Said::PhoneTurnedOff]);
        }
        let (calls, messages, repeat) = (rule["calls"].as_i64(), rule["messages"].as_i64(), rule["repeat"].as_i64());
        let senders = match (calls, messages, repeat) {
            (Some(STARRED), Some(STARRED), Some(ALLOW)) => Some(Senders::Starred),
            (Some(NONE), Some(NONE), Some(DISALLOW)) => Some(Senders::Nobody),
            _ => None,
        };
        let asked = if ask.people { Senders::Starred } else { Senders::Nobody };
        let mut said = match senders {
            Some(senders) if senders == asked => vec![Said::PhoneOn(senders)],
            // Changed in Android's settings (its modes, from Android 15): said as it is.
            Some(senders) => vec![Said::PhoneOn(senders), Said::PhoneSetThere],
            None => vec![Said::PhoneAsSet(named)],
        };
        let alarms = match rule["alarms"].as_i64() {
            Some(ALLOW) => Some(true),
            Some(DISALLOW) => Some(false),
            _ => None,
        };
        let doses = match (ask.doses, answer["doses"] == true) {
            (false, _) => Doses::Shade,
            (true, true) => Doses::Through,
            (true, false) => Doses::Blocked,
        };
        said.push(Said::PhoneThrough { alarms, doses });
        if answer["already"] == true {
            said.push(Said::PhoneAlready);
        }
        Done { on: true, said, ..Done::default() }
    }

    /// `leave`: {was, still}: whether Sioul's mode was on; a do-not-disturb
    /// on before the pause, and still on.
    pub(super) fn left(answer: &Value) -> Done {
        if answer["was"] != true || answer["too_old"] == true {
            return Done::default();
        }
        let mut said = vec![Said::PhoneOff];
        if answer["still"] == true {
            said.push(Said::PhoneStill);
        }
        Done::said(false, said)
    }
}

/// The phone's side. Compiled for the tests too, so that a computer checks
/// it; Java answers nothing there.
#[cfg(any(target_os = "android", test))]
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
mod phone {
    use super::*;
    use serde_json::Value;
    #[cfg(target_os = "android")]
    use std::ffi::{CStr, CString, c_char};

    #[cfg(target_os = "android")]
    unsafe extern "C" {
        /// PauseMode.call (android/main.cpp): a verb and its JSON, a JSON answer or null.
        fn sioul_android_dnd(verb: *const c_char, json: *const c_char) -> *mut c_char;
        /// An answer of `sioul_android_dnd`, given back.
        fn sioul_android_dnd_free(text: *mut c_char);
    }

    /// Tests on a computer: no Java.
    #[cfg(not(target_os = "android"))]
    pub(super) fn call(_verb: &str, _json: &str) -> Value {
        Value::Null
    }

    /// Java's answer to `verb`; null when it gave none.
    #[cfg(target_os = "android")]
    pub(super) fn call(verb: &str, json: &str) -> Value {
        let (Ok(verb), Ok(json)) = (CString::new(verb), CString::new(json)) else { return Value::Null };
        // SAFETY: two zero-terminated texts, valid for the call.
        let answer = unsafe { sioul_android_dnd(verb.as_ptr(), json.as_ptr()) };
        if answer.is_null() {
            return Value::Null;
        }
        // SAFETY: a zero-terminated text from sioul_android_dnd, read before it is given back.
        let text = unsafe { CStr::from_ptr(answer) }.to_string_lossy().to_string();
        // SAFETY: given back once, as sioul_android_dnd asks; not read after.
        unsafe { sioul_android_dnd_free(answer) };
        serde_json::from_str(&text).unwrap_or(Value::Null)
    }

    /// The phone: Sioul's own modes, kept by Android (and by Java's list of
    /// the pauses on, which survives Sioul and the phone's restarts).
    #[derive(Default)]
    pub(crate) struct Phone;

    impl Platform for Phone {
        fn can(&mut self, _tr: &Translator) -> Done {
            // The country whose numbers the pause shows (sioul_core::pause::country).
            let config = crate::backend::load_config();
            let callback = sioul_core::pause::callback(&sioul_core::pause::country(&config.pause, config.contacts.region.as_deref()));
            answers::can(&call("can", ""), callback)
        }

        fn on(&mut self, ask: &Ask, _kept: &mut Kept, tr: &Translator) -> Done {
            let trigger = match ask.which {
                Which::FreeTime => tr.text("dnd-trigger-free-time", None),
                Which::Paused => tr.text("dnd-trigger-pause", None),
                Which::Global => tr.text("dnd-trigger-global", None),
            };
            let asked = serde_json::json!({
                "kind": ask.which.key(),
                "name": ask.which.name(tr),
                "trigger": trigger,
                "people": ask.people,
                "doses": ask.doses,
                "channel": tr.text("dnd-doses-channel", None),
            });
            answers::entered(ask, &call("enter", &asked.to_string()), &ask.which.name(tr))
        }

        fn off(&mut self, which: Which, _others: &[Ask], _kept: &mut Kept) -> Done {
            answers::left(&call("leave", which.key()))
        }
    }
}

// ---------------------------------------------------------------- Linux and the BSDs

/// The desktop's logic, compiled where it runs and for the tests; the session
/// itself (D-Bus, gsettings, Plasma's files) only where it runs.
#[cfg(any(all(unix, not(any(target_os = "macos", target_os = "android"))), test))]
#[cfg_attr(not(all(unix, not(any(target_os = "macos", target_os = "android")))), allow(dead_code))]
mod desktop {
    use super::*;
    use std::time::{Duration, Instant};

    /// Sioul's desktop file, as Plasma and GNOME know the application.
    pub(super) const DESKTOP_ENTRY: &str = "com.aurelienpierre.Sioul";
    /// dconf's word of Sioul's own switch may come this long after it: changes
    /// heard later are the person's.
    const SETTLE: Duration = Duration::from_secs(3);

    /// What kind of desktop serves the notifications.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub(super) enum Kind {
        /// A server that inhibits on request (Plasma); `kde`: Plasma's own,
        /// whose settings say whether critical notifications show meanwhile.
        Inhibits { kde: bool },
        Gnome,
        /// Another, by its server's name ("" unknown).
        Other(String),
    }

    /// The session's side, behind a trait: tested with a stand-in.
    pub(super) trait Desk {
        fn kind(&mut self) -> Kind;
        /// Plasma's inhibition for `reason`, held until released or dropped.
        fn inhibit(&mut self, reason: &str) -> Result<Box<dyn Held>, String>;
        /// Whether Plasma shows Sioul's notifications during its do-not-disturb.
        fn plasma_shows_doses(&self) -> bool;
        /// GNOME's banners shown (its Do Not Disturb off).
        fn banners(&self) -> Result<bool, String>;
        fn set_banners(&self, shown: bool) -> Result<(), String>;
        /// dconf's changes of GNOME's switch, heard from now on.
        fn watch_banners(&self) -> Option<Box<dyn Heard>>;
        /// Sioul runs in a Flatpak: GNOME's settings out of reach.
        fn sandboxed(&self) -> bool;
    }

    /// An inhibition held.
    pub(super) trait Held: Send {
        fn alive(&self) -> bool;
        fn release(self: Box<Self>) -> Result<(), String>;
    }

    /// Changes heard.
    pub(super) trait Heard: Send {
        fn since(&self, at: Instant) -> usize;
    }

    /// A computer's desktop: Plasma's inhibitions held, by pause; GNOME's
    /// switch and what dconf says of it.
    pub(crate) struct Desktop<D: Desk> {
        pub(super) desk: D,
        held: Vec<(Which, Box<dyn Held>)>,
        /// dconf's changes of GNOME's switch while Sioul holds it, and from
        /// when they are the person's.
        heard: Option<(Box<dyn Heard>, Instant)>,
    }

    impl<D: Desk + Default> Default for Desktop<D> {
        fn default() -> Self {
            Desktop::new(D::default())
        }
    }

    impl<D: Desk> Desktop<D> {
        pub(super) fn new(desk: D) -> Self {
            Desktop { desk, held: Vec::new(), heard: None }
        }

        fn holding(&self, which: Which) -> bool {
            self.held.iter().any(|(w, held)| *w == which && held.alive())
        }

        /// GNOME's switch, as asked: on only from off, only with the person's yes.
        pub(super) fn gnome_on(&mut self, ask: &Ask, kept: &mut Kept) -> Done {
            if !ask.desktop {
                return Done { said: vec![Said::GnomeCannot], offers: vec!["desktop"], consent: true, ..Done::default() };
            }
            if self.desk.sandboxed() {
                return Done::said(false, vec![Said::GnomeSandboxed]);
            }
            if kept.gnome {
                // Turned on by Sioul already: for another pause, or before a restart.
                if self.heard.is_none() {
                    self.heard = self.desk.watch_banners().map(|heard| (heard, Instant::now()));
                }
                return Done::said(true, vec![Said::GnomeOn]);
            }
            match self.desk.banners() {
                // Already on: the person's. Never turned off by Sioul.
                Ok(false) => Done::said(true, vec![Said::GnomeAlready]),
                Ok(true) => {
                    // Listening before the switch, so that its own word comes first.
                    let heard = self.desk.watch_banners();
                    match self.desk.set_banners(false) {
                        Ok(()) => {
                            kept.gnome = true;
                            self.heard = heard.map(|heard| (heard, Instant::now() + SETTLE));
                            Done::said(true, vec![Said::GnomeOn])
                        }
                        Err(why) => Done { said: vec![Said::GnomeFailed(why)], retry: true, ..Done::default() },
                    }
                }
                Err(why) => Done { said: vec![Said::GnomeFailed(why)], retry: true, ..Done::default() },
            }
        }

        /// GNOME's switch off again when the last pause that wanted it leaves,
        /// and only if nobody touched it meanwhile.
        pub(super) fn gnome_off(&mut self, others: &[Ask], kept: &mut Kept) -> Vec<Said> {
            if !kept.gnome || others.iter().any(|a| a.desktop) {
                return Vec::new();
            }
            let touched = self.heard.take().is_some_and(|(heard, after)| heard.since(after) > 0);
            if touched {
                kept.gnome = false;
                return vec![Said::GnomeLeft];
            }
            match self.desk.banners() {
                Ok(false) => match self.desk.set_banners(true) {
                    Ok(()) => {
                        kept.gnome = false;
                        vec![Said::GnomeOff]
                    }
                    // Kept: tried again at the next start.
                    Err(why) => vec![Said::GnomeFailed(why)],
                },
                // Turned off meanwhile, by the person.
                Ok(true) => {
                    kept.gnome = false;
                    vec![Said::GnomeOff]
                }
                Err(why) => vec![Said::GnomeFailed(why)],
            }
        }
    }

    impl<D: Desk> Platform for Desktop<D> {
        fn can(&mut self, _tr: &Translator) -> Done {
            match self.desk.kind() {
                Kind::Inhibits { kde } => {
                    let mut done = Done::said(true, vec![Said::PlasmaCan]);
                    if kde && !self.desk.plasma_shows_doses() {
                        done.said.push(Said::PlasmaDoses(false));
                        done.offers.push("plasma");
                    }
                    done
                }
                Kind::Gnome if self.desk.sandboxed() => Done::said(false, vec![Said::GnomeSandboxed]),
                Kind::Gnome => Done { on: true, said: vec![Said::GnomeCan], offers: vec!["desktop"], consent: true, retry: false },
                Kind::Other(name) => Done::said(false, vec![Said::DesktopCannot(name)]),
            }
        }

        fn on(&mut self, ask: &Ask, kept: &mut Kept, tr: &Translator) -> Done {
            match self.desk.kind() {
                Kind::Inhibits { kde } => {
                    if !self.holding(ask.which) {
                        self.held.retain(|(w, _)| *w != ask.which);
                        match self.desk.inhibit(&ask.which.name(tr)) {
                            Ok(held) => self.held.push((ask.which, held)),
                            Err(why) => return Done { said: vec![Said::PlasmaFailed(why)], retry: true, ..Done::default() },
                        }
                    }
                    let mut done = Done::said(true, vec![Said::PlasmaOn]);
                    if ask.doses {
                        let shown = !kde || self.desk.plasma_shows_doses();
                        done.said.push(Said::PlasmaDoses(shown));
                        if !shown {
                            done.offers.push("plasma");
                        }
                    }
                    done
                }
                Kind::Gnome if !ask.desktop && kept.gnome => {
                    // The yes taken back during the pause: switched back now,
                    // unless another pause on still has it.
                    let others: Vec<Ask> = kept.entered.iter().filter(|a| a.which != ask.which).cloned().collect();
                    let mut said = self.gnome_off(&others, kept);
                    let mut done = self.gnome_on(ask, kept);
                    said.append(&mut done.said);
                    done.said = said;
                    done
                }
                Kind::Gnome => self.gnome_on(ask, kept),
                Kind::Other(name) => Done::said(false, vec![Said::DesktopCannot(name)]),
            }
        }

        fn off(&mut self, which: Which, others: &[Ask], kept: &mut Kept) -> Done {
            let mut said = Vec::new();
            if let Some(at) = self.held.iter().position(|(w, _)| *w == which) {
                let (_, held) = self.held.remove(at);
                // Refused: lifted already (the person, from Plasma's applet), or the server gone.
                let _ = held.release();
                said.push(Said::PlasmaOff);
            }
            said.extend(self.gnome_off(others, kept));
            Done::said(false, said)
        }

        fn holds(&self, which: Which) -> bool {
            // An inhibition's connection gone (the session bus lost): asked again.
            self.held.iter().all(|(w, held)| *w != which || held.alive())
        }
    }

    /// Whether Plasma shows Sioul's notifications (all sent as critical) during
    /// do-not-disturb, from its settings: critical ones shown then
    /// (`CriticalInDndMode`, true unless turned off), or Sioul allowed "Show in
    /// do not disturb mode" (`ShowPopupsInDndMode`); never when Sioul's
    /// popups are off altogether (`ShowPopups`).
    pub(super) fn shows_doses(critical: Option<&str>, allowed: Option<&str>, popups: Option<&str>) -> bool {
        let yes = |value: Option<&str>, default: bool| value.map_or(default, |v| v.eq_ignore_ascii_case("true"));
        yes(popups, true) && (yes(critical, true) || yes(allowed, false))
    }

    /// A key's value in a KDE settings file, in a group written as its header
    /// (`[Notifications]`, `[Applications][com.aurelienpierre.Sioul]`).
    pub(super) fn kconfig<'a>(text: &'a str, group: &str, key: &str) -> Option<&'a str> {
        let mut inside = false;
        let mut found = None;
        for line in text.lines().map(str::trim) {
            if line.starts_with('[') {
                // A group's header, maybe marked immutable: "[Group][$i]".
                inside = line.strip_suffix("[$i]").unwrap_or(line) == group;
                continue;
            }
            if !inside || line.starts_with('#') {
                continue;
            }
            if let Some((name, value)) = line.split_once('=') {
                // "Key[$i]=": immutable; "Key[fr]=": a language's, not this one.
                let name = name.trim();
                if name.strip_suffix("[$i]").unwrap_or(name) == key {
                    found = Some(value.trim());
                }
            }
        }
        found
    }

    /// The person's session: its bus, GNOME's settings through `gsettings`,
    /// Plasma's through its files.
    #[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
    #[derive(Default)]
    pub(crate) struct Session {
        /// Variables for `gsettings` (tests: a settings file of their own).
        pub(super) gsettings_env: Vec<(String, String)>,
        /// Where Plasma's settings are read from (tests: a folder of their own).
        pub(super) config_dirs: Option<Vec<PathBuf>>,
    }

    /// GNOME's Do Not Disturb, inverted: banners shown.
    #[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
    const BANNERS: [&str; 2] = ["org.gnome.desktop.notifications", "show-banners"];
    #[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
    const BANNERS_PATH: &str = "/org/gnome/desktop/notifications/show-banners";

    #[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
    impl Session {
        fn gsettings(&self, args: &[&str]) -> Result<String, String> {
            let mut command = std::process::Command::new("gsettings");
            command.args(args);
            for (name, value) in &self.gsettings_env {
                command.env(name, value);
            }
            let out = command.output().map_err(|e| format!("gsettings: {e}"))?;
            if !out.status.success() {
                let said = String::from_utf8_lossy(&out.stderr).trim().to_string();
                return Err(if said.is_empty() { format!("gsettings: {}", out.status) } else { said });
            }
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        }

        /// Plasma's settings files, the person's first: theirs, the look's
        /// defaults, the system's.
        fn plasma_files(&self) -> Vec<PathBuf> {
            if let Some(dirs) = &self.config_dirs {
                return dirs.iter().map(|dir| dir.join("plasmanotifyrc")).collect();
            }
            let home = std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()).map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")));
            let mut files: Vec<PathBuf> = home.iter().flat_map(|home| [home.join("plasmanotifyrc"), home.join("kdedefaults").join("plasmanotifyrc")]).collect();
            let system = std::env::var("XDG_CONFIG_DIRS").ok().filter(|v| !v.is_empty()).unwrap_or_else(|| "/etc/xdg".to_string());
            files.extend(system.split(':').filter(|d| !d.is_empty()).map(|dir| Path::new(dir).join("plasmanotifyrc")));
            files
        }

        /// A value of Plasma's notification settings, from the first file that sets it.
        fn plasma(&self, group: &str, key: &str) -> Option<String> {
            self.plasma_files().iter().filter_map(|file| std::fs::read_to_string(file).ok()).find_map(|text| kconfig(&text, group, key).map(str::to_string))
        }
    }

    #[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
    impl Desk for Session {
        fn kind(&mut self) -> Kind {
            match sioul_sync::dnd::server() {
                Some(server) if server.inhibits => Kind::Inhibits { kde: server.vendor == "KDE" },
                Some(server) if server.name == "gnome-shell" => Kind::Gnome,
                Some(server) => Kind::Other(server.name),
                None if desktop_is("GNOME") => Kind::Gnome,
                None => Kind::Other(String::new()),
            }
        }

        fn inhibit(&mut self, reason: &str) -> Result<Box<dyn Held>, String> {
            sioul_sync::dnd::inhibit(DESKTOP_ENTRY, reason).map(|held| Box::new(held) as Box<dyn Held>)
        }

        fn plasma_shows_doses(&self) -> bool {
            let group = format!("[Applications][{DESKTOP_ENTRY}]");
            shows_doses(self.plasma("[Notifications]", "CriticalInDndMode").as_deref(), self.plasma(&group, "ShowPopupsInDndMode").as_deref(), self.plasma(&group, "ShowPopups").as_deref())
        }

        fn banners(&self) -> Result<bool, String> {
            match self.gsettings(&["get", BANNERS[0], BANNERS[1]])?.as_str() {
                "true" => Ok(true),
                "false" => Ok(false),
                other => Err(format!("gsettings: {other}")),
            }
        }

        fn set_banners(&self, shown: bool) -> Result<(), String> {
            self.gsettings(&["set", BANNERS[0], BANNERS[1], if shown { "true" } else { "false" }]).map(drop)
        }

        fn watch_banners(&self) -> Option<Box<dyn Heard>> {
            // Tests write a settings file of their own, which dconf never tells of.
            if !self.gsettings_env.is_empty() {
                return None;
            }
            sioul_sync::dnd::watch_dconf(BANNERS_PATH).ok().map(|changes| Box::new(changes) as Box<dyn Heard>)
        }

        fn sandboxed(&self) -> bool {
            std::env::var_os("FLATPAK_ID").is_some() || Path::new("/.flatpak-info").exists()
        }
    }

    #[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
    impl Held for sioul_sync::dnd::Inhibition {
        fn alive(&self) -> bool {
            sioul_sync::dnd::Inhibition::alive(self)
        }

        fn release(self: Box<Self>) -> Result<(), String> {
            sioul_sync::dnd::Inhibition::release(*self, Duration::from_secs(3))
        }
    }

    #[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
    impl Heard for sioul_sync::dnd::Changes {
        fn since(&self, at: Instant) -> usize {
            sioul_sync::dnd::Changes::since(self, at)
        }
    }

    /// Whether the session says it is this desktop (XDG_CURRENT_DESKTOP: "KDE", "ubuntu:GNOME").
    #[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
    fn desktop_is(name: &str) -> bool {
        std::env::var("XDG_CURRENT_DESKTOP").is_ok_and(|desktops| desktops.split(':').any(|d| d.eq_ignore_ascii_case(name)))
    }

    /// Plasma's notification settings, at Sioul's page, where "Show in do not
    /// disturb mode" is: System Settings, else its module alone.
    #[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
    pub(super) fn open_plasma_settings() {
        let entry = format!("--desktop-entry {DESKTOP_ENTRY}");
        for program in ["systemsettings", "kcmshell6"] {
            let started = std::process::Command::new(program)
                .args(["kcm_notifications", "--args", entry.as_str()])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
            if let Ok(mut window) = started {
                // Waited for on a thread of its own, so that it leaves no zombie behind.
                std::thread::spawn(move || window.wait());
                return;
            }
        }
    }
}

// ---------------------------------------------------------------- macOS

#[cfg(any(target_os = "macos", test))]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod mac {
    use super::*;

    /// The Shortcuts app's side, behind a trait: tested with a stand-in.
    pub(super) trait Run {
        /// The person's shortcuts by name; none when they cannot be listed.
        fn list(&self) -> Option<Vec<String>>;
        fn run(&self, name: &str) -> Result<(), String>;
    }

    /// The `shortcuts` command (macOS 12 and later).
    #[derive(Default)]
    pub(crate) struct Shortcuts;

    impl Run for Shortcuts {
        fn list(&self) -> Option<Vec<String>> {
            let out = std::process::Command::new("shortcuts").arg("list").output().ok().filter(|out| out.status.success())?;
            Some(String::from_utf8_lossy(&out.stdout).lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect())
        }

        fn run(&self, name: &str) -> Result<(), String> {
            let out = std::process::Command::new("shortcuts").args(["run", name]).output().map_err(|e| e.to_string())?;
            if out.status.success() {
                return Ok(());
            }
            let said = String::from_utf8_lossy(&out.stderr).trim().to_string();
            Err(if said.is_empty() { out.status.to_string() } else { said })
        }
    }

    /// A Mac: macOS lets no application turn a Focus on; the person's own
    /// shortcuts can, run at the start of the first pause and the end of the
    /// last. Sioul cannot read whether a Focus was on before: the end
    /// shortcut does what the person made it do.
    #[derive(Default)]
    pub(crate) struct Mac<R: Run> {
        pub(super) shortcuts: R,
    }

    impl<R: Run> Mac<R> {
        fn has(&self, name: &str) -> bool {
            self.shortcuts.list().is_some_and(|names| names.iter().any(|n| n == name))
        }
    }

    impl<R: Run> Platform for Mac<R> {
        fn can(&mut self, _tr: &Translator) -> Done {
            if self.has(MAC_ON) { Done::said(true, vec![Said::MacCan]) } else { Done::said(false, vec![Said::MacCannot]) }
        }

        fn on(&mut self, _ask: &Ask, kept: &mut Kept, _tr: &Translator) -> Done {
            if kept.mac {
                return Done::said(true, vec![Said::MacOn]);
            }
            if !self.has(MAC_ON) {
                return Done::said(false, vec![Said::MacCannot]);
            }
            match self.shortcuts.run(MAC_ON) {
                Ok(()) => {
                    kept.mac = true;
                    Done::said(true, vec![Said::MacOn])
                }
                Err(why) => Done { said: vec![Said::MacFailed(MAC_ON.to_string(), why)], retry: true, ..Done::default() },
            }
        }

        fn off(&mut self, _which: Which, others: &[Ask], kept: &mut Kept) -> Done {
            if !kept.mac || !others.is_empty() {
                return Done::default();
            }
            kept.mac = false;
            if !self.has(MAC_OFF) {
                return Done::said(false, vec![Said::MacNoOff]);
            }
            match self.shortcuts.run(MAC_OFF) {
                Ok(()) => Done::said(false, vec![Said::MacOff]),
                Err(why) => Done::said(false, vec![Said::MacFailed(MAC_OFF.to_string(), why)]),
            }
        }
    }
}

// ---------------------------------------------------------------- Windows, and elsewhere

/// Windows: starting a Focus session is a Limited Access Feature, which needs
/// Microsoft's approval; an application can only read the mode.
#[cfg(any(windows, test))]
#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Default)]
struct WindowsPlatform;

#[cfg(any(windows, test))]
impl Platform for WindowsPlatform {
    fn can(&mut self, _tr: &Translator) -> Done {
        Done::said(false, vec![Said::WindowsCannot])
    }

    fn on(&mut self, _ask: &Ask, _kept: &mut Kept, _tr: &Translator) -> Done {
        Done::said(false, vec![Said::WindowsCannot])
    }

    fn off(&mut self, _which: Which, _others: &[Ask], _kept: &mut Kept) -> Done {
        Done::default()
    }
}

/// A system Sioul knows no do-not-disturb of.
#[cfg(not(any(unix, windows)))]
#[derive(Default)]
struct Nothing;

#[cfg(not(any(unix, windows)))]
impl Platform for Nothing {
    fn can(&mut self, _tr: &Translator) -> Done {
        Done::said(false, vec![Said::DesktopCannot(String::new())])
    }

    fn on(&mut self, _ask: &Ask, _kept: &mut Kept, _tr: &Translator) -> Done {
        Done::said(false, vec![Said::DesktopCannot(String::new())])
    }

    fn off(&mut self, _which: Which, _others: &[Ask], _kept: &mut Kept) -> Done {
        Done::default()
    }
}

#[cfg(test)]
mod tests {
    use super::desktop::{Desk, Desktop, Heard, Held, Kind};
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::time::Instant;

    fn ask(which: Which) -> Ask {
        Ask { which, people: true, doses: true, desktop: true }
    }

    /// A platform that counts its calls and answers as told.
    #[derive(Default)]
    struct Counting {
        on: Rc<RefCell<Vec<Ask>>>,
        off: Rc<RefCell<Vec<Which>>>,
        answer_on: bool,
        retry: bool,
        held: bool,
    }

    impl Platform for Counting {
        fn can(&mut self, _tr: &Translator) -> Done {
            Done::said(true, vec![Said::PlasmaCan])
        }

        fn on(&mut self, ask: &Ask, _kept: &mut Kept, _tr: &Translator) -> Done {
            self.on.borrow_mut().push(ask.clone());
            self.held = true;
            Done { on: self.answer_on, said: vec![if self.answer_on { Said::PlasmaOn } else { Said::DesktopCannot("dunst".into()) }], retry: self.retry, ..Done::default() }
        }

        fn off(&mut self, which: Which, _others: &[Ask], _kept: &mut Kept) -> Done {
            self.off.borrow_mut().push(which);
            Done::said(false, vec![Said::PlasmaOff])
        }

        fn holds(&self, _which: Which) -> bool {
            self.held
        }
    }

    #[test]
    fn entering_twice_does_it_once_and_says_the_same() {
        let en = Translator::new("en");
        let platform = Counting { answer_on: true, ..Counting::default() };
        let calls = Rc::clone(&platform.on);
        let mut layer = Layer::new(platform, None);
        let first = layer.enter(&ask(Which::Paused), &en);
        let again = layer.enter(&ask(Which::Paused), &en);
        assert_eq!(first, again);
        assert_eq!(calls.borrow().len(), 1, "asked once of the system");
        assert!(first.on && first.line.contains("silenced"), "{first:?}");
        // Changed settings are brought to the system.
        let mut fewer = ask(Which::Paused);
        fewer.people = false;
        layer.enter(&fewer, &en);
        assert_eq!(calls.borrow().len(), 2);
        // What no longer holds (an inhibition's connection gone) is asked again.
        layer.platform.held = false;
        layer.enter(&fewer, &en);
        assert_eq!(calls.borrow().len(), 3);
    }

    #[test]
    fn a_refusal_that_may_change_is_asked_again_and_says_what_sioul_holds() {
        let en = Translator::new("en");
        let platform = Counting { answer_on: false, retry: true, ..Counting::default() };
        let calls = Rc::clone(&platform.on);
        let mut layer = Layer::new(platform, None);
        let report = layer.enter(&ask(Which::Paused), &en);
        assert!(!report.on);
        assert!(report.line.contains("dunst") && report.line.ends_with("Sioul holds its own notifications, except dose reminders."), "{report:?}");
        layer.enter(&ask(Which::Paused), &en);
        assert_eq!(calls.borrow().len(), 2, "access may have been given meanwhile");
        // One that cannot change is not.
        layer.said.iter_mut().for_each(|(_, _, retry)| *retry = false);
        layer.enter(&ask(Which::Paused), &en);
        assert_eq!(calls.borrow().len(), 2);
    }

    #[test]
    fn leaving_twice_or_what_was_not_entered_does_nothing_more() {
        let en = Translator::new("en");
        let platform = Counting { answer_on: true, ..Counting::default() };
        let offs = Rc::clone(&platform.off);
        let mut layer = Layer::new(platform, None);
        // The first leave of a run heals what a crash may have left.
        layer.leave(Which::FreeTime, &en);
        assert_eq!(offs.borrow().len(), 1);
        assert_eq!(layer.leave(Which::FreeTime, &en), Report::default());
        assert_eq!(offs.borrow().len(), 1, "then nothing");
        layer.enter(&ask(Which::Paused), &en);
        let left = layer.leave(Which::Paused, &en);
        assert!(!left.on && left.line == "This desktop's notifications show again.", "{left:?}");
        assert_eq!(layer.leave(Which::Paused, &en), Report::default());
        assert_eq!(offs.borrow().as_slice(), &[Which::FreeTime, Which::Paused]);
        // Entered again after, it is asked again.
        layer.enter(&ask(Which::Paused), &en);
        assert_eq!(layer.platform.on.borrow().len(), 2);
    }

    #[test]
    fn what_was_turned_on_is_kept_for_the_next_run() {
        let dir = std::env::temp_dir().join(format!("sioul-dnd-{}-{}", std::process::id(), line!()));
        let path = dir.join("dnd.toml");
        let en = Translator::new("en");
        let mut layer = Layer::new(Counting { answer_on: true, ..Counting::default() }, Some(path.clone()));
        layer.enter(&ask(Which::Paused), &en);
        layer.kept.gnome = true;
        layer.save();
        // A crash: the next run knows the pause was entered, and what Sioul turned on.
        let next = Layer::new(Counting::default(), Some(path.clone()));
        assert_eq!(next.kept.entered, vec![ask(Which::Paused)]);
        assert!(next.kept.gnome);
        let mut next = next;
        next.leave(Which::Paused, &en);
        assert!(read_kept(&path).entered.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A desktop session as told: GNOME's switch in memory, inhibitions counted.
    #[derive(Default)]
    struct FakeDesk {
        kind: Option<Kind>,
        banners: Rc<RefCell<Option<bool>>>,
        writes: Rc<RefCell<Vec<bool>>>,
        heard: Arc<AtomicUsize>,
        inhibited: Arc<AtomicUsize>,
        released: Arc<AtomicUsize>,
        alive: Arc<AtomicBool>,
        doses_shown: bool,
        sandboxed: bool,
    }

    struct FakeHeld {
        alive: Arc<AtomicBool>,
        released: Arc<AtomicUsize>,
    }

    impl Held for FakeHeld {
        fn alive(&self) -> bool {
            self.alive.load(Ordering::Relaxed)
        }

        fn release(self: Box<Self>) -> Result<(), String> {
            self.released.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
    }

    struct FakeHeard(Arc<AtomicUsize>);

    impl Heard for FakeHeard {
        fn since(&self, _at: Instant) -> usize {
            self.0.load(Ordering::Relaxed)
        }
    }

    impl Desk for FakeDesk {
        fn kind(&mut self) -> Kind {
            self.kind.clone().unwrap_or(Kind::Gnome)
        }

        fn inhibit(&mut self, _reason: &str) -> Result<Box<dyn Held>, String> {
            self.inhibited.fetch_add(1, Ordering::Relaxed);
            self.alive.store(true, Ordering::Relaxed);
            Ok(Box::new(FakeHeld { alive: Arc::clone(&self.alive), released: Arc::clone(&self.released) }))
        }

        fn plasma_shows_doses(&self) -> bool {
            self.doses_shown
        }

        fn banners(&self) -> Result<bool, String> {
            self.banners.borrow().ok_or_else(|| "No such schema".to_string())
        }

        fn set_banners(&self, shown: bool) -> Result<(), String> {
            self.writes.borrow_mut().push(shown);
            *self.banners.borrow_mut() = Some(shown);
            Ok(())
        }

        fn watch_banners(&self) -> Option<Box<dyn Heard>> {
            Some(Box::new(FakeHeard(Arc::clone(&self.heard))))
        }

        fn sandboxed(&self) -> bool {
            self.sandboxed
        }
    }

    fn gnome(banners: bool) -> Desktop<FakeDesk> {
        Desktop::new(FakeDesk { kind: Some(Kind::Gnome), banners: Rc::new(RefCell::new(Some(banners))), ..FakeDesk::default() })
    }

    #[test]
    fn gnome_switch_is_turned_on_with_consent_and_restored_after() {
        let en = Translator::new("en");
        let mut layer = Layer::new(gnome(true), None);
        let report = layer.enter(&ask(Which::Paused), &en);
        assert!(report.on && report.line.starts_with("This desktop's notification banners are hidden"), "{report:?}");
        assert_eq!(*layer.platform.desk.banners.borrow(), Some(false));
        assert!(layer.kept.gnome);
        let left = layer.leave(Which::Paused, &en);
        assert_eq!(left.line, "GNOME's Do Not Disturb is off again, as it was before.");
        assert_eq!(*layer.platform.desk.writes.borrow(), vec![false, true]);
        assert!(!layer.kept.gnome);
    }

    #[test]
    fn gnome_switch_the_person_set_is_never_turned_off() {
        let en = Translator::new("en");
        // On before the pause: left alone, before and after.
        let mut layer = Layer::new(gnome(false), None);
        let report = layer.enter(&ask(Which::Paused), &en);
        assert!(report.on && report.line == "GNOME's Do Not Disturb was already on; Sioul leaves it as you set it.", "{report:?}");
        layer.leave(Which::Paused, &en);
        assert!(layer.platform.desk.writes.borrow().is_empty(), "never written");
        assert_eq!(*layer.platform.desk.banners.borrow(), Some(false));
        // Changed during the pause (heard from dconf): left as it is.
        let mut layer = Layer::new(gnome(true), None);
        layer.enter(&ask(Which::Paused), &en);
        layer.platform.desk.heard.store(1, Ordering::Relaxed);
        let left = layer.leave(Which::Paused, &en);
        assert_eq!(left.line, "GNOME's Do Not Disturb was changed meanwhile; Sioul leaves it as it is.");
        assert_eq!(*layer.platform.desk.writes.borrow(), vec![false], "not switched back");
    }

    #[test]
    fn gnome_without_consent_or_from_a_sandbox_says_what_it_cannot_do() {
        let en = Translator::new("en");
        let mut layer = Layer::new(gnome(true), None);
        let mut no = ask(Which::Paused);
        no.desktop = false;
        let report = layer.enter(&no, &en);
        assert!(!report.on, "{report:?}");
        assert!(report.line.starts_with("This desktop's own notifications cannot be silenced by Sioul: GNOME keeps"), "{report:?}");
        assert!(report.offers.contains(&"desktop") && !report.consent.is_empty());
        assert!(layer.platform.desk.writes.borrow().is_empty());
        let mut sandboxed = gnome(true);
        sandboxed.desk.sandboxed = true;
        let mut layer = Layer::new(sandboxed, None);
        let report = layer.enter(&ask(Which::Paused), &en);
        assert!(!report.on && report.line.contains("Flatpak"), "{report:?}");
        assert!(layer.platform.desk.writes.borrow().is_empty());
    }

    #[test]
    fn gnome_switch_stays_on_until_the_last_pause_leaves() {
        let en = Translator::new("en");
        let mut layer = Layer::new(gnome(true), None);
        layer.enter(&ask(Which::FreeTime), &en);
        layer.enter(&ask(Which::Paused), &en);
        assert_eq!(*layer.platform.desk.writes.borrow(), vec![false], "switched once");
        layer.leave(Which::FreeTime, &en);
        assert_eq!(*layer.platform.desk.banners.borrow(), Some(false), "the other pause still wants it");
        layer.leave(Which::Paused, &en);
        assert_eq!(*layer.platform.desk.banners.borrow(), Some(true));
    }

    #[test]
    fn gnome_yes_taken_back_during_the_pause_switches_it_back_at_once() {
        let en = Translator::new("en");
        let mut layer = Layer::new(gnome(true), None);
        layer.enter(&ask(Which::Paused), &en);
        let mut no = ask(Which::Paused);
        no.desktop = false;
        let report = layer.enter(&no, &en);
        assert!(report.line.starts_with("GNOME's Do Not Disturb is off again, as it was before."), "{report:?}");
        assert_eq!(*layer.platform.desk.banners.borrow(), Some(true));
        assert!(!layer.kept.gnome);
        // Nothing more at the end.
        layer.leave(Which::Paused, &en);
        assert_eq!(*layer.platform.desk.writes.borrow(), vec![false, true]);
    }

    #[test]
    fn global_do_not_disturb_overlaps_the_pauses_and_leaves_them_alone() {
        let en = Translator::new("en");
        let mut layer = Layer::new(gnome(true), None);
        layer.enter(&ask(Which::Global), &en);
        layer.enter(&ask(Which::Paused), &en);
        layer.leave(Which::Global, &en);
        assert_eq!(*layer.platform.desk.banners.borrow(), Some(false), "the pause still wants GNOME's switch");
        layer.leave(Which::Paused, &en);
        assert_eq!(*layer.platform.desk.writes.borrow(), vec![false, true]);
        // Plasma: one inhibition each, lifted each on its own.
        let mut layer = Layer::new(plasma(true), None);
        layer.enter(&ask(Which::Global), &en);
        layer.enter(&ask(Which::FreeTime), &en);
        assert_eq!(layer.platform.desk.inhibited.load(Ordering::Relaxed), 2);
        layer.leave(Which::Global, &en);
        assert_eq!(layer.platform.desk.released.load(Ordering::Relaxed), 1);
        assert!(layer.kept.entered.iter().any(|a| a.which == Which::FreeTime));
    }

    #[test]
    fn gnome_switch_left_by_a_crash_is_restored_at_the_next_start() {
        let en = Translator::new("en");
        let dir = std::env::temp_dir().join(format!("sioul-dnd-{}-{}", std::process::id(), line!()));
        let path = dir.join("dnd.toml");
        let mut layer = Layer::new(gnome(true), Some(path.clone()));
        layer.enter(&ask(Which::Paused), &en);
        // Sioul ends without leaving; the pause ends meanwhile, elsewhere.
        let banners = Rc::clone(&layer.platform.desk.banners);
        drop(layer);
        let mut next = gnome(false);
        next.desk.banners = Rc::clone(&banners);
        let mut next = Layer::new(next, Some(path.clone()));
        let left = next.leave(Which::Paused, &en);
        assert_eq!(left.line, "GNOME's Do Not Disturb is off again, as it was before.");
        assert_eq!(*banners.borrow(), Some(true));
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn plasma(doses_shown: bool) -> Desktop<FakeDesk> {
        Desktop::new(FakeDesk { kind: Some(Kind::Inhibits { kde: true }), doses_shown, ..FakeDesk::default() })
    }

    #[test]
    fn plasma_inhibits_each_pause_and_lifts_only_its_own() {
        let en = Translator::new("en");
        let mut layer = Layer::new(plasma(true), None);
        let report = layer.enter(&ask(Which::Paused), &en);
        assert_eq!(report.line, "This desktop's notifications are silenced, with Plasma's do-not-disturb. Sioul's dose reminders still show.");
        layer.enter(&ask(Which::Paused), &en);
        assert_eq!(layer.platform.desk.inhibited.load(Ordering::Relaxed), 1, "once");
        layer.enter(&ask(Which::FreeTime), &en);
        assert_eq!(layer.platform.desk.inhibited.load(Ordering::Relaxed), 2, "one per pause");
        let left = layer.leave(Which::FreeTime, &en);
        assert_eq!(left.line, "This desktop's notifications show again.");
        assert_eq!(layer.platform.desk.released.load(Ordering::Relaxed), 1);
        // The connection lost (the session bus gone and back): asked again.
        layer.platform.desk.alive.store(false, Ordering::Relaxed);
        layer.enter(&ask(Which::Paused), &en);
        assert_eq!(layer.platform.desk.inhibited.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn plasma_hiding_critical_notifications_is_said_with_its_settings() {
        let en = Translator::new("en");
        let mut layer = Layer::new(plasma(false), None);
        let report = layer.enter(&ask(Which::Paused), &en);
        assert!(report.on && report.line.contains("cannot show here while it silences this desktop") && report.offers == vec!["plasma"], "{report:?}");
        let can = layer.can(&en);
        assert!(can.on && can.offers == vec!["plasma"], "{can:?}");
        // A pause that holds the doses anyway needs no word of it.
        let mut no_doses = ask(Which::FreeTime);
        no_doses.doses = false;
        let report = layer.enter(&no_doses, &en);
        assert_eq!(report.line, "This desktop's notifications are silenced, with Plasma's do-not-disturb.");
    }

    #[test]
    fn plasma_settings_say_whether_critical_notifications_show() {
        use super::desktop::{kconfig, shows_doses};
        // The owner's own file, as read on 6 October 2026.
        let owners = "[Applications][code]\nSeen=true\n\n[DoNotDisturb]\nWhenScreenSharing=false\n\n[Notifications]\nCriticalInDndMode=false\nLowPriorityPopups=false\n";
        assert_eq!(kconfig(owners, "[Notifications]", "CriticalInDndMode"), Some("false"));
        assert_eq!(kconfig(owners, "[Applications][com.aurelienpierre.Sioul]", "ShowPopupsInDndMode"), None);
        assert!(!shows_doses(Some("false"), None, None));
        // Plasma's default, and Sioul allowed in do-not-disturb.
        assert!(shows_doses(None, None, None));
        let allowed = "[Applications][com.aurelienpierre.Sioul]\nShowPopupsInDndMode[$i]=true\n[Notifications]\nCriticalInDndMode=false\n";
        assert!(shows_doses(kconfig(allowed, "[Notifications]", "CriticalInDndMode"), kconfig(allowed, "[Applications][com.aurelienpierre.Sioul]", "ShowPopupsInDndMode"), None));
        // Sioul's popups off altogether: nothing shows.
        assert!(!shows_doses(Some("true"), Some("true"), Some("false")));
    }

    #[test]
    fn other_desktops_and_windows_say_what_sioul_holds() {
        let en = Translator::new("en");
        let mut layer = Layer::new(Desktop::new(FakeDesk { kind: Some(Kind::Other("dunst".into())), ..FakeDesk::default() }), None);
        let report = layer.enter(&ask(Which::Paused), &en);
        assert_eq!(report.line, "This desktop's notifications cannot be silenced by Sioul: dunst gives applications no way to do it. Sioul holds its own notifications, except dose reminders.");
        let mut layer = Layer::new(WindowsPlatform, None);
        let report = layer.enter(&ask(Which::FreeTime), &en);
        assert!(!report.on && report.line.starts_with("This computer's notifications cannot be silenced by Sioul: Windows"), "{report:?}");
        assert_eq!(layer.leave(Which::FreeTime, &en).line, "");
    }

    /// The Shortcuts app as told.
    #[derive(Default)]
    struct FakeShortcuts {
        names: Vec<String>,
        ran: RefCell<Vec<String>>,
    }

    impl mac::Run for FakeShortcuts {
        fn list(&self) -> Option<Vec<String>> {
            Some(self.names.clone())
        }

        fn run(&self, name: &str) -> Result<(), String> {
            self.ran.borrow_mut().push(name.to_string());
            Ok(())
        }
    }

    #[test]
    fn mac_runs_the_persons_shortcuts_once_for_overlapping_pauses() {
        let en = Translator::new("en");
        let shortcuts = FakeShortcuts { names: vec![MAC_ON.into(), MAC_OFF.into()], ..FakeShortcuts::default() };
        let mut layer = Layer::new(mac::Mac { shortcuts }, None);
        assert_eq!(layer.enter(&ask(Which::FreeTime), &en).line, "Sioul ran your shortcut “Sioul pause on”.");
        layer.enter(&ask(Which::Paused), &en);
        layer.leave(Which::FreeTime, &en);
        assert_eq!(*layer.platform.shortcuts.ran.borrow(), vec![MAC_ON.to_string()], "the end waits for the last pause");
        assert_eq!(layer.leave(Which::Paused, &en).line, "Sioul ran your shortcut “Sioul pause off”.");
        assert_eq!(*layer.platform.shortcuts.ran.borrow(), vec![MAC_ON.to_string(), MAC_OFF.to_string()]);
        // Without the person's shortcut: said, nothing run.
        let mut layer = Layer::new(mac::Mac { shortcuts: FakeShortcuts::default() }, None);
        let report = layer.enter(&ask(Which::Paused), &en);
        assert!(!report.on && report.line.contains("“Sioul pause on”"), "{report:?}");
        assert!(layer.platform.shortcuts.ran.borrow().is_empty());
    }

    #[test]
    fn phone_answers_become_what_is_said() {
        let en = Translator::new("en");
        let entered = |ask: &Ask, answer: serde_json::Value| answers::entered(ask, &answer, "Pause").report(&en);
        let starred = serde_json::json!({ "access": true, "rule": { "calls": 3, "messages": 3, "repeat": 1, "alarms": 1 }, "doses": true });
        let report = entered(&ask(Which::Paused), starred.clone());
        assert_eq!(report.line, "Your phone's calls and messages are silenced, except starred contacts and repeat callers. Alarms and dose reminders still come.");
        assert!(report.on);
        // Doses held in that pause: silent, in the shade.
        let mut no_doses = ask(Which::Paused);
        no_doses.doses = false;
        assert_eq!(entered(&no_doses, starred).line, "Your phone's calls and messages are silenced, except starred contacts and repeat callers. Alarms still ring. Dose reminders wait in the notification shade, without a sound.");
        // Nobody, the doses' channel changed by the person, a do-not-disturb on before.
        let mut nobody = ask(Which::Paused);
        nobody.people = false;
        let answer = serde_json::json!({ "access": true, "rule": { "calls": 4, "messages": 4, "repeat": 2, "alarms": 1 }, "doses": false, "already": true });
        assert_eq!(entered(&nobody, answer).line, "Your phone's calls and messages are silenced, from everyone. Alarms still ring. Dose reminders are silenced too: their channel was changed in Android's settings. A do-not-disturb was already on; Sioul leaves it as it was.");
        // Set otherwise in Android's settings (Android 15's modes): said as it is.
        let edited = serde_json::json!({ "access": true, "rule": { "calls": 2, "messages": 4, "repeat": 1, "alarms": 2 }, "doses": true });
        let report = answers::entered(&ask(Which::Paused), &edited, "Pause").report(&en);
        assert!(report.line.starts_with("Your phone is silenced as its “Pause” mode is set in Android's settings. Alarms are silenced too"), "{report:?}");
        // No access: said, with Android's page offered, and asked again later.
        let done = answers::entered(&ask(Which::Paused), &serde_json::json!({ "access": false }), "Pause");
        assert!(done.retry && done.offers == vec!["access"]);
        assert_eq!(done.report(&en).line, "Your phone is not silenced: Sioul does not have Android's “Do Not Disturb access”.");
        // Too old, or no answer at all.
        assert_eq!(entered(&ask(Which::Paused), serde_json::json!({ "too_old": true })).line, "This phone's Android is too old for Sioul to silence it: it takes Android 10 or later.");
        assert!(answers::entered(&ask(Which::Paused), &serde_json::Value::Null, "Pause").retry);
        // Leaving: said only for what was on; another do-not-disturb still on is said.
        assert_eq!(answers::left(&serde_json::json!({ "was": false, "still": true })), Done::default());
        assert_eq!(answers::left(&serde_json::json!({ "was": true, "still": false })).report(&en).line, "Your phone is no longer silenced by Sioul.");
        assert_eq!(answers::left(&serde_json::json!({ "was": true, "still": true })).report(&en).line, "Your phone is no longer silenced by Sioul. A do-not-disturb is still on, as it was before.");
        // The mode turned off by the person, in Android's settings or during the pause: left off, said.
        let off = answers::entered(&ask(Which::Paused), &serde_json::json!({ "access": true, "disabled": true, "rule": { "name": "En pause" } }), "Pause");
        assert!(!off.on && !off.retry);
        assert_eq!(off.report(&en).line, "Your phone is not silenced: its “En pause” mode is turned off in Android's settings.");
        let off = answers::entered(&ask(Which::Paused), &serde_json::json!({ "access": true, "turned_off": true }), "Pause");
        assert_eq!(off.report(&en).line, "Do-not-disturb was turned off on the phone meanwhile; Sioul leaves it off.");
        // Setup: access missing, or given.
        let can = answers::can(&serde_json::json!({ "access": false }), None).report(&en);
        assert!(!can.on && can.offers == vec!["access", "starred"], "{can:?}");
        let can = answers::can(&serde_json::json!({ "access": true }), sioul_core::pause::callback("FR")).report(&en);
        assert!(can.on && can.line.ends_with("Emergency services may call back from a number you do not know (0 800 112 112): a second call within 15 minutes gets through, and you can star that number in your contacts."), "{can:?}");
        // Where no callback number is known, none is said.
        let can = answers::can(&serde_json::json!({ "access": true }), sioul_core::pause::callback("GB")).report(&en);
        assert!(can.line.ends_with("Emergency services may call back from a number you do not know: a second call within 15 minutes gets through."), "{can:?}");
    }

    #[test]
    fn a_phone_that_does_not_answer_is_said_and_asked_again() {
        let en = Translator::new("en");
        let mut layer = Layer::new(phone::Phone, None);
        let report = layer.enter(&ask(Which::Paused), &en);
        assert!(!report.on);
        assert_eq!(report.line, "Your phone could not be silenced: Android did not answer. Sioul holds its own notifications, except dose reminders.");
        assert!(layer.said.iter().all(|(_, _, retry)| *retry), "asked again at the next minute");
        assert_eq!(layer.leave(Which::Paused, &en), Report::default(), "nothing to say of what was not on");
    }

    #[test]
    fn every_sentence_has_words_in_each_language() {
        let all = [
            Said::PhoneCan,
            Said::PhoneNeedsAccess,
            Said::PhoneNoAccess,
            Said::PhoneTooOld,
            Said::PhoneCallback(Some("0 800 112 112".into())),
            Said::PhoneCallback(None),
            Said::PhoneOn(Senders::Starred),
            Said::PhoneOn(Senders::Nobody),
            Said::PhoneAsSet("Pause".into()),
            Said::PhoneNoAnswer,
            Said::PhoneDisabled("Pause".into()),
            Said::PhoneTurnedOff,
            Said::PhoneSetThere,
            Said::PhoneThrough { alarms: Some(true), doses: Doses::Through },
            Said::PhoneThrough { alarms: Some(false), doses: Doses::Through },
            Said::PhoneThrough { alarms: None, doses: Doses::Shade },
            Said::PhoneThrough { alarms: Some(true), doses: Doses::Blocked },
            Said::PhoneAlready,
            Said::PhoneFailed("x".into()),
            Said::PhoneOff,
            Said::PhoneStill,
            Said::PlasmaCan,
            Said::PlasmaOn,
            Said::PlasmaDoses(true),
            Said::PlasmaDoses(false),
            Said::PlasmaFailed("x".into()),
            Said::PlasmaOff,
            Said::GnomeCan,
            Said::GnomeCannot,
            Said::GnomeOn,
            Said::GnomeAlready,
            Said::GnomeSandboxed,
            Said::GnomeFailed("x".into()),
            Said::GnomeOff,
            Said::GnomeLeft,
            Said::DesktopCannot(String::new()),
            Said::DesktopCannot("dunst".into()),
            Said::MacCan,
            Said::MacCannot,
            Said::MacOn,
            Said::MacFailed(MAC_ON.into(), "x".into()),
            Said::MacOff,
            Said::MacNoOff,
            Said::WindowsCannot,
            Said::OwnHeld { doses: true },
            Said::OwnHeld { doses: false },
        ];
        for language in ["en", "fr"] {
            let tr = Translator::new(language);
            for said in &all {
                let words = sentence(said, &tr);
                assert!(!words.is_empty() && !words.contains("dnd-") && !words.contains('{'), "{language}: {said:?}: {words}");
            }
        }
        let fr = Translator::new("fr");
        assert_eq!(sentence(&Said::PhoneOn(Senders::Starred), &fr), "Les appels et messages de votre téléphone sont en silence, sauf ceux des contacts favoris et les appels répétés.");
        assert_eq!(Which::Paused.name(&fr), "En pause");
        assert_eq!(Which::FreeTime.name(&fr), "Temps libre");
        assert_eq!(Which::Global.name(&fr), "Ne pas déranger (Sioul)");
        assert_eq!(Which::Global.name(&Translator::new("en")), "Do not disturb (Sioul)");
    }

    /// GNOME's real switch through `gsettings`, against a settings file of the
    /// test's own (GSETTINGS_BACKEND=keyfile in a folder of its own): the
    /// person's own settings are never read or written. Skipped where
    /// `gsettings` or GNOME's schema is not installed.
    #[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
    #[test]
    fn gnome_switch_through_gsettings_in_a_file_of_the_tests_own() {
        let dir = std::env::temp_dir().join(format!("sioul-dnd-gsettings-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        // GIO without its virtual file systems: no gvfs daemon started for the test.
        let own = vec![
            ("GSETTINGS_BACKEND".to_string(), "keyfile".to_string()),
            ("XDG_CONFIG_HOME".to_string(), dir.to_string_lossy().to_string()),
            ("GIO_USE_VFS".to_string(), "local".to_string()),
        ];
        let mut desktop = Desktop::new(desktop::Session { gsettings_env: own, config_dirs: Some(vec![dir.clone()]) });
        if desktop.desk.banners() != Ok(true) {
            eprintln!("No gsettings or no GNOME schema here: skipped.");
            let _ = std::fs::remove_dir_all(&dir);
            return;
        }
        let en = Translator::new("en");
        let mut kept = Kept::default();
        let on = desktop.gnome_on(&ask(Which::Paused), &mut kept);
        assert!(on.on && kept.gnome, "{on:?}");
        assert_eq!(desktop.desk.banners(), Ok(false));
        let file = std::fs::read_to_string(dir.join("glib-2.0").join("settings").join("keyfile")).unwrap_or_default();
        assert!(file.contains("show-banners=false"), "written in the test's own file: {file}");
        let said = desktop.gnome_off(&[], &mut kept);
        assert_eq!(Done::said(false, said).report(&en).line, "GNOME's Do Not Disturb is off again, as it was before.");
        assert_eq!(desktop.desk.banners(), Ok(true));
        assert!(!kept.gnome);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
