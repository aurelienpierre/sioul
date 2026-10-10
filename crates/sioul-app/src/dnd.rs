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
//!   "Do not disturb (Sioul)"), letting through what the matrix of what
//!   reaches you lets through then, as far as Android can say it
//!   (`Ask::silence`: calls and messages from nobody, starred contacts,
//!   contacts or anyone, repeat callers, priority conversations), alarms,
//!   and Sioul's dose reminders and an event's alarms on channels that pass
//!   (android/package/src/com/aurelienpierre/sioul/PauseMode.java). Needs
//!   Android's "Do Not Disturb access".
//! - **Plasma**: its notification server's inhibition, held while the pause
//!   lasts (sioul-sync's dnd.rs); it ends with Sioul's process.
//! - **GNOME**: only the person's own switch exists; Sioul turns it on and off
//!   again only with their yes, given in the pause's settings.
//! - **macOS**: the person's own shortcuts. **Windows**: nothing an
//!   application may do.
//!
//! Never turns off a do-not-disturb the person set while Sioul's holds: only
//! what Sioul turned on is turned off then. Asked again unchanged, nothing is
//! redone (the pauses ask each minute); what was turned on is kept in the
//! state folder (dnd.toml), so that a crash is healed at the next start.
//!
//! Both ways (docs/do-not-disturb.md): each system's own do-not-disturb is
//! read (`Platform::system_on`) and followed (`listen`: Plasma's `Inhibited`, GNOME's
//! switch; Android's receiver in Java), and when Sioul's goes off on this
//! device, the system's own is turned off too where Sioul can (`system_off`:
//! Android 12 to 14, Plasma's own, GNOME's with the person's yes), or said
//! (`still_on`). A switch "on" taken from this device's own system adds no
//! mode of Sioul's over it (`Ask::stack`).

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
    /// Android: what the matrix of what reaches you lets through this mode
    /// (`attention::Silence`): calls and messages from nobody, starred
    /// contacts, contacts or anyone; a second call; important conversations;
    /// an event's alarms on a channel that passes. Given to Java beside
    /// `people` and `doses`, which say the same as far as today's Java reads.
    #[serde(default)]
    pub silence: Option<sioul_core::attention::Silence>,
    /// Sioul's own mode is put on the system; false when the system's own
    /// do-not-disturb holds the device for the switch already (a switch "on"
    /// taken from this device's own system, `everywhere::held_by_system`):
    /// then Sioul adds nothing over it, so that its end stays visible.
    #[serde(default = "yes")]
    pub stack: bool,
}

fn yes() -> bool {
    true
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
    /// The person turned this device's do-not-disturb off in its system, and
    /// Sioul leaves it off until it next turns it on (`on` false).
    pub turned_off: bool,
}

impl Report {
    /// For the window: {on, line, consent, turned_off, offers: [{key, label}]}.
    pub(crate) fn json(&self) -> serde_json::Value {
        let offers: Vec<serde_json::Value> = self.offers.iter().map(|key| serde_json::json!({ "key": key, "label": offer_label(key) })).collect();
        serde_json::json!({ "on": self.on, "line": self.line, "consent": self.consent, "turned_off": self.turned_off, "offers": offers })
    }
}

/// A change of a system's own do-not-disturb, heard as it happened: on or off.
pub(crate) type Told = std::sync::Arc<dyn Fn(bool) + Send + Sync>;

/// Sioul's do-not-disturb went off here: the system's own off too, once,
/// where Sioul can (`consent`: GNOME's yes). Whether something was turned off.
pub(crate) fn system_off(consent: bool) -> bool {
    with(|layer| layer.platform.system_off(consent))
}

/// While Sioul's do-not-disturb is off here: the system's own still on, said
/// ("… stays on: turn it off …"), with what may be opened; none when it is off.
pub(crate) fn still_on() -> Option<Report> {
    with(|layer| layer.platform.still_on().map(|done| done.report(tr())))
}

/// Sioul quits on a computer: whether its system stays silenced without it
/// (GNOME's switch, a Mac's Focus, which outlive Sioul); Plasma's inhibitions
/// end with Sioul's process.
pub(crate) fn still_after_quit() -> bool {
    with(|layer| layer.platform.outlives(&layer.kept))
}

/// "Silence this device again": `ask`'s mode left and entered afresh, the
/// person's earlier "off" in the system set aside.
pub(crate) fn again(ask: &Ask) -> Report {
    with(|layer| {
        layer.leave(ask.which, tr());
        layer.enter(ask, tr())
    })
}

/// This system's own do-not-disturb followed, each change heard as it happens
/// told to `told` (Plasma's `Inhibited`, GNOME's switch); on Android, Java's
/// receiver hears it. Once, while the window runs.
pub(crate) fn listen(told: Told) {
    with(|layer| layer.platform.listen(told));
}

/// Whether Sioul's do-not-disturb held on this device at the last apply; `on`,
/// what holds now, kept for the next (in the state folder, across restarts).
pub(crate) fn on_here(on: bool) -> bool {
    with(|layer| {
        let was = layer.kept.on_here;
        if was != on {
            layer.kept.on_here = on;
            layer.save();
        }
        was
    })
}

/// On Android, what Java keeps of Sioul's do-not-disturb here: its state (the
/// tile, the receiver's filter); nothing elsewhere.
pub(crate) fn tell_system(on: bool) {
    #[cfg(target_os = "android")]
    phone::call("flags", &serde_json::json!({ "on": on }).to_string());
    #[cfg(not(target_os = "android"))]
    let _ = on;
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
        "modes" => tr().text("dnd-offer-modes", None),
        _ => String::new(),
    }
}

/// One of `Report::offers`, opened: "access", "starred", "plasma", "modes".
pub(crate) fn open(key: &str) {
    match key {
        "access" => open_access(),
        "starred" => open_starred(),
        "plasma" => open_plasma(),
        "modes" => open_modes(),
        _ => {}
    }
}

/// Android's own do-not-disturb settings, where the person turns it off (Sioul
/// may not, from Android 15). Nothing elsewhere.
pub(crate) fn open_modes() {
    #[cfg(target_os = "android")]
    phone::call("open", "modes");
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
    /// Silenced, with these exceptions as the mode is set (read back from
    /// Android): who may call, who may write, a second call within 15
    /// minutes, the conversations marked priority in Android.
    PhoneLets { calls: Senders, messages: Senders, repeat: bool, conversations: bool },
    /// Android 10: no priority conversations to let through.
    PhoneNoConversations,
    /// An event's alarms during the mode: through (their channel passes), or silenced (changed in Android's settings).
    PhoneEvents(bool),
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
    /// Silenced by the phone's own do-not-disturb, which turned Sioul's on.
    PhoneHeld,
    /// Sioul's do-not-disturb off, the phone's own still on (Android 15 and later, or unheard).
    PhoneStaysOn,
    PhoneOff,
    PhoneStill,
    PlasmaCan,
    PlasmaOn,
    /// Whether Plasma shows Sioul's dose reminders during its do-not-disturb.
    PlasmaDoses(bool),
    PlasmaFailed(String),
    PlasmaOff,
    /// Plasma's do-not-disturb turned off from its applet (Sioul's inhibition dropped with it): left off.
    PlasmaTurnedOff,
    /// Silenced by Plasma's own do-not-disturb, which turned Sioul's on.
    PlasmaHeld,
    /// Sioul's do-not-disturb off, Plasma's own still on (`[DoNotDisturb] Until` ahead).
    PlasmaOwnStaysOn,
    /// Sioul's do-not-disturb off, Plasma holding notifications back for another application or a full-screen window.
    PlasmaStaysOn,
    GnomeCan,
    GnomeCannot,
    GnomeOn,
    GnomeAlready,
    GnomeSandboxed,
    GnomeFailed(String),
    GnomeOff,
    GnomeLeft,
    /// GNOME's Do Not Disturb turned off from its top bar while Sioul held it: left off.
    GnomeTurnedOff,
    /// Silenced by GNOME's own Do Not Disturb, which turned Sioul's on.
    GnomeHeld,
    /// Sioul's do-not-disturb off, GNOME's own still on (no yes to turn it off).
    GnomeStaysOn,
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

/// Who gets through the phone's mode, as Sioul sets it (`attention::Senders`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Senders {
    Starred,
    Nobody,
    Contacts,
    Anyone,
}

impl Senders {
    fn of(senders: sioul_core::attention::Senders) -> Senders {
        match senders {
            sioul_core::attention::Senders::None => Senders::Nobody,
            sioul_core::attention::Senders::Starred => Senders::Starred,
            sioul_core::attention::Senders::Contacts => Senders::Contacts,
            sioul_core::attention::Senders::Anyone => Senders::Anyone,
        }
    }
}

/// "calls from your contacts, messages from starred contacts, repeat
/// callers and priority conversations": what a phone's mode lets through.
fn lets(tr: &Translator, calls: Senders, messages: Senders, repeat: bool, conversations: bool) -> String {
    let from = |senders: Senders| match senders {
        Senders::Starred => tr.text("dnd-phone-from-starred", None),
        Senders::Contacts => tr.text("dnd-phone-from-contacts", None),
        Senders::Anyone => tr.text("dnd-phone-from-anyone", None),
        Senders::Nobody => String::new(),
    };
    let with = |key: &str, senders: Senders| {
        let mut args = i18n::args();
        args.set("from", from(senders));
        tr.text(key, Some(&args))
    };
    let mut parts = Vec::new();
    if calls == messages && calls != Senders::Nobody {
        parts.push(with("dnd-phone-lets-both", calls));
    } else {
        if calls != Senders::Nobody {
            parts.push(with("dnd-phone-lets-calls", calls));
        }
        if messages != Senders::Nobody {
            parts.push(with("dnd-phone-lets-messages", messages));
        }
    }
    if repeat {
        parts.push(tr.text("dnd-phone-lets-repeat", None));
    }
    if conversations {
        parts.push(tr.text("dnd-phone-lets-conversations", None));
    }
    let Some(last) = parts.pop() else { return tr.text("dnd-phone-nobody", None) };
    let what = if parts.is_empty() { last } else { format!("{} {} {last}", parts.join(", "), tr.text("word-and", None)) };
    let mut args = i18n::args();
    args.set("what", what);
    tr.text("dnd-phone-lets", Some(&args))
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
        Said::PhoneOn(senders) => lets(tr, *senders, *senders, true, false),
        Said::PhoneLets { calls, messages, repeat, conversations } => lets(tr, *calls, *messages, *repeat, *conversations),
        Said::PhoneNoConversations => tr.text("dnd-phone-no-conversations", None),
        Said::PhoneEvents(true) => tr.text("dnd-phone-events", None),
        Said::PhoneEvents(false) => tr.text("dnd-phone-events-blocked", None),
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
        Said::PhoneHeld => tr.text("dnd-phone-held", None),
        Said::PhoneStaysOn => tr.text("dnd-phone-stays-on", None),
        Said::PhoneOff => tr.text("dnd-phone-off", None),
        Said::PhoneStill => tr.text("dnd-phone-still", None),
        Said::PlasmaCan => tr.text("dnd-plasma-can", None),
        Said::PlasmaOn => tr.text("dnd-plasma-on", None),
        Said::PlasmaDoses(true) => tr.text("dnd-plasma-doses", None),
        Said::PlasmaDoses(false) => tr.text("dnd-plasma-doses-hidden", None),
        Said::PlasmaFailed(why) => with("dnd-plasma-failed", &[("why", why.as_str())]),
        Said::PlasmaOff => tr.text("dnd-plasma-off", None),
        Said::PlasmaTurnedOff => tr.text("dnd-plasma-turned-off", None),
        Said::PlasmaHeld => tr.text("dnd-plasma-held", None),
        Said::PlasmaOwnStaysOn => tr.text("dnd-plasma-own-stays-on", None),
        Said::PlasmaStaysOn => tr.text("dnd-plasma-stays-on", None),
        Said::GnomeCan => tr.text("dnd-gnome-can", None),
        Said::GnomeCannot => tr.text("dnd-gnome-cannot", None),
        Said::GnomeOn => tr.text("dnd-gnome-on", None),
        Said::GnomeAlready => tr.text("dnd-gnome-already", None),
        Said::GnomeSandboxed => tr.text("dnd-gnome-sandboxed", None),
        Said::GnomeFailed(why) => with("dnd-gnome-failed", &[("why", why.as_str())]),
        Said::GnomeOff => tr.text("dnd-gnome-off", None),
        Said::GnomeLeft => tr.text("dnd-gnome-left", None),
        Said::GnomeTurnedOff => tr.text("dnd-gnome-turned-off", None),
        Said::GnomeHeld => tr.text("dnd-gnome-held", None),
        Said::GnomeStaysOn => tr.text("dnd-gnome-stays-on", None),
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
    /// The person turned it off in the system; Sioul leaves it off.
    turned_off: bool,
}

impl Done {
    fn said(on: bool, said: Vec<Said>) -> Done {
        Done { on, said, ..Done::default() }
    }

    /// Turned off by the person in the system, said so.
    fn turned_off(said: Vec<Said>) -> Done {
        Done { said, turned_off: true, ..Done::default() }
    }

    fn report(self, tr: &Translator) -> Report {
        let line = self.said.iter().map(|s| sentence(s, tr)).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ");
        Report { on: self.on, line, offers: self.offers, consent: if self.consent { tr.text("dnd-gnome-consent", None) } else { String::new() }, turned_off: self.turned_off }
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
    /// This device's own do-not-disturb, as its system says it now; none
    /// where it cannot be read. (A computer's says it through `still_on`.)
    #[cfg_attr(not(any(target_os = "android", test)), allow(dead_code))]
    fn system_on(&mut self) -> Option<bool> {
        None
    }
    /// Sioul's do-not-disturb went off here: the system's own off too, once,
    /// where Sioul can (`consent`: GNOME's yes). Whether something was turned off.
    fn system_off(&mut self, _consent: bool) -> bool {
        false
    }
    /// While Sioul's do-not-disturb is off here: the system's own still on, said.
    fn still_on(&mut self) -> Option<Done> {
        None
    }
    /// Follows this system's own do-not-disturb, telling each change heard.
    fn listen(&mut self, _told: Told) {}
    /// Whether what Sioul turned on outlives Sioul's process (`kept`, as Sioul quits).
    fn outlives(&mut self, _kept: &Kept) -> bool {
        false
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
    /// Sioul's do-not-disturb held here at the last apply: its going off is
    /// when the system's own is turned off too (`system_off`). Before
    /// `entered`: TOML writes plain values before tables.
    #[serde(default)]
    on_here: bool,
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

    /// ZenPolicy's PEOPLE_TYPE_ANYONE, _CONTACTS, _STARRED, _NONE; STATE_ALLOW,
    /// STATE_DISALLOW; CONVERSATION_SENDERS_ANYONE, _IMPORTANT, _NONE.
    const ANYONE: i64 = 1;
    const CONTACTS: i64 = 2;
    const STARRED: i64 = 3;
    const NONE: i64 = 4;
    const ALLOW: i64 = 1;
    const DISALLOW: i64 = 2;
    const CONVERSATIONS_ANY: i64 = 1;
    const CONVERSATIONS_IMPORTANT: i64 = 2;
    const CONVERSATIONS_NONE: i64 = 3;

    /// What a mode lets through, as asked or as Android has it: who may call,
    /// who may write, a second call, the conversations marked priority (none:
    /// not said, Android 10 having none).
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Lets {
        calls: Senders,
        messages: Senders,
        repeat: bool,
        conversations: Option<bool>,
    }

    impl Lets {
        /// As the mode asks it: the matrix's (`Ask::silence`), else the older "people".
        fn asked(ask: &Ask) -> Lets {
            match ask.silence {
                Some(s) => Lets { calls: Senders::of(s.calls), messages: Senders::of(s.messages), repeat: s.repeat, conversations: Some(s.conversations) },
                None => {
                    let who = if ask.people { Senders::Starred } else { Senders::Nobody };
                    Lets { calls: who, messages: who, repeat: ask.people, conversations: Some(false) }
                }
            }
        }

        /// As Java read the mode back from Android; none when a part is not one Sioul sets.
        fn read(rule: &Value) -> Option<Lets> {
            let senders = |value: &Value| match value.as_i64() {
                Some(ANYONE) => Some(Senders::Anyone),
                Some(CONTACTS) => Some(Senders::Contacts),
                Some(STARRED) => Some(Senders::Starred),
                Some(NONE) => Some(Senders::Nobody),
                _ => None,
            };
            let repeat = match rule["repeat"].as_i64() {
                Some(ALLOW) => true,
                Some(DISALLOW) => false,
                _ => return None,
            };
            let conversations = match rule["conversations"].as_i64() {
                Some(CONVERSATIONS_ANY | CONVERSATIONS_IMPORTANT) => Some(true),
                Some(CONVERSATIONS_NONE) => Some(false),
                _ => None,
            };
            Some(Lets { calls: senders(&rule["calls"])?, messages: senders(&rule["messages"])?, repeat, conversations })
        }

        /// Whether `read` is this, as far as Android says it.
        fn holds(&self, read: &Lets) -> bool {
            self.calls == read.calls && self.messages == read.messages && self.repeat == read.repeat && (read.conversations.is_none() || read.conversations == self.conversations)
        }

        /// In words: today's two sentences where they fit, else the parts.
        fn said(&self) -> Said {
            let conversations = self.conversations.unwrap_or(false);
            match (self.calls, self.messages, self.repeat, conversations) {
                (Senders::Starred, Senders::Starred, true, false) => Said::PhoneOn(Senders::Starred),
                (Senders::Nobody, Senders::Nobody, false, false) => Said::PhoneOn(Senders::Nobody),
                (calls, messages, repeat, conversations) => Said::PhoneLets { calls, messages, repeat, conversations },
            }
        }
    }

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
        let mut said = vec![Said::PhoneCan, Said::PhoneCallback(callback)];
        // Android 10: no priority conversations, said where the modes are set up.
        if answer["api"].as_i64().is_some_and(|api| api < 30) {
            said.push(Said::PhoneNoConversations);
        }
        Done { on: true, said, offers: vec!["starred"], ..Done::default() }
    }

    /// `enter`: {api, access, too_old, error, rule: {name, calls, messages,
    /// repeat, alarms, conversations (Android 11 and later)}, doses, events,
    /// already, disabled, turned_off}; `name`, the mode's name as Sioul gives it.
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
        // Turned off by the person in Android's settings: Android's page offered, where it is turned on again.
        if answer["disabled"] == true {
            return Done { offers: vec!["modes"], ..Done::turned_off(vec![Said::PhoneDisabled(named)]) };
        }
        if answer["turned_off"] == true {
            return Done::turned_off(vec![Said::PhoneTurnedOff]);
        }
        // Held by the phone's own do-not-disturb, which turned Sioul's on: no mode of Sioul's over it.
        if answer["held"] == true {
            return Done::said(true, vec![Said::PhoneHeld]);
        }
        let asked = Lets::asked(ask);
        let mut said = match Lets::read(rule) {
            Some(read) if asked.holds(&read) => vec![read.said()],
            // Changed in Android's settings (its modes, from Android 15): said as it is.
            Some(read) => vec![read.said(), Said::PhoneSetThere],
            None => vec![Said::PhoneAsSet(named)],
        };
        // Android 10 has no priority conversations: Always through's come as messages there.
        if asked.conversations == Some(true) && answer["api"].as_i64().is_some_and(|api| api < 30) {
            said.push(Said::PhoneNoConversations);
        }
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
        // An event's alarms, asked through by the matrix and said by a Java that knows them.
        if ask.silence.is_some_and(|s| s.events) && answer.get("events").is_some() {
            said.push(Said::PhoneEvents(answer["events"] == true));
        }
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
    // SAFETY: declared as android/main.cpp defines them: extern "C", the same types.
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
            let mut asked = serde_json::json!({
                "kind": ask.which.key(),
                "name": ask.which.name(tr),
                "trigger": trigger,
                "stack": ask.stack,
                "people": ask.people,
                "doses": ask.doses,
                "channel": tr.text("dnd-doses-channel", None),
                "events_channel": tr.text("dnd-events-channel", None),
            });
            // What the matrix lets through this mode (docs/attention.md, §3.3), beside
            // "people" and "doses": {calls, messages: "none" | "starred" | "contacts" |
            // "anyone", repeat, conversations, alarms, doses, events}.
            if let (Some(silence), Some(fields)) = (ask.silence, asked.as_object_mut())
                && let Ok(serde_json::Value::Object(said)) = serde_json::to_value(silence)
            {
                fields.extend(said);
            }
            answers::entered(ask, &call("enter", &asked.to_string()), &ask.which.name(tr))
        }

        fn off(&mut self, which: Which, _others: &[Ask], _kept: &mut Kept) -> Done {
            answers::left(&call("leave", which.key()))
        }

        fn holds(&self, which: Which) -> bool {
            // Asked of Android at each look (a cheap question): a mode turned
            // off in the system, unheard, is said at the next minute.
            call("holds", which.key())["holds"] != false
        }

        fn system_on(&mut self) -> Option<bool> {
            call("own", "")["on"].as_bool()
        }

        fn system_off(&mut self, _consent: bool) -> bool {
            // Android 12 to 14 only: from 15, an application may end only a mode of its own.
            call("quiet-off", "")["done"] == true
        }

        fn still_on(&mut self) -> Option<Done> {
            (self.system_on() == Some(true)).then(|| Done { said: vec![Said::PhoneStaysOn], offers: vec!["modes"], ..Done::default() })
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
    use std::sync::{Arc, Mutex};
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
        /// The notification server's `Inhibited` now: Plasma's do-not-disturb, whatever its cause.
        fn inhibited(&self) -> Option<bool>;
        /// The end of Plasma's own do-not-disturb (`[DoNotDisturb] Until`, Unix seconds), when ahead.
        fn plasma_until(&self) -> Option<i64>;
        /// Plasma's own do-not-disturb ended: `Until` deleted, Plasma told.
        fn clear_plasma_until(&self) -> Result<(), String>;
        /// Follows the session's own do-not-disturb (Plasma's `Inhibited`,
        /// GNOME's switch), telling each change once Sioul's own settled (`quiet`).
        fn follow(&self, kind: &Kind, quiet: Arc<Mutex<Instant>>, told: Told) -> Option<Box<dyn Watching>>;
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

    /// The session's own do-not-disturb, followed.
    pub(super) trait Watching: Send {
        /// What was last read: on, off; none before it could be.
        fn seen(&self) -> Option<bool>;
    }

    /// A computer's desktop: Plasma's inhibitions held, by pause; GNOME's
    /// switch and what dconf says of it.
    pub(crate) struct Desktop<D: Desk> {
        pub(super) desk: D,
        /// Plasma's inhibitions held, by pause, and since when.
        pub(super) held: Vec<(Which, Box<dyn Held>, Instant)>,
        /// dconf's changes of GNOME's switch while Sioul holds it, and from
        /// when they are the person's.
        heard: Option<(Box<dyn Heard>, Instant)>,
        /// Set before each change of Sioul's own: what follows the session's
        /// do-not-disturb waits until then, so as never to take it for the person's.
        quiet: Arc<Mutex<Instant>>,
        /// The session's own do-not-disturb followed, while the window runs.
        pub(super) watch: Option<Box<dyn Watching>>,
    }

    impl<D: Desk + Default> Default for Desktop<D> {
        fn default() -> Self {
            Desktop::new(D::default())
        }
    }

    impl<D: Desk> Desktop<D> {
        pub(super) fn new(desk: D) -> Self {
            Desktop { desk, held: Vec::new(), heard: None, quiet: Arc::new(Mutex::new(Instant::now())), watch: None }
        }

        fn holding(&self, which: Which) -> bool {
            self.held.iter().any(|(w, held, _)| *w == which && held.alive())
        }

        /// Plasma's inhibition for `which` ended from its applet (the person
        /// turned do-not-disturb off there, which drops every application's),
        /// as the session's `Inhibited` says: off while Sioul holds one.
        pub(super) fn revoked(&self, which: Which) -> bool {
            let held = self.held.iter().any(|(w, held, since)| *w == which && held.alive() && since.elapsed() >= SETTLE);
            held && self.watch.as_ref().is_some_and(|watch| watch.seen() == Some(false))
        }

        /// A change of Sioul's own comes: what follows the session waits a moment.
        fn hush(&self) {
            if let Ok(mut quiet) = self.quiet.lock() {
                *quiet = Instant::now() + SETTLE;
            }
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
                // Turned off by the person meanwhile, unheard (Sioul closed): left off, said.
                if self.desk.banners() == Ok(true) {
                    return Done::turned_off(vec![Said::GnomeTurnedOff]);
                }
                return Done::said(true, vec![Said::GnomeOn]);
            }
            match self.desk.banners() {
                // Already on: the person's. Never turned off by Sioul.
                Ok(false) => Done::said(true, vec![Said::GnomeAlready]),
                Ok(true) => {
                    // Listening before the switch, so that its own word comes first.
                    let heard = self.desk.watch_banners();
                    self.hush();
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
                Ok(false) => match {
                    self.hush();
                    self.desk.set_banners(true)
                } {
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
                Kind::Gnome => Done { on: true, said: vec![Said::GnomeCan], offers: vec!["desktop"], consent: true, ..Done::default() },
                Kind::Other(name) => Done::said(false, vec![Said::DesktopCannot(name)]),
            }
        }

        fn on(&mut self, ask: &Ask, kept: &mut Kept, tr: &Translator) -> Done {
            match self.desk.kind() {
                Kind::Inhibits { kde } => {
                    if !ask.stack {
                        // Held by Plasma's own do-not-disturb, which turned Sioul's on:
                        // nothing of Sioul's over it, so that its end is heard.
                        if let Some(at) = self.held.iter().position(|(w, _, _)| *w == ask.which) {
                            self.hush();
                            let (_, held, _) = self.held.remove(at);
                            let _ = held.release();
                        }
                        return match self.desk.inhibited() {
                            // Its end not heard as it happened: said, as turned off here.
                            Some(false) => Done::turned_off(vec![Said::PlasmaTurnedOff]),
                            _ => Done::said(true, vec![Said::PlasmaHeld]),
                        };
                    }
                    if self.revoked(ask.which) {
                        // Ended from Plasma's applet, not heard as it happened: left off.
                        return Done::turned_off(vec![Said::PlasmaTurnedOff]);
                    }
                    if !self.holding(ask.which) {
                        self.held.retain(|(w, _, _)| *w != ask.which);
                        self.hush();
                        match self.desk.inhibit(&ask.which.name(tr)) {
                            Ok(held) => self.held.push((ask.which, held, Instant::now())),
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
                // Held by GNOME's own Do Not Disturb, which turned Sioul's on: nothing of Sioul's over it.
                Kind::Gnome if !ask.stack => match self.desk.banners() {
                    Ok(false) => Done::said(true, vec![Said::GnomeHeld]),
                    Ok(true) => Done::turned_off(vec![Said::GnomeTurnedOff]),
                    Err(why) => Done { said: vec![Said::GnomeFailed(why)], retry: true, ..Done::default() },
                },
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
            if let Some(at) = self.held.iter().position(|(w, _, _)| *w == which) {
                let (_, held, _) = self.held.remove(at);
                self.hush();
                // Refused: lifted already (the person, from Plasma's applet), or the server gone.
                let _ = held.release();
                said.push(Said::PlasmaOff);
            }
            said.extend(self.gnome_off(others, kept));
            Done::said(false, said)
        }

        fn system_on(&mut self) -> Option<bool> {
            match self.desk.kind() {
                Kind::Inhibits { .. } => self.desk.inhibited(),
                Kind::Gnome if !self.desk.sandboxed() => self.desk.banners().ok().map(|shown| !shown),
                _ => None,
            }
        }

        fn system_off(&mut self, consent: bool) -> bool {
            match self.desk.kind() {
                // Plasma's own do-not-disturb, as its applet sets it; the other
                // causes (an application's inhibition, a full-screen window) are not Sioul's to end.
                Kind::Inhibits { kde: true } if self.desk.plasma_until().is_some() => {
                    self.hush();
                    self.desk.clear_plasma_until().map_err(|why| eprintln!("sioul: do-not-disturb: {why}")).is_ok()
                }
                // GNOME's switch, with the person's yes.
                Kind::Gnome if consent && !self.desk.sandboxed() && self.desk.banners() == Ok(false) => {
                    self.hush();
                    self.desk.set_banners(true).map_err(|why| eprintln!("sioul: do-not-disturb: {why}")).is_ok()
                }
                _ => false,
            }
        }

        fn still_on(&mut self) -> Option<Done> {
            // What the follower last read, where it runs; else read now.
            let seen = self.watch.as_ref().and_then(|watch| watch.seen());
            match self.desk.kind() {
                Kind::Inhibits { .. } if self.held.is_empty() && seen.or_else(|| self.desk.inhibited()) == Some(true) => {
                    Some(Done::said(false, vec![if self.desk.plasma_until().is_some() { Said::PlasmaOwnStaysOn } else { Said::PlasmaStaysOn }]))
                }
                Kind::Gnome if !self.desk.sandboxed() && seen.or_else(|| self.desk.banners().ok().map(|shown| !shown)) == Some(true) => Some(Done::said(false, vec![Said::GnomeStaysOn])),
                _ => None,
            }
        }

        fn outlives(&mut self, kept: &Kept) -> bool {
            // GNOME's switch stays as Sioul set it; Plasma's inhibitions end with Sioul.
            matches!(self.desk.kind(), Kind::Gnome) && kept.gnome && self.desk.banners() == Ok(false)
        }

        fn listen(&mut self, told: Told) {
            if self.watch.is_none() {
                let kind = self.desk.kind();
                self.watch = self.desk.follow(&kind, Arc::clone(&self.quiet), told);
            }
        }

        fn holds(&self, which: Which) -> bool {
            // An inhibition's connection gone (the session bus lost): asked again;
            // one ended from Plasma's applet: said so.
            self.held.iter().all(|(w, held, _)| *w != which || held.alive()) && !self.revoked(which)
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

    /// A date and time as KConfig writes it ("2027,10,7,22,13,41.5": year,
    /// month, day, hour, minute, seconds, in local time `zone`, or with a time
    /// zone as a seventh part): Unix seconds; none when it does not read.
    pub(super) fn kconfig_time(text: &str, zone: &jiff::tz::TimeZone) -> Option<i64> {
        let parts: Vec<&str> = text.split(',').map(str::trim).collect();
        if parts.len() < 6 {
            return None;
        }
        let part = |at: usize| parts[at].parse::<i64>().ok();
        let (year, month, day, hour, minute) = (part(0)?, part(1)?, part(2)?, part(3)?, part(4)?);
        let seconds = parts[5].parse::<f64>().ok().filter(|s| s.is_finite() && *s >= 0.0)?;
        let civil = jiff::civil::DateTime::new(
            i16::try_from(year).ok()?,
            i8::try_from(month).ok()?,
            i8::try_from(day).ok()?,
            i8::try_from(hour).ok()?,
            i8::try_from(minute).ok()?,
            i8::try_from(seconds.trunc() as i64).ok()?,
            0,
        )
        .ok()?;
        let zone = match parts.get(6).filter(|id| !id.is_empty()) {
            Some(id) => jiff::tz::TimeZone::get(id).ok()?,
            None => zone.clone(),
        };
        civil.to_zoned(zone).ok().map(|at| at.timestamp().as_second())
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
    #[derive(Default, Clone)]
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

        fn inhibited(&self) -> Option<bool> {
            // Tests, in a folder of their own: never the person's session.
            if self.config_dirs.is_some() {
                return None;
            }
            sioul_sync::dnd::inhibited()
        }

        fn plasma_until(&self) -> Option<i64> {
            let until = kconfig_time(&self.plasma("[DoNotDisturb]", "Until")?, &jiff::tz::TimeZone::system())?;
            (until > jiff::Timestamp::now().as_second()).then_some(until)
        }

        fn clear_plasma_until(&self) -> Result<(), String> {
            // KDE's own tool, as Plasma writes its settings; `--notify` tells
            // Plasma at once (its settings reload the group live). Tests write a
            // folder of their own, and tell nobody.
            let mut command = std::process::Command::new("kwriteconfig6");
            command.args(["--file", "plasmanotifyrc", "--group", "DoNotDisturb", "--key", "Until", "--delete"]);
            match self.config_dirs.as_ref().and_then(|dirs| dirs.first()) {
                Some(dir) => {
                    command.env("XDG_CONFIG_HOME", dir);
                }
                None => {
                    command.arg("--notify");
                }
            }
            let out = command.arg("").output().map_err(|e| format!("kwriteconfig6: {e}"))?;
            if out.status.success() {
                return Ok(());
            }
            let said = String::from_utf8_lossy(&out.stderr).trim().to_string();
            Err(if said.is_empty() { format!("kwriteconfig6: {}", out.status) } else { said })
        }

        fn follow(&self, kind: &Kind, quiet: Arc<Mutex<Instant>>, told: Told) -> Option<Box<dyn Watching>> {
            // Tests write files of their own, which nothing on the bus tells of.
            if self.config_dirs.is_some() || !self.gsettings_env.is_empty() {
                return None;
            }
            match kind {
                Kind::Inhibits { .. } => sioul_sync::dnd::watch_inhibited(quiet, Box::new(move |on| told(on))).ok().map(|watch| Box::new(watch) as Box<dyn Watching>),
                Kind::Gnome if !self.sandboxed() => {
                    let seen = Arc::new(Mutex::new(self.banners().ok().map(|shown| !shown)));
                    let (session, noted, reading) = (self.clone(), Arc::clone(&seen), Arc::new(Mutex::new(())));
                    let heard = move || {
                        let (session, noted, reading, told, quiet) = (session.clone(), Arc::clone(&noted), Arc::clone(&reading), Arc::clone(&told), Arc::clone(&quiet));
                        // Read once the change held a second and Sioul's own change settled.
                        std::thread::spawn(move || {
                            std::thread::sleep(Duration::from_secs(1));
                            while let Some(wait) = quiet.lock().ok().map(|until| until.saturating_duration_since(Instant::now())).filter(|wait| !wait.is_zero()) {
                                std::thread::sleep(wait);
                            }
                            let _one = reading.lock();
                            let Ok(shown) = session.banners() else { return };
                            let now = !shown;
                            let before = noted.lock().ok().and_then(|mut seen| seen.replace(now));
                            if before.is_some_and(|before| before != now) {
                                told(now);
                            }
                        });
                    };
                    let changes = sioul_sync::dnd::watch_dconf_told(BANNERS_PATH, Box::new(heard)).ok()?;
                    Some(Box::new(GnomeWatch { _changes: changes, seen }))
                }
                _ => None,
            }
        }
    }

    /// GNOME's switch followed: dconf's word of each change, and what was read then.
    #[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
    struct GnomeWatch {
        _changes: sioul_sync::dnd::Changes,
        seen: Arc<Mutex<Option<bool>>>,
    }

    #[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
    impl Watching for GnomeWatch {
        fn seen(&self) -> Option<bool> {
            self.seen.lock().ok().and_then(|seen| *seen)
        }
    }

    #[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
    impl Watching for sioul_sync::dnd::Watch {
        fn seen(&self) -> Option<bool> {
            sioul_sync::dnd::Watch::seen(self)
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

        fn outlives(&mut self, kept: &Kept) -> bool {
            // The Focus the person's shortcut turned on stays until "Sioul pause off" runs.
            kept.mac
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
    use super::desktop::{Desk, Desktop, Heard, Held, Kind, Watching};
    use super::*;
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;
    use std::sync::{Arc, Mutex};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::time::Instant;

    fn ask(which: Which) -> Ask {
        Ask { which, people: true, doses: true, desktop: true, silence: None, stack: true }
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
        layer.kept.on_here = true;
        layer.save();
        // A crash: the next run knows the pause was entered, and what Sioul turned on.
        let next = Layer::new(Counting::default(), Some(path.clone()));
        assert_eq!(next.kept.entered, vec![ask(Which::Paused)]);
        assert!(next.kept.gnome);
        assert!(next.kept.on_here, "do-not-disturb held here, for its going off: {}", std::fs::read_to_string(&path).unwrap_or_default());
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
        /// What Plasma's server says of `Inhibited`; its own do-not-disturb's
        /// end; how often Sioul ended it.
        server: Rc<Cell<Option<bool>>>,
        until: Rc<Cell<Option<i64>>>,
        cleared: Rc<Cell<u32>>,
    }

    /// The session's do-not-disturb as last read, told.
    struct FakeWatch(Option<bool>);

    impl Watching for FakeWatch {
        fn seen(&self) -> Option<bool> {
            self.0
        }
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

        fn inhibited(&self) -> Option<bool> {
            self.server.get()
        }

        fn plasma_until(&self) -> Option<i64> {
            self.until.get()
        }

        fn clear_plasma_until(&self) -> Result<(), String> {
            self.cleared.set(self.cleared.get() + 1);
            self.until.set(None);
            Ok(())
        }

        fn follow(&self, _kind: &Kind, _quiet: Arc<Mutex<Instant>>, _told: Told) -> Option<Box<dyn Watching>> {
            None
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
        // A file as Plasma writes it, critical notifications hidden in do-not-disturb.
        let owners = "[DoNotDisturb]\nWhenScreenSharing=false\n\n[Notifications]\nCriticalInDndMode=false\nLowPriorityPopups=false\n";
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
        assert_eq!(report.line, "Your phone is silenced, except calls from your contacts and repeat callers. That mode is set otherwise in Android's settings, which win. Alarms are silenced too, as that mode is set. Dose reminders still show.");
        // A part Sioul never sets (unset): the mode named, as set there.
        let unset = serde_json::json!({ "access": true, "rule": { "calls": 0, "messages": 4, "repeat": 1, "alarms": 2 }, "doses": true });
        assert!(entered(&ask(Which::Paused), unset).line.starts_with("Your phone is silenced as its “Pause” mode is set in Android's settings. Alarms are silenced too"));
        // The matrix's own (docs/attention.md, §3.3): this phone screens calls, so its contacts' calls
        // ring through the mode; messages from the starred; priority conversations; an event's alarms.
        use sioul_core::attention::{Senders as Matrix, Silence};
        let silence = Silence { calls: Matrix::Contacts, messages: Matrix::Starred, repeat: true, conversations: true, alarms: true, doses: true, events: true };
        let matrix = Ask { silence: Some(silence), ..ask(Which::Paused) };
        let read = serde_json::json!({ "api": 31, "access": true, "rule": { "calls": 2, "messages": 3, "repeat": 1, "alarms": 1, "conversations": 2 }, "doses": true, "events": true });
        assert_eq!(entered(&matrix, read).line, "Your phone is silenced, except calls from your contacts, messages from starred contacts, repeat callers and priority conversations. Alarms and dose reminders still come. Your events' alarms ring too.");
        // Android 10: no priority conversations (none read back), said; an older Java: no word of events.
        let ten = serde_json::json!({ "api": 29, "access": true, "rule": { "calls": 2, "messages": 3, "repeat": 1, "alarms": 1 }, "doses": true });
        assert_eq!(entered(&matrix, ten).line, "Your phone is silenced, except calls from your contacts, messages from starred contacts and repeat callers. This phone's Android has no priority conversations (they came with Android 11): the conversations that always get through come here as messages do. Alarms and dose reminders still come.");
        // Everyone, and an event alarms' channel changed by the person.
        let anyone = Ask { silence: Some(Silence { calls: Matrix::Anyone, messages: Matrix::Anyone, ..silence }), ..ask(Which::FreeTime) };
        let read = serde_json::json!({ "api": 35, "access": true, "rule": { "calls": 1, "messages": 1, "repeat": 1, "alarms": 1, "conversations": 2, "channels": 1 }, "doses": true, "events": false });
        assert_eq!(entered(&anyone, read).line, "Your phone is silenced, except calls and messages from anyone, repeat callers and priority conversations. Alarms and dose reminders still come. Your events' alarms are silenced too: their channel was changed in Android's settings.");
        // Nothing let through by the matrix: from everyone, as before.
        let none = Ask { silence: Some(Silence { calls: Matrix::None, messages: Matrix::None, repeat: false, conversations: false, ..silence }), ..ask(Which::FreeTime) };
        let read = serde_json::json!({ "api": 31, "access": true, "rule": { "calls": 4, "messages": 4, "repeat": 2, "alarms": 1, "conversations": 3 }, "doses": true, "events": true });
        assert!(entered(&none, read).line.starts_with("Your phone's calls and messages are silenced, from everyone. Alarms and dose reminders still come."));
        // Priority conversations alone (the starred no more): asked so, said so.
        let read = serde_json::json!({ "api": 31, "access": true, "rule": { "calls": 4, "messages": 4, "repeat": 2, "alarms": 1, "conversations": 2 }, "doses": true });
        let alone = Ask { silence: Some(Silence { conversations: true, ..none.silence.unwrap() }), ..ask(Which::Paused) };
        assert!(entered(&alone, read).line.starts_with("Your phone is silenced, except priority conversations. Alarms"));
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
        assert!(off.turned_off && !off.on);
        assert_eq!(off.report(&en).line, "You turned do-not-disturb off on this phone; Sioul leaves it off until it turns it on again.");
        // Turned off in Android's settings: said, with Android's page to turn it on again there.
        let disabled = answers::entered(&ask(Which::Paused), &serde_json::json!({ "access": true, "disabled": true, "rule": { "name": "Pause" } }), "Pause");
        assert!(disabled.turned_off && disabled.offers == vec!["modes"]);
        // Held by the phone's own do-not-disturb, which turned Sioul's on: silenced, nothing of Sioul's over it.
        let held = answers::entered(&ask(Which::Global), &serde_json::json!({ "access": true, "held": true }), "Do not disturb (Sioul)").report(&en);
        assert!(held.on && !held.turned_off);
        assert_eq!(held.line, "This phone is silenced by its own do-not-disturb, as set in Android's settings, which turned Sioul's on.");
        // Setup: access missing, or given.
        let can = answers::can(&serde_json::json!({ "access": false }), None).report(&en);
        assert!(!can.on && can.offers == vec!["access", "starred"], "{can:?}");
        let can = answers::can(&serde_json::json!({ "access": true }), sioul_core::pause::callback("FR")).report(&en);
        assert!(can.on && can.line.ends_with("Emergency services may call back from a number you do not know (0 800 112 112): a second call within 15 minutes gets through, and you can star that number in your contacts."), "{can:?}");
        // Where no callback number is known, none is said.
        let can = answers::can(&serde_json::json!({ "access": true }), sioul_core::pause::callback("GB")).report(&en);
        assert!(can.line.ends_with("Emergency services may call back from a number you do not know: a second call within 15 minutes gets through."), "{can:?}");
        // Android 10, said where the modes are set up; Android 12, nothing more.
        let can = answers::can(&serde_json::json!({ "api": 29, "access": true }), None).report(&en);
        assert!(can.line.ends_with("This phone's Android has no priority conversations (they came with Android 11): the conversations that always get through come here as messages do."), "{can:?}");
        let can = answers::can(&serde_json::json!({ "api": 31, "access": true }), None).report(&en);
        assert!(!can.line.contains("priority"), "{can:?}");
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
            Said::PhoneOn(Senders::Contacts),
            Said::PhoneLets { calls: Senders::Contacts, messages: Senders::Starred, repeat: true, conversations: true },
            Said::PhoneLets { calls: Senders::Anyone, messages: Senders::Anyone, repeat: false, conversations: false },
            Said::PhoneLets { calls: Senders::Nobody, messages: Senders::Contacts, repeat: false, conversations: true },
            Said::PhoneLets { calls: Senders::Nobody, messages: Senders::Nobody, repeat: false, conversations: false },
            Said::PhoneNoConversations,
            Said::PhoneEvents(true),
            Said::PhoneEvents(false),
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
            Said::PhoneHeld,
            Said::PhoneStaysOn,
            Said::PlasmaTurnedOff,
            Said::PlasmaHeld,
            Said::PlasmaOwnStaysOn,
            Said::PlasmaStaysOn,
            Said::GnomeTurnedOff,
            Said::GnomeHeld,
            Said::GnomeStaysOn,
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
        assert_eq!(
            sentence(&Said::PhoneLets { calls: Senders::Contacts, messages: Senders::Starred, repeat: true, conversations: true }, &fr),
            "Votre téléphone est en silence, sauf les appels de vos contacts, les messages des contacts favoris, les appels répétés et les conversations prioritaires."
        );
        assert_eq!(Which::Paused.name(&fr), "En pause");
        assert_eq!(Which::FreeTime.name(&fr), "Temps libre");
        assert_eq!(Which::Global.name(&fr), "Ne pas déranger (Sioul)");
        assert_eq!(Which::Global.name(&Translator::new("en")), "Do not disturb (Sioul)");
    }

    #[test]
    fn plasma_held_by_its_own_adds_nothing_and_its_end_unheard_is_said() {
        let en = Translator::new("en");
        let mut layer = Layer::new(plasma(true), None);
        layer.platform.desk.server.set(Some(true));
        let held = Ask { stack: false, ..ask(Which::Global) };
        let report = layer.enter(&held, &en);
        assert!(report.on && report.line == "This computer is silenced by Plasma's own do-not-disturb, which turned Sioul's on.", "{report:?}");
        assert_eq!(layer.platform.desk.inhibited.load(Ordering::Relaxed), 0, "nothing of Sioul's over it");
        // Sioul's own mode first (another reason), then held: Sioul's taken away.
        let mut layer = Layer::new(plasma(true), None);
        layer.platform.desk.server.set(Some(true));
        layer.enter(&ask(Which::Global), &en);
        layer.enter(&held, &en);
        assert_eq!(layer.platform.desk.released.load(Ordering::Relaxed), 1);
        // Its end not heard as it happened (Sioul busy, closed): said as turned off here.
        layer.platform.desk.server.set(Some(false));
        let report = layer.enter(&Ask { doses: false, ..held.clone() }, &en);
        assert!(!report.on && report.turned_off, "{report:?}");
        assert!(report.line.starts_with("You turned Plasma's do-not-disturb off on this computer"), "{report:?}");
    }

    #[test]
    fn plasma_inhibition_ended_from_its_applet_is_said_and_never_asked_again() {
        let en = Translator::new("en");
        let mut layer = Layer::new(plasma(true), None);
        layer.enter(&ask(Which::Global), &en);
        assert_eq!(layer.platform.desk.inhibited.load(Ordering::Relaxed), 1);
        // A while later, the session says Plasma's do-not-disturb is off: the applet dropped it.
        for held in &mut layer.platform.held {
            held.2 = Instant::now().checked_sub(std::time::Duration::from_secs(5)).unwrap_or(held.2);
        }
        layer.platform.watch = Some(Box::new(FakeWatch(Some(false))));
        let report = layer.enter(&ask(Which::Global), &en);
        assert!(!report.on && report.turned_off, "{report:?}");
        assert_eq!(layer.platform.desk.inhibited.load(Ordering::Relaxed), 1, "not asked again over the person's off");
        // Left, then asked anew (Sioul's next "on"): inhibited again.
        layer.leave(Which::Global, &en);
        layer.platform.watch = Some(Box::new(FakeWatch(Some(true))));
        assert!(layer.enter(&ask(Which::Global), &en).on);
        assert_eq!(layer.platform.desk.inhibited.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn sioul_off_turns_the_systems_own_off_where_it_can_and_says_the_rest() {
        let en = Translator::new("en");
        // Plasma: its own do-not-disturb ended; another cause is not Sioul's to end.
        let mut layer = Layer::new(plasma(true), None);
        layer.platform.desk.until.set(Some(4_000_000_000));
        assert!(layer.platform.system_off(false));
        assert_eq!(layer.platform.desk.cleared.get(), 1);
        layer.platform.desk.server.set(Some(true));
        let still = layer.platform.still_on().map(|done| done.report(&en).line);
        assert_eq!(still.as_deref(), Some("Plasma keeps this computer's notifications back for another application or a full-screen window; it ends with them."));
        assert!(!layer.platform.system_off(false), "nothing more of Sioul's to end");
        layer.platform.desk.server.set(Some(false));
        assert_eq!(layer.platform.still_on(), None);
        // GNOME: with the person's yes only.
        let mut layer = Layer::new(gnome(false), None);
        assert_eq!(layer.platform.system_on(), Some(true));
        assert!(!layer.platform.system_off(false), "no yes, no write");
        assert_eq!(layer.platform.still_on().map(|done| done.report(&en).line).as_deref(), Some("GNOME's Do Not Disturb stays on: turn it off in the top bar."));
        assert!(layer.platform.system_off(true));
        assert_eq!(*layer.platform.desk.banners.borrow(), Some(true));
        assert_eq!(layer.platform.still_on(), None);
        // A phone that does not answer, Windows: nothing read, nothing done.
        let mut phone = phone::Phone;
        assert_eq!((phone.system_on(), phone.system_off(true), phone.still_on()), (None, false, None));
        let mut windows = WindowsPlatform;
        assert_eq!((windows.system_on(), windows.system_off(true)), (None, false));
    }

    #[test]
    fn gnome_held_by_its_own_and_turned_off_while_sioul_was_closed() {
        let en = Translator::new("en");
        // Held by GNOME's own switch: nothing written.
        let mut layer = Layer::new(gnome(false), None);
        let report = layer.enter(&Ask { stack: false, ..ask(Which::Global) }, &en);
        assert!(report.on && report.line == "This computer is silenced by GNOME's own Do Not Disturb, which turned Sioul's on.", "{report:?}");
        assert!(layer.platform.desk.writes.borrow().is_empty());
        // Sioul turned it on, then was closed; the person turned it off meanwhile: said, left off.
        let mut layer = Layer::new(gnome(true), None);
        layer.kept.gnome = true;
        let report = layer.enter(&ask(Which::Paused), &en);
        assert!(!report.on && report.turned_off && report.line.starts_with("You turned GNOME's Do Not Disturb off"), "{report:?}");
        assert!(layer.platform.desk.writes.borrow().is_empty(), "not turned on again over the person's off");
    }

    #[test]
    fn what_outlives_sioul_as_it_quits() {
        let en = Translator::new("en");
        // Plasma's inhibition ends with Sioul's process.
        let mut layer = Layer::new(plasma(true), None);
        layer.enter(&ask(Which::Global), &en);
        assert!(!layer.platform.outlives(&layer.kept));
        // GNOME's switch, as Sioul set it, stays.
        let mut layer = Layer::new(gnome(true), None);
        layer.enter(&ask(Which::Global), &en);
        assert!(layer.platform.outlives(&layer.kept));
        // The person's own GNOME switch is not Sioul's to report.
        let mut layer = Layer::new(gnome(false), None);
        layer.enter(&ask(Which::Global), &en);
        assert!(!layer.platform.outlives(&layer.kept));
    }

    #[test]
    fn again_leaves_and_enters_afresh() {
        let en = Translator::new("en");
        let mut layer = Layer::new(plasma(true), None);
        layer.enter(&ask(Which::Global), &en);
        layer.leave(Which::Global, &en);
        let report = layer.enter(&ask(Which::Global), &en);
        assert!(report.on);
        assert_eq!((layer.platform.desk.inhibited.load(Ordering::Relaxed), layer.platform.desk.released.load(Ordering::Relaxed)), (2, 1));
    }

    #[test]
    fn plasma_dates_read_as_kconfig_writes_them() {
        use super::desktop::kconfig_time;
        let paris = jiff::tz::TimeZone::get("Europe/Paris").unwrap();
        // Local time, as the applet writes "until turned off" (a year ahead).
        assert_eq!(kconfig_time("2027,10,7,22,13,41.5", &paris), Some(jiff::civil::date(2027, 10, 7).at(22, 13, 41, 0).to_zoned(paris.clone()).unwrap().timestamp().as_second()));
        // With a zone of its own, the seventh part.
        assert_eq!(kconfig_time("2027,1,1,0,0,0,UTC", &paris), Some(1_798_761_600));
        assert_eq!(kconfig_time("", &paris), None);
        assert_eq!(kconfig_time("2027,13,1,0,0,0", &paris), None);
        assert_eq!(kconfig_time("not,a,date,at,all,x", &paris), None);
    }

    /// Plasma's own do-not-disturb ended with `kwriteconfig6`, in a folder of
    /// the test's own, telling nobody: the person's settings are never read nor
    /// written. Skipped where `kwriteconfig6` is not installed.
    #[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
    #[test]
    fn plasma_until_is_ended_with_kdes_own_tool_in_a_folder_of_the_tests_own() {
        let dir = std::env::temp_dir().join(format!("sioul-dnd-kwrite-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("plasmanotifyrc");
        std::fs::write(&file, "[DoNotDisturb]\nUntil=2099,1,1,0,0,0\nWhenScreenSharing=false\n").unwrap();
        let session = desktop::Session { gsettings_env: Vec::new(), config_dirs: Some(vec![dir.clone()]) };
        assert!(session.plasma_until().is_some_and(|until| until > 4_000_000_000));
        match session.clear_plasma_until() {
            Err(why) if why.starts_with("kwriteconfig6:") && why.contains("No such file") => {
                eprintln!("No kwriteconfig6 here: skipped.");
                let _ = std::fs::remove_dir_all(&dir);
                return;
            }
            done => done.expect("ended"),
        }
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(!text.contains("Until") && text.contains("WhenScreenSharing=false"), "{text}");
        assert_eq!(session.plasma_until(), None);
        let _ = std::fs::remove_dir_all(&dir);
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
