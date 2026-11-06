// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Meetings you organize, by your own acts in the window (decisions 7 and 8
//! of the event card, 5 November 2026): guests invited from the event's
//! form, taken off it, the meeting changed, moved on the timeline or
//! deleted, and a guest's answer read from their REPLY.
//!
//! Every write here goes through the organizer's door
//! (`vdir::write_organized`, `remove_organized`), which this window alone
//! opens (`agenda::open_organizer_door`) and which refuses a meeting someone
//! else organizes. This file is the door's one caller: agents (`sioul mcp`),
//! the command line and the sync go through the ordinary doors, which refuse
//! every meeting with others (`agenda` tests,
//! `the_organizers_door_is_the_persons_alone`).
//!
//! **Who tells the guests** (`sioul_core::organizer`): the calendar's
//! server, when it schedules by itself for your address there
//! (`sioul_sync::scheduling`); else a draft in Mail, which leaves when you
//! send it. Never both.

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

/// Who tells the guests of an event in `calendar` you organize as
/// `organizer`: its server, when it is a server's calendar its server says is
/// yours (one shared with you, or whose owner it never said, is not: a server
/// schedules with its owner's addresses, review of 1e8551e, F18) and whose
/// server schedules for that address; else you, by a draft.
pub(crate) fn sender_for(calendar: &Collection, organizer: &str) -> Sender {
    if crate::pim::here_only(calendar) {
        return Sender::Draft;
    }
    let owned = vdir::State::load(&vdir::state_path(&calendar.account, Kind::Calendars, &calendar.id)).owned;
    if owned != Some(true) {
        return Sender::Draft;
    }
    if sioul_sync::scheduling::schedules_for(&calendar.account, organizer) { Sender::Server } else { Sender::Draft }
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

/// What writing a meeting you organize does, worked out before it is
/// written, and said before you are asked (`asked`).
#[derive(Debug, Clone)]
pub(crate) struct Plan {
    pub path: PathBuf,
    pub before: Option<String>,
    pub text: String,
    pub calendar: Collection,
    pub sender: Sender,
    pub guests: Vec<Guest>,
    pub added: Vec<Guest>,
    pub removed: Vec<Guest>,
    /// The guests kept must hear of it: its time, title, place, notes or status changed.
    pub told: bool,
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
}

impl Plan {
    /// Who receives an invitation (an update, for those asked before): every
    /// guest when the meeting changed, else the new ones.
    fn invited(&self) -> Vec<Guest> {
        if self.told { self.guests.clone() } else { self.added.clone() }
    }

    /// Whether anyone hears of it.
    pub(crate) fn tells(&self) -> bool {
        !self.invited().is_empty() || !self.removed.is_empty()
    }

    /// What the window says before writing it, so that you choose: "" when
    /// nobody hears of it.
    pub(crate) fn asked(&self) -> String {
        let names = |guests: &[Guest]| guests.iter().map(Guest::said).collect::<Vec<_>>().join(", ");
        let mut said = Vec::new();
        if let Some(organizer) = self.replaced.as_ref().filter(|_| !self.added.is_empty()) {
            said.push(say("invite-ask-organizer", &[("who", organizer.said())]));
        }
        let invited = self.invited();
        if !invited.is_empty() {
            let new = self.added.len() == invited.len();
            let id = match (self.sender, new) {
                (Sender::Server, true) => "invite-ask-server",
                (Sender::Server, false) => "invite-ask-update-server",
                (Sender::Draft, true) => "invite-ask-draft",
                (Sender::Draft, false) => "invite-ask-update-draft",
            };
            said.push(say(id, &[("who", names(&invited))]));
        }
        if !self.removed.is_empty() {
            let id = if self.sender == Sender::Server { "invite-ask-removed-server" } else { "invite-ask-removed-draft" };
            said.push(say(id, &[("who", names(&self.removed))]));
        }
        said.join(" ")
    }

    /// What the status line says once it is written: who was told, and how.
    pub(crate) fn done(&self) -> String {
        if !self.tells() {
            return String::new();
        }
        tr().text(if self.sender == Sender::Server { "invite-done-server" } else { "invite-done-draft" }, None)
    }
}

/// The plan of a write to the event at `path` (made there when it is new):
/// `before` its file as it is, `applied` its text as the form or a drag
/// made it, guests aside; `guests` those the form lists (none: as its file
/// has them); `calendar` where it is. You are its organizer as it says, or
/// as `organizer_for` makes you; refused when it is someone else's.
pub(crate) fn plan(path: &Path, before: Option<&str>, applied: &str, guests: Option<&[Guest]>, calendar: &Collection) -> Result<Plan, String> {
    let own = agenda::own_addresses();
    if before.is_some_and(|b| agenda::meeting_with_others(b, &own) && !agenda::organized_by_you(b, &own)) {
        return Err(tr().text("event-change-with-others", None));
    }
    let organizer = before
        .and_then(organizer::organizer_of)
        .filter(|o| own.contains(&o.address.to_lowercase()))
        .or_else(|| organizer_for(calendar))
        .ok_or_else(|| tr().text("invite-no-address", None))?;
    let sender = sender_for(calendar, &organizer.address);
    let had = before.map(organizer::guests_of).unwrap_or_default();
    let guests = guests.map_or_else(|| had.clone(), cleaned);
    let text = organizer::with_guests(applied, &organizer, &guests, sender).ok_or_else(|| tr().text("agenda-gone", None))?;
    // Someone else named as its organizer, with nobody invited yet: you take their place, said first (F12).
    let replaced = before.and_then(organizer::organizer_of).filter(|o| !o.address.trim().is_empty() && !own.contains(&o.address.trim().to_lowercase()));
    // A meeting's file holds what its guests may read alone; yours goes into your own files (F5).
    let (text, aside) = if agenda::meeting_with_others(&text, &own) { organizer::set_aside(&text) } else { (text, organizer::Aside::default()) };
    let is_in = |list: &[Guest], g: &Guest| list.iter().any(|h| h.address.eq_ignore_ascii_case(&g.address));
    let added: Vec<Guest> = guests.iter().filter(|g| !is_in(&had, g)).cloned().collect();
    let removed: Vec<Guest> = had.iter().filter(|g| !is_in(&guests, g)).cloned().collect();
    let kept = guests.iter().any(|g| is_in(&had, g));
    let told = kept && before.is_some_and(|b| organizer::told(b, &text));
    Ok(Plan { path: path.to_path_buf(), before: before.map(str::to_string), text, calendar: calendar.clone(), sender, guests, added, removed, told, replaced, aside, own_given: false })
}

/// What a plan sets aside (`Plan::aside`), kept in your own files before its
/// meeting is written: your own time around it (`own_time`), the form's
/// replacing what was there, else only where nothing is; its ties among
/// yours (`links::LocalLinks`), which every view of ties reads.
fn keep_aside(text: &str, aside: &organizer::Aside, given: bool) -> Result<(), String> {
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

/// A draft for `to` carrying `ics` (iMIP, RFC 6047), its words by `cancel`.
fn draft(account: &sioul_core::config::Account, to: &[Guest], text: &str, ics: String, cancel: bool) -> Draft {
    let (title, when) = crate::children::named(text);
    let place = sioul_core::lines::unfold(text).iter().find(|l| sioul_core::lines::name(l) == "LOCATION").map(|l| sioul_core::lines::unescape(sioul_core::lines::value(l).trim())).unwrap_or_default();
    let mut draft = Draft::new(&account.id);
    draft.to = to.iter().map(|g| if g.name.trim().is_empty() { g.address.clone() } else { format!("{} <{}>", g.name.trim(), g.address) }).collect();
    let pairs = [("title", title), ("when", when), ("place", place.clone())];
    let (subject, body) = if cancel { ("invite-cancel-subject", "invite-cancel-body") } else if place.is_empty() { ("invite-subject", "invite-body") } else { ("invite-subject", "invite-body-place") };
    draft.subject = say(subject, &pairs);
    draft.body = say(body, &pairs);
    draft.add_signature(account.signature.as_deref());
    draft.calendar = Some(ics);
    if let Some(uid) = sioul_core::lines::unfold(text).iter().find(|l| sioul_core::lines::name(l) == "UID").map(|l| sioul_core::lines::value(l).trim().to_string()) {
        draft.links = vec![sioul_core::links::event_uri(&uid)];
    }
    draft
}

/// The drafts a plan asks for (`Sender::Draft`): the invitation, to every
/// guest or to the new ones (`Plan::invited`), and the cancellation, to
/// those taken off. None when the server tells them.
fn drafts_of(plan: &Plan) -> Result<Vec<Draft>, String> {
    if plan.sender == Sender::Server {
        return Ok(Vec::new());
    }
    let organizer = organizer::organizer_of(&plan.text).map(|o| o.address).unwrap_or_default();
    let mut out = Vec::new();
    let invited = plan.invited();
    if !invited.is_empty() {
        let account = sending_account(&plan.calendar, &organizer)?;
        let ics = organizer::request(&plan.text).ok_or_else(|| tr().text("agenda-gone", None))?;
        out.push(draft(&account, &invited, &plan.text, ics, false));
    }
    if !plan.removed.is_empty() {
        let account = sending_account(&plan.calendar, &organizer)?;
        let to: Vec<String> = plan.removed.iter().map(|g| g.address.clone()).collect();
        let ics = organizer::cancel(plan.before.as_deref().unwrap_or(&plan.text), &to).ok_or_else(|| tr().text("agenda-gone", None))?;
        out.push(draft(&account, &plan.removed, &plan.text, ics, true));
    }
    Ok(out)
}

/// What a write through the door did: the file as before and after (for
/// Undo), and the drafts it made, by id.
#[derive(Debug, Clone, Default)]
pub(crate) struct Written {
    pub file: Option<FileChange>,
    pub drafts: Vec<String>,
    /// A deletion: who tells the guests it is cancelled (`delete`).
    pub cancelled_by: Option<Sender>,
}

/// A plan written: through the organizer's door, then its drafts saved; a
/// draft that cannot be saved puts the file back, and says why.
pub(crate) fn write(plan: &Plan) -> Result<Written, String> {
    let drafts = drafts_of(plan)?;
    keep_aside(&plan.text, &plan.aside, plan.own_given)?;
    vdir::write_organized(&plan.path, &plan.text)?;
    let mut ids = Vec::new();
    for draft in drafts {
        if let Err(e) = draft.save() {
            let _ = match &plan.before {
                Some(before) => vdir::write_organized(&plan.path, before),
                None => vdir::remove_organized(&plan.path),
            };
            ids.iter().filter_map(|id: &String| Draft::by_id(id)).for_each(|d| d.discard());
            return Err(e);
        }
        ids.push(draft.id.clone());
    }
    Ok(Written { file: Some(FileChange { path: plan.path.clone(), before: plan.before.clone(), after: Some(plan.text.clone()) }), drafts: ids, cancelled_by: None })
}

/// A write through the door taken back (Undo): the file as it was, while it
/// is still as it was left (a sync, another device, changed none since);
/// the drafts it made, still unsent in Drafts, discarded. When one was sent
/// meanwhile, its guests heard of it: the change back is told them as any
/// change is (a new draft, or the server). Whether it was put back.
pub(crate) fn put_back(written: &Written) -> bool {
    let Some(file) = &written.file else { return false };
    if std::fs::read_to_string(&file.path).ok() != file.after {
        return false;
    }
    // Sent, or discarded by hand: no longer in Drafts.
    let sent = written.drafts.iter().any(|id| Draft::by_id(id).is_none());
    let back = match (&file.before, file.after.as_deref()) {
        (Some(before), Some(after)) if sent => crate::pim::collection_of(&file.path)
            .and_then(|calendar| plan(&file.path, Some(after), before, Some(&organizer::guests_of(before)), &calendar).ok())
            .is_some_and(|again| write(&again).is_ok()),
        (Some(before), _) => vdir::write_organized(&file.path, before).is_ok(),
        (None, Some(_)) if sent => delete(&file.path).is_ok(),
        (None, _) => vdir::remove_organized(&file.path).is_ok(),
    };
    if back {
        written.drafts.iter().filter_map(|id| Draft::by_id(id)).for_each(|d| d.discard());
    }
    back
}

/// A meeting you organize deleted, as its Delete asks (after "Deleting it
/// tells every guest"): the cancellation drafted for every guest when the
/// server does not send it (`organizer::cancel`), then the file taken out
/// through the door; the server sends its own when it schedules. What Undo
/// needs to put it back.
pub(crate) fn delete(path: &Path) -> Result<Written, String> {
    let before = std::fs::read_to_string(path).map_err(|_| tr().text("agenda-gone", None))?;
    let calendar = crate::pim::collection_of(path).ok_or_else(|| tr().text("agenda-gone", None))?;
    let own = agenda::own_addresses();
    if !agenda::organized_meeting(&before, &own) {
        return Err(tr().text("event-delete-with-others", None));
    }
    let organizer = organizer::organizer_of(&before).map(|o| o.address).unwrap_or_default();
    let sender = sender_for(&calendar, &organizer);
    let guests = organizer::guests_of(&before);
    let cancelled = if sender == Sender::Draft {
        let account = sending_account(&calendar, &organizer)?;
        let ics = organizer::cancel(&before, &[]).ok_or_else(|| tr().text("agenda-gone", None))?;
        Some(draft(&account, &guests, &before, ics, true))
    } else {
        None
    };
    vdir::remove_organized(path)?;
    let mut drafts = Vec::new();
    if let Some(draft) = cancelled {
        if let Err(e) = draft.save() {
            let _ = vdir::write_organized(path, &before);
            return Err(e);
        }
        drafts.push(draft.id.clone());
    }
    Ok(Written { file: Some(FileChange { path: path.to_path_buf(), before: Some(before), after: None }), drafts, cancelled_by: Some(sender) })
}

/// A deletion taken back (Undo), while nothing took its place: what the
/// status line says then (`Some("")`: nothing more than Undo's own word), or
/// None when it could not be put back. Never silent towards the guests
/// (review of 1e8551e, F14):
/// - its cancellation still unsent in Drafts: discarded, the file as it was;
/// - sent (no longer in Drafts): the file written back one SEQUENCE past
///   the cancellation (`organizer::after_cancel`), and an invitation for
///   every guest drafted, which says so;
/// - its server's to tell: written back so too, and the line says the
///   server invites them again if it told them it was cancelled.
pub(crate) fn undelete(written: &Written) -> Option<String> {
    let Some(FileChange { path, before: Some(before), .. }) = &written.file else { return None };
    if path.exists() {
        return None;
    }
    let sent = written.drafts.iter().any(|id| Draft::by_id(id).is_none());
    if written.cancelled_by == Some(Sender::Server) {
        let again = organizer::after_cancel(before)?;
        vdir::write_organized(path, &again).ok()?;
        return Some(tr().text("invite-undeleted-server", None));
    }
    if sent {
        let again = organizer::after_cancel(before)?;
        let calendar = crate::pim::collection_of(path)?;
        let organizer = organizer::organizer_of(&again).map(|o| o.address).unwrap_or_default();
        let account = sending_account(&calendar, &organizer).ok()?;
        let invitation = draft(&account, &organizer::guests_of(&again), &again, organizer::request(&again)?, false);
        vdir::write_organized(path, &again).ok()?;
        if invitation.save().is_err() {
            let _ = vdir::remove_organized(path);
            return None;
        }
        return Some(tr().text("invite-undeleted-draft", None));
    }
    vdir::write_organized(path, before).ok()?;
    written.drafts.iter().filter_map(|id| Draft::by_id(id)).for_each(|d| d.discard());
    Some(String::new())
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
    let stored = agenda::stored(invitation);
    let text = organizer::with_guests(&stored, &organizer, &organizer::guests_of(&stored), Sender::Draft).ok_or_else(|| tr().text("agenda-gone", None))?;
    let (text, aside) = organizer::set_aside(&text);
    keep_aside(&text, &aside, false)?;
    vdir::write_organized(path, &text)
}

/// Once, at the window's start (review of 1e8551e, F5): the meetings you
/// organize whose files hold Sioul's own lines or ties, which an older Sioul
/// wrote there, written without them (`organizer::without_own_lines`) through
/// the door, what they held moved into your own files first (your own time
/// where none is kept, your ties). No guest hears of it: no time, title,
/// place, notes nor guest changes. Done once its pass ends, marked in the
/// state folder (`organized-tidied`, holding the pass's version, 1); not
/// marked while none of your addresses is known. How many were written.
pub(crate) fn tidy_once() -> usize {
    const VERSION: &str = "1";
    let mark = sioul_core::config::state_dir().join("organized-tidied");
    if std::fs::read_to_string(&mark).is_ok_and(|v| v.trim() == VERSION) {
        return 0;
    }
    let own = agenda::own_addresses();
    if own.is_empty() {
        return 0;
    }
    let mut written = 0;
    for calendar in vdir::collections(Kind::Calendars).into_iter().filter(|c| !c.read_only && c.holds("VEVENT")) {
        for path in calendar.items() {
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            if !agenda::organized_meeting(&text, &own) {
                continue;
            }
            let cleaned = organizer::without_own_lines(&text);
            if cleaned == text {
                continue;
            }
            if keep_aside(&text, &organizer::aside_of(&text), false).is_err() {
                return written;
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
        assert_eq!((plan.sender, plan.added.len(), plan.removed.len(), plan.told), (Sender::Draft, 1, 0, false));
        assert!(plan.asked().contains("Anne") && plan.tells(), "{}", plan.asked());
        let written = write(&plan).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let unfolded = sioul_core::lines::unfold(&text).join("\n");
        assert!(unfolded.contains("ORGANIZER;CN=Camille:mailto:camille@example.org") && unfolded.contains("SCHEDULE-AGENT=CLIENT:mailto:anne@example.net"), "{unfolded}");
        let draft = Draft::by_id(&written.drafts[0]).expect("the invitation drafted");
        assert!(draft.to[0].contains("anne@example.net") && draft.calendar.as_deref().is_some_and(|ics| ics.contains("METHOD:REQUEST")), "{draft:?}");
        assert_eq!(draft.account, "camille");
        // Undo before it was sent: as it was, the draft gone.
        assert!(put_back(&written));
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
        assert_eq!(gone.cancelled_by, Some(Sender::Draft));
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
        let served = Written { cancelled_by: Some(Sender::Server), drafts: Vec::new(), ..gone };
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
        assert!(!unfolded.contains("METHOD") && !unfolded.contains("X-SIOUL") && !unfolded.contains("X-MOZ"), "{unfolded}");
    }

    /// A calendar of a server that schedules for your address, whose owner
    /// it never said: Sioul drafts, never leaves it to the server; one it
    /// says is yours: the server (review of 1e8551e, F18).
    #[test]
    fn a_calendar_of_an_unknown_owner_is_never_its_server_s_to_schedule() {
        sioul_core::config::stand_ins_only();
        let account = format!("unknown-owner-{}", std::process::id());
        let calendar = vdir::create(Kind::Calendars, &account, "Shared, owner unsaid", None, &["VEVENT"]).unwrap();
        let state = sioul_core::config::state_dir().join("dav").join(&account);
        std::fs::create_dir_all(&state).unwrap();
        std::fs::write(state.join("scheduling.toml"), "auto = true\naddresses = [\"camille@example.org\"]\nchecked = 4102444800\n").unwrap();
        assert!(sioul_sync::scheduling::schedules_for(&account, "camille@example.org"), "the stand-in says it schedules");
        let marks = vdir::state_path(&calendar.account, Kind::Calendars, &calendar.id);
        for (owned, sender) in [(None, Sender::Draft), (Some(false), Sender::Draft), (Some(true), Sender::Server)] {
            let mut kept = vdir::State::load(&marks);
            kept.owned = owned;
            kept.save(&marks).unwrap();
            assert_eq!(sender_for(&calendar, "camille@example.org"), sender, "{owned:?}");
        }
        let _ = std::fs::remove_dir_all(Kind::Calendars.root().join(&account));
        let _ = std::fs::remove_dir_all(&state);
    }

    /// Meetings you organize that an older Sioul wrote its own lines and ties
    /// into: written once without them, what they held moved into your own
    /// files (your own time where none was kept, your ties), another
    /// program's alarm left as it is; then marked done (review of 1e8551e, F5).
    #[test]
    fn the_once_only_pass_takes_sioul_s_lines_out_of_your_meetings() {
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
        assert_eq!(std::fs::read_to_string(&mark).unwrap(), "1");
        vdir::store_item(&path, &old).unwrap();
        assert_eq!(tidy_once(), 0, "once");
        let _ = sioul_core::own_time::set(&sioul_core::own_time::default_path(), "tidied-1", None);
        let _ = sioul_core::links::LocalLinks::change(&sioul_core::links::LocalLinks::default_path(), |l| {
            let n = l.links.len();
            l.links.retain(|e| e.from != "sioul:event/tidied-1");
            l.links.len() != n
        });
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
