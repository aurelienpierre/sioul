// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Meetings you organize, by your own acts in the window (decisions 7 and 8
//! of the event card, 5 November 2026): guests invited from the event's
//! form, taken off it, the meeting changed, moved on the timeline, given a
//! status, deleted or one time of it left out, and a guest's answer read
//! from their REPLY.
//!
//! Every write here goes through the organizer's door
//! (`vdir::write_organized`, `remove_organized`), which this window alone
//! opens (`agenda::open_organizer_door`) and which refuses a meeting someone
//! else organizes. This file is the door's one caller: agents (`sioul mcp`),
//! the command line and the sync go through the ordinary doors, which refuse
//! every meeting with others (`agenda` tests,
//! `the_organizers_door_is_the_persons_alone`).
//!
//! **Who tells the guests**, per meeting and per guest, read from its file
//! (`sioul_core::organizer::teller`; the owner's word, 6 November 2026): a
//! guest left to you (`SCHEDULE-AGENT=CLIENT`, every guest of a meeting made
//! in Sioul) by a draft in Mail, which leaves when you send it; one not left
//! to you (a meeting made elsewhere) by the calendar's server when it
//! schedules by itself for your address there (`sioul_sync::scheduling`),
//! else by a draft too. Never both for one guest and one change.

use crate::backend::{say, tr};
use sioul_core::agenda;
use sioul_core::agentwrites::FileChange;
use sioul_core::compose::Draft;
use sioul_core::organizer::{self, Guest, Sender};
use sioul_core::vdir::{self, Collection, Kind};
use std::path::{Path, PathBuf};

/// You as the organizer of an event in `calendar`: the address its server
/// schedules for, when it names one of yours; else the calendar's account's
/// address, else your first. Your name from that account, else any.
pub(crate) fn organizer_for(calendar: &Collection) -> Option<Guest> {
    let config = crate::backend::load_config();
    let own = agenda::own_addresses();
    let account = config.every_account().find(|a| a.id == calendar.account);
    let address_of_account = account.and_then(|a| a.address.clone()).map(|a| a.trim().to_lowercase()).filter(|a| !a.is_empty());
    let known = sioul_sync::scheduling::of(&calendar.account).map(|s| s.addresses).unwrap_or_default();
    let address = address_of_account
        .clone()
        .filter(|a| known.is_empty() || known.contains(a))
        .or_else(|| known.iter().find(|a| own.contains(*a)).cloned())
        .or(address_of_account)
        .or_else(|| own.first().cloned())?;
    let name = account.and_then(|a| a.name.clone()).filter(|n| !n.trim().is_empty()).or_else(|| config.every_account().find_map(|a| a.name.clone().filter(|n| !n.trim().is_empty()))).unwrap_or_default();
    Some(Guest { address, name })
}

/// Who tells the guests of a meeting in `calendar` you organize as
/// `organizer` that are not left to you (a meeting made elsewhere): its
/// server, when its server said, in its last clear answer, that it schedules
/// by itself for that address, and that the calendar is yours; your drafts,
/// when it said it does not schedule for that address, and for a calendar
/// of this device's own, carried by the sharing or not, which no server
/// holds (second review of the organizer's door, N10). Unknown, so that a
/// write that would tell such a guest is refused, never guessed (nobody told
/// twice nor not at all: review of 1e8551e, F1): while its server never
/// answered clearly; for a calendar whose owner it does not say (F18); for a
/// calendar shared with you, whose server schedules for you there or not by
/// its version (Nextcloud from version 30 schedules for the person it is
/// shared with, its sources say; 29 does not: second review, N4); and for a
/// Google calendar, unless its server answers the check clearly that it
/// schedules (Google's CalDAV says nothing of it in its documentation). The
/// guests left to you need none of this: your drafts tell them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Teller {
    Server,
    Drafts,
    Unknown,
}

pub(crate) fn server_for(calendar: &Collection, organizer: &str) -> Teller {
    if calendar.account == vdir::LOCAL {
        return Teller::Drafts;
    }
    let google = sioul_core::capabilities::provider_of_collection(crate::backend::load_config().account(&calendar.account), calendar) == sioul_core::capabilities::Provider::Google;
    let Some(said) = sioul_sync::scheduling::of(&calendar.account) else { return Teller::Unknown };
    if !said.auto || !said.addresses.iter().any(|a| a.eq_ignore_ascii_case(organizer.trim())) {
        return if google { Teller::Unknown } else { Teller::Drafts };
    }
    match vdir::State::load(&vdir::state_path(&calendar.account, Kind::Calendars, &calendar.id)).owned {
        Some(true) => Teller::Server,
        _ => Teller::Unknown,
    }
}

/// The guests the form gives, as written: addresses trimmed, each once,
/// never one of yours, only what reads as an address.
pub(crate) fn cleaned(given: &[Guest]) -> Vec<Guest> {
    let own = agenda::own_addresses();
    let mut out: Vec<Guest> = Vec::new();
    for guest in given {
        let address = guest.address.trim().trim_start_matches("mailto:").trim().to_string();
        let lower = address.to_lowercase();
        let looks = lower.split_once('@').is_some_and(|(user, domain)| !user.is_empty() && domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.')) && !lower.chars().any(|c| c.is_whitespace() || c.is_control() || "<>\",;:()[]\\".contains(c));
        if !looks || own.contains(&lower) || out.iter().any(|g| g.address.eq_ignore_ascii_case(&address)) {
            continue;
        }
        out.push(Guest { address, name: guest.name.trim().to_string() });
    }
    out
}

/// What a write's message to its guests says, when it is told by a draft
/// (`drafts_of`): an invitation or its update (RFC 5546 §3.2.2), or a
/// cancellation (§3.2.5), of the whole meeting or of one time of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Says {
    #[default]
    Invitation,
    /// The meeting's status set to Cancelled (`status_plan`).
    Cancelled,
    /// The time starting then left out (`time_plan`): its guests alone are told, by a CANCEL naming it.
    TimeCancelled(i64),
}

/// What writing a meeting you organize does, worked out before it is
/// written, and said before you are asked (`asked`).
#[derive(Debug, Clone)]
pub(crate) struct Plan {
    pub path: PathBuf,
    pub before: Option<String>,
    pub text: String,
    pub calendar: Collection,
    /// Who tells its guests not left to you (`server_for`).
    pub server: Teller,
    /// The guests the form lists, as written.
    pub guests: Vec<Guest>,
    pub added: Vec<Guest>,
    pub removed: Vec<Guest>,
    /// Some guest kept must hear of it: the times of it they are asked to
    /// changed, or its title, place, notes or status.
    #[cfg_attr(not(test), allow(dead_code))]
    pub told: bool,
    /// The guests kept who must hear of it (`organizer::told_guest`): the
    /// series' and those asked to some times of it alone (another program
    /// put them there: second review of the organizer's door, N10).
    pub told_guests: Vec<Guest>,
    /// A change a scheduling server sends by itself (`organizer::significant`).
    pub significant: bool,
    /// The server-told guests kept, asked `SCHEDULE-FORCE-SEND=REQUEST` for
    /// this write: a change it would not send by itself (review of 1e8551e, F3).
    pub forced: Vec<String>,
    /// The organizer an event alone named, whom you replace by inviting
    /// guests to it (review of 1e8551e, F12): said before you are asked.
    pub replaced: Option<Guest>,
    /// What its file held that is yours alone, moved into your own files as
    /// it is written (`organizer::set_aside`; review of 1e8551e, F5): the
    /// file names guests, so it holds only what they may read.
    pub aside: organizer::Aside,
    /// `aside.own` is what the form set (the event's form): it replaces what
    /// your own file kept of it; else it goes there only when nothing is.
    pub own_given: bool,
    /// A child's event this write makes a meeting: this device keeps an
    /// entry of its own lines (`meeting_lines::ensure`), even of none, and
    /// so knows who takes it (third review of the organizer's door, R1).
    pub known: bool,
    /// The meeting this write makes no meeting any more: its own lines kept
    /// apart went back into `text`, and their entry goes once it is written.
    pub lines_back: bool,
    /// What its drafts say (`Says`).
    pub says: Says,
    /// Its unsent cancellations, by id, that the status going back from
    /// Cancelled takes out of Drafts once it is written (`status_plan`): what
    /// they say no longer holds, and their guests heard nothing of it.
    pub withdrawn: Vec<String>,
}

/// Those of `guests` told by drafts and those told by the server, apart.
type Apart = (Vec<Guest>, Vec<Guest>);

impl Plan {
    /// Who receives an invitation (an update, for those asked before): the
    /// new guests, and those kept who must hear of it (`told_guests`).
    fn invited(&self) -> Vec<Guest> {
        let is_in = |list: &[Guest], g: &Guest| list.iter().any(|h| h.address.eq_ignore_ascii_case(&g.address));
        // In the form's order, then those asked to some times alone.
        let mut out: Vec<Guest> = self.guests.iter().filter(|g| is_in(&self.added, g) || is_in(&self.told_guests, g)).cloned().collect();
        out.extend(self.told_guests.iter().filter(|g| !is_in(&self.guests, g)).cloned().collect::<Vec<_>>());
        out
    }

    /// Whether `guest`, of `text`, is told by the server (`organizer::teller`).
    fn by_server(&self, text: &str, guest: &Guest) -> bool {
        self.server == Teller::Server && organizer::teller(text, &guest.address, Sender::Server) == Sender::Server
    }

    /// The text whose lines say who tells the guests it invites: what it
    /// writes; for one time left out, the file as it was, since the guests of
    /// that time alone go with it and their lines said how they are told.
    fn marks(&self) -> &str {
        match self.says {
            Says::TimeCancelled(_) => self.before.as_deref().unwrap_or(&self.text),
            _ => &self.text,
        }
    }

    /// The invited apart (`invited`): by drafts, by the server. A guest the
    /// server tells hears from it: invited, their invitation; kept, a change
    /// it sends by itself, or one asked of it (`forced`); a change it cannot
    /// be asked for (a guest of one time alone, whose force it would never
    /// take out of its copy) is drafted for them instead, never both.
    fn invited_apart(&self) -> Apart {
        let (mut drafts, mut served) = (Vec::new(), Vec::new());
        for guest in self.invited() {
            let new = self.added.iter().any(|g| g.address.eq_ignore_ascii_case(&guest.address));
            let reached = new || self.significant || self.forced.iter().any(|f| f.eq_ignore_ascii_case(&guest.address));
            if self.by_server(self.marks(), &guest) && reached { served.push(guest) } else { drafts.push(guest) }
        }
        (drafts, served)
    }

    /// Those taken off apart, as their lines said before: by drafts, by the server.
    fn removed_apart(&self) -> Apart {
        let before = self.before.as_deref().unwrap_or(&self.text);
        self.removed.iter().cloned().partition(|g| !self.by_server(before, g))
    }

    /// Whether anyone hears of it.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn tells(&self) -> bool {
        !self.invited().is_empty() || !self.removed.is_empty()
    }

    /// Whether the plan needs to know who tells the guests not left to you,
    /// and does not (`Teller::Unknown`): then it is refused (`plan`).
    fn unknown(&self) -> bool {
        if self.server != Teller::Unknown {
            return false;
        }
        let before = self.before.as_deref().unwrap_or(&self.text);
        let not_left = |text: &str, g: &Guest| organizer::teller(text, &g.address, Sender::Server) == Sender::Server;
        self.invited().iter().any(|g| not_left(self.marks(), g)) || self.removed.iter().any(|g| not_left(before, g))
    }

    /// A guest it would tell who is left to you on some of their lines and
    /// not on others, where its server schedules (`organizer::mixed`): its
    /// server would tell them of some times, your draft of the rest; a guest
    /// told once is the rule, so it is refused (second review of the
    /// organizer's door, N10).
    fn told_two_ways(&self) -> Option<Guest> {
        if self.server != Teller::Server {
            return None;
        }
        let before = self.before.as_deref().unwrap_or(&self.text);
        let (now, then) = (organizer::mixed(&self.text), organizer::mixed(before));
        let mixed = |g: &Guest| now.iter().chain(&then).any(|m| m.eq_ignore_ascii_case(&g.address));
        self.invited().into_iter().chain(self.removed.iter().cloned()).find(|g| mixed(g))
    }

    /// What the window says before writing it, so that you choose (the
    /// owner's words, 6 November 2026): "Sioul drafts the messages to Anne;
    /// you send them from Mail." and "Your server tells Bob.", per guest;
    /// "" when nobody hears of it.
    pub(crate) fn asked(&self) -> String {
        let names = |guests: &[Guest]| guests.iter().map(Guest::said).collect::<Vec<_>>().join(", ");
        let mut said = Vec::new();
        if let Some(organizer) = self.replaced.as_ref().filter(|_| !self.added.is_empty()) {
            said.push(say("invite-ask-organizer", &[("who", organizer.said())]));
        }
        let ((mut drafts, mut served), (off_drafts, off_served)) = (self.invited_apart(), self.removed_apart());
        drafts.extend(off_drafts);
        served.extend(off_served);
        if !drafts.is_empty() {
            said.push(say("invite-ask-drafts", &[("who", names(&drafts))]));
        }
        if !served.is_empty() {
            said.push(say("invite-ask-served", &[("who", names(&served))]));
        }
        said.join(" ")
    }

    /// What the status line says once it is written: how its guests were told.
    pub(crate) fn done(&self) -> String {
        let ((drafts, served), (off_drafts, off_served)) = (self.invited_apart(), self.removed_apart());
        let mut said = Vec::new();
        if !drafts.is_empty() || !off_drafts.is_empty() {
            said.push(tr().text("invite-done-draft", None));
        }
        if !served.is_empty() || !off_served.is_empty() {
            said.push(tr().text("invite-done-server", None));
        }
        said.join(" ")
    }
}

/// The plan of a write to the event at `path` (made there when it is new):
/// `before` its file as it is, `applied` its text as the form or a drag
/// made it, guests aside; `guests` those the form lists (none: as its file
/// has them); `calendar` where it is. You are its organizer as it says, or
/// as `organizer_for` makes you; refused when it is someone else's, when it
/// would tell a guest not left to you while nobody knows yet whether its
/// server does (`Teller::Unknown`), and when it would tell a guest marked
/// two ways where its server schedules (`Plan::told_two_ways`). A force an
/// earlier write asked of its server (`SCHEDULE-FORCE-SEND`) stays in the
/// file while its server has not had that write (second review of the
/// organizer's door, N1); once the sync agreed on the file as it is
/// (`vdir::agreed_hash`), its server sent that change and takes the force
/// out of its copy, so this write carries none of it, an ETag said or not
/// (third review, R3).
///
/// Sioul's own lines of a child's event (who takes it, its kind) follow it
/// (R1): a write that makes it a meeting keeps an entry of them here, even
/// of none (`Plan::known`); one that makes it no meeting any more puts the
/// lines kept here back into its file (`Plan::lines_back`), and is refused
/// when this device keeps none while a stand-in of it is in your agendas
/// (`children::held_elsewhere`): the lines are on another device.
pub(crate) fn plan(path: &Path, before: Option<&str>, applied: &str, guests: Option<&[Guest]>, calendar: &Collection) -> Result<Plan, String> {
    let own = agenda::own_addresses();
    if before.is_some_and(|b| agenda::meeting_with_others(b, &own) && !agenda::organized_by_you(b, &own)) {
        return Err(tr().text("event-change-with-others", None));
    }
    // Its server has the file as it is: a force it held was sent, never asked again (R3).
    let pushed = before.is_some_and(|b| vdir::agreed_hash(path) == Some(vdir::content_hash(b.as_bytes())));
    let applied = if pushed { organizer::without_force_send(applied) } else { applied.to_string() };
    let applied = applied.as_str();
    let organizer = before
        .and_then(organizer::organizer_of)
        .filter(|o| own.contains(&o.address.to_lowercase()))
        .or_else(|| organizer_for(calendar))
        .ok_or_else(|| tr().text("invite-no-address", None))?;
    let server = server_for(calendar, &organizer.address);
    let had = before.map(organizer::guests_of).unwrap_or_default();
    let guests = guests.map_or_else(|| had.clone(), cleaned);
    // New guests marked as its others are.
    let marks = organizer::marks_new_guests(before.unwrap_or(applied));
    let text = organizer::with_guests(applied, &organizer, &guests, marks).ok_or_else(|| tr().text("agenda-gone", None))?;
    // Someone else named as its organizer, with nobody invited yet: you take their place, said first (F12).
    let replaced = before.and_then(organizer::organizer_of).filter(|o| !o.address.trim().is_empty() && !own.contains(&o.address.trim().to_lowercase()));
    // A meeting's file holds what its guests may read alone; yours goes into your own files (F5, N6).
    let meeting = agenda::meeting_with_others(&text, &own);
    let was = before.is_some_and(|b| agenda::meeting_with_others(b, &own));
    let (text, aside) = if meeting { organizer::set_aside(&text) } else { (text, organizer::Aside::default()) };
    // No meeting any more: Sioul's own lines kept here back into it (R1); none here while
    // your agendas hold a stand-in of it, they are on another device: refused.
    let entry = sioul_core::meeting_lines::load(&sioul_core::meeting_lines::default_path()).events.get(&sioul_core::meeting_lines::uid_of(&text)).cloned();
    let lines_back = was && !meeting && entry.is_some();
    if was && !meeting && entry.is_none() && crate::children::held_elsewhere(path) {
        return Err(tr().text("invite-holder-elsewhere", None));
    }
    let text = match entry.filter(|_| lines_back) {
        Some(kept) => sioul_core::meeting_lines::folded(&text, &kept),
        None => text,
    };
    let known = meeting && !was && crate::children::in_child_agenda(path);
    let is_in = |list: &[Guest], g: &Guest| list.iter().any(|h| h.address.eq_ignore_ascii_case(&g.address));
    let added: Vec<Guest> = guests.iter().filter(|g| !is_in(&had, g)).cloned().collect();
    let removed: Vec<Guest> = had.iter().filter(|g| !is_in(&guests, g)).cloned().collect();
    // Who must hear of it: each guest kept whose times of it changed, the series' and those of one time alone (N10).
    let told_of = |g: &Guest| before.is_some_and(|b| organizer::told_guest(b, &text, &g.address));
    let kept: Vec<Guest> = guests.iter().filter(|g| is_in(&had, g) && told_of(g)).cloned().collect();
    let earlier = before.map(organizer::every_guest).unwrap_or_default();
    let one_time: Vec<Guest> = organizer::one_time_guests(&text).into_iter().filter(|g| is_in(&earlier, g) && told_of(g)).collect();
    let told = !kept.is_empty() || !one_time.is_empty();
    // A cancelled meeting stays as its guests heard it: any change they would see goes after
    // another status, which tells them it is on again. A server that schedules would send every
    // later write of it as one more cancellation (sabre's broker, for its status).
    let cancelled = |t: &str| agenda::status_of(t) == "cancelled";
    if before.is_some_and(cancelled) && cancelled(&text) && (told || !added.is_empty() || !removed.is_empty()) {
        return Err(tr().text("invite-cancelled-first", None));
    }
    let says = if before.is_some_and(|b| !cancelled(b)) && cancelled(&text) { Says::Cancelled } else { Says::Invitation };
    let significant = before.is_some_and(|b| organizer::significant(b, &text));
    // A change its server would not send, to the series' kept guests it tells: asked of it for this write (F3).
    let (text, forced) = if told && !significant && server == Teller::Server {
        let to: Vec<String> = kept.iter().filter(|g| organizer::teller(&text, &g.address, Sender::Server) == Sender::Server).map(|g| g.address.clone()).collect();
        organizer::with_force_send(&text, &to)
    } else {
        (text, Vec::new())
    };
    let mut told_guests = kept;
    told_guests.extend(one_time);
    let plan = Plan { path: path.to_path_buf(), before: before.map(str::to_string), text, calendar: calendar.clone(), server, guests, added, removed, told, told_guests, significant, forced, replaced, aside, own_given: false, known, lines_back, says, withdrawn: Vec::new() };
    if plan.unknown() {
        return Err(tr().text("invite-server-unknown", None));
    }
    if let Some(guest) = plan.told_two_ways() {
        return Err(say("invite-marks-mixed", &[("who", guest.said())]));
    }
    Ok(plan)
}

/// The plan of a meeting you organize given a status, from the event card's
/// chooser: "confirmed" or "tentative" a new invitation at a later SEQUENCE,
/// "cancelled" a cancellation (RFC 5546 §2.1.4, §3.2.5), each guest told as
/// their lines say (`plan`); refused as `plan` refuses. None when it says so
/// already.
///
/// Going back from Cancelled, its cancellation still unsent (review of
/// f85f504, M6): that draft is withdrawn, since it says what no longer holds,
/// and a guest it was to tell hears nothing more when the status is the one
/// they last heard (`CancelNote::was`). Unknown (cancelled on another device,
/// its draft brought here by the sharing), they are told: one message more
/// rather than none.
pub(crate) fn status_plan(path: &Path, before: &str, status: &str, calendar: &Collection) -> Result<Option<Plan>, String> {
    let text = organizer::with_status(before, status).ok_or_else(|| tr().text("agenda-gone", None))?;
    if text == before {
        return Ok(None);
    }
    let mut plan = plan(path, Some(before), &text, None, calendar)?;
    if agenda::status_of(before) == "cancelled" {
        let uid = sioul_core::meeting_lines::uid_of(before);
        let waiting = organizer::unsent_cancellations(&uid, &agenda::own_addresses());
        // No status said is what Confirmed says to a guest.
        let word = |status: &str| if status.is_empty() { "confirmed".to_string() } else { status.trim().to_ascii_lowercase() };
        let heard_the_same = organizer::cancellation_notes(&uid).into_iter().any(|n| n.time.is_none() && word(&n.was) == word(status));
        if heard_the_same {
            plan.told_guests.retain(|g| !waiting.iter().any(|p| p.to.iter().any(|t| t.eq_ignore_ascii_case(g.address.trim()))));
            plan.told = !plan.told_guests.is_empty();
        }
        plan.withdrawn = waiting.into_iter().map(|p| p.id).collect();
    }
    Ok(Some(plan))
}

/// The plan of one time of a repeating meeting you organize left out, the
/// time starting at `start`: left out of the series (EXDATE), its own event
/// taken out if it has one (`organizer::without_time`), and the guests of that
/// time told, those asked to it alone included: by a CANCEL naming it where
/// your drafts tell them (`organizer::cancel_time`), by its server where it
/// does, each as their lines said (`Plan::marks`). A guest excused from that
/// time gets no draft; one of the series its server tells hears from it all
/// the same, since an EXDATE is a change it sends the series' guests by itself,
/// and is named first (review of f85f504, L4). Refused as `plan` refuses, and
/// when no time starts there.
pub(crate) fn time_plan(path: &Path, before: &str, start: i64, calendar: &Collection) -> Result<Plan, String> {
    let zone = sioul_core::zone::current();
    let text = organizer::without_time(before, start, &zone).ok_or_else(|| tr().text("agenda-gone", None))?;
    let mut plan = plan(path, Some(before), &text, None, calendar)?;
    plan.says = Says::TimeCancelled(start);
    plan.told_guests = organizer::time_guests(before, start, &zone);
    if plan.significant {
        let excused: Vec<Guest> = organizer::guests_of(before).into_iter().filter(|g| !plan.told_guests.iter().any(|t| t.address.eq_ignore_ascii_case(&g.address)) && plan.by_server(before, g)).collect();
        plan.told_guests.extend(excused);
    }
    plan.told = !plan.told_guests.is_empty();
    if plan.unknown() {
        return Err(tr().text("invite-server-unknown", None));
    }
    if let Some(guest) = plan.told_two_ways() {
        return Err(say("invite-marks-mixed", &[("who", guest.said())]));
    }
    Ok(plan)
}

/// What a plan sets aside (`Plan::aside`), kept in your own files before its
/// meeting is written: your own time around it (`own_time`), the form's
/// replacing what was there, else only where nothing is; its ties among
/// yours (`links::LocalLinks`), which every view of ties reads; Sioul's
/// other lines of it (`meeting_lines`: who takes a child's event, its kind),
/// which their readers read there (second review of the organizer's door,
/// N6), their entry made even of none when `known` (`Plan::known`).
fn keep_aside(text: &str, aside: &organizer::Aside, given: bool, known: bool) -> Result<(), String> {
    let uid = sioul_core::lines::unfold(text).iter().find(|l| sioul_core::lines::name(l) == "UID").map(|l| sioul_core::lines::value(l).trim().to_string()).unwrap_or_default();
    if uid.is_empty() {
        return Ok(());
    }
    if let Some(own) = &aside.own {
        let path = sioul_core::own_time::default_path();
        if given || !sioul_core::own_time::load(&path).events.contains_key(&uid) {
            sioul_core::own_time::set(&path, &uid, Some(own.clone()))?;
        }
    }
    sioul_core::links::LocalLinks::add_all(&sioul_core::links::LocalLinks::default_path(), &aside.ties)?;
    sioul_core::meeting_lines::add(&sioul_core::meeting_lines::default_path(), &uid, &aside.lines)?;
    if known {
        sioul_core::meeting_lines::ensure(&sioul_core::meeting_lines::default_path(), &uid)?;
    }
    // Its entry knows the meeting it is kept for: a file older than that, or of a lower SEQUENCE, is behind it (H1).
    sioul_core::meeting_lines::stamped_as(&sioul_core::meeting_lines::default_path(), &uid, text)?;
    Ok(())
}

/// The account a draft for `calendar` goes out from: the calendar's own
/// when it sends mail, else the one of your address as organizer, else your
/// first that sends mail.
fn sending_account(calendar: &Collection, organizer: &str) -> Result<sioul_core::config::Account, String> {
    let config = crate::backend::load_config();
    config
        .accounts
        .iter()
        .find(|a| a.id == calendar.account && a.syncs())
        .or_else(|| config.accounts.iter().find(|a| a.syncs() && a.address.as_deref().is_some_and(|x| x.trim().eq_ignore_ascii_case(organizer))))
        .or_else(|| config.accounts.iter().find(|a| a.syncs()))
        .cloned()
        .ok_or_else(|| tr().text("mail-no-account", None))
}

/// A draft for `to` carrying `ics` (iMIP, RFC 6047), its words by `cancel` (`organizer::draft`).
fn draft(account: &sioul_core::config::Account, to: &[Guest], text: &str, ics: String, cancel: bool) -> Draft {
    organizer::draft(&account.id, account.signature.as_deref(), to, text, ics, cancel, tr())
}

/// The drafts a plan asks for, for the guests told by drafts
/// (`Plan::invited_apart`, `removed_apart`): the invitation, to every guest
/// or to the new ones, the times they are asked to alone for those of some
/// times of it (`organizer::request_for`), and the cancellation, to those
/// taken off; the meeting or one time of it cancelled, that cancellation
/// alone (`Says`). None for the guests the server tells.
fn drafts_of(plan: &Plan) -> Result<Vec<Draft>, String> {
    let organizer = organizer::organizer_of(&plan.text).map(|o| o.address).unwrap_or_default();
    let mut out = Vec::new();
    if plan.says != Says::Invitation {
        let before = plan.before.as_deref().unwrap_or(&plan.text);
        let (drafted, _) = plan.invited_apart();
        if drafted.is_empty() {
            return Ok(out);
        }
        let account = sending_account(&plan.calendar, &organizer)?;
        let to: Vec<String> = drafted.iter().map(|g| g.address.clone()).collect();
        let gone = || tr().text("agenda-gone", None);
        out.push(match plan.says {
            Says::TimeCancelled(start) => organizer::time_draft(&account.id, account.signature.as_deref(), &drafted, organizer::cancel_time(before, start, &to, &sioul_core::zone::current()).ok_or_else(gone)?, tr()),
            _ => draft(&account, &drafted, before, organizer::cancel(before, &to, true).ok_or_else(gone)?, true),
        });
        return Ok(out);
    }
    let ((invited, _), (removed, _)) = (plan.invited_apart(), plan.removed_apart());
    let series = organizer::guests_of(&plan.text);
    let (whole, apart): (Vec<Guest>, Vec<Guest>) = invited.into_iter().partition(|g| series.iter().any(|s| s.address.eq_ignore_ascii_case(&g.address)));
    if !whole.is_empty() {
        let account = sending_account(&plan.calendar, &organizer)?;
        let ics = organizer::request(&plan.text).ok_or_else(|| tr().text("agenda-gone", None))?;
        out.push(draft(&account, &whole, &plan.text, ics, false));
    }
    if !apart.is_empty() {
        let account = sending_account(&plan.calendar, &organizer)?;
        let ics = organizer::request_for(&plan.text, &apart.iter().map(|g| g.address.clone()).collect::<Vec<_>>()).ok_or_else(|| tr().text("agenda-gone", None))?;
        out.push(draft(&account, &apart, &plan.text, ics, false));
    }
    if !removed.is_empty() {
        let account = sending_account(&plan.calendar, &organizer)?;
        let to: Vec<String> = removed.iter().map(|g| g.address.clone()).collect();
        let ics = organizer::cancel(plan.before.as_deref().unwrap_or(&plan.text), &to, false).ok_or_else(|| tr().text("agenda-gone", None))?;
        out.push(draft(&account, &removed, &plan.text, ics, true));
    }
    Ok(out)
}

/// What a write through the door did: the file as before and after (for
/// Undo), and the drafts it made, by id, with their guests' addresses.
#[derive(Debug, Clone, Default)]
pub(crate) struct Written {
    pub file: Option<FileChange>,
    pub drafts: Vec<String>,
    /// Each draft's guests (by its id), as their addresses: who heard of it once it left Drafts.
    pub to: std::collections::BTreeMap<String, Vec<String>>,
    /// The guests its server told (a change, an invitation, a cancellation):
    /// what Undo tells them back.
    pub served: Vec<Guest>,
    /// It made the meeting no meeting, its own lines kept here put back into
    /// its file (`Plan::lines_back`): Undo keeps them apart again (R1).
    pub lines_back: bool,
    /// The unsent cancellations it took out of Drafts (`Plan::withdrawn`), and
    /// the notes of cancellations its text no longer stands for
    /// (`organizer::cancellations_settled`): Undo puts both back.
    pub withdrawn: Vec<Draft>,
    pub settled: Vec<organizer::CancelNote>,
}

/// A draft's guests' addresses, in lower case.
fn addresses(draft: &Draft) -> Vec<String> {
    draft.to.iter().map(|t| sioul_core::compose::address_of(t).unwrap_or_else(|| t.trim().to_string()).to_lowercase()).collect()
}

/// A plan written: through the organizer's door, then its drafts saved; a
/// draft that cannot be saved puts the file back, and says why.
pub(crate) fn write(plan: &Plan) -> Result<Written, String> {
    let drafts = drafts_of(plan)?;
    keep_aside(&plan.text, &plan.aside, plan.own_given, plan.known)?;
    vdir::write_organized(&plan.path, &plan.text)?;
    let (mut ids, mut to) = (Vec::new(), std::collections::BTreeMap::new());
    for draft in drafts {
        if let Err(e) = draft.save() {
            let _ = match &plan.before {
                Some(before) => vdir::write_organized(&plan.path, before),
                None => vdir::remove_organized(&plan.path),
            };
            ids.iter().filter_map(|id: &String| Draft::by_id(id)).for_each(|d| d.discard());
            return Err(e);
        }
        // A cancellation drafted here: what the sync reads if its server's version comes back holding it.
        if plan.says != Says::Invitation
            && let Some(ics) = draft.calendar.as_deref()
        {
            organizer::note_cancellation(ics, &addresses(&draft), &plan.before.as_deref().map(agenda::status_of).unwrap_or_default());
        }
        to.insert(draft.id.clone(), addresses(&draft));
        ids.push(draft.id.clone());
    }
    // Its own lines back in its file, written whole: their entry goes (R1). A draft refused above
    // put the meeting back, and its entry stays.
    if plan.lines_back {
        sioul_core::meeting_lines::forget(&sioul_core::meeting_lines::default_path(), &sioul_core::meeting_lines::uid_of(&plan.text))?;
    }
    // The status back from Cancelled: its cancellation still unsent says what no longer holds (M6).
    let withdrawn: Vec<Draft> = plan.withdrawn.iter().filter_map(|id| Draft::by_id(id)).collect();
    withdrawn.iter().for_each(Draft::discard);
    let settled = organizer::cancellations_settled(&plan.text);
    let ((_, mut served), (_, off_served)) = (plan.invited_apart(), plan.removed_apart());
    served.extend(off_served);
    Ok(Written { file: Some(FileChange { path: plan.path.clone(), before: plan.before.clone(), after: Some(plan.text.clone()) }), drafts: ids, to, served, lines_back: plan.lines_back, withdrawn, settled })
}

/// What a write took back with it, restored once its Undo went through: the
/// cancellations it withdrew from Drafts and the notes it settled; then the
/// notes the meeting as it is now no longer stands for taken out.
fn restored(written: &Written, path: &Path) {
    organizer::notes_back(&written.settled);
    for draft in &written.withdrawn {
        if let Err(e) = draft.save() {
            eprintln!("Sioul: a cancellation withdrawn could not come back to Drafts: {e}");
        }
    }
    if let Ok(text) = std::fs::read_to_string(path) {
        organizer::cancellations_settled(&text);
    }
}

/// A write through the door taken back (Undo), while its file is still as
/// it was left (a sync, another device, changed none since); else "too
/// late", or, when its server took the change and told its guests (its copy
/// came back the same for them), that it did, and to change it again to
/// tell them otherwise (second review of the organizer's door, N7).
///
/// - Nobody heard of it (its drafts still unsent in Drafts, and its server
///   never had it: not sent yet, `vdir::agreed_hash`): the file as it was,
///   its drafts discarded, nothing asked of the server, which still holds
///   what its guests know.
/// - Some heard of it (a draft sent, or its server had it and told its
///   guests): the change back told them as any change is, one SEQUENCE past
///   the version they received (`organizer::past`), each as their line says;
///   a guest who never heard of it hears nothing of either.
pub(crate) fn put_back(written: &Written) -> Result<(), String> {
    let too_late = || tr().text("undo-too-late", None);
    let Some(file) = &written.file else { return Err(too_late()) };
    let current = std::fs::read_to_string(&file.path).ok();
    if current != file.after {
        // Its server took it: its copy came back, the same for its guests, the sync's last word on it.
        if let (Some(now), Some(after)) = (current.as_deref(), file.after.as_deref())
            && !written.served.is_empty()
            && !organizer::told(after, now)
            && vdir::agreed_hash(&file.path) == Some(vdir::content_hash(now.as_bytes()))
        {
            return Err(tr().text("invite-undo-served", None));
        }
        return Err(too_late());
    }
    // Its server has this write: the sync's last word on the file is it.
    let pushed = file.after.as_deref().is_some_and(|after| vdir::agreed_hash(&file.path) == Some(vdir::content_hash(after.as_bytes())));
    // A write that made the meeting no meeting put its own lines back into its file: kept apart
    // again, its entry as this device knew it, before the meeting comes back (R1).
    let own = agenda::own_addresses();
    let store = sioul_core::meeting_lines::default_path();
    if written.lines_back
        && let Some(after) = file.after.as_deref()
    {
        sioul_core::meeting_lines::set_from(&store, &sioul_core::meeting_lines::uid_of(after), after)?;
    }
    // Sent, or discarded by hand: no longer in Drafts. Its guests heard of it.
    let heard: Vec<String> = written.drafts.iter().filter(|id| Draft::by_id(id).is_none()).flat_map(|id| written.to.get(id).cloned().unwrap_or_default()).collect();
    let served: Vec<Guest> = if pushed { written.served.clone() } else { Vec::new() };
    let unsent = || written.drafts.iter().filter_map(|id| Draft::by_id(id)).for_each(|d| d.discard());
    if heard.is_empty() && served.is_empty() {
        match &file.before {
            // No meeting before it: Sioul's own lines kept apart since (who takes it, answered
            // meanwhile) back into it, their entry gone (R1).
            Some(before) if !agenda::meeting_with_others(before, &own) => {
                let uid = sioul_core::meeting_lines::uid_of(before);
                let entry = sioul_core::meeting_lines::load(&store).events.get(&uid).cloned();
                vdir::write_organized(&file.path, &entry.as_ref().map_or_else(|| before.clone(), |kept| sioul_core::meeting_lines::folded(before, kept)))?;
                if entry.is_some() {
                    sioul_core::meeting_lines::forget(&store, &uid)?;
                }
            }
            Some(before) => vdir::write_organized(&file.path, before)?,
            None => vdir::remove_organized(&file.path)?,
        }
        unsent();
        restored(written, &file.path);
        return Ok(());
    }
    let calendar = crate::pim::collection_of(&file.path).ok_or_else(too_late)?;
    // Told back: only those who heard of it; the drafts for the others taken out again.
    let only_heard = |again: &Written| {
        for draft in again.drafts.iter().filter_map(|id| Draft::by_id(id)) {
            let mut kept = draft.clone();
            kept.to.retain(|t| heard.contains(&sioul_core::compose::address_of(t).unwrap_or_else(|| t.trim().to_string()).to_lowercase()));
            if kept.to.is_empty() {
                draft.discard();
            } else if kept.to != draft.to {
                let _ = kept.save();
            }
        }
    };
    match (&file.before, file.after.as_deref()) {
        (Some(before), Some(after)) => {
            let back = organizer::past(before, after).ok_or_else(too_late)?;
            let mut again = plan(&file.path, Some(after), &back, Some(&organizer::guests_of(before)), &calendar)?;
            // One who heard and whose time comes back (a guest of one time left out, which went
            // with their lines): told back too.
            let more: Vec<Guest> = organizer::every_guest(&back).into_iter().filter(|g| heard.iter().any(|h| g.address.eq_ignore_ascii_case(h)) && !again.invited().iter().any(|i| i.address.eq_ignore_ascii_case(&g.address))).collect();
            again.told_guests.extend(more);
            // Its server never had the change: nothing more asked of it than `before` asked.
            if !pushed {
                again.text = organizer::with_force_send(&organizer::without_force_send(&again.text), &organizer::forced_of(before)).0;
                again.forced.clear();
            }
            let done = write(&again)?;
            only_heard(&done);
        }
        (None, Some(_)) => {
            let done = delete(&file.path)?;
            only_heard(&done);
        }
        _ => return Err(too_late()),
    }
    unsent();
    restored(written, &file.path);
    Ok(())
}

/// Who hears of a meeting you organize deleted: those its server tells, and
/// those your drafts tell, every event's guests each once; none of those
/// when it is cancelled already, whose cancellation told them. Refused while
/// nobody knows yet whether its server tells those not left to you
/// (`Teller::Unknown`).
fn told_of_deletion(before: &str, calendar: &Collection) -> Result<Apart, String> {
    let organizer = organizer::organizer_of(before).map(|o| o.address).unwrap_or_default();
    let server = server_for(calendar, &organizer);
    let not_left = |g: &Guest| organizer::teller(before, &g.address, Sender::Server) == Sender::Server;
    let guests = organizer::every_guest(before);
    if server == Teller::Unknown && guests.iter().any(not_left) {
        return Err(tr().text("invite-server-unknown", None));
    }
    let (served, mut drafted): Apart = guests.into_iter().partition(|g| server == Teller::Server && not_left(g));
    if agenda::status_of(before) == "cancelled" {
        drafted.clear();
    }
    Ok((served, drafted))
}

/// What deleting a meeting you organize says before you are asked: every
/// guest hears it is cancelled, and how each is told; "" when nobody hears
/// of it (cancelled already, your drafts told them).
pub(crate) fn deletion_asked(before: &str, calendar: &Collection) -> Result<String, String> {
    let (served, drafted) = told_of_deletion(before, calendar)?;
    if served.is_empty() && drafted.is_empty() {
        return Ok(String::new());
    }
    let names = |guests: &[Guest]| guests.iter().map(Guest::said).collect::<Vec<_>>().join(", ");
    let mut said = vec![tr().text("invite-delete-tells", None)];
    if !drafted.is_empty() {
        said.push(say("invite-ask-drafts", &[("who", names(&drafted))]));
    }
    if !served.is_empty() {
        said.push(say("invite-ask-served", &[("who", names(&served))]));
    }
    Ok(said.join(" "))
}

/// A meeting you organize deleted, as its Delete asks (after "Deleting it
/// tells every guest"): the cancellation drafted for the guests told by
/// drafts, those of one time of it alone too (`organizer::cancel`), none when
/// it is cancelled already (`told_of_deletion`), then the file taken out
/// through the door; its server sends its own to the guests it tells. Refused
/// while nobody knows yet whether its server tells those not left to you
/// (`Teller::Unknown`). What Undo needs to put it back.
pub(crate) fn delete(path: &Path) -> Result<Written, String> {
    let before = std::fs::read_to_string(path).map_err(|_| tr().text("agenda-gone", None))?;
    let calendar = crate::pim::collection_of(path).ok_or_else(|| tr().text("agenda-gone", None))?;
    let own = agenda::own_addresses();
    if !agenda::organized_meeting(&before, &own) {
        return Err(tr().text("event-delete-with-others", None));
    }
    let (served, drafted) = told_of_deletion(&before, &calendar)?;
    let organizer = organizer::organizer_of(&before).map(|o| o.address).unwrap_or_default();
    let cancelled = if drafted.is_empty() {
        None
    } else {
        let account = sending_account(&calendar, &organizer)?;
        let to: Vec<String> = drafted.iter().map(|g| g.address.clone()).collect();
        let ics = organizer::cancel(&before, &to, true).ok_or_else(|| tr().text("agenda-gone", None))?;
        Some(draft(&account, &drafted, &before, ics, true))
    };
    vdir::remove_organized(path)?;
    let (mut drafts, mut to) = (Vec::new(), std::collections::BTreeMap::new());
    if let Some(draft) = cancelled {
        if let Err(e) = draft.save() {
            let _ = vdir::write_organized(path, &before);
            return Err(e);
        }
        if let Some(ics) = draft.calendar.as_deref() {
            organizer::note_cancellation(ics, &addresses(&draft), &agenda::status_of(&before));
        }
        to.insert(draft.id.clone(), addresses(&draft));
        drafts.push(draft.id.clone());
    }
    Ok(Written { file: Some(FileChange { path: path.to_path_buf(), before: Some(before), after: None }), drafts, to, served, lines_back: false, ..Written::default() })
}

/// A deletion taken back (Undo), while nothing took its place: what the
/// status line says then (`Some("")`: nothing more than Undo's own word), or
/// None when it could not be put back. Never silent towards the guests
/// (review of 1e8551e, F14), each told as their line says:
/// - its cancellation still unsent in Drafts: discarded;
/// - sent (no longer in Drafts): an invitation drafted for the guests told
///   by drafts, which the line says;
/// - the guests its server told: the line says it invites them again if it
///   told them it was cancelled.
///
/// Written back as it was when nobody heard of it, else one SEQUENCE past
/// the cancellation (`organizer::after_cancel`), which guests' programs take
/// as a later version.
pub(crate) fn undelete(written: &Written) -> Option<String> {
    let Some(FileChange { path, before: Some(before), .. }) = &written.file else { return None };
    if path.exists() {
        return None;
    }
    let sent = written.drafts.iter().any(|id| Draft::by_id(id).is_none());
    let served = !written.served.is_empty();
    if !sent && !served {
        vdir::write_organized(path, before).ok()?;
        written.drafts.iter().filter_map(|id| Draft::by_id(id)).for_each(|d| d.discard());
        organizer::cancellations_settled(before);
        return Some(String::new());
    }
    let again = organizer::after_cancel(before)?;
    let mut said = Vec::new();
    let mut invitations = Vec::new();
    if sent {
        let calendar = crate::pim::collection_of(path)?;
        let organizer = organizer::organizer_of(&again).map(|o| o.address).unwrap_or_default();
        let account = sending_account(&calendar, &organizer).ok()?;
        let drafted: Vec<Guest> = organizer::every_guest(&again).into_iter().filter(|g| !written.served.iter().any(|s| s.address.eq_ignore_ascii_case(&g.address))).collect();
        // The series' guests its whole invitation; those of some times alone, those times.
        let series = organizer::guests_of(&again);
        let (whole, apart): (Vec<Guest>, Vec<Guest>) = drafted.into_iter().partition(|g| series.iter().any(|s| s.address.eq_ignore_ascii_case(&g.address)));
        let addresses: Vec<String> = apart.iter().map(|g| g.address.clone()).collect();
        for (to, ics) in [(whole, organizer::request(&again)), (apart, organizer::request_for(&again, &addresses))] {
            if let Some(ics) = ics.filter(|_| !to.is_empty()) {
                invitations.push(draft(&account, &to, &again, ics, false));
            }
        }
    }
    vdir::write_organized(path, &again).ok()?;
    if !invitations.is_empty() {
        if invitations.iter().any(|i| i.save().is_err()) {
            invitations.iter().for_each(Draft::discard);
            let _ = vdir::remove_organized(path);
            return None;
        }
        said.push(tr().text("invite-undeleted-draft", None));
    }
    if served {
        said.push(tr().text("invite-undeleted-server", None));
    }
    written.drafts.iter().filter_map(|id| Draft::by_id(id)).for_each(|d| d.discard());
    organizer::cancellations_settled(&again);
    Some(said.join(" "))
}

/// A guest's REPLY to a meeting you organize, read in your mail (RFC 5546
/// §3.2.3), as the other parent's is (`children::replied`): their answer
/// written into the meeting, its PARTSTAT alone, once, when the message is
/// from the address it answers for and the Porch would take it as coming
/// from there (`children::proven_sender`): a forged "Anne accepts" would say
/// a guest comes who does not. The line the reader says ("Anne accepted
/// “Planning”."), also when the meeting says it already; for one time of a
/// series, that time said too, kept on that time (`organizer::replied`;
/// review of 1e8551e, F13). None for a REPLY of no meeting of yours.
pub(crate) fn replied(reply: &str, message: &sioul_core::card::Card, trusted_ids: &[String], own_domains: &[String]) -> Option<String> {
    let lines = sioul_core::lines::unfold(reply);
    let uid = lines.iter().find(|l| sioul_core::lines::name(l) == "UID").map(|l| sioul_core::lines::value(l).trim().to_string())?;
    let own = agenda::own_addresses();
    let path = meeting_of(&uid, &own)?;
    let text = std::fs::read_to_string(&path).ok()?;
    let (title, _) = crate::children::named(&text);
    let got = organizer::replied(&text, reply)?;
    if !message.from_address.as_deref().is_some_and(|from| got.guest.address.eq_ignore_ascii_case(from.trim())) {
        return None;
    }
    let who = organizer::guests_of(&text).into_iter().find(|g| g.address.eq_ignore_ascii_case(&got.guest.address)).map_or_else(|| got.guest.said(), |g| g.said());
    if !crate::children::proven_sender(message, trusted_ids, own_domains) {
        return Some(say("invite-replied-unproven", &[("who", who), ("title", title)]));
    }
    if let Some(written) = &got.written {
        vdir::write_organized(&path, written).ok()?;
    }
    let id = match got.answer.as_str() {
        "accepted" => "invite-replied-yes",
        "declined" => "invite-replied-no",
        _ => "invite-replied-maybe",
    };
    let line = say(id, &[("who", who), ("title", title)]);
    let zone = sioul_core::zone::current();
    let when = got.at.and_then(|at| jiff::Timestamp::from_second(at).ok()).map(|at| tr().date(&at.to_zoned(zone), true));
    Some(match when {
        Some(when) => format!("{line} {}", say("invite-replied-time", &[("when", when)])),
        None => line,
    })
}

/// Whether your own invitation, seen in your mail, was sent by you (review
/// of 1e8551e, F6): its From is the address it names as its organizer, one
/// of yours, and the sender's domain vouches for it (DMARC or an aligned
/// DKIM signature: `trust::Trust::Verified`), as the Porch asks of a
/// message from yourself (`porch`'s `from_yourself`). A copy kept in Sent
/// never went through a server that says so: unproven.
fn sent_by_you(message: &sioul_core::card::Card, organizer: &str, trusted_ids: &[String], own: &[String]) -> bool {
    use sioul_core::trust;
    let auth = trust::read_auth_results(&message.headers, trusted_ids);
    let (judged, _) = trust::judge_sender(auth.as_ref(), None, message.sender_domain());
    let organizer = organizer.trim().to_lowercase();
    judged == trust::Trust::Verified && own.contains(&organizer) && message.from_address.as_deref().is_some_and(|from| from.trim().eq_ignore_ascii_case(&organizer))
}

/// Your own invitation, seen in your mail, added to your calendar at `path`
/// (Accept, Maybe or Add of an invitation naming you as the organizer of
/// guests; review of 1e8551e, F6): only when you sent it (`sent_by_you`),
/// else refused, said in one sentence; never stored as it came, whose
/// server would invite every guest again from you. Written through the
/// door, as its guests may read it (`organizer::set_aside`), each guest
/// left to you (`SCHEDULE-AGENT=CLIENT`, RFC 6638 §7.1): your message told
/// them. No answer goes to yourself.
pub(crate) fn adopt(path: &Path, invitation: &str, message: &sioul_core::card::Card, trusted_ids: &[String]) -> Result<(), String> {
    let own = agenda::own_addresses();
    let organizer = organizer::organizer_of(invitation).ok_or_else(|| tr().text("invitation-organizer-unproven", None))?;
    if !sent_by_you(message, &organizer.address, trusted_ids, &own) {
        return Err(tr().text("invitation-organizer-unproven", None));
    }
    // As it came, its METHOD aside (`organizer::for_guests`): no line of Sioul's is added to a meeting of yours.
    let text = organizer::with_guests(invitation, &organizer, &organizer::guests_of(invitation), Sender::Draft).ok_or_else(|| tr().text("agenda-gone", None))?;
    let (text, aside) = organizer::set_aside(&organizer::every_guest_left_to_you(&text));
    keep_aside(&text, &aside, false, false)?;
    vdir::write_organized(path, &text)
}

/// Once, at the window's start (review of 1e8551e, F5; its second review,
/// N6): the meetings you organize whose files hold Sioul's own lines, ties,
/// alarms or addresses, which an older Sioul wrote there, written as their
/// guests may read them (`organizer::for_guests`: every other program's
/// lines kept) through the door, what they held moved into your own files
/// first (your own time where none is kept, your ties, Sioul's other lines:
/// who takes a child's event, its kind). No guest hears of it: no time,
/// title, place, notes nor guest changes. The pass of 6 November 2026
/// (version 1) took Sioul's other lines out without keeping them: for a
/// meeting it passed over, they come back from this device's earlier
/// versions of its file (`recovered`), when one holds them. Done once its
/// pass ends, marked in the state folder (`organized-tidied`, holding the
/// pass's version, 2); not marked while none of your addresses is known.
/// How many were written.
pub(crate) fn tidy_once() -> usize {
    const VERSION: &str = "2";
    let mark = sioul_core::config::state_dir().join("organized-tidied");
    let was = std::fs::read_to_string(&mark).map(|v| v.trim().to_string()).unwrap_or_default();
    if was == VERSION {
        return 0;
    }
    let own = agenda::own_addresses();
    if own.is_empty() {
        return 0;
    }
    let kept = sioul_core::meeting_lines::load(&sioul_core::meeting_lines::default_path());
    let mut written = 0;
    for calendar in vdir::collections(Kind::Calendars).into_iter().filter(|c| !c.read_only && c.holds("VEVENT")) {
        for path in calendar.items() {
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            if !agenda::organized_meeting(&text, &own) {
                continue;
            }
            let cleaned = organizer::for_guests(&text);
            let mut aside = organizer::aside_of(&text);
            let uid = sioul_core::lines::unfold(&text).iter().find(|l| sioul_core::lines::name(l) == "UID").map(|l| sioul_core::lines::value(l).trim().to_string()).unwrap_or_default();
            if was == "1" && aside.lines.is_empty() && !kept.events.contains_key(&uid) {
                aside.lines = recovered(&path);
            }
            if cleaned == text && aside.lines.is_empty() {
                continue;
            }
            if keep_aside(&text, &aside, false, false).is_err() {
                return written;
            }
            if cleaned == text {
                continue;
            }
            match vdir::write_organized(&path, &cleaned) {
                Ok(()) => written += 1,
                Err(e) => eprintln!("Sioul: a meeting you organize kept Sioul's lines: {e}"),
            }
        }
    }
    if let Some(folder) = mark.parent() {
        let _ = std::fs::create_dir_all(folder);
    }
    let _ = std::fs::write(&mark, VERSION);
    written
}

/// Sioul's other lines of a meeting's file (`organizer::Aside::lines`) as
/// this device's newest earlier version of it holds them: your accounts'
/// items set aside by their sync, and this device's own calendars' files
/// the sharing kept (`sioul_sync::history`). None when no version holds any.
fn recovered(path: &Path) -> Vec<(String, String)> {
    let Ok(relative) = path.strip_prefix(sioul_core::config::data_dir()) else { return Vec::new() };
    let file = format!("data/{}", relative.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/"));
    let root = sioul_sync::history::root(&sioul_core::config::state_dir().join("share").join("memory.json"));
    let mut versions: Vec<(i64, PathBuf)> = Vec::new();
    for part in [sioul_sync::history::ACCOUNTS, "lists"] {
        for version in sioul_sync::history::versions(&root, part, &file) {
            if let Some(sioul_sync::history::Kept::Copy(copy)) = sioul_sync::history::version(&root, part, &file, &version.stamp) {
                versions.push((version.at, copy));
            }
        }
    }
    versions.sort_by(|a, b| b.0.cmp(&a.0));
    versions.into_iter().filter_map(|(_, copy)| std::fs::read_to_string(copy).ok()).map(|text| organizer::aside_of(&text).lines).find(|lines| !lines.is_empty()).unwrap_or_default()
}

/// The file of the meeting you organize with this UID, in a calendar you may write.
fn meeting_of(uid: &str, own: &[String]) -> Option<PathBuf> {
    if uid.is_empty() {
        return None;
    }
    vdir::collections(Kind::Calendars).into_iter().filter(|c| !c.read_only).flat_map(|c| c.items()).find(|path| {
        std::fs::read_to_string(path).is_ok_and(|text| agenda::organized_meeting(&text, own) && sioul_core::lines::unfold(&text).iter().any(|l| sioul_core::lines::name(l) == "UID" && sioul_core::lines::value(l).trim() == uid))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Your own act, end to end (decisions 7 and 8 of the event card), in a
    /// calendar of this device's own (Sioul drafts): an event of yours given a
    /// guest through the door, the invitation drafted, each guest left to you
    /// to tell; Undo before it is sent puts it back and discards the draft;
    /// moved, every guest drafted the change; never through the ordinary door;
    /// deleted, its cancellation drafted, and undone; someone else's meeting
    /// refused. Your addresses and the door for this test's thread alone.
    #[test]
    fn a_meeting_you_organize_goes_through_its_door_and_is_told() {
        sioul_core::config::stand_ins_only();
        let dir = sioul_core::config::data_dir().join(format!("organized-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let config = dir.join("config.toml");
        std::fs::write(&config, "language = \"en\"\n\n[[account]]\nid = \"camille\"\nkind = \"imap\"\naddress = \"camille@example.org\"\nname = \"Camille\"\nhost = \"imap.example.org\"\n").unwrap();
        crate::backend::TEST_CONFIG.with(|own| *own.borrow_mut() = Some(config.clone()));
        agenda::own_addresses_here(Some(&["camille@example.org"]));
        agenda::open_organizer_door_here(true);
        let calendar = vdir::create(Kind::Calendars, vdir::LOCAL, "Organized here", None, &["VEVENT"]).unwrap();
        let path = calendar.dir.join("planning.ics");
        let own = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:organized-planning\r\nDTSTAMP:20991101T090000Z\r\nSEQUENCE:0\r\nDTSTART:20991110T090000Z\r\nDTEND:20991110T100000Z\r\nSUMMARY:Planning\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        vdir::write_item(&path, own).unwrap();
        let anne = [Guest { address: "anne@example.net".into(), name: "Anne".into() }];
        // Invited: the draft, the file through the door.
        let plan = plan(&path, Some(own), own, Some(&anne), &calendar).unwrap();
        assert_eq!((plan.server, plan.added.len(), plan.removed.len(), plan.told), (Teller::Drafts, 1, 0, false));
        assert!(plan.asked().contains("Anne") && plan.tells(), "{}", plan.asked());
        let written = write(&plan).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let unfolded = sioul_core::lines::unfold(&text).join("\n");
        assert!(unfolded.contains("ORGANIZER;CN=Camille:mailto:camille@example.org") && unfolded.contains("SCHEDULE-AGENT=CLIENT:mailto:anne@example.net"), "{unfolded}");
        let draft = Draft::by_id(&written.drafts[0]).expect("the invitation drafted");
        assert!(draft.to[0].contains("anne@example.net") && draft.calendar.as_deref().is_some_and(|ics| ics.contains("METHOD:REQUEST")), "{draft:?}");
        assert_eq!(draft.account, "camille");
        // Undo before it was sent: as it was, the draft gone.
        assert_eq!(put_back(&written), Ok(()));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), own);
        assert!(Draft::by_id(&written.drafts[0]).is_none());
        // Invited again, then moved: every guest told of the change.
        let written = write(&super::plan(&path, Some(own), own, Some(&anne), &calendar).unwrap()).unwrap();
        let before = std::fs::read_to_string(&path).unwrap();
        let zone = sioul_core::zone::current();
        let (start, end) = agenda::first_times(&before, &zone).unwrap();
        let moved = agenda::moved(&before, start, start + 3600, end + 3600, false, &zone).unwrap();
        assert!(vdir::write_item(&path, &moved).is_err(), "never through the ordinary door");
        let move_plan = super::plan(&path, Some(&before), &moved, None, &calendar).unwrap();
        assert!(move_plan.told && move_plan.asked().contains("Anne"), "{}", move_plan.asked());
        let move_written = write(&move_plan).unwrap();
        assert_eq!(move_written.drafts.len(), 1, "the change drafted for Anne");
        assert!(sioul_core::lines::unfold(&std::fs::read_to_string(&path).unwrap()).iter().any(|l| l == "SEQUENCE:1"), "a later SEQUENCE");
        // Deleted, its cancellation drafted; undone.
        let gone = delete(&path).unwrap();
        assert!(!path.exists());
        assert!(Draft::by_id(&gone.drafts[0]).and_then(|d| d.calendar).is_some_and(|ics| ics.contains("METHOD:CANCEL")));
        assert!(undelete(&gone) == Some(String::new()) && path.exists() && Draft::by_id(&gone.drafts[0]).is_none(), "its cancellation unsent: discarded, nothing more said");
        // Someone else's meeting: refused, by the plan and by the door.
        let theirs = own.replace("SUMMARY:Planning", "SUMMARY:Board\r\nORGANIZER:mailto:chair@example.org\r\nATTENDEE:mailto:camille@example.org").replace("organized-planning", "organized-board");
        let board = calendar.dir.join("board.ics");
        vdir::store_item(&board, &theirs).unwrap();
        assert!(super::plan(&board, Some(&theirs), &theirs, Some(&anne), &calendar).is_err());
        assert!(vdir::write_organized(&board, &theirs.replace("Board", "Mine")).is_err());
        for id in written.drafts.iter().chain(&move_written.drafts) {
            if let Some(d) = Draft::by_id(id) {
                d.discard();
            }
        }
        agenda::open_organizer_door_here(false);
        agenda::own_addresses_here(None);
        crate::backend::TEST_CONFIG.with(|own| *own.borrow_mut() = None);
        let _ = std::fs::remove_dir_all(&calendar.dir);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The guests the form gives: each once, never yours, only addresses.
    #[test]
    fn the_form_s_guests_are_cleaned() {
        let given = [
            Guest { address: " anne@example.net ".into(), name: "Anne".into() },
            Guest { address: "ANNE@example.net".into(), name: String::new() },
            Guest { address: "mailto:bob@example.org".into(), name: String::new() },
            Guest { address: "not an address".into(), name: String::new() },
            Guest { address: "eve@localhost".into(), name: String::new() },
            Guest { address: "x@example.org>\r\nBcc: y@example.org".into(), name: String::new() },
        ];
        let cleaned = cleaned(&given);
        assert_eq!(cleaned.iter().map(|g| g.address.as_str()).collect::<Vec<_>>(), ["anne@example.net", "bob@example.org"]);
        assert_eq!(cleaned[0].name, "Anne");
    }

    /// One test's own world: its configuration (one account, `address`),
    /// your addresses and the door for its thread, a calendar of this
    /// device's own (Sioul drafts). Undone by `Here::drop`.
    struct Here {
        dir: PathBuf,
        calendar: Collection,
    }

    impl Here {
        fn new(name: &str, address: &str) -> Here {
            sioul_core::config::stand_ins_only();
            let dir = sioul_core::config::data_dir().join(format!("{name}-{}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            let config = dir.join("config.toml");
            std::fs::write(&config, format!("language = \"en\"\n\n[[account]]\nid = \"{name}\"\nkind = \"imap\"\naddress = \"{address}\"\nname = \"Camille\"\nhost = \"imap.example.org\"\n")).unwrap();
            crate::backend::TEST_CONFIG.with(|own| *own.borrow_mut() = Some(config.clone()));
            agenda::own_addresses_here(Some(&[address]));
            agenda::open_organizer_door_here(true);
            let calendar = vdir::create(Kind::Calendars, vdir::LOCAL, name, None, &["VEVENT"]).unwrap();
            Here { dir, calendar }
        }

        /// A meeting you organize, its guests invited through the door (their drafts discarded).
        fn meeting(&self, file: &str, text: &str, guests: &[Guest]) -> PathBuf {
            let path = self.calendar.dir.join(file);
            vdir::write_item(&path, text).unwrap();
            let written = write(&plan(&path, Some(text), text, Some(guests), &self.calendar).unwrap()).unwrap();
            written.drafts.iter().filter_map(|id| Draft::by_id(id)).for_each(|d| d.discard());
            path
        }
    }

    impl Drop for Here {
        fn drop(&mut self) {
            agenda::open_organizer_door_here(false);
            agenda::own_addresses_here(None);
            crate::backend::TEST_CONFIG.with(|own| *own.borrow_mut() = None);
            let _ = std::fs::remove_dir_all(&self.calendar.dir);
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn anne() -> Vec<Guest> {
        vec![Guest { address: "anne@example.net".into(), name: "Anne".into() }]
    }

    const PLANNING: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:{uid}\r\nDTSTAMP:20991101T090000Z\r\nSEQUENCE:0\r\nDTSTART:20991110T090000Z\r\nDTEND:20991110T100000Z\r\nSUMMARY:Planning\r\n{more}END:VEVENT\r\nEND:VCALENDAR\r\n";

    fn planning(uid: &str, more: &str) -> String {
        PLANNING.replace("{uid}", uid).replace("{more}", more)
    }

    fn sequence(path: &Path) -> String {
        sioul_core::lines::unfold(&std::fs::read_to_string(path).unwrap()).into_iter().find(|l| l.starts_with("SEQUENCE:")).unwrap_or_default()
    }

    /// Undo of a deletion once its cancellation went (no longer in Drafts):
    /// the meeting back one SEQUENCE past the cancellation, an invitation
    /// for every guest drafted, and said; its server's to tell, the line says
    /// the server invites them again if it told them (review of 1e8551e, F14).
    #[test]
    fn undoing_a_deletion_after_its_cancellation_went_invites_again() {
        let here = Here::new("undeleted", "camille.undo@example.org");
        let path = here.meeting("planning.ics", &planning("undeleted-1", ""), &anne());
        let gone = delete(&path).unwrap();
        assert!(gone.served.is_empty() && gone.drafts.len() == 1, "drafted for Anne, left to you");
        // Sent from Mail: no longer in Drafts.
        Draft::by_id(&gone.drafts[0]).unwrap().discard();
        assert_eq!(undelete(&gone).as_deref(), Some(tr().text("invite-undeleted-draft", None).as_str()));
        assert_eq!(sequence(&path), "SEQUENCE:2", "one past the cancellation's (1)");
        let again: Vec<Draft> = Draft::all().into_iter().filter(|d| d.calendar.as_deref().is_some_and(|ics| ics.contains("UID:undeleted-1") && ics.contains("METHOD:REQUEST"))).collect();
        assert_eq!(again.len(), 1, "invited again");
        assert!(again[0].to[0].contains("anne@example.net") && again[0].calendar.as_deref().unwrap().contains("SEQUENCE:2"));
        again.iter().for_each(Draft::discard);
        // The server's to tell: written back so too, and said.
        let gone = delete(&path).unwrap();
        gone.drafts.iter().filter_map(|id| Draft::by_id(id)).for_each(|d| d.discard());
        let served = Written { served: anne(), drafts: Vec::new(), ..gone };
        assert_eq!(undelete(&served).as_deref(), Some(tr().text("invite-undeleted-server", None).as_str()));
        assert_eq!(sequence(&path), "SEQUENCE:4");
    }

    /// A colleague's event in a shared calendar, naming them as its organizer
    /// with nobody invited: inviting someone to it says first that you take
    /// their place (review of 1e8551e, F12).
    #[test]
    fn taking_an_organizer_s_place_is_said_first() {
        let here = Here::new("replaced", "camille.place@example.org");
        let theirs = planning("replaced-1", "ORGANIZER;CN=Colleague:mailto:colleague@example.org\r\n");
        let path = here.calendar.dir.join("theirs.ics");
        vdir::write_item(&path, &theirs).unwrap();
        let asked = plan(&path, Some(&theirs), &theirs, Some(&anne()), &here.calendar).unwrap();
        assert_eq!(asked.replaced.as_ref().map(|o| o.address.as_str()), Some("colleague@example.org"));
        assert!(asked.asked().starts_with(&say("invite-ask-organizer", &[("who", "Colleague".into())])), "{}", asked.asked());
        let mine = planning("replaced-2", "ORGANIZER:mailto:camille.place@example.org\r\n");
        assert!(plan(&path, Some(&mine), &mine, Some(&anne()), &here.calendar).unwrap().replaced.is_none(), "yours: nothing to say");
    }

    /// A meeting you organize given a status, by your drafts (made in Sioul):
    /// "To be confirmed" a REQUEST at a later SEQUENCE, said first; Undo before
    /// it is sent as it was, its draft discarded. "Cancelled" a CANCEL for
    /// every guest, one past the highest version, in the same way; sent, Undo
    /// tells them back one version past it. A cancelled meeting takes no
    /// change its guests would see, until it has another status; deleted, it
    /// tells nobody again, and Undo says nothing.
    #[test]
    fn a_status_goes_to_your_guests_by_your_drafts() {
        let here = Here::new("status-drafts", "camille.status@example.org");
        let path = here.meeting("status.ics", &planning("status-1", ""), &anne());
        let before = std::fs::read_to_string(&path).unwrap();
        let asked = status_plan(&path, &before, "tentative", &here.calendar).unwrap().unwrap();
        assert_eq!(asked.asked(), say("invite-ask-drafts", &[("who", "Anne".into())]));
        assert!(status_plan(&path, &before, "maybe", &here.calendar).is_err());
        let written = write(&asked).unwrap();
        let invitation = Draft::by_id(&written.drafts[0]).unwrap().calendar.unwrap();
        let flat_invitation = flat(&invitation);
        assert!(flat_invitation.contains("METHOD:REQUEST") && flat_invitation.contains("STATUS:TENTATIVE") && flat_invitation.contains("SEQUENCE:1"), "{flat_invitation}");
        assert!(flat(&std::fs::read_to_string(&path).unwrap()).contains("SCHEDULE-AGENT=CLIENT:mailto:anne@example.net"), "Anne's mark as it was");
        assert_eq!(put_back(&written), Ok(()));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), before, "as it was");
        assert!(Draft::by_id(&written.drafts[0]).is_none(), "its draft discarded");
        // Cancelled: its CANCEL alone.
        let cancelling = status_plan(&path, &before, "cancelled", &here.calendar).unwrap().unwrap();
        let written = write(&cancelling).unwrap();
        assert_eq!(written.drafts.len(), 1, "one message");
        let cancel = flat(&Draft::by_id(&written.drafts[0]).unwrap().calendar.unwrap());
        assert!(cancel.contains("METHOD:CANCEL") && cancel.contains("STATUS:CANCELLED") && cancel.contains("SEQUENCE:1") && cancel.contains("mailto:anne@example.net"), "{cancel}");
        let cancelled = std::fs::read_to_string(&path).unwrap();
        assert_eq!(agenda::status_of(&cancelled), "cancelled");
        assert!(flat(&cancelled).contains("SEQUENCE:1"));
        // Sent: Undo tells Anne back, one past it.
        Draft::by_id(&written.drafts[0]).unwrap().discard();
        assert_eq!(put_back(&written), Ok(()));
        let back = std::fs::read_to_string(&path).unwrap();
        assert_ne!(agenda::status_of(&back), "cancelled");
        assert!(flat(&back).contains("SEQUENCE:2"), "{back}");
        let told_back = drafts_of_uid("status-1");
        assert_eq!(told_back.len(), 1);
        let told = flat(told_back[0].calendar.as_deref().unwrap());
        assert!(told.contains("METHOD:REQUEST") && told.contains("SEQUENCE:2") && told_back[0].to[0].contains("anne@example.net"), "{told}");
        told_back.iter().for_each(Draft::discard);
        // Cancelled again: no change its guests would see, until another status.
        let written = write(&status_plan(&path, &back, "cancelled", &here.calendar).unwrap().unwrap()).unwrap();
        written.drafts.iter().filter_map(|id| Draft::by_id(id)).for_each(|d| d.discard());
        let off = std::fs::read_to_string(&path).unwrap();
        let refused = tr().text("invite-cancelled-first", None);
        assert_eq!(plan(&path, Some(&off), &off.replace("SUMMARY:Planning", "SUMMARY:Board"), None, &here.calendar).map(|_| ()), Err(refused.clone()));
        assert_eq!(plan(&path, Some(&off), &off, Some(&[]), &here.calendar).map(|_| ()), Err(refused));
        assert!(plan(&path, Some(&off), &off, None, &here.calendar).is_ok(), "nothing its guests see");
        assert_eq!(deletion_asked(&off, &here.calendar), Ok(String::new()), "they heard it");
        let on = status_plan(&path, &off, "confirmed", &here.calendar).unwrap().unwrap();
        assert_eq!(on.says, Says::Invitation);
        assert!(flat(&request_of(&on)).contains("STATUS:CONFIRMED"));
        // Deleted, cancelled already: nobody told again, and its Undo says nothing.
        let gone = delete(&path).unwrap();
        assert!(gone.drafts.is_empty() && gone.served.is_empty() && drafts_of_uid("status-1").is_empty());
        assert_eq!(undelete(&gone).as_deref(), Some(""));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), off);
    }

    /// The invitation a plan drafts.
    fn request_of(plan: &Plan) -> String {
        drafts_of(plan).unwrap().remove(0).calendar.unwrap()
    }

    /// A meeting you organize given a status, by its server (made elsewhere):
    /// nothing drafted, nothing forced (a status is a change it sends by
    /// itself), said first, and Undo as for its other acts. Refused while
    /// nobody knows yet whether its server tells them. Both ways in one
    /// meeting: each guest once.
    #[test]
    fn a_status_goes_to_your_guests_by_its_server() {
        let here = Here::new("status-served", "camille.served@example.org");
        let (calendar, state) = served(&here, "status-served-cal", "camille.served@example.org");
        let theirs = planning("status-2", "ORGANIZER;CN=Camille:mailto:camille.served@example.org\r\nATTENDEE;CN=Anne;PARTSTAT=ACCEPTED:mailto:anne@example.net\r\nATTENDEE;CN=Bob;SCHEDULE-AGENT=CLIENT:mailto:bob@example.net\r\n");
        let path = calendar.dir.join("status.ics");
        vdir::store_item(&path, &theirs).unwrap();
        agreed(&calendar, &path, &theirs);
        for status in ["tentative", "cancelled"] {
            let asked = status_plan(&path, &theirs, status, &calendar).unwrap().unwrap();
            assert!(asked.significant && asked.forced.is_empty() && !flat(&asked.text).contains("SCHEDULE-FORCE-SEND"), "{}", asked.text);
            assert_eq!(asked.asked(), [say("invite-ask-drafts", &[("who", "Bob".into())]), say("invite-ask-served", &[("who", "Anne".into())])].join(" "));
            let written = write(&asked).unwrap();
            assert_eq!(written.served.iter().map(|g| g.address.as_str()).collect::<Vec<_>>(), ["anne@example.net"], "Anne by its server");
            let drafted = drafts_of_uid("status-2");
            assert_eq!(drafted.len(), 1, "Bob's alone");
            assert!(drafted[0].to.len() == 1 && drafted[0].to[0].contains("bob@example.net"), "{:?}", drafted[0].to);
            let method = if status == "cancelled" { "METHOD:CANCEL" } else { "METHOD:REQUEST" };
            assert!(flat(drafted[0].calendar.as_deref().unwrap()).contains(method));
            // Not sent yet: as it was, its draft discarded, nothing asked of the server.
            assert_eq!(put_back(&written), Ok(()));
            assert_eq!(std::fs::read_to_string(&path).unwrap(), theirs);
            assert!(drafts_of_uid("status-2").is_empty());
        }
        // Its server has it: told back as any change is.
        let written = write(&status_plan(&path, &theirs, "cancelled", &calendar).unwrap().unwrap()).unwrap();
        let after = std::fs::read_to_string(&path).unwrap();
        agreed(&calendar, &path, &after);
        drafts_of_uid("status-2").iter().for_each(Draft::discard);
        assert_eq!(put_back(&written), Ok(()));
        let back = flat(&std::fs::read_to_string(&path).unwrap());
        assert!(!back.contains("STATUS:CANCELLED") && back.contains("SEQUENCE:2") && !back.contains("SCHEDULE-FORCE-SEND"), "{back}");
        drafts_of_uid("status-2").iter().for_each(Draft::discard);
        // Nobody knows yet whether its server tells Anne: refused, both acts.
        std::fs::remove_file(state.join("scheduling.toml")).unwrap();
        assert_eq!(status_plan(&path, &theirs, "tentative", &calendar).map(|_| ()), Err(tr().text("invite-server-unknown", None)));
        gone_served(&calendar, &state);
    }

    /// One time of a weekly meeting you organize, by your drafts: said first;
    /// EXDATE, nothing else asked; a CANCEL naming that time to its guests
    /// alone, Anne of the series, never a whole update; Undo before it is sent
    /// as it was, its draft discarded; sent, Anne told back one version past
    /// it. A time moved and given Carl alone: Carl and Anne told, its event
    /// gone; told back, Carl by that time alone. One excused: not told.
    #[test]
    fn one_time_left_out_tells_its_guests_by_your_drafts() {
        let here = Here::new("one-time-off", "camille.off@example.org");
        let zone = sioul_core::zone::current();
        let path = here.meeting("weekly.ics", &planning("off-1", "RRULE:FREQ=WEEKLY\r\n"), &anne());
        let series = std::fs::read_to_string(&path).unwrap();
        let (first, _) = agenda::first_times(&series, &zone).unwrap();
        let second = first + 7 * 86400;
        let asked = time_plan(&path, &series, second, &here.calendar).unwrap();
        assert_eq!(asked.asked(), say("invite-ask-drafts", &[("who", "Anne".into())]));
        assert!(time_plan(&path, &series, second + 60, &here.calendar).is_err(), "no time starts there");
        let written = write(&asked).unwrap();
        assert_eq!(written.drafts.len(), 1, "one message");
        let draft = Draft::by_id(&written.drafts[0]).unwrap();
        let said = flat(draft.calendar.as_deref().unwrap());
        assert!(said.contains("METHOD:CANCEL") && said.contains("RECURRENCE-ID") && !said.contains("RRULE") && said.contains("SEQUENCE:1") && draft.to[0].contains("anne@example.net"), "{said}");
        let without = flat(&std::fs::read_to_string(&path).unwrap());
        assert!(without.contains("EXDATE") && without.contains("RRULE") && without.contains("SCHEDULE-AGENT=CLIENT:mailto:anne@example.net"), "{without}");
        assert_eq!(put_back(&written), Ok(()));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), series, "as it was");
        assert!(Draft::by_id(&written.drafts[0]).is_none());
        // Sent: Anne told back, one version past it.
        let written = write(&time_plan(&path, &series, second, &here.calendar).unwrap()).unwrap();
        Draft::by_id(&written.drafts[0]).unwrap().discard();
        assert_eq!(put_back(&written), Ok(()));
        let back = flat(&std::fs::read_to_string(&path).unwrap());
        assert!(!back.contains("EXDATE") && back.contains("SEQUENCE:2"), "{back}");
        let told = drafts_of_uid("off-1");
        assert!(told.len() == 1 && flat(told[0].calendar.as_deref().unwrap()).contains("METHOD:REQUEST") && told[0].to[0].contains("anne@example.net"), "{told:?}");
        told.iter().for_each(Draft::discard);
        // A time moved, Carl asked to it alone by another app; another excusing Anne, asking Bob.
        let now = std::fs::read_to_string(&path).unwrap();
        let carl = "BEGIN:VEVENT\r\nUID:off-1\r\nRECURRENCE-ID:20991117T090000Z\r\nDTSTAMP:20991101T090000Z\r\nSEQUENCE:6\r\nDTSTART:20991117T140000Z\r\nDTEND:20991117T150000Z\r\nSUMMARY:Planning\r\nORGANIZER:mailto:camille.off@example.org\r\nATTENDEE;SCHEDULE-AGENT=CLIENT:mailto:anne@example.net\r\nATTENDEE;CN=Carl;SCHEDULE-AGENT=CLIENT:mailto:carl@example.net\r\nEND:VEVENT\r\n";
        let bob = "BEGIN:VEVENT\r\nUID:off-1\r\nRECURRENCE-ID:20991124T090000Z\r\nDTSTAMP:20991101T090000Z\r\nSEQUENCE:2\r\nDTSTART:20991124T090000Z\r\nDTEND:20991124T100000Z\r\nSUMMARY:Planning\r\nORGANIZER:mailto:camille.off@example.org\r\nATTENDEE;CN=Bob;SCHEDULE-AGENT=CLIENT:mailto:bob@example.net\r\nEND:VEVENT\r\n";
        let wide = now.replace("END:VCALENDAR", &format!("{carl}{bob}END:VCALENDAR"));
        vdir::store_item(&path, &wide).unwrap();
        let moved = jiff::civil::date(2099, 11, 17).at(14, 0, 0, 0).to_zoned(jiff::tz::TimeZone::UTC).unwrap().timestamp().as_second();
        let asked = time_plan(&path, &wide, moved, &here.calendar).unwrap();
        assert_eq!(asked.asked(), say("invite-ask-drafts", &[("who", "Anne, Carl".into())]));
        let written = write(&asked).unwrap();
        let draft = Draft::by_id(&written.drafts[0]).unwrap();
        let said = flat(draft.calendar.as_deref().unwrap());
        assert!(written.drafts.len() == 1 && draft.to.len() == 2 && said.contains("mailto:carl@example.net") && said.contains("RECURRENCE-ID:20991117T090000Z") && said.contains("SEQUENCE:7") && !said.contains("bob@"), "{said}");
        let without = flat(&std::fs::read_to_string(&path).unwrap());
        assert!(!without.contains("carl@") && without.contains("bob@"), "its event gone, the other kept: {without}");
        // Sent: Carl told back too, by that time alone; Anne by the whole of it.
        Draft::by_id(&written.drafts[0]).unwrap().discard();
        assert_eq!(put_back(&written), Ok(()));
        assert!(flat(&std::fs::read_to_string(&path).unwrap()).contains("carl@"));
        let told = drafts_of_uid("off-1");
        let to_carl = told.iter().find(|d| d.to.iter().any(|t| t.contains("carl@"))).expect("Carl told back");
        let carl_said = flat(to_carl.calendar.as_deref().unwrap());
        assert!(carl_said.contains("METHOD:REQUEST") && carl_said.contains("RECURRENCE-ID:20991117T090000Z") && !carl_said.contains("RRULE"), "{carl_said}");
        assert!(told.iter().any(|d| d.to.iter().any(|t| t.contains("anne@"))), "{told:?}");
        assert!(!told.iter().any(|d| d.to.iter().any(|t| t.contains("bob@"))), "Bob never heard of it");
        told.iter().for_each(Draft::discard);
    }

    /// One time of a meeting made elsewhere, by its server: nothing drafted
    /// for those it tells, nothing forced, a guest left to you drafted alone,
    /// each told once. Refused while nobody knows yet whether it tells them,
    /// and for a guest told two ways.
    #[test]
    fn one_time_left_out_tells_its_guests_by_its_server() {
        let here = Here::new("one-time-served", "camille.os@example.org");
        let zone = sioul_core::zone::current();
        let (calendar, state) = served(&here, "one-time-served-cal", "camille.os@example.org");
        let theirs = planning("off-2", "RRULE:FREQ=WEEKLY\r\nORGANIZER;CN=Camille:mailto:camille.os@example.org\r\nATTENDEE;CN=Anne;PARTSTAT=ACCEPTED:mailto:anne@example.net\r\nATTENDEE;CN=Bob;SCHEDULE-AGENT=CLIENT:mailto:bob@example.net\r\n");
        let path = calendar.dir.join("weekly.ics");
        vdir::store_item(&path, &theirs).unwrap();
        agreed(&calendar, &path, &theirs);
        let (first, _) = agenda::first_times(&theirs, &zone).unwrap();
        let asked = time_plan(&path, &theirs, first + 7 * 86400, &calendar).unwrap();
        assert!(asked.significant && asked.forced.is_empty());
        assert_eq!(asked.asked(), [say("invite-ask-drafts", &[("who", "Bob".into())]), say("invite-ask-served", &[("who", "Anne".into())])].join(" "));
        let written = write(&asked).unwrap();
        assert_eq!(written.served.iter().map(|g| g.address.as_str()).collect::<Vec<_>>(), ["anne@example.net"]);
        let drafted = drafts_of_uid("off-2");
        assert!(drafted.len() == 1 && drafted[0].to.len() == 1 && drafted[0].to[0].contains("bob@") && flat(drafted[0].calendar.as_deref().unwrap()).contains("RECURRENCE-ID"), "{drafted:?}");
        // Sent, its server has it: Anne told back by its server, Bob by a draft.
        let after = std::fs::read_to_string(&path).unwrap();
        agreed(&calendar, &path, &after);
        drafted.iter().for_each(Draft::discard);
        assert_eq!(put_back(&written), Ok(()));
        let back = flat(&std::fs::read_to_string(&path).unwrap());
        assert!(!back.contains("EXDATE") && back.contains("SEQUENCE:2") && !back.contains("SCHEDULE-FORCE-SEND"), "{back}");
        let told = drafts_of_uid("off-2");
        assert!(told.len() == 1 && told[0].to.iter().all(|t| t.contains("bob@")), "{told:?}");
        told.iter().for_each(Draft::discard);
        // Bob marked two ways: refused where a server schedules.
        let mixed = theirs.replace("END:VCALENDAR", "BEGIN:VEVENT\r\nUID:off-2\r\nRECURRENCE-ID:20991117T090000Z\r\nDTSTAMP:20991101T090000Z\r\nDTSTART:20991117T090000Z\r\nDTEND:20991117T100000Z\r\nSUMMARY:Planning\r\nORGANIZER:mailto:camille.os@example.org\r\nATTENDEE:mailto:bob@example.net\r\nEND:VEVENT\r\nEND:VCALENDAR");
        assert_eq!(time_plan(&path, &mixed, first + 7 * 86400, &calendar).map(|_| ()), Err(say("invite-marks-mixed", &[("who", "Bob".into())])));
        std::fs::remove_file(state.join("scheduling.toml")).unwrap();
        assert_eq!(time_plan(&path, &theirs, first + 7 * 86400, &calendar).map(|_| ()), Err(tr().text("invite-server-unknown", None)));
        gone_served(&calendar, &state);
    }

    // ---- The review of f85f504: its probes turned.

    /// M6 (`probe_o1`): Cancelled by mistake, its cancellation unsent, then
    /// Confirmed from the chooser: nobody heard of it, so nothing is asked and
    /// nothing is drafted, and the unsent cancellation goes. Undo: it is back in
    /// Drafts beside the meeting cancelled again, and its note with it.
    #[test]
    fn confirmed_again_withdraws_a_cancellation_still_unsent() {
        let here = Here::new("m6-unsent", "camille.m6@example.org");
        let path = here.meeting("m6.ics", &planning("m6", ""), &anne());
        let before = std::fs::read_to_string(&path).unwrap();
        let cancelled = write(&status_plan(&path, &before, "cancelled", &here.calendar).unwrap().unwrap()).unwrap();
        assert_eq!(organizer::cancellation_notes("m6").len(), 1, "noted");
        let off = std::fs::read_to_string(&path).unwrap();
        let on = status_plan(&path, &off, "confirmed", &here.calendar).unwrap().unwrap();
        assert_eq!(on.asked(), "", "nobody hears of it");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), off, "a plan writes nothing");
        assert!(Draft::by_id(&cancelled.drafts[0]).is_some(), "a plan takes nothing out of Drafts");
        let written = write(&on).unwrap();
        assert!(written.drafts.is_empty() && drafts_of_uid("m6").is_empty(), "{:?}", drafts_of_uid("m6"));
        assert!(organizer::cancellation_notes("m6").is_empty(), "its story is over");
        assert_eq!(agenda::status_of(&std::fs::read_to_string(&path).unwrap()), "confirmed");
        assert_eq!(put_back(&written), Ok(()));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), off, "cancelled again");
        let back = drafts_of_uid("m6");
        assert!(back.len() == 1 && back[0].calendar.as_deref().unwrap().contains("METHOD:CANCEL"), "{back:?}");
        assert_eq!(organizer::cancellation_notes("m6").len(), 1, "noted again");
        back.iter().for_each(Draft::discard);
        assert!(organizer::cancellations_settled(&before).len() == 1);
    }

    /// M6's other cases. Its cancellation sent: told back as any change, past
    /// it. Its cancellation unsent, the status not the one they last heard: the
    /// draft goes, and they hear the new status. Its draft brought from another
    /// device, what they last heard unknown here: the draft goes, and they are
    /// told: one message more rather than none.
    #[test]
    fn a_status_back_from_cancelled_tells_whom_it_must() {
        let here = Here::new("m6-told", "camille.m6t@example.org");
        let request_of = |uid: &str| {
            let left = drafts_of_uid(uid);
            let said: Vec<String> = left.iter().map(|d| flat(d.calendar.as_deref().unwrap()).lines().filter(|l| l.starts_with("METHOD") || l.starts_with("SEQUENCE")).collect::<Vec<_>>().join(" ")).collect();
            left.iter().for_each(Draft::discard);
            said
        };
        // Sent.
        let path = here.meeting("sent.ics", &planning("m6-sent", ""), &anne());
        let before = std::fs::read_to_string(&path).unwrap();
        write(&status_plan(&path, &before, "cancelled", &here.calendar).unwrap().unwrap()).unwrap();
        drafts_of_uid("m6-sent").iter().for_each(Draft::discard);
        let off = std::fs::read_to_string(&path).unwrap();
        let on = status_plan(&path, &off, "confirmed", &here.calendar).unwrap().unwrap();
        assert!(on.asked().contains("Anne"), "{}", on.asked());
        write(&on).unwrap();
        assert_eq!(request_of("m6-sent"), ["METHOD:REQUEST SEQUENCE:2"], "past the cancellation (1)");
        // Unsent, another status than they last heard.
        let path = here.meeting("other.ics", &planning("m6-other", ""), &anne());
        let before = std::fs::read_to_string(&path).unwrap();
        write(&status_plan(&path, &before, "cancelled", &here.calendar).unwrap().unwrap()).unwrap();
        let off = std::fs::read_to_string(&path).unwrap();
        write(&status_plan(&path, &off, "tentative", &here.calendar).unwrap().unwrap()).unwrap();
        assert_eq!(request_of("m6-other"), ["METHOD:REQUEST SEQUENCE:2"], "one message, no cancellation");
        // Unsent, drafted elsewhere: no note here.
        let path = here.meeting("there.ics", &planning("m6-there", ""), &anne());
        let before = std::fs::read_to_string(&path).unwrap();
        write(&status_plan(&path, &before, "cancelled", &here.calendar).unwrap().unwrap()).unwrap();
        organizer::cancellations_settled(&before);
        let off = std::fs::read_to_string(&path).unwrap();
        write(&status_plan(&path, &off, "confirmed", &here.calendar).unwrap().unwrap()).unwrap();
        assert_eq!(request_of("m6-there"), ["METHOD:REQUEST SEQUENCE:2"], "one message, no cancellation");
    }

    /// M5: what the door drafts is noted, its deletion and one time left out
    /// too, and Undo settles it: only a cancellation drafted here reads as heard.
    #[test]
    fn a_cancellation_is_noted_where_it_is_drafted() {
        let here = Here::new("m5-noted", "camille.m5@example.org");
        let zone = sioul_core::zone::current();
        let path = here.meeting("weekly.ics", &planning("m5-weekly", "RRULE:FREQ=WEEKLY\r\n"), &anne());
        let series = std::fs::read_to_string(&path).unwrap();
        let (first, _) = agenda::first_times(&series, &zone).unwrap();
        let one_time = write(&time_plan(&path, &series, first + 7 * 86400, &here.calendar).unwrap()).unwrap();
        let notes = organizer::cancellation_notes("m5-weekly");
        assert!(notes.len() == 1 && notes[0].time == Some(first + 7 * 86400) && notes[0].to == ["anne@example.net"], "{notes:?}");
        assert_eq!(put_back(&one_time), Ok(()));
        assert!(organizer::cancellation_notes("m5-weekly").is_empty() && drafts_of_uid("m5-weekly").is_empty(), "undone unsent");
        let deleted = delete(&path).unwrap();
        let notes = organizer::cancellation_notes("m5-weekly");
        assert!(notes.len() == 1 && notes[0].time.is_none() && notes[0].sequence == organizer::top_sequence(&series) + 1, "{notes:?}");
        assert_eq!(undelete(&deleted).as_deref(), Some(""));
        assert!(organizer::cancellation_notes("m5-weekly").is_empty() && drafts_of_uid("m5-weekly").is_empty());
    }

    /// `probe_o2`, kept: a series with no guest of its own, Carl asked to one
    /// time alone: that time left out keeps its organizer and tells Carl, once;
    /// Undo tells them back.
    #[test]
    fn a_series_whose_only_guest_is_of_one_time() {
        let here = Here::new("o2-carl", "camille.o2@example.org");
        let text = planning("o2-carl", "RRULE:FREQ=WEEKLY\r\nORGANIZER:mailto:camille.o2@example.org\r\n").replace("END:VCALENDAR", "BEGIN:VEVENT\r\nUID:o2-carl\r\nRECURRENCE-ID:20991117T090000Z\r\nDTSTAMP:20991101T090000Z\r\nSEQUENCE:0\r\nDTSTART:20991117T140000Z\r\nDTEND:20991117T150000Z\r\nSUMMARY:Planning\r\nORGANIZER:mailto:camille.o2@example.org\r\nATTENDEE;CN=Carl;SCHEDULE-AGENT=CLIENT:mailto:carl@example.net\r\nEND:VEVENT\r\nEND:VCALENDAR");
        let path = here.calendar.dir.join("o2.ics");
        vdir::store_item(&path, &text).unwrap();
        let moved = jiff::civil::date(2099, 11, 17).at(14, 0, 0, 0).to_zoned(jiff::tz::TimeZone::UTC).unwrap().timestamp().as_second();
        let written = write(&time_plan(&path, &text, moved, &here.calendar).unwrap()).unwrap();
        let drafted = drafts_of_uid("o2-carl");
        assert!(drafted.len() == 1 && drafted[0].to[0].contains("carl@"), "{drafted:?}");
        drafted.iter().for_each(Draft::discard);
        assert_eq!(put_back(&written), Ok(()));
        let told = drafts_of_uid("o2-carl");
        told.iter().for_each(Draft::discard);
        assert!(flat(&std::fs::read_to_string(&path).unwrap()).contains("carl@"));
        assert_eq!(told.len(), 1, "Carl told back");
    }

    /// `probe_o3`, kept: cancelled, its cancellation unsent: one time left out
    /// is refused; deleted whole, nobody new is told and that cancellation
    /// stays; undeleted, the same text.
    #[test]
    fn a_cancelled_meeting_deleted_tells_nobody_new() {
        let here = Here::new("o3-off", "camille.o3@example.org");
        let zone = sioul_core::zone::current();
        let path = here.meeting("o3.ics", &planning("o3-off", "RRULE:FREQ=WEEKLY\r\n"), &anne());
        let series = std::fs::read_to_string(&path).unwrap();
        let (first, _) = agenda::first_times(&series, &zone).unwrap();
        let written = write(&status_plan(&path, &series, "cancelled", &here.calendar).unwrap().unwrap()).unwrap();
        let off = std::fs::read_to_string(&path).unwrap();
        assert!(time_plan(&path, &off, first + 7 * 86400, &here.calendar).is_err(), "refused");
        let gone = delete(&path).unwrap();
        assert!(gone.drafts.is_empty());
        assert!(Draft::by_id(&written.drafts[0]).is_some(), "the unsent cancellation stays: Anne will hear");
        assert_eq!(organizer::cancellation_notes("o3-off").len(), 1, "and its note");
        assert_eq!(undelete(&gone).as_deref(), Some(""));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), off);
        drafts_of_uid("o3-off").iter().for_each(Draft::discard);
    }

    /// L4 (`probe_o4`): one time left out by a server that schedules, Bob of
    /// the series, excused from that time: the server sends them an update for
    /// the EXDATE, so what is said first names them; no draft for them.
    #[test]
    fn what_is_said_first_names_whom_its_server_tells() {
        let here = Here::new("l4-served", "camille.l4@example.org");
        let (calendar, state) = served(&here, "l4-served-cal", "camille.l4@example.org");
        let theirs = planning("l4", "RRULE:FREQ=WEEKLY\r\nORGANIZER:mailto:camille.l4@example.org\r\nATTENDEE;CN=Anne:mailto:anne@example.net\r\nATTENDEE;CN=Bob:mailto:bob@example.net\r\n").replace("END:VCALENDAR", "BEGIN:VEVENT\r\nUID:l4\r\nRECURRENCE-ID:20991117T090000Z\r\nDTSTAMP:20991101T090000Z\r\nDTSTART:20991117T090000Z\r\nDTEND:20991117T100000Z\r\nSUMMARY:Planning\r\nORGANIZER:mailto:camille.l4@example.org\r\nATTENDEE;CN=Anne:mailto:anne@example.net\r\nEND:VEVENT\r\nEND:VCALENDAR");
        let path = calendar.dir.join("weekly.ics");
        vdir::store_item(&path, &theirs).unwrap();
        agreed(&calendar, &path, &theirs);
        let at = jiff::civil::date(2099, 11, 17).at(9, 0, 0, 0).to_zoned(jiff::tz::TimeZone::UTC).unwrap().timestamp().as_second();
        let asked = time_plan(&path, &theirs, at, &calendar).unwrap();
        let said = asked.asked();
        assert!(said.contains("Anne") && said.contains("Bob"), "{said}");
        assert!(drafts_of(&asked).unwrap().is_empty(), "no draft: both hear from their server");
        gone_served(&calendar, &state);
    }

    /// A guest's answer for one time of a weekly meeting Sioul drafts for:
    /// kept on that time, said with that time; said again, nothing written
    /// (review of 1e8551e, F13).
    #[test]
    fn a_guest_s_answer_for_one_time_is_kept_and_said() {
        let here = Here::new("one-time", "camille.once@example.org");
        let path = here.meeting("weekly.ics", &planning("once-1", "RRULE:FREQ=WEEKLY\r\n"), &anne());
        let reply = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nMETHOD:REPLY\r\nBEGIN:VEVENT\r\nUID:once-1\r\nRECURRENCE-ID:20991117T090000Z\r\nSEQUENCE:0\r\nDTSTAMP:20991101T090000Z\r\nORGANIZER:mailto:camille.once@example.org\r\nATTENDEE;PARTSTAT=DECLINED:mailto:anne@example.net\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let message = sioul_core::card::Card::from_bytes(b"From: Anne <anne@example.net>\r\nTo: camille.once@example.org\r\nSubject: Declined: Planning\r\nMessage-ID: <once-reply@example.net>\r\n\r\nDeclined.\r\n").unwrap();
        let said = replied(reply, &message, &[], &[]).unwrap();
        assert!(said.starts_with(&say("invite-replied-no", &[("who", "Anne".into()), ("title", "Planning".into())])) && said.contains(&tr().text("invite-replied-time", None).split('{').next().unwrap().trim().to_string()), "{said}");
        let text = std::fs::read_to_string(&path).unwrap();
        let unfolded = sioul_core::lines::unfold(&text).join("\n");
        let (series, that_time) = unfolded.split_at(unfolded.rfind("BEGIN:VEVENT").unwrap());
        assert!(series.contains("RRULE") && !series.contains("DECLINED") && that_time.contains("DECLINED") && that_time.contains("RECURRENCE-ID"), "{unfolded}");
        assert_eq!(replied(reply, &message, &[], &[]).as_deref(), Some(said.as_str()), "said again");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text, "nothing written again");
    }

    /// Your own invitation seen in your mail (an invitation naming you as
    /// the organizer of guests): refused while nothing proves you sent it, a
    /// copy in Sent or a forged one, said in one sentence; sent by you (its
    /// From yours and its organizer, DMARC passing for your domain), added
    /// through the door, as its guests may read it, each guest left to you
    /// (review of 1e8551e, F6).
    #[test]
    fn your_own_invitation_is_added_only_when_you_sent_it() {
        let here = Here::new("own-invite", "camille.invite@example.org");
        let invitation = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nMETHOD:REQUEST\r\nBEGIN:VEVENT\r\nUID:own-invite-1\r\nDTSTAMP:20991101T090000Z\r\nSEQUENCE:0\r\nDTSTART:20991110T090000Z\r\nDTEND:20991110T100000Z\r\nSUMMARY:Board\r\nX-MOZ-GENERATION:2\r\nORGANIZER;CN=Camille:mailto:camille.invite@example.org\r\nATTENDEE;PARTSTAT=NEEDS-ACTION;RSVP=TRUE:mailto:anne@example.net\r\nATTENDEE;PARTSTAT=NEEDS-ACTION;RSVP=TRUE:mailto:bob@example.net\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        assert!(agenda::scheduled_for_you(invitation, &agenda::own_addresses()));
        let message = |from: &str, results: &str| {
            let raw = format!("From: Camille <{from}>\r\nTo: anne@example.net\r\nSubject: Invitation: Board\r\nMessage-ID: <own-invite@example.org>\r\n{results}\r\nInvited.\r\n");
            sioul_core::card::Card::from_bytes(raw.as_bytes()).unwrap()
        };
        let trusted = vec!["mx.example.org".to_string()];
        let target = here.calendar.dir.join("board.ics");
        let refused = tr().text("invitation-organizer-unproven", None);
        for unproven in [message("camille.invite@example.org", ""), message("camille.invite@example.org", "Authentication-Results: mx.example.org; spf=pass smtp.mailfrom=example.org; dmarc=fail header.from=example.org\r\n"), message("eve@example.net", "Authentication-Results: mx.example.org; dmarc=pass header.from=example.net\r\n")] {
            assert_eq!(adopt(&target, invitation, &unproven, &trusted), Err(refused.clone()));
            assert!(!target.exists(), "nothing stored");
        }
        let sent = message("camille.invite@example.org", "Authentication-Results: mx.example.org; dkim=pass header.d=example.org; dmarc=pass header.from=example.org\r\n");
        adopt(&target, invitation, &sent, &trusted).unwrap();
        let text = std::fs::read_to_string(&target).unwrap();
        let unfolded = sioul_core::lines::unfold(&text).join("\n");
        assert!(agenda::organized_meeting(&text, &agenda::own_addresses()), "{unfolded}");
        assert_eq!(unfolded.matches("SCHEDULE-AGENT=CLIENT").count(), 2, "each guest left to you: {unfolded}");
        assert!(!unfolded.contains("METHOD") && !unfolded.contains("X-SIOUL") && unfolded.contains("X-MOZ-GENERATION:2"), "another program's line kept (N6): {unfolded}");
    }

    /// Who tells the guests not left to you, by what the calendar's server
    /// last said clearly: never asked, or a calendar whose owner it does not
    /// say, unknown; one it says is not yours, or a server that does not
    /// schedule for your address, your drafts; yours on a server that
    /// schedules for your address, the server (review of 1e8551e, F18, F1).
    #[test]
    fn who_tells_the_guests_not_left_to_you() {
        sioul_core::config::stand_ins_only();
        let account = format!("unknown-owner-{}", std::process::id());
        let calendar = vdir::create(Kind::Calendars, &account, "Shared, owner unsaid", None, &["VEVENT"]).unwrap();
        assert_eq!(server_for(&calendar, "camille@example.org"), Teller::Unknown, "never asked");
        let state = sioul_core::config::state_dir().join("dav").join(&account);
        std::fs::create_dir_all(&state).unwrap();
        let said = |auto: bool| std::fs::write(state.join("scheduling.toml"), format!("auto = {auto}\naddresses = [\"camille@example.org\"]\nchecked = 4102444800\n")).unwrap();
        said(true);
        let marks = vdir::state_path(&calendar.account, Kind::Calendars, &calendar.id);
        let owned = |owned: Option<bool>| {
            let mut kept = vdir::State::load(&marks);
            kept.owned = owned;
            kept.save(&marks).unwrap();
        };
        // Shared with you: Nextcloud from version 30 schedules for you there, 29 does not (N4).
        for (whose, teller) in [(None, Teller::Unknown), (Some(false), Teller::Unknown), (Some(true), Teller::Server)] {
            owned(whose);
            assert_eq!(server_for(&calendar, "camille@example.org"), teller, "{whose:?}");
        }
        assert_eq!(server_for(&calendar, "other@example.org"), Teller::Drafts, "not an address it schedules for");
        said(false);
        assert_eq!(server_for(&calendar, "camille@example.org"), Teller::Drafts, "it does not schedule");
        let _ = std::fs::remove_dir_all(Kind::Calendars.root().join(&account));
        let _ = std::fs::remove_dir_all(&state);
    }

    /// A Google calendar (its account signs in with Google): who tells its
    /// guests not left to you is unknown unless its server answers the check
    /// clearly that it schedules for your address there, which it is then
    /// trusted with; a calendar of this device's own has no server: drafts
    /// (the second review's N10).
    #[test]
    fn a_google_calendar_is_never_guessed() {
        let here = Here::new("n10-google", "camille.google@example.org");
        let id = format!("n10-google-{}", std::process::id());
        let config = here.dir.join("config.toml");
        std::fs::write(&config, format!("language = \"en\"\n\n[[account]]\nid = \"{id}\"\nkind = \"dav\"\nauth = \"google\"\naddress = \"camille.google@example.org\"\n")).unwrap();
        let calendar = vdir::create(Kind::Calendars, &id, "Google", None, &["VEVENT"]).unwrap();
        assert_eq!(server_for(&calendar, "camille.google@example.org"), Teller::Unknown, "never asked");
        let state = sioul_core::config::state_dir().join("dav").join(&id);
        std::fs::create_dir_all(&state).unwrap();
        std::fs::write(state.join("scheduling.toml"), "auto = false\naddresses = []\nchecked = 4102444800\n").unwrap();
        assert_eq!(server_for(&calendar, "camille.google@example.org"), Teller::Unknown, "no word of scheduling: Google may mail all the same");
        std::fs::write(state.join("scheduling.toml"), "auto = true\naddresses = [\"camille.google@example.org\"]\nchecked = 4102444800\n").unwrap();
        let marks = vdir::state_path(&calendar.account, Kind::Calendars, &calendar.id);
        let mut kept = vdir::State::load(&marks);
        kept.owned = Some(true);
        kept.save(&marks).unwrap();
        assert_eq!(server_for(&calendar, "camille.google@example.org"), Teller::Server, "a clear answer");
        assert_eq!(server_for(&here.calendar, "camille.google@example.org"), Teller::Drafts, "this device's own");
        gone_served(&calendar, &state);
    }

    /// A calendar of a server that schedules for your address, yours there
    /// (`Teller::Server`), for one test.
    fn served(here: &Here, name: &str, address: &str) -> (Collection, PathBuf) {
        let account = format!("{name}-{}", std::process::id());
        let calendar = vdir::create(Kind::Calendars, &account, name, None, &["VEVENT"]).unwrap();
        let state = sioul_core::config::state_dir().join("dav").join(&account);
        std::fs::create_dir_all(&state).unwrap();
        std::fs::write(state.join("scheduling.toml"), format!("auto = true\naddresses = [\"{address}\"]\nchecked = 4102444800\n")).unwrap();
        let marks = vdir::state_path(&calendar.account, Kind::Calendars, &calendar.id);
        let mut kept = vdir::State::load(&marks);
        kept.owned = Some(true);
        kept.save(&marks).unwrap();
        assert_eq!(server_for(&calendar, address), Teller::Server);
        let _ = &here.dir;
        (calendar, state)
    }

    fn gone_served(calendar: &Collection, state: &Path) {
        let _ = std::fs::remove_dir_all(Kind::Calendars.root().join(&calendar.account));
        let _ = std::fs::remove_dir_all(state);
    }

    fn drafts_of_uid(uid: &str) -> Vec<Draft> {
        Draft::all().into_iter().filter(|d| d.calendar.as_deref().is_some_and(|ics| ics.contains(&format!("UID:{uid}")))).collect()
    }

    /// A meeting made in Sioul (every guest left to you), in a calendar whose
    /// server schedules: written on a device whatever its check says, moved,
    /// a guest added; each guest's line kept as it is, the new one left to you
    /// too, and every message drafted, nothing left to the server: no server
    /// cancels it for them (review of 1e8551e, F1, the probe
    /// `probe_server_then_draft_adds_client_untold` turned; F2 its other way).
    #[test]
    fn a_meeting_made_in_sioul_is_told_by_your_drafts_alone() {
        let here = Here::new("made-here", "camille.made@example.org");
        let (calendar, state) = served(&here, "made-here-served", "camille.made@example.org");
        let made = planning("made-here-1", "ORGANIZER;CN=Camille:mailto:camille.made@example.org\r\nATTENDEE;CN=Anne;PARTSTAT=ACCEPTED;SCHEDULE-AGENT=CLIENT:mailto:anne@example.net\r\n");
        let path = calendar.dir.join("made.ics");
        vdir::store_item(&path, &made).unwrap();
        let moved = made.replace("DTSTART:20991110T090000Z", "DTSTART:20991110T100000Z").replace("DTEND:20991110T100000Z", "DTEND:20991110T110000Z");
        let guests = [anne()[0].clone(), Guest { address: "bob@example.net".into(), name: "Bob".into() }];
        let asked = plan(&path, Some(&made), &moved, Some(&guests), &calendar).unwrap();
        let text = sioul_core::lines::unfold(&asked.text).join("\n");
        assert!(text.contains("ATTENDEE;CN=Anne;PARTSTAT=ACCEPTED;SCHEDULE-AGENT=CLIENT:mailto:anne@example.net"), "Anne's line as it was: {text}");
        assert!(text.contains("SCHEDULE-AGENT=CLIENT:mailto:bob@example.net"), "Bob left to you too: {text}");
        assert!(!text.contains("SCHEDULE-FORCE-SEND"), "{text}");
        assert_eq!(asked.asked(), say("invite-ask-drafts", &[("who", "Anne, Bob".into())]));
        let written = write(&asked).unwrap();
        assert!(written.served.is_empty() && drafts_of_uid("made-here-1").len() == 1, "drafted, never the server's");
        drafts_of_uid("made-here-1").iter().for_each(Draft::discard);
        gone_served(&calendar, &state);
    }

    /// A meeting made elsewhere (no guest left to you) in a calendar whose
    /// server schedules, moved in Sioul: told by the server, nothing drafted,
    /// no mark added; a guest added unmarked, told by it too; its new place
    /// alone, which the server would not send, asked of it once
    /// (`SCHEDULE-FORCE-SEND=REQUEST` on the first event's kept guests, for
    /// that write alone), never drafted too; the next write asks it nothing
    /// again (review of 1e8551e, F1, F2, F3: the probes
    /// `probe_draft_then_server_keeps_client` and the new place turned).
    #[test]
    fn a_meeting_made_elsewhere_is_told_by_its_server_once() {
        let here = Here::new("made-elsewhere", "camille.there@example.org");
        let (calendar, state) = served(&here, "made-elsewhere-served", "camille.there@example.org");
        let theirs = planning("elsewhere-1", "LOCATION:Room 2\r\nORGANIZER;CN=Camille:mailto:camille.there@example.org\r\nATTENDEE;CN=Anne;PARTSTAT=ACCEPTED:mailto:anne@example.net\r\n");
        let path = calendar.dir.join("elsewhere.ics");
        vdir::store_item(&path, &theirs).unwrap();
        // Moved: the server's to send.
        let moved = theirs.replace("DTSTART:20991110T090000Z", "DTSTART:20991110T100000Z").replace("DTEND:20991110T100000Z", "DTEND:20991110T110000Z");
        let asked = plan(&path, Some(&theirs), &moved, None, &calendar).unwrap();
        assert!(asked.significant && asked.forced.is_empty() && !sioul_core::lines::unfold(&asked.text).join("").contains("SCHEDULE-"), "{}", asked.text);
        assert_eq!(asked.asked(), say("invite-ask-served", &[("who", "Anne".into())]));
        let written = write(&asked).unwrap();
        assert!(written.drafts.is_empty() && written.served.len() == 1, "the server tells Anne, nothing drafted");
        // Bob added: unmarked, as Anne is.
        let now = std::fs::read_to_string(&path).unwrap();
        let bob = [anne()[0].clone(), Guest { address: "bob@example.net".into(), name: "Bob".into() }];
        let added = plan(&path, Some(&now), &now, Some(&bob), &calendar).unwrap();
        let flat = sioul_core::lines::unfold(&added.text).join("\n");
        assert!(flat.contains("mailto:bob@example.net") && !flat.contains("SCHEDULE-AGENT"), "{flat}");
        assert_eq!(added.asked(), say("invite-ask-served", &[("who", "Bob".into())]));
        // Its place alone: asked of the server once, for Anne, never drafted.
        let placed = now.replace("LOCATION:Room 2", "LOCATION:3 rue de l'Exemple");
        let asked = plan(&path, Some(&now), &placed, None, &calendar).unwrap();
        assert!(asked.told && !asked.significant && asked.forced == ["anne@example.net"], "{asked:?}");
        let unfolded = sioul_core::lines::unfold(&asked.text).join("\n");
        assert!(unfolded.contains("ATTENDEE;CN=Anne;PARTSTAT=ACCEPTED;SCHEDULE-FORCE-SEND=REQUEST:mailto:anne@example.net"), "{unfolded}");
        assert_eq!(asked.asked(), say("invite-ask-served", &[("who", "Anne".into())]));
        let written = write(&asked).unwrap();
        assert!(written.drafts.is_empty() && drafts_of_uid("elsewhere-1").is_empty(), "never both");
        // The next write, its server's copy not back yet: the force asked stays, nothing more asked (N1).
        let after = std::fs::read_to_string(&path).unwrap();
        let again = plan(&path, Some(&after), &after.replace("DTSTART:20991110T100000Z", "DTSTART:20991110T110000Z").replace("DTEND:20991110T110000Z", "DTEND:20991110T120000Z"), None, &calendar).unwrap();
        assert!(sioul_core::lines::unfold(&again.text).join("").contains("SCHEDULE-FORCE-SEND=REQUEST:mailto:anne") && again.forced.is_empty(), "{}", again.text);
        // Never asked here (a new device): a change for Anne refused, said; nothing guessed.
        let _ = std::fs::remove_file(state.join("scheduling.toml"));
        assert_eq!(plan(&path, Some(&after), &after.replace("LOCATION:3 rue", "LOCATION:4 rue"), None, &calendar).map(|p| p.asked()), Err(tr().text("invite-server-unknown", None)));
        assert_eq!(delete(&path).map(|_| ()), Err(tr().text("invite-server-unknown", None)));
        gone_served(&calendar, &state);
    }

    /// A meeting whose guests are told both ways (Anne left to you, Bob not),
    /// its place changed in a calendar whose server schedules: Anne's update
    /// drafted, Bob's asked of the server; a guest added left to you, as the
    /// meeting already leaves one; deleted, Anne's cancellation drafted and
    /// Bob's the server's; Undo after Anne's went invites Anne again by draft
    /// and says the server invites Bob again.
    #[test]
    fn a_meeting_told_both_ways_tells_each_guest_once() {
        let here = Here::new("mixed", "camille.mixed@example.org");
        let (calendar, state) = served(&here, "mixed-served", "camille.mixed@example.org");
        let mixed = planning("mixed-1", "LOCATION:Room 2\r\nORGANIZER;CN=Camille:mailto:camille.mixed@example.org\r\nATTENDEE;CN=Anne;SCHEDULE-AGENT=CLIENT:mailto:anne@example.net\r\nATTENDEE;CN=Bob:mailto:bob@example.net\r\n");
        let path = calendar.dir.join("mixed.ics");
        vdir::store_item(&path, &mixed).unwrap();
        let placed = mixed.replace("LOCATION:Room 2", "LOCATION:Hall");
        let asked = plan(&path, Some(&mixed), &placed, None, &calendar).unwrap();
        assert_eq!(asked.forced, ["bob@example.net"]);
        assert_eq!(asked.asked(), format!("{} {}", say("invite-ask-drafts", &[("who", "Anne".into())]), say("invite-ask-served", &[("who", "Bob".into())])));
        let written = write(&asked).unwrap();
        let drafted = drafts_of_uid("mixed-1");
        assert!(drafted.len() == 1 && drafted[0].to.len() == 1 && drafted[0].to[0].contains("anne@example.net"), "Anne's alone: {drafted:?}");
        assert_eq!(written.served.iter().map(|g| g.address.as_str()).collect::<Vec<_>>(), ["bob@example.net"]);
        drafted.iter().for_each(Draft::discard);
        let now = std::fs::read_to_string(&path).unwrap();
        let carl = [anne()[0].clone(), Guest { address: "bob@example.net".into(), name: "Bob".into() }, Guest { address: "carl@example.net".into(), name: "Carl".into() }];
        let with_carl = sioul_core::lines::unfold(&plan(&path, Some(&now), &now, Some(&carl), &calendar).unwrap().text).join("\n");
        assert!(with_carl.contains("SCHEDULE-AGENT=CLIENT:mailto:carl@example.net"), "Carl left to you, as Anne is: {with_carl}");
        let gone = delete(&path).unwrap();
        let cancel = Draft::by_id(&gone.drafts[0]).unwrap();
        assert!(cancel.to.len() == 1 && cancel.to[0].contains("anne@example.net") && cancel.calendar.as_deref().unwrap().contains("METHOD:CANCEL"));
        assert_eq!(gone.served.iter().map(|g| g.address.as_str()).collect::<Vec<_>>(), ["bob@example.net"]);
        cancel.discard();
        let said = undelete(&gone).unwrap();
        assert_eq!(said, format!("{} {}", tr().text("invite-undeleted-draft", None), tr().text("invite-undeleted-server", None)));
        let again = drafts_of_uid("mixed-1");
        assert!(again.len() == 1 && again[0].to.len() == 1 && again[0].to[0].contains("anne@example.net"), "{again:?}");
        again.iter().for_each(Draft::discard);
        gone_served(&calendar, &state);
    }

    /// Meetings you organize that an older Sioul wrote its own lines and ties
    /// into: written once without them, what they held moved into your own
    /// files (your own time where none was kept, your ties), another
    /// program's alarm left as it is; then marked done (review of 1e8551e, F5).
    /// The once-only pass's tests, one at a time: each writes its mark.
    static TIDY: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn the_once_only_pass_takes_sioul_s_lines_out_of_your_meetings() {
        let _one = TIDY.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let here = Here::new("tidied", "camille.tidy@example.org");
        let mark = sioul_core::config::state_dir().join("organized-tidied");
        let _ = std::fs::remove_file(&mark);
        let old = planning("tidied-1", "X-SIOUL-BEFORE:20\r\nX-SIOUL-REMIND:45\r\nX-SIOUL-LINK;LINKREL=via:mid:tidied@mail.example\r\nX-SIOUL-REFID:tidied-project\r\nORGANIZER:mailto:camille.tidy@example.org\r\nATTENDEE;SCHEDULE-AGENT=CLIENT:mailto:anne@example.net\r\nBEGIN:VALARM\r\nACTION:DISPLAY\r\nTRIGGER:-PT15M\r\nEND:VALARM\r\n");
        let path = here.calendar.dir.join("tidied.ics");
        vdir::store_item(&path, &old).unwrap();
        assert!(tidy_once() >= 1);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("X-SIOUL") && text.contains("BEGIN:VALARM") && text.contains("SCHEDULE-AGENT=CLIENT"), "{text}");
        let own = sioul_core::own_time::load(&sioul_core::own_time::default_path()).events.get("tidied-1").cloned().unwrap_or_default();
        assert_eq!((own.before, own.remind.as_str()), (20, "45"));
        let ties = sioul_core::links::LocalLinks::load(&sioul_core::links::LocalLinks::default_path());
        assert!(ties.links.iter().any(|e| e.from == "sioul:event/tidied-1" && e.to == "mid:tidied@mail.example" && e.how == "source"));
        assert!(ties.links.iter().any(|e| e.from == "sioul:event/tidied-1" && e.to == "sioul:project/tidied-project"));
        assert_eq!(std::fs::read_to_string(&mark).unwrap(), "2");
        vdir::store_item(&path, &old).unwrap();
        assert_eq!(tidy_once(), 0, "once");
        let _ = sioul_core::own_time::set(&sioul_core::own_time::default_path(), "tidied-1", None);
        let _ = sioul_core::links::LocalLinks::change(&sioul_core::links::LocalLinks::default_path(), |l| {
            let n = l.links.len();
            l.links.retain(|e| e.from != "sioul:event/tidied-1");
            l.links.len() != n
        });
    }

    // ---- The second review of the organizer's door (6 November 2026): each probe turned.

    fn flat(text: &str) -> String {
        sioul_core::lines::unfold(text).join("\n")
    }

    /// The sync's last word on the file at `path` of `calendar`: its server agreed on `text`.
    fn agreed(calendar: &Collection, path: &Path, text: &str) {
        let marks = vdir::state_path(&calendar.account, Kind::Calendars, &calendar.id);
        let mut kept = vdir::State::load(&marks);
        let file = path.file_name().unwrap().to_string_lossy().to_string();
        kept.items.retain(|i| i.file != file);
        kept.items.push(vdir::ItemState { href: format!("/cal/{file}"), file, etag: "\"1\"".into(), hash: vdir::content_hash(text.as_bytes()), ..vdir::ItemState::default() });
        kept.save(&marks).unwrap();
    }

    /// N1 (the probe `probe2_app_force_lost_by_the_next_write` turned): a
    /// meeting made elsewhere, its place changed, its server asked to tell
    /// Anne; before its sync sends it, Bob added, then a write telling
    /// nobody: Anne's force stays in the file through both, until its
    /// server's copy comes back without it; then nothing asks it again.
    #[test]
    fn a_force_asked_stays_until_its_server_has_it() {
        let here = Here::new("n1-force", "camille.n1@example.org");
        let (calendar, state) = served(&here, "n1-force-served", "camille.n1@example.org");
        let original = planning("n1-force-1", "LOCATION:Room 2\r\nORGANIZER;CN=Camille:mailto:camille.n1@example.org\r\nATTENDEE;CN=Anne;PARTSTAT=ACCEPTED:mailto:anne@example.net\r\n");
        let path = calendar.dir.join("force.ics");
        vdir::store_item(&path, &original).unwrap();
        let first = plan(&path, Some(&original), &original.replace("LOCATION:Room 2", "LOCATION:Hall"), None, &calendar).unwrap();
        assert_eq!(first.forced, ["anne@example.net"]);
        write(&first).unwrap();
        let now = std::fs::read_to_string(&path).unwrap();
        let bob = [anne()[0].clone(), Guest { address: "bob@example.net".into(), name: "Bob".into() }];
        let second = plan(&path, Some(&now), &now, Some(&bob), &calendar).unwrap();
        assert_eq!(second.asked(), say("invite-ask-served", &[("who", "Bob".into())]));
        write(&second).unwrap();
        let last = std::fs::read_to_string(&path).unwrap();
        assert!(flat(&last).contains("ATTENDEE;CN=Anne;PARTSTAT=ACCEPTED;SCHEDULE-FORCE-SEND=REQUEST:mailto:anne@example.net"), "Anne's force kept for the push: {}", flat(&last));
        let quiet = plan(&path, Some(&last), &last, None, &calendar).unwrap();
        assert!(!quiet.tells() && flat(&quiet.text).contains("SCHEDULE-FORCE-SEND=REQUEST:mailto:anne"), "a write telling nobody keeps it: {}", quiet.text);
        // Its server's copy, without it (it told Anne): nothing asks it again.
        let theirs = last.replace(";SCHEDULE-FORCE-SEND=REQUEST", ";SCHEDULE-STATUS=1.2");
        vdir::store_item(&path, &theirs).unwrap();
        let after = plan(&path, Some(&theirs), &theirs.replace("DTSTART:20991110T090000Z", "DTSTART:20991110T100000Z").replace("DTEND:20991110T100000Z", "DTEND:20991110T110000Z"), None, &calendar).unwrap();
        assert!(!flat(&after.text).contains("SCHEDULE-FORCE-SEND") && after.significant, "{}", after.text);
        gone_served(&calendar, &state);
    }

    /// R3 and R6 of the third review (their scenarios turned): Anne's new
    /// place forced on Anne's line; the sync sent it, its server's answer saying
    /// no ETag and its copy not back yet (the sync agreed on the file as it
    /// is). Bob added then: Anne's force goes, the server sent it already,
    /// and only Bob hears. Undo of that write once pushed: the place stays
    /// Hall without any force, Anne hearing nothing again, Bob taken off by
    /// the server.
    #[test]
    fn a_force_sent_is_never_asked_again() {
        let here = Here::new("r3-force", "camille.r3@example.org");
        let (calendar, state) = served(&here, "r3-force-served", "camille.r3@example.org");
        let original = planning("r3-force-1", "LOCATION:Room 2\r\nORGANIZER;CN=Camille:mailto:camille.r3@example.org\r\nATTENDEE;CN=Anne;PARTSTAT=ACCEPTED:mailto:anne@example.net\r\n");
        let path = calendar.dir.join("force.ics");
        vdir::store_item(&path, &original).unwrap();
        agreed(&calendar, &path, &original);
        let first = plan(&path, Some(&original), &original.replace("LOCATION:Room 2", "LOCATION:Hall"), None, &calendar).unwrap();
        assert_eq!(first.forced, ["anne@example.net"]);
        write(&first).unwrap();
        let sent = std::fs::read_to_string(&path).unwrap();
        // Pushed, its ETag unsaid: the sync's last word is this text, its copy not back.
        agreed(&calendar, &path, &sent);
        let bob = [anne()[0].clone(), Guest { address: "bob@example.net".into(), name: "Bob".into() }];
        let second = plan(&path, Some(&sent), &sent, Some(&bob), &calendar).unwrap();
        assert!(!flat(&second.text).contains("SCHEDULE-FORCE-SEND"), "Anne's force sent already: {}", second.text);
        assert_eq!(second.asked(), say("invite-ask-served", &[("who", "Bob".into())]), "Bob alone hears of it");
        let written = write(&second).unwrap();
        let after = std::fs::read_to_string(&path).unwrap();
        agreed(&calendar, &path, &after);
        assert_eq!(put_back(&written), Ok(()));
        let back = flat(&std::fs::read_to_string(&path).unwrap());
        assert!(back.contains("LOCATION:Hall") && !back.contains("SCHEDULE-FORCE-SEND") && !back.contains("bob@example.net"), "nothing asked again for Anne: {back}");
        gone_served(&calendar, &state);
    }

    /// N2 (the probe `probe2_app_nbsp_in_notes_tells_every_guest` turned):
    /// French notes another program wrote, given back by the form as Qt
    /// gives them (a no-break space as a space), a margin changed and Bob
    /// added: Anne, kept, hears nothing; Bob alone is invited; the notes
    /// stay as they were written.
    #[test]
    fn notes_given_back_by_the_form_tell_nobody() {
        let here = Here::new("n2-nbsp", "camille.n2@example.org");
        let (calendar, state) = served(&here, "n2-nbsp-served", "camille.n2@example.org");
        let notes = "DESCRIPTION:Salle\u{a0}2 : apportez «\u{a0}les plans\u{a0}»";
        let theirs = planning("n2-nbsp-1", &format!("{notes}\r\nORGANIZER;CN=Camille:mailto:camille.n2@example.org\r\nATTENDEE;CN=Anne;PARTSTAT=ACCEPTED:mailto:anne@example.net\r\n"));
        let path = calendar.dir.join("nbsp.ics");
        vdir::store_item(&path, &theirs).unwrap();
        let zone = sioul_core::zone::current();
        let mut edit = agenda::edit_of_text(&theirs, &zone).unwrap();
        edit.notes = edit.notes.replace('\u{a0}', " ");
        edit.margins.before = 10;
        let applied = agenda::apply(&theirs, &edit, &zone).unwrap();
        let bob = [anne()[0].clone(), Guest { address: "bob@example.net".into(), name: "Bob".into() }];
        let asked = plan(&path, Some(&theirs), &applied, Some(&bob), &calendar).unwrap();
        assert!(!asked.told && asked.forced.is_empty(), "{asked:?}");
        assert_eq!(asked.asked(), say("invite-ask-served", &[("who", "Bob".into())]));
        assert!(flat(&asked.text).contains(notes), "the notes as written: {}", asked.text);
        gone_served(&calendar, &state);
    }

    /// N7 (the probe `probe2_app_undo_before_push_forces_again` turned): a
    /// meeting made elsewhere, its place changed (its server asked to tell
    /// Anne), undone before its sync sent it: as it was, nothing asked of its
    /// server. Sent (its sync agreed on it), Undo tells Anne back through
    /// the server, one SEQUENCE past what Anne received. Its server's copy
    /// back (it told Anne): Undo says so, and puts nothing back.
    #[test]
    fn undo_tells_back_only_those_who_heard() {
        let here = Here::new("n7-undo", "camille.n7@example.org");
        let (calendar, state) = served(&here, "n7-undo-served", "camille.n7@example.org");
        let original = planning("n7-undo-1", "LOCATION:Room 2\r\nORGANIZER;CN=Camille:mailto:camille.n7@example.org\r\nATTENDEE;CN=Anne;PARTSTAT=ACCEPTED:mailto:anne@example.net\r\n");
        let path = calendar.dir.join("undo.ics");
        vdir::store_item(&path, &original).unwrap();
        agreed(&calendar, &path, &original);
        let hall = |from: &str| write(&plan(&path, Some(from), &from.replace("LOCATION:Room 2", "LOCATION:Hall"), None, &calendar).unwrap()).unwrap();
        // Not sent yet.
        let written = hall(&original);
        assert_eq!(written.served.len(), 1);
        assert_eq!(put_back(&written), Ok(()));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original, "as it was, no force asked");
        // Sent: Anne heard it from the server.
        let written = hall(&original);
        let after = std::fs::read_to_string(&path).unwrap();
        agreed(&calendar, &path, &after);
        assert_eq!(put_back(&written), Ok(()));
        let back = flat(&std::fs::read_to_string(&path).unwrap());
        assert!(back.contains("LOCATION:Room 2") && back.contains("SEQUENCE:1") && back.contains("SCHEDULE-FORCE-SEND=REQUEST:mailto:anne"), "told back, a later version: {back}");
        assert!(drafts_of_uid("n7-undo-1").is_empty(), "never drafted too");
        // Its server's copy back: it told Anne.
        let now = std::fs::read_to_string(&path).unwrap();
        let written = hall(&now);
        let theirs = std::fs::read_to_string(&path).unwrap().replace(";SCHEDULE-FORCE-SEND=REQUEST", ";SCHEDULE-STATUS=1.2");
        vdir::store_item(&path, &theirs).unwrap();
        agreed(&calendar, &path, &theirs);
        assert_eq!(put_back(&written), Err(tr().text("invite-undo-served", None)));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), theirs);
        gone_served(&calendar, &state);
    }

    /// A calendar of a server that schedules for your address, shared with
    /// you (`owned` false), for one test.
    fn shared_with_you(name: &str, address: &str) -> (Collection, PathBuf) {
        let account = format!("{name}-{}", std::process::id());
        let calendar = vdir::create(Kind::Calendars, &account, name, None, &["VEVENT"]).unwrap();
        let state = sioul_core::config::state_dir().join("dav").join(&account);
        std::fs::create_dir_all(&state).unwrap();
        std::fs::write(state.join("scheduling.toml"), format!("auto = true\naddresses = [\"{address}\"]\nchecked = 4102444800\n")).unwrap();
        let marks = vdir::state_path(&calendar.account, Kind::Calendars, &calendar.id);
        let mut kept = vdir::State::load(&marks);
        kept.owned = Some(false);
        kept.save(&marks).unwrap();
        (calendar, state)
    }

    /// N4: a calendar shared with you on a server that schedules for your
    /// address (Nextcloud schedules there from version 30, not before): a
    /// meeting made elsewhere there (Anne unmarked) is neither changed nor
    /// deleted, said; one made in Sioul (Anne left to you) is drafted as
    /// anywhere. And a calendar of this device's own, carried by the sharing
    /// or not (no server): a meeting there with a guest unmarked (an older
    /// import) is drafted for, said so, never "try again after its next
    /// sync" (N10).
    #[test]
    fn a_calendar_shared_with_you_is_never_guessed() {
        let here = Here::new("n4-shared", "camille.n4@example.org");
        let (calendar, state) = shared_with_you("n4-shared-cal", "camille.n4@example.org");
        assert_eq!(server_for(&calendar, "camille.n4@example.org"), Teller::Unknown);
        let theirs = planning("n4-theirs", "ORGANIZER;CN=Camille:mailto:camille.n4@example.org\r\nATTENDEE;CN=Anne;PARTSTAT=ACCEPTED:mailto:anne@example.net\r\n");
        let path = calendar.dir.join("theirs.ics");
        vdir::store_item(&path, &theirs).unwrap();
        let moved = theirs.replace("DTSTART:20991110T090000Z", "DTSTART:20991110T100000Z").replace("DTEND:20991110T100000Z", "DTEND:20991110T110000Z");
        assert_eq!(plan(&path, Some(&theirs), &moved, None, &calendar).map(|p| p.asked()), Err(tr().text("invite-server-unknown", None)));
        assert_eq!(delete(&path).map(|_| ()), Err(tr().text("invite-server-unknown", None)));
        let made = theirs.replace("PARTSTAT=ACCEPTED:mailto:anne", "PARTSTAT=ACCEPTED;SCHEDULE-AGENT=CLIENT:mailto:anne").replace("n4-theirs", "n4-made");
        let made_path = calendar.dir.join("made.ics");
        vdir::store_item(&made_path, &made).unwrap();
        let moved = made.replace("DTSTART:20991110T090000Z", "DTSTART:20991110T100000Z").replace("DTEND:20991110T100000Z", "DTEND:20991110T110000Z");
        assert_eq!(plan(&made_path, Some(&made), &moved, None, &calendar).map(|p| p.asked()), Ok(say("invite-ask-drafts", &[("who", "Anne".into())])));
        // This device's own calendar: no server, Sioul drafts.
        assert_eq!(server_for(&here.calendar, "camille.n4@example.org"), Teller::Drafts);
        let local = here.calendar.dir.join("imported.ics");
        let imported = theirs.replace("n4-theirs", "n4-imported");
        vdir::store_item(&local, &imported).unwrap();
        assert_eq!(plan(&local, Some(&imported), &imported.replace("SUMMARY:Planning", "SUMMARY:Plans"), None, &here.calendar).map(|p| p.asked()), Ok(say("invite-ask-drafts", &[("who", "Anne".into())])));
        gone_served(&calendar, &state);
    }

    /// N10: Bob, asked by another program to one time of a weekly meeting
    /// made in Sioul alone: that time moved, Bob's invitation is drafted
    /// with that time alone, Anne's with the meeting; the series' title
    /// changed, which Bob's time does not take, Bob hears nothing; deleted,
    /// Bob's cancellation is drafted too.
    #[test]
    fn a_guest_of_one_time_alone_is_told_of_it() {
        let here = Here::new("n10-once", "camille.once10@example.org");
        let series = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:n10-once-1\r\nDTSTAMP:20991101T090000Z\r\nSEQUENCE:0\r\nDTSTART:20991110T090000Z\r\nDTEND:20991110T100000Z\r\nRRULE:FREQ=WEEKLY\r\nSUMMARY:Weekly\r\nORGANIZER:mailto:camille.once10@example.org\r\nATTENDEE;CN=Anne;SCHEDULE-AGENT=CLIENT:mailto:anne@example.net\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:n10-once-1\r\nRECURRENCE-ID:20991117T090000Z\r\nDTSTAMP:20991101T090000Z\r\nSEQUENCE:0\r\nDTSTART:20991117T090000Z\r\nDTEND:20991117T100000Z\r\nSUMMARY:Weekly\r\nORGANIZER:mailto:camille.once10@example.org\r\nATTENDEE;CN=Anne;SCHEDULE-AGENT=CLIENT:mailto:anne@example.net\r\nATTENDEE;CN=Bob;SCHEDULE-AGENT=CLIENT:mailto:bob@example.net\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let path = here.calendar.dir.join("weekly.ics");
        vdir::store_item(&path, series).unwrap();
        let zone = sioul_core::zone::current();
        let at = |d: i8, h: i8| jiff::civil::date(2099, 11, d).at(h, 0, 0, 0).to_zoned(jiff::tz::TimeZone::UTC).unwrap().timestamp().as_second();
        let moved = agenda::moved(series, at(17, 9), at(17, 11), at(17, 12), true, &zone).unwrap();
        let asked = plan(&path, Some(series), &moved, None, &here.calendar).unwrap();
        assert_eq!(asked.told_guests.iter().map(|g| g.address.as_str()).collect::<Vec<_>>(), ["anne@example.net", "bob@example.net"]);
        let written = write(&asked).unwrap();
        let drafted = drafts_of_uid("n10-once-1");
        let to_bob = drafted.iter().find(|d| d.to.iter().any(|t| t.contains("bob@example.net"))).expect("Bob's");
        let to_anne = drafted.iter().find(|d| d.to.iter().any(|t| t.contains("anne@example.net"))).expect("Anne's");
        let (bob_ics, anne_ics) = (flat(to_bob.calendar.as_deref().unwrap()), flat(to_anne.calendar.as_deref().unwrap()));
        assert!(bob_ics.contains("RECURRENCE-ID:20991117T090000Z") && !bob_ics.contains("RRULE") && to_bob.to.len() == 1, "Bob's time alone: {bob_ics}");
        assert!(anne_ics.contains("RRULE") && to_anne.to.len() == 1, "{anne_ics}");
        drafted.iter().for_each(Draft::discard);
        let _ = written;
        let now = std::fs::read_to_string(&path).unwrap();
        let renamed = {
            let mut edit = agenda::edit_of_text(&now, &zone).unwrap();
            edit.title = "Weekly review".into();
            agenda::apply(&now, &edit, &zone).unwrap()
        };
        let asked = plan(&path, Some(&now), &renamed, None, &here.calendar).unwrap();
        assert_eq!(asked.told_guests.iter().map(|g| g.address.as_str()).collect::<Vec<_>>(), ["anne@example.net"], "Bob's time keeps its own title");
        let gone = delete(&path).unwrap();
        let cancel = Draft::by_id(&gone.drafts[0]).unwrap();
        assert!(cancel.to.iter().any(|t| t.contains("bob@example.net")) && cancel.to.iter().any(|t| t.contains("anne@example.net")), "{cancel:?}");
        cancel.discard();
    }

    /// N10: Anne left to you in the series and not in one time of it
    /// (another program's copy of it), in a calendar whose server
    /// schedules: a change that would tell Anne is refused, said with Anne's
    /// name, since the server would tell Anne of that time and your draft of
    /// the rest; deleting it is the server's to tell Anne (its cancellation
    /// is the whole meeting's), never drafted too.
    #[test]
    fn a_guest_marked_two_ways_is_never_told_twice() {
        let here = Here::new("n10-mixed", "camille.mixed10@example.org");
        let (calendar, state) = served(&here, "n10-mixed-served", "camille.mixed10@example.org");
        let mixed = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:n10-mixed-1\r\nDTSTAMP:20991101T090000Z\r\nSEQUENCE:0\r\nDTSTART:20991110T090000Z\r\nDTEND:20991110T100000Z\r\nRRULE:FREQ=WEEKLY\r\nSUMMARY:Weekly\r\nLOCATION:Room 2\r\nORGANIZER:mailto:camille.mixed10@example.org\r\nATTENDEE;CN=Anne;SCHEDULE-AGENT=CLIENT:mailto:anne@example.net\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:n10-mixed-1\r\nRECURRENCE-ID:20991117T090000Z\r\nDTSTAMP:20991101T090000Z\r\nDTSTART:20991117T090000Z\r\nDTEND:20991117T100000Z\r\nSUMMARY:Weekly\r\nORGANIZER:mailto:camille.mixed10@example.org\r\nATTENDEE;CN=Anne:mailto:anne@example.net\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let path = calendar.dir.join("mixed.ics");
        vdir::store_item(&path, mixed).unwrap();
        assert_eq!(plan(&path, Some(mixed), &mixed.replace("LOCATION:Room 2", "LOCATION:Hall"), None, &calendar).map(|p| p.asked()), Err(say("invite-marks-mixed", &[("who", "Anne".into())])));
        let gone = delete(&path).unwrap();
        assert!(gone.drafts.is_empty() && gone.served.iter().any(|g| g.address == "anne@example.net"), "{gone:?}");
        gone_served(&calendar, &state);
    }

    /// N6 (the probe `probe2_app_tidy_once` turned): the once-only pass over
    /// meetings you organize: someone else's untouched; yours written as
    /// its guests may read them, every guest's line as it was, another
    /// program's alarm and categories kept, nothing a server would mail; a
    /// child's meeting's kind and holder kept in Sioul's own store; a meeting
    /// the first pass (version 1) went over already gets them back from this
    /// device's earlier version of its file.
    #[test]
    fn the_once_only_pass_keeps_what_it_takes_out() {
        let _one = TIDY.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let here = Here::new("n6-tidy", "camille.n6tidy@example.org");
        let mark = sioul_core::config::state_dir().join("organized-tidied");
        std::fs::write(&mark, "1").unwrap();
        let theirs = planning("n6-tidy-theirs", "X-SIOUL-BEFORE:20\r\nORGANIZER:mailto:chair@example.org\r\nATTENDEE:mailto:camille.n6tidy@example.org\r\nATTENDEE:mailto:anne@example.net\r\n");
        let mixed = planning("n6-tidy-mixed", "CATEGORIES:Board\r\nX-SIOUL-REMIND:30\r\nX-SIOUL-LINK;LINKREL=via:mid:x@mail.example\r\nORGANIZER:mailto:camille.n6tidy@example.org\r\nATTENDEE;SCHEDULE-AGENT=CLIENT:mailto:anne@example.net\r\nATTENDEE;PARTSTAT=ACCEPTED:mailto:bob@example.net\r\nBEGIN:VALARM\r\nACTION:DISPLAY\r\nTRIGGER:-PT1H\r\nEND:VALARM\r\n");
        let holder = "X-SIOUL-TAKEN-BY;CN=Sam;PARTSTAT=ACCEPTED:mailto:sam@example.org";
        let kind = "X-SIOUL-CONCEPT:tag:aurelienpierre.com,2026:sioul/child-kind/health";
        let child = planning("n6-tidy-child", &format!("{kind}\r\n{holder}\r\nX-SIOUL-BEFORE:5\r\nORGANIZER:mailto:camille.n6tidy@example.org\r\nATTENDEE;SCHEDULE-AGENT=CLIENT:mailto:dr@example.net\r\n"));
        // Gone over by the first pass already: its lines only in this device's earlier version of it.
        let earlier = planning("n6-tidy-older", &format!("{holder}\r\nORGANIZER:mailto:camille.n6tidy@example.org\r\nATTENDEE;SCHEDULE-AGENT=CLIENT:mailto:dr@example.net\r\n"));
        let older = sioul_core::lines::fold(&sioul_core::lines::unfold(&earlier).into_iter().filter(|l| !l.starts_with("X-SIOUL")).collect::<Vec<_>>());
        let paths: Vec<PathBuf> = [("theirs.ics", &theirs), ("mixed.ics", &mixed), ("child.ics", &child), ("older.ics", &older)]
            .iter()
            .map(|(f, t)| {
                let p = here.calendar.dir.join(f);
                vdir::store_item(&p, t).unwrap();
                p
            })
            .collect();
        let root = sioul_sync::history::root(&sioul_core::config::state_dir().join("share").join("memory.json"));
        let file = format!("data/{}", paths[3].strip_prefix(sioul_core::config::data_dir()).unwrap().to_string_lossy());
        sioul_sync::history::keep_bytes(&root, sioul_sync::history::ACCOUNTS, &file, earlier.as_bytes(), 4_000_000_000_000).unwrap();
        assert!(tidy_once() >= 2);
        let read = |p: &PathBuf| std::fs::read_to_string(p).unwrap();
        assert_eq!(read(&paths[0]), theirs, "someone else's meeting untouched");
        let attendees = |t: &str| sioul_core::lines::unfold(t).into_iter().filter(|l| l.starts_with("ATTENDEE")).collect::<Vec<_>>();
        assert_eq!(attendees(&read(&paths[1])), attendees(&mixed), "every guest's line as it was");
        assert!(!organizer::told(&mixed, &read(&paths[1])) && !organizer::significant(&mixed, &read(&paths[1])));
        assert!(read(&paths[1]).contains("CATEGORIES:Board") && read(&paths[1]).contains("TRIGGER:-PT1H") && !read(&paths[1]).contains("X-SIOUL"), "{}", read(&paths[1]));
        assert!(!read(&paths[2]).contains("child-kind") && !read(&paths[2]).contains("TAKEN-BY"));
        let kept = sioul_core::meeting_lines::load(&sioul_core::meeting_lines::default_path());
        let lines = |uid: &str| kept.events.get(uid).map(|k| k.lines("")).unwrap_or_default();
        assert!(lines("n6-tidy-child").contains(&holder.to_string()) && lines("n6-tidy-child").contains(&kind.to_string()), "{kept:?}");
        assert_eq!(lines("n6-tidy-older"), vec![holder.to_string()], "recovered from its earlier version");
        let merged = sioul_core::meeting_lines::merged(&read(&paths[2]));
        assert_eq!(sioul_core::holders::kind_of(&merged), Some(sioul_core::holders::Kind::Health));
        assert_eq!(std::fs::read_to_string(&mark).unwrap(), "2");
        for uid in ["n6-tidy-child", "n6-tidy-older", "n6-tidy-mixed"] {
            let _ = sioul_core::meeting_lines::forget(&sioul_core::meeting_lines::default_path(), uid);
            let _ = sioul_core::own_time::set(&sioul_core::own_time::default_path(), uid, None);
        }
        let _ = std::fs::remove_dir_all(sioul_sync::history::root(&sioul_core::config::state_dir().join("share").join("memory.json")).join(sioul_sync::history::ACCOUNTS).join("data").join("calendars").join(vdir::LOCAL).join(&here.calendar.id));
    }

    /// A build with the test hooks says so in its binary, for
    /// tools/check-test-marks.sh, whatever other feature it has (review of
    /// 1e8551e, F17): this test's own program is one.
    #[test]
    fn a_build_with_the_test_hooks_carries_their_mark() {
        let program = std::fs::read(std::env::current_exe().unwrap()).unwrap();
        assert!(program.windows(16).any(|w| w == b"SIOUL_TEST_HOOKS"), "SIOUL_TEST_HOOKS in the test's program");
    }
}
