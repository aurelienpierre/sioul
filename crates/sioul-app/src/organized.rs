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
/// `organizer`: its server, when it is a server's calendar of yours (not one
/// shared with you) whose server schedules for that address; else you, by a draft.
pub(crate) fn sender_for(calendar: &Collection, organizer: &str) -> Sender {
    if crate::pim::here_only(calendar) {
        return Sender::Draft;
    }
    let owned = vdir::State::load(&vdir::state_path(&calendar.account, Kind::Calendars, &calendar.id)).owned;
    if owned == Some(false) {
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
    let is_in = |list: &[Guest], g: &Guest| list.iter().any(|h| h.address.eq_ignore_ascii_case(&g.address));
    let added: Vec<Guest> = guests.iter().filter(|g| !is_in(&had, g)).cloned().collect();
    let removed: Vec<Guest> = had.iter().filter(|g| !is_in(&guests, g)).cloned().collect();
    let kept = guests.iter().any(|g| is_in(&had, g));
    let told = kept && before.is_some_and(|b| organizer::told(b, &text));
    Ok(Plan { path: path.to_path_buf(), before: before.map(str::to_string), text, calendar: calendar.clone(), sender, guests, added, removed, told })
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
}

/// A plan written: through the organizer's door, then its drafts saved; a
/// draft that cannot be saved puts the file back, and says why.
pub(crate) fn write(plan: &Plan) -> Result<Written, String> {
    let drafts = drafts_of(plan)?;
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
    Ok(Written { file: Some(FileChange { path: plan.path.clone(), before: plan.before.clone(), after: Some(plan.text.clone()) }), drafts: ids })
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
    Ok(Written { file: Some(FileChange { path: path.to_path_buf(), before: Some(before), after: None }), drafts })
}

/// A deletion taken back (Undo): the file written again, while nothing took
/// its place, and its cancellation discarded while unsent.
pub(crate) fn undelete(written: &Written) -> bool {
    let Some(FileChange { path, before: Some(before), .. }) = &written.file else { return false };
    if path.exists() {
        return false;
    }
    let back = vdir::write_organized(path, before).is_ok();
    if back {
        written.drafts.iter().filter_map(|id| Draft::by_id(id)).for_each(|d| d.discard());
    }
    back
}

/// A guest's REPLY to a meeting you organize, read in your mail (RFC 5546
/// §3.2.3), as the other parent's is (`children::replied`): their answer
/// written into the meeting, once, when the message is from the address it
/// answers for and the Porch would take it as coming from there
/// (`children::proven_sender`): a forged "Anne accepts" would say a guest
/// comes who does not. The line the reader says ("Anne accepted
/// “Planning”."); none for a REPLY of no meeting of yours.
pub(crate) fn replied(reply: &str, message: &sioul_core::card::Card, trusted_ids: &[String], own_domains: &[String]) -> Option<String> {
    let lines = sioul_core::lines::unfold(reply);
    let uid = lines.iter().find(|l| sioul_core::lines::name(l) == "UID").map(|l| sioul_core::lines::value(l).trim().to_string())?;
    let own = agenda::own_addresses();
    let path = meeting_of(&uid, &own)?;
    let text = std::fs::read_to_string(&path).ok()?;
    let (title, _) = crate::children::named(&text);
    let (written, guest, answer) = organizer::replied(&text, reply)?;
    if !message.from_address.as_deref().is_some_and(|from| guest.address.eq_ignore_ascii_case(from.trim())) {
        return None;
    }
    let who = organizer::guests_of(&text).into_iter().find(|g| g.address.eq_ignore_ascii_case(&guest.address)).map_or_else(|| guest.said(), |g| g.said());
    if !crate::children::proven_sender(message, trusted_ids, own_domains) {
        return Some(say("invite-replied-unproven", &[("who", who), ("title", title)]));
    }
    vdir::write_organized(&path, &written).ok()?;
    let id = match answer.as_str() {
        "accepted" => "invite-replied-yes",
        "declined" => "invite-replied-no",
        _ => "invite-replied-maybe",
    };
    Some(say(id, &[("who", who), ("title", title)]))
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
        assert!(undelete(&gone) && path.exists() && Draft::by_id(&gone.drafts[0]).is_none());
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
}
