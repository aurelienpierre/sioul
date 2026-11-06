// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Children's events (several agendas, part 3; docs/client.md, "Children's
//! events"): who takes each, asked once; your stand-ins, made, following
//! their child event and taken out; the invitation drafted when the other
//! parent answers by mail, and their answer read; the follow-ups. The
//! rules are `sioul_core::holders`'; here, what this device reads and writes.

use crate::backend::{QtThread, Shared, json, load_config, say, tell, tr};
use serde::{Deserialize, Serialize};
use sioul_core::agendas::{self, Agenda, Answers, Whose};
use sioul_core::config::Config;
use sioul_core::holders::{self, Act, Answer, Asked, Asking, Holder, Kind, Taken, Who};
use sioul_core::vdir::{self, Collection};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The agendas here, each with its collection.
fn agendas_here(config: &Config) -> Vec<(Agenda, Collection)> {
    vdir::collections(vdir::Kind::Calendars).into_iter().map(|c| (agendas::agenda_of(&c, config), c)).collect()
}

/// You, as a child agenda's holder line names you: the name and address of
/// the account that holds that agenda, else your first ones.
fn me_in(config: &Config, agenda: &Agenda) -> (String, String) {
    let own = sioul_core::agenda::own_addresses();
    let account = config.every_account().find(|a| a.id == agenda.account);
    let address = account.and_then(|a| a.address.clone()).filter(|a| !a.trim().is_empty()).or_else(|| own.first().cloned()).unwrap_or_default();
    let name = account.and_then(|a| a.name.clone()).filter(|n| !n.trim().is_empty()).or_else(|| config.every_account().find_map(|a| a.name.clone().filter(|n| !n.trim().is_empty()))).unwrap_or_default();
    (name, address)
}

/// Whether you may write events into an agenda of yours.
fn yours_to_write(agenda: &Agenda, collection: &Collection) -> bool {
    agenda.whose == Whose::Yours && agenda.events && !agenda.read_only() && !collection.read_only
}

/// Where your stand-in of a kind goes: the agenda its setting names
/// (`[agenda.stand_ins]`) when it is yours to write; else your personal
/// agenda: the one new events go into when it is not for work alone, else
/// your first one that is not (research/agendas.md §6, recommendation 12).
fn target_of(config: &Config, here: &[(Agenda, Collection)], kind: Option<Kind>) -> Option<holders::Target> {
    let target = |a: &Agenda, c: &Collection| holders::Target { place: a.id.clone(), dir: c.dir.clone(), work: a.for_work() };
    if let Some(place) = kind.and_then(|k| config.agenda.stand_ins.get(k.id()))
        && let Some((a, c)) = here.iter().find(|(a, c)| &a.id == place && yours_to_write(a, c))
    {
        return Some(target(a, c));
    }
    let default = sioul_core::agenda::default_calendar_of(config.agenda.calendar.as_deref()).map(|c| agendas::place(&c));
    here.iter()
        .find(|(a, c)| Some(&a.id) == default.as_ref() && yours_to_write(a, c) && !a.for_work())
        .or_else(|| here.iter().find(|(a, c)| yours_to_write(a, c) && !a.for_work()))
        .map(|(a, c)| target(a, c))
}

/// "Details: Tom’s agenda", for a child's agenda.
fn details_of(agenda: &Agenda) -> String {
    if agenda.person.is_empty() {
        say("holder-details-agenda", &[("agenda", agenda.name.clone())])
    } else {
        say("holder-details", &[("name", agenda.person.clone())])
    }
}

/// The child's name, as the lines say it: its agenda's name for it, else the agenda's own.
fn child_name(agenda: &Agenda) -> String {
    if agenda.person.is_empty() { agenda.name.clone() } else { agenda.person.clone() }
}

/// A kind's follow-up in a child's agenda (`holders::FollowUp`): none when
/// follow-ups are off, the kind has no rhythm, or the child has no list
/// you may write (the child's agenda itself when it takes tasks, else a
/// child's list of the same name).
fn follow_up_of(config: &Config, here: &[(Agenda, Collection)], agenda: &Agenda, kind: Kind) -> Option<holders::FollowUp> {
    if !config.agenda.follow_ups() {
        return None;
    }
    let months = config.agenda.rhythm(kind)?;
    let writable = |a: &Agenda, c: &Collection| a.whose == Whose::Child && a.tasks && !a.read_only() && !c.read_only;
    let list = here
        .iter()
        .find(|(a, c)| a.id == agenda.id && writable(a, c))
        .or_else(|| here.iter().find(|(a, c)| writable(a, c) && !agenda.person.is_empty() && a.person.eq_ignore_ascii_case(&agenda.person)))?;
    let name = child_name(agenda);
    let title = match kind {
        Kind::Dentist => say("holder-follow-up-dentist", &[("name", name)]),
        Kind::Health => say("holder-follow-up-health", &[("name", name)]),
        other => say("holder-follow-up-other", &[("name", name), ("kind", tr().text(&format!("holder-kind-{}", other.id()), None))]),
    };
    Some(holders::FollowUp { title, months, dir: list.1.dir.clone() })
}

/// A child item's title and day, as the lines name it: "“Dentist”, Tuesday 10 November at 10:00".
pub(crate) fn named(text: &str) -> (String, String) {
    let zone = sioul_core::zone::current();
    let lines = sioul_core::lines::unfold(text);
    let title = lines.iter().find(|l| sioul_core::lines::name(l) == "SUMMARY").map(|l| sioul_core::lines::unescape(sioul_core::lines::value(l).trim())).filter(|t| !t.is_empty()).unwrap_or_else(|| tr().text("agenda-untitled", None));
    let when = sioul_core::agenda::first_times(text, &zone)
        .and_then(|(start, _)| jiff::Timestamp::from_second(start).ok())
        .map(|t| t.to_zoned(zone.clone()))
        .map(|start| if text.contains("DTSTART;VALUE=DATE:") { tr().day(start.date()) } else { tr().date(&start, true) })
        .unwrap_or_default();
    (title, when)
}

// ---------------------------------------------------------------- the upkeep

/// What an invitation drafted said of its event, by the child event's UID
/// (`holder-invitations.toml` in the state folder, this device's own): an
/// update is drafted when the event changes, a cancellation when it is
/// cancelled or taken back (RFC 5546's SEQUENCE counting them).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Sent {
    #[serde(default)]
    invitations: BTreeMap<String, Invited>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Invited {
    /// Its times, title, place and status as the last invitation said them (`holders::fingerprint`).
    said: String,
    sequence: u32,
    /// The last invitation's draft, by its id: while it waits in Drafts, nothing was sent,
    /// and a change rewrites it in place, a cancellation drafts nothing.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    draft: String,
}

fn sent_path() -> PathBuf {
    sioul_core::config::state_dir().join("holder-invitations.toml")
}

fn read_sent() -> Sent {
    std::fs::read_to_string(sent_path()).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default()
}

fn write_sent(sent: &Sent) -> Result<(), String> {
    let path = sent_path();
    let text = toml::to_string(sent).map_err(|e| e.to_string())?;
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    }
    sioul_core::filelock::replace(&path, text.as_bytes()).map_err(|e| format!("{}: {e}", path.display()))
}

/// A child's item as this device reads it (`read_with`): its file, its
/// text, its agenda, and whether who takes it is unknown here.
struct ChildRead {
    path: PathBuf,
    text: String,
    agenda: Agenda,
    unknown: bool,
}

/// The children's items here, each read with this device's store of
/// meetings' own lines (`read_with`), with its agenda.
fn child_items(here: &[(Agenda, Collection)]) -> Vec<ChildRead> {
    let store = sioul_core::meeting_lines::load(&sioul_core::meeting_lines::default_path());
    here.iter()
        .filter(|(a, _)| a.whose == Whose::Child)
        .flat_map(|(a, c)| c.items().into_iter().map(move |path| (path, a)))
        .filter_map(|(path, agenda)| read_with(&path, &store).map(|(text, unknown)| ChildRead { path, text, agenda: agenda.clone(), unknown }))
        .collect()
}

/// A child's item or a stand-in as its readers read it on this device
/// (`meeting_lines::read`): its file, and for a meeting with others (a
/// guest was invited to it) Sioul's own lines of it put back from their own
/// store: who takes it, its kind, which its file no longer holds once its
/// guests read it (second review of the organizer's door, N6). A meeting
/// this device keeps nothing of is unknown here (`true`, its file as it
/// is): its lines may be on another device, which the sharing does not
/// carry yet (third review, R1); one that is no meeting any more reads its
/// lines from the store until they are back in its file.
fn read_with(path: &Path, store: &sioul_core::meeting_lines::MeetingLines) -> Option<(String, bool)> {
    let file = std::fs::read_to_string(path).ok()?;
    let meeting = sioul_core::agenda::with_others(&file);
    Some(match sioul_core::meeting_lines::read(&file, store, meeting) {
        sioul_core::meeting_lines::Read::Known(text) => (text, false),
        sioul_core::meeting_lines::Read::Unknown => (file, true),
    })
}

/// `read_with` this device's store as it is now: the text alone.
fn read_item(path: &Path) -> Option<String> {
    read_with(path, &sioul_core::meeting_lines::load(&sioul_core::meeting_lines::default_path())).map(|(text, _)| text)
}

/// Whether one of your agendas holds a stand-in of the child's event at
/// `path` and this device keeps nothing of who takes it (a meeting made on
/// another device): taking its last guest off here would leave its file
/// without its kind, and your stand-in would follow its title (third review
/// of the organizer's door, R1). The organizer's door refuses it then.
pub(crate) fn held_elsewhere(path: &Path) -> bool {
    let config = load_config();
    let Some(collection) = crate::pim::collection_of(path) else { return false };
    if agendas::agenda_of(&collection, &config).whose != Whose::Child {
        return false;
    }
    let store = sioul_core::meeting_lines::load(&sioul_core::meeting_lines::default_path());
    read_with(path, &store).is_some_and(|(text, unknown)| unknown && config_stand_in(&config, &text))
}

/// Whether the event at `path` is in a child's agenda: the door keeps an
/// entry of its own lines when it makes it a meeting, even of none (R1).
pub(crate) fn in_child_agenda(path: &Path) -> bool {
    crate::pim::collection_of(path).is_some_and(|c| agendas::agenda_of(&c, &load_config()).whose == Whose::Child)
}

/// Sioul's own lines of an item that is no meeting any more, kept in the
/// store, back into its file (`meeting_lines::read` reads them from there
/// meanwhile), then its entry forgotten; its entry alone forgotten when its
/// file holds lines of its own (written since: the file's word). Nothing
/// for a meeting, or an item with no entry. Whether its file was written.
fn lines_back(path: &Path, store: &sioul_core::meeting_lines::MeetingLines) -> bool {
    let Ok(file) = std::fs::read_to_string(path) else { return false };
    let uid = sioul_core::meeting_lines::uid_of(&file);
    let Some(kept) = store.events.get(&uid) else { return false };
    if sioul_core::agenda::with_others(&file) {
        return false;
    }
    let at = sioul_core::meeting_lines::default_path();
    if !sioul_core::organizer::aside_of(&file).lines.is_empty() {
        let _ = sioul_core::meeting_lines::forget(&at, &uid);
        return false;
    }
    let back = sioul_core::meeting_lines::merged_with(&file, kept);
    if back != file
        && let Err(e) = vdir::write_item(path, &back)
    {
        eprintln!("Sioul: a child's event kept its lines apart: {e}");
        return false;
    }
    let _ = sioul_core::meeting_lines::forget(&at, &uid);
    back != file
}

/// Whether the file at `path` is a meeting with others: nothing of the
/// children's upkeep writes it (its server would tell its guests), and
/// Sioul's own lines of it go to their own store (`write_child`).
fn a_meeting(path: &Path) -> bool {
    std::fs::read_to_string(path).is_ok_and(|text| sioul_core::agenda::with_others(&text))
}

/// A child's item written as `text` (its file as `read_item` read it, then
/// who takes it or its kind changed): into its file through the agenda's
/// one rule; for a meeting with others, Sioul's own lines of `text` into
/// their own store instead (`meeting_lines::set_from`), never into the
/// meeting, whose server would send them to its guests (second review of
/// the organizer's door, N6). The file changed, for Undo.
fn write_child(path: &Path, before: &str, text: &str) -> Result<sioul_core::agentwrites::FileChange, String> {
    if a_meeting(path) {
        let store = sioul_core::meeting_lines::default_path();
        let had = std::fs::read_to_string(&store).ok();
        let uid = holders::uid_of(text).ok_or_else(|| tr().text("agenda-gone", None))?;
        sioul_core::meeting_lines::set_from(&store, &uid, text)?;
        return Ok(sioul_core::agentwrites::FileChange { after: std::fs::read_to_string(&store).ok(), path: store, before: had });
    }
    vdir::write_item(path, text)?;
    Ok(sioul_core::agentwrites::FileChange { path: path.to_path_buf(), before: Some(before.to_string()), after: Some(text.to_string()) })
}

/// What the holder lines ask of your agendas, from the plan's run (the
/// rides' moment, `work`): your stand-ins made, followed or taken out, the
/// follow-ups added, each said once in the status line; the invitations'
/// updates and cancellations drafted; what came in since the last look
/// noted, to be asked once; an answer "not this time" said once. `events`:
/// the pages' events, where your stand-ins are found (by their parent, or
/// their UID alone where a server dropped their lines).
pub(crate) fn upkeep(qt: &QtThread, shared: &Arc<Shared>, events: &[sioul_core::agenda::EventRef]) {
    let (lines, accounts) = upkeep_here(events);
    for line in lines {
        tell(qt, shared, line);
    }
    for account in &accounts {
        crate::pim::nudge(shared, account);
    }
    if !accounts.is_empty() {
        crate::pim::show_pim(qt, shared);
    }
}

/// `upkeep`'s writes: the lines to say, the accounts written to.
fn upkeep_here(events: &[sioul_core::agenda::EventRef]) -> (Vec<String>, BTreeSet<String>) {
    upkeep_at(events, &Asked::path())
}

/// `upkeep_here`, this device's questions kept at `asked_path` (a test's own).
fn upkeep_at(events: &[sioul_core::agenda::EventRef], asked_path: &Path) -> (Vec<String>, BTreeSet<String>) {
    let mut said: Vec<String> = Vec::new();
    let mut accounts: BTreeSet<String> = BTreeSet::new();
    let config = load_config();
    let here = agendas_here(&config);
    if !here.iter().any(|(a, _)| a.whose == Whose::Child) {
        return (said, accounts);
    }
    let zone = sioul_core::zone::current();
    let now = sioul_core::zone::now().timestamp().as_second();
    let own = sioul_core::agenda::own_addresses();
    // Sioul's own lines of an event that is no meeting any more, back into its file first (R1):
    // the children's items, and your events this device keeps lines of (a stand-in given guests).
    let store = sioul_core::meeting_lines::load(&sioul_core::meeting_lines::default_path());
    if !store.events.is_empty() {
        let children = here.iter().filter(|(a, _)| a.whose == Whose::Child).flat_map(|(_, c)| c.items());
        let yours = events.iter().filter(|e| store.events.contains_key(&e.uid)).map(|e| PathBuf::from(&e.key));
        for path in children.chain(yours) {
            if lines_back(&path, &store) {
                accounts.extend(crate::pim::account_of(&path));
            }
        }
    }
    let items = child_items(&here);
    let uids: BTreeSet<String> = items.iter().filter_map(|c| holders::uid_of(&c.text)).collect();
    // Your agendas' events that may stand in for a child's.
    let places: BTreeMap<PathBuf, (String, bool)> = here.iter().filter(|(a, _)| a.whose == Whose::Yours && a.events).map(|(a, c)| (c.dir.clone(), (a.id.clone(), a.for_work()))).collect();
    let derived: BTreeSet<String> = uids.iter().flat_map(|u| places.values().map(move |(place, _)| holders::stand_in_uid(u, place))).collect();
    let candidates: Vec<(PathBuf, String, String, bool)> = events
        .iter()
        .filter(|e| derived.contains(&e.uid) || e.related.iter().any(|r| r.kind.eq_ignore_ascii_case("PARENT") && uids.contains(&r.uid)))
        .filter_map(|e| {
            let path = PathBuf::from(&e.key);
            let (place, work) = path.parent().and_then(|d| places.get(d)).cloned()?;
            let text = read_item(&path)?;
            Some((path, text, place, work))
        })
        .collect();
    // Who takes it unknown here (a meeting made on another device): nothing asked of its stand-ins,
    // which stay exactly as they are, never unmasked nor rewritten (third review, R1).
    let children: Vec<holders::ChildItem> = items.iter().filter(|c| !c.unknown).map(|c| holders::ChildItem { path: &c.path, text: &c.text, agenda: &c.agenda }).collect();
    let mine: Vec<holders::Mine> = candidates.iter().map(|(path, text, place, work)| holders::Mine { path, text, place, work: *work }).collect();
    let target = |kind: Option<Kind>| target_of(&config, &here, kind);
    let follow_up = |agenda: &Agenda, kind: Kind| follow_up_of(&config, &here, agenda, kind);
    let away = tr().text("holder-away", None);
    let cx = holders::Context { own: &own, target: &target, details: &details_of, away: &away, follow_up: &follow_up, now, zone: &zone };
    let acts = holders::upkeep(&children, &mine, &cx);
    let named_child = |uid: &str| items.iter().find(|c| holders::uid_of(&c.text).as_deref() == Some(uid)).map(|c| (child_name(&c.agenda), named(&c.text)));
    for act in acts {
        // A stand-in given guests is a meeting you organize now: yours to change from its form (N6).
        let at = match &act {
            Act::Made { path, .. } | Act::Followed { path, .. } | Act::Given { path, .. } | Act::FollowUp { path, .. } => path,
        };
        if a_meeting(at) {
            continue;
        }
        let (written, line) = match &act {
            Act::Made { path, text, child } => (vdir::write_item(path, text).map(|()| path.clone()), named_child(child).map(|(name, (title, when))| say("holder-made", &[("child", name), ("title", title), ("when", when)]))),
            Act::Followed { path, text, child } => (vdir::write_item(path, text).map(|()| path.clone()), named_child(child).map(|(name, (title, when))| say("holder-followed", &[("child", name), ("title", title), ("when", when)]))),
            Act::Given { path, child } => (vdir::remove_item(path).map(|()| path.clone()), named_child(child).map(|(name, (title, _))| say("holder-given", &[("child", name), ("title", title)]))),
            Act::FollowUp { path, text, child } => (vdir::write_item(path, text).map(|()| path.clone()), named_child(child).map(|(name, _)| say("holder-follow-up-added", &[("child", name), ("title", sioul_core::lines::unfold(text).iter().find(|l| sioul_core::lines::name(l) == "SUMMARY").map(|l| sioul_core::lines::unescape(sioul_core::lines::value(l))).unwrap_or_default())]))),
        };
        match written {
            Ok(path) => {
                accounts.extend(crate::pim::account_of(&path));
                said.extend(line);
            }
            Err(e) => said.push(e),
        }
    }
    // The invitations' updates and cancellations, drafted (never sent: Send is yours).
    let mut sent = read_sent();
    let before = sent.clone();
    for ChildRead { text, agenda, .. } in items.iter().filter(|c| !c.unknown) {
        let Some(uid) = holders::uid_of(text) else { continue };
        let Some(invited) = sent.invitations.get(&uid).cloned() else { continue };
        let taken = holders::taken(text, None, &zone);
        let still = taken.to.as_ref().is_some_and(|h| h.is(&agenda.parent_address) && h.answer != Answer::Declined);
        let cancelled = sioul_core::lines::unfold(text).iter().any(|l| sioul_core::lines::name(l) == "STATUS" && sioul_core::lines::value(l).trim().eq_ignore_ascii_case("CANCELLED"));
        let fingerprint = holders::fingerprint(text);
        let method = if !still || cancelled { "CANCEL" } else if fingerprint != invited.said { "REQUEST" } else { continue };
        // Its last draft still waits in Drafts: nothing was sent. Taken back or cancelled, nothing
        // to cancel; changed, that draft says the change, its sequence the same.
        if !invited.draft.is_empty() && sioul_core::compose::Draft::by_id(&invited.draft).is_some() {
            if method == "CANCEL" {
                sent.invitations.remove(&uid);
                continue;
            }
            match draft_invitation(&config, agenda, text, "REQUEST", invited.sequence).and_then(|draft| sioul_core::compose::Draft { id: invited.draft.clone(), ..draft }.save().map(|_| ())) {
                Ok(()) => {
                    sent.invitations.insert(uid.clone(), Invited { said: fingerprint, ..invited.clone() });
                    said.push(say("holder-invite-redrafted", &[("who", agenda.parent.clone())]));
                }
                Err(e) => said.push(e),
            }
            continue;
        }
        match draft_invitation(&config, agenda, text, method, invited.sequence + 1).and_then(|draft| draft.save().map(|_| draft.id.clone())) {
            Ok(id) => {
                if method == "CANCEL" {
                    sent.invitations.remove(&uid);
                } else {
                    sent.invitations.insert(uid.clone(), Invited { said: fingerprint, sequence: invited.sequence + 1, draft: id });
                }
                said.push(say(if method == "CANCEL" { "holder-invite-cancel-drafted" } else { "holder-invite-update-drafted" }, &[("who", agenda.parent.clone())]));
            }
            Err(e) => said.push(e),
        }
    }
    if sent != before {
        let _ = write_sent(&sent);
    }
    // What came in since the last look, to be asked once; "not this time" said once.
    let path = asked_path.to_path_buf();
    let mut asked = Asked::load(&path);
    let mut changed = asked.look(&uids);
    for ChildRead { text, agenda, .. } in items.iter().filter(|c| !c.unknown) {
        let Some(uid) = holders::uid_of(text) else { continue };
        if holders::asking(text, &own, &zone) != Some(Asking::Reopened) {
            continue;
        }
        let since = holders::taken(text, None, &zone).to.and_then(|h| h.since).unwrap_or_default();
        if asked.say_once(&format!("{uid}|{since}")) {
            changed = true;
            let (title, when) = named(text);
            said.push(say("holder-reopened-said", &[("who", agenda.parent.clone()), ("child", child_name(agenda)), ("title", title), ("when", when)]));
        }
    }
    if changed {
        let _ = asked.save(&path);
    }
    (said, accounts)
}

// ---------------------------------------------------------------- the invitation

/// The invitation to the other parent, as a draft (iTIP over iMIP,
/// research/agendas.md §6, recommendation 13): from the child agenda's
/// account (else the one of your address, else your first that sends mail),
/// to them, its text saying what is asked, the REQUEST or CANCEL beside it.
/// Saved into Drafts, it leaves only when you press Send.
fn draft_invitation(config: &Config, agenda: &Agenda, child: &str, method: &str, sequence: u32) -> Result<sioul_core::compose::Draft, String> {
    if agenda.parent_address.is_empty() {
        return Err(tr().text("holder-no-parent", None));
    }
    let (me_name, me_address) = me_in(config, agenda);
    let account = config
        .accounts
        .iter()
        .find(|a| a.id == agenda.account && a.syncs())
        .or_else(|| config.accounts.iter().find(|a| a.syncs() && a.address.as_deref().is_some_and(|x| x.eq_ignore_ascii_case(&me_address))))
        .or_else(|| config.accounts.iter().find(|a| a.syncs()))
        .ok_or_else(|| tr().text("mail-no-account", None))?;
    let (title, when) = named(child);
    let uid = holders::uid_of(child).unwrap_or_default();
    let address = sioul_core::links::event_uri(&uid);
    let child_name = child_name(agenda);
    let details = say("holder-invite-details", &[("child", child_name.clone()), ("title", title.clone()), ("when", when.clone())]);
    let invite = holders::Invite {
        method,
        organizer: holders::Person { name: &me_name, address: &me_address },
        holder: holders::Person { name: &agenda.parent, address: &agenda.parent_address },
        details: &details,
        url: &address,
        sequence,
        now: sioul_core::zone::now().timestamp().as_second(),
    };
    let ics = holders::invitation(child, &invite).ok_or_else(|| tr().text("agenda-gone", None))?;
    let mut draft = sioul_core::compose::Draft::new(&account.id);
    draft.to = vec![if agenda.parent.is_empty() { agenda.parent_address.clone() } else { format!("{} <{}>", agenda.parent, agenda.parent_address) }];
    let pairs = [("child", child_name), ("title", title), ("when", when), ("who", agenda.parent.clone())];
    let (subject, body) = if method == "CANCEL" { ("holder-invite-cancel-subject", "holder-invite-cancel-body") } else { ("holder-invite-subject", "holder-invite-body") };
    draft.subject = say(subject, &pairs);
    draft.body = say(body, &pairs);
    draft.add_signature(account.signature.as_deref());
    draft.calendar = Some(ics);
    draft.links = vec![address];
    Ok(draft)
}

// ---------------------------------------------------------------- your answers

/// The child's item a card or a question names, with its agenda: its file,
/// text, agenda. Refused while who takes it is unknown here (a meeting made
/// on another device, R1): an answer here would go into this device's store
/// alone, beside another one's.
fn child_item(key: &str) -> Result<(PathBuf, String, Agenda, Collection), String> {
    let path = crate::pim::ours(key).ok_or_else(|| tr().text("agenda-gone", None))?;
    let store = sioul_core::meeting_lines::load(&sioul_core::meeting_lines::default_path());
    let (text, unknown) = read_with(&path, &store).ok_or_else(|| tr().text("agenda-gone", None))?;
    let collection = crate::pim::collection_of(&path).ok_or_else(|| tr().text("agenda-gone", None))?;
    let agenda = agendas::agenda_of(&collection, &load_config());
    if agenda.whose != Whose::Child {
        return Err(tr().text("holder-not-child", None));
    }
    if unknown {
        return Err(tr().text("holder-unknown", None));
    }
    Ok((path, text, agenda, collection))
}

/// Who takes a child's event or task, as you answer (its card, or the
/// question): "me", "other" (the other parent is asked: their Sioul answers
/// in the child's agenda, or an invitation is drafted for them), "accept"
/// or "decline" (a proposal to you, or one you took: not this time),
/// "later" (asked again at the next admin window, once). `start`: the time
/// of a repeating event asked about (a changed occurrence carries its own
/// line), 0 for the series. Said with Undo, your stand-in made or taken out
/// at once. Returns what went wrong, else "".
pub(crate) fn take(qt: &QtThread, shared: &Arc<Shared>, key: &str, start: i64, who: &str) -> String {
    let me = child_item(key).map(|(_, _, agenda, _)| me_of(&load_config(), &agenda));
    match me.and_then(|me| take_files(key, start, who, &me)) {
        Ok(Some((files, line))) => {
            crate::agents::say(qt, shared, sioul_core::agentwrites::Said { at: 0, line, files });
            String::new()
        }
        Ok(None) => String::new(),
        Err(e) => e,
    }
}

/// You, as the answers write you: the holder line's name and address, and
/// all your addresses (whether a line names you).
struct Me {
    name: String,
    address: String,
    own: Vec<String>,
}

fn me_of(config: &Config, agenda: &Agenda) -> Me {
    let (name, address) = me_in(config, agenda);
    Me { name, address, own: sioul_core::agenda::own_addresses() }
}

/// `take`'s writes: the files changed, each before and after, and the line
/// to say; none for "later", which writes this device's question alone.
/// Nothing is written when the invitation cannot be made, nor without an
/// address of yours to write.
fn take_files(key: &str, start: i64, who: &str, me: &Me) -> Result<Option<(Vec<sioul_core::agentwrites::FileChange>, String)>, String> {
    let (path, text, agenda, _) = child_item(key)?;
    let config = load_config();
    let zone = sioul_core::zone::current();
    let now = sioul_core::zone::now().timestamp().as_second();
    let uid = holders::uid_of(&text).ok_or_else(|| tr().text("agenda-gone", None))?;
    let asked_path = Asked::path();
    if who == "later" {
        let mut asked = Asked::load(&asked_path);
        let from = sioul_core::window::next_opening(&config.windows, &sioul_core::zone::now()).map_or(now + 86_400, |z| z.timestamp().as_second());
        asked.put_off(&uid, from);
        asked.save(&asked_path)?;
        return Ok(None);
    }
    if me.address.trim().is_empty() {
        return Err(tr().text("holder-no-address", None));
    }
    let at = Some(start).filter(|s| *s > 0);
    let mut taken = holders::taken(&text, at, &zone);
    let own = &me.own;
    let me = Holder { name: me.name.clone(), address: me.address.clone(), answer: Answer::Accepted, since: Some(now), back: false };
    let other = Holder { name: agenda.parent.clone(), address: agenda.parent_address.clone(), answer: Answer::NeedsAction, since: Some(now), back: false };
    // Who brings the child back (`PART=BACK`), when another than who takes it: you, the other
    // parent (asked), or nobody apart.
    if let Some(back) = who.strip_prefix("back-") {
        taken.back = match back {
            "me" => Some(Holder { back: true, ..me }),
            "other" if agenda.parent_address.is_empty() => return Err(tr().text("holder-no-parent", None)),
            "other" => Some(Holder { name: agenda.parent.clone(), address: agenda.parent_address.clone(), answer: Answer::NeedsAction, since: Some(now), back: true }),
            "none" => None,
            _ => return Err(format!("{who}: back-me, back-other or back-none")),
        };
        // The same one who takes it brings the child back: no line apart.
        if taken.back.as_ref().zip(taken.to.as_ref()).is_some_and(|(b, t)| b.address.eq_ignore_ascii_case(&t.address)) {
            taken.back = None;
        }
        let written = holders::with_taken(&text, at, &taken, &zone).ok_or_else(|| tr().text("agenda-gone", None))?;
        let file = write_child(&path, &text, &written)?;
        let (title, when) = named(&written);
        let line = say("holder-said-back", &[("child", child_name(&agenda)), ("title", title), ("when", when)]);
        return Ok(Some((vec![file], line)));
    }
    let (holder, line) = match who {
        "me" | "accept" => (me, "holder-said-me"),
        "decline" => (Holder { answer: Answer::Declined, since: Some(now), ..taken.to.clone().filter(|h| h.among(own)).unwrap_or(me) }, "holder-said-declined"),
        "other" if agenda.parent_address.is_empty() => return Err(tr().text("holder-no-parent", None)),
        "other" => (other, if agenda.parent_answers == Answers::Mail { "holder-said-other-mail" } else { "holder-said-other" }),
        _ => return Err(format!("{who}: me, other, accept, decline or later")),
    };
    taken.to = Some(holder);
    let written = holders::with_taken(&text, at, &taken, &zone).ok_or_else(|| tr().text("agenda-gone", None))?;
    if !a_meeting(&path) {
        sioul_core::agenda::guard(&path, sioul_core::agenda::Change::Write(&written)).map_err(|r| sioul_core::agenda::said(r, &sioul_core::agenda::Change::Write(&written)))?;
    }
    // The other parent asked by mail: the invitation made first, nothing written without it.
    let mut sent = read_sent();
    let sequence = sent.invitations.get(&uid).map_or(0, |i| i.sequence + 1);
    let draft = if who == "other" && agenda.parent_answers == Answers::Mail { Some(draft_invitation(&config, &agenda, &written, "REQUEST", sequence)?) } else { None };
    let mut files = vec![write_child(&path, &text, &written)?];
    // Your stand-in at once, as the next plan would make or take it out.
    let here = agendas_here(&config);
    let candidates: Vec<(PathBuf, String, String, bool)> = here
        .iter()
        .filter(|(a, _)| a.whose == Whose::Yours && a.events)
        .filter_map(|(a, c)| {
            let file = c.dir.join(format!("{}.ics", holders::stand_in_uid(&uid, &a.id)));
            read_item(&file).map(|t| (file, t, a.id.clone(), a.for_work()))
        })
        .collect();
    let child = [holders::ChildItem { path: &path, text: &written, agenda: &agenda }];
    let mine: Vec<holders::Mine> = candidates.iter().map(|(p, t, place, work)| holders::Mine { path: p, text: t, place, work: *work }).collect();
    let target = |kind: Option<Kind>| target_of(&config, &here, kind);
    let no_follow_up = |_: &Agenda, _: Kind| None;
    let away = tr().text("holder-away", None);
    let cx = holders::Context { own, target: &target, details: &details_of, away: &away, follow_up: &no_follow_up, now, zone: &zone };
    for act in holders::upkeep(&child, &mine, &cx) {
        match act {
            Act::Made { path, text, .. } | Act::Followed { path, text, .. } => {
                let before = std::fs::read_to_string(&path).ok();
                if vdir::write_item(&path, &text).is_ok() {
                    files.push(sioul_core::agentwrites::FileChange { path, before, after: Some(text) });
                }
            }
            Act::Given { path, .. } => {
                let before = std::fs::read_to_string(&path).ok();
                if vdir::remove_item(&path).is_ok() {
                    files.push(sioul_core::agentwrites::FileChange { path, before, after: None });
                }
            }
            Act::FollowUp { .. } => {}
        }
    }
    if let Some(draft) = draft {
        draft.save()?;
        sent.invitations.insert(uid.clone(), Invited { said: holders::fingerprint(&written), sequence, draft: draft.id.clone() });
        let _ = write_sent(&sent);
    }
    let mut asked = Asked::load(&asked_path);
    asked.answered(&uid);
    let _ = asked.save(&asked_path);
    let (title, when) = named(&written);
    let line = say(line, &[("child", child_name(&agenda)), ("title", title), ("when", when), ("who", agenda.parent.clone())]);
    Ok(Some((files, line)))
}

/// A child's event's or task's kind, written on its series (`holders::with_kind`); "" takes it out.
pub(crate) fn set_kind(qt: &QtThread, shared: &Arc<Shared>, key: &str, kind: &str) -> String {
    let result = (|| -> Result<(), String> {
        let (path, text, _, _) = child_item(key)?;
        let written = holders::with_kind(&text, Kind::parse(kind)).ok_or_else(|| tr().text("agenda-gone", None))?;
        if written != text {
            write_child(&path, &text, &written)?;
            if let Some(account) = crate::pim::account_of(&path) {
                crate::pim::nudge(shared, &account);
            }
            crate::pim::show_pim(qt, shared);
        }
        Ok(())
    })();
    result.err().unwrap_or_default()
}

// ---------------------------------------------------------------- what the card and the questions say

/// Who takes a child's event, as its card says it (part 3), as JSON; none
/// for an event of no child's agenda. Its kind and the kinds; one line of
/// who takes it and since when ("Sam takes it, since 3 November."), who
/// brings the child back; what you may answer (`state`); the proposal and
/// why ("Usually Sam.", "Tom is with Sam that day.").
pub(crate) fn card(path: &Path, start: i64) -> Option<serde_json::Value> {
    card_as(path, start, &sioul_core::agenda::own_addresses())
}

/// `card`, you being `own`.
fn card_as(path: &Path, start: i64, own: &[String]) -> Option<serde_json::Value> {
    let collection = crate::pim::collection_of(path)?;
    let config = load_config();
    let agenda = agendas::agenda_of(&collection, &config);
    if agenda.whose != Whose::Child {
        return None;
    }
    let store = sioul_core::meeting_lines::load(&sioul_core::meeting_lines::default_path());
    let (text, unknown) = read_with(path, &store)?;
    // Who takes it kept on another device (R1): said, nothing to answer here.
    if unknown {
        return Some(serde_json::json!({ "kind": "", "kinds": [], "state": "unknown", "line": tr().text("holder-line-unknown", None), "back": "", "back_who": "", "child": child_name(&agenda), "other": agenda.parent, "proposal": "", "why": "", "busy": "", "writable": false }));
    }
    let zone = sioul_core::zone::current();
    let taken = holders::taken(&text, Some(start).filter(|s| *s > 0), &zone);
    let kind = holders::kind_of(&text);
    let since = |h: &Holder| h.since.and_then(|s| jiff::Timestamp::from_second(s).ok()).map(|t| tr().day(t.to_zoned(zone.clone()).date())).unwrap_or_default();
    // Someone of your household by their address (an agenda's other parent): their person's name now (docs/persons.md).
    let persons = sioul_core::persons::Persons::cached();
    let name_of = |h: &Holder| {
        if h.among(own) {
            return String::new();
        }
        let person = sioul_core::persons::by_address(&persons, &h.address, &[], &config.agenda.calendars).and_then(|id| persons.get(&id).map(|p| p.name.clone()));
        person.unwrap_or_else(|| if !h.name.is_empty() { h.name.clone() } else { h.address.clone() })
    };
    // A stand-in of yours stands for your hold where a server dropped the line (Google's).
    let stand_in = config_stand_in(&config, &text);
    let state = match &taken.to {
        None if stand_in => "yours",
        None => "nobody",
        Some(h) if h.among(own) => match h.answer {
            Answer::Accepted => "yours",
            Answer::NeedsAction => "proposed",
            Answer::Declined => "declined",
        },
        Some(h) => match h.answer {
            Answer::Accepted => "theirs",
            Answer::NeedsAction => "asked",
            Answer::Declined => "reopened",
        },
    };
    let line = match (&taken.to, state) {
        (_, "nobody") => tr().text("holder-line-nobody", None),
        (None, _) => tr().text("holder-line-yours-plain", None),
        (Some(h), state) => say(&format!("holder-line-{state}"), &[("who", name_of(h)), ("since", since(h))]),
    };
    let back = taken.back.as_ref().map(|h| if h.among(own) { say("holder-line-back-you", &[("child", child_name(&agenda))]) } else { say("holder-line-back", &[("who", name_of(h)), ("child", child_name(&agenda))]) }).unwrap_or_default();
    let back_who = taken.back.as_ref().map(|h| if h.among(own) { "me" } else { "other" }).unwrap_or_default();
    let (proposal, why) = proposal(&agenda, &text, kind, &config, &zone);
    // The other parent busy then, by their agenda for availability: said here, never as a notice.
    let busy = sioul_core::agenda::first_times(&text, &zone).map(|(s, e)| busy_then(&agendas_here(&config), &agenda.parent, if start > 0 { start } else { s }, if start > 0 { start + (e - s) } else { e })).unwrap_or_default();
    let kinds: Vec<serde_json::Value> = Kind::ALL.iter().map(|k| serde_json::json!({ "id": k.id(), "label": tr().text(&format!("holder-kind-{}", k.id()), None) })).collect();
    Some(serde_json::json!({
        "kind": kind.map(Kind::id).unwrap_or_default(),
        "kinds": kinds,
        "state": state,
        "line": line,
        "back": back,
        "back_who": back_who,
        "child": child_name(&agenda),
        "other": agenda.parent,
        "proposal": proposal.map(Who::id).unwrap_or_default(),
        "why": why,
        "busy": busy,
        "writable": !agenda.read_only() && !collection.read_only,
    }))
}

/// Whether one of your agendas holds a stand-in of this child event (by its UID derived, its file named after it).
fn config_stand_in(config: &Config, child: &str) -> bool {
    let Some(uid) = holders::uid_of(child) else { return false };
    agendas_here(config).iter().filter(|(a, _)| a.whose == Whose::Yours && a.events).any(|(a, c)| c.dir.join(format!("{}.ics", holders::stand_in_uid(&uid, &a.id))).exists())
}

/// Who Sioul proposes for a child's event, and why (`holders::propose`):
/// its standing holder ("Usually Sam."), else who has the child that day by
/// its custody days ("Tom is with Sam that day."), else nobody.
fn proposal(agenda: &Agenda, text: &str, kind: Option<Kind>, config: &Config, zone: &jiff::tz::TimeZone) -> (Option<Who>, String) {
    let who_name = |who: Who| if who == Who::Me { tr().text("holder-you", None) } else { agenda.parent.clone() };
    if let Some(standing) = kind.and_then(|k| agenda.standing.get(&k).copied()) {
        return (Some(standing), say("holder-why-standing", &[("who", who_name(standing))]));
    }
    let Some((start, _)) = sioul_core::agenda::first_times(text, zone) else { return (None, String::new()) };
    let Ok(day) = jiff::Timestamp::from_second(start).map(|t| t.to_zoned(zone.clone()).date()) else { return (None, String::new()) };
    let Some(collection) = vdir::collections(vdir::Kind::Calendars).into_iter().find(|c| agendas::place(c) == agenda.id) else { return (None, String::new()) };
    let midnight = day.to_zoned(zone.clone()).map_or(start, |z| z.timestamp().as_second());
    let whole: Vec<String> = collection.items().iter().flat_map(|p| sioul_core::agenda::file_occurrences(p, &collection, midnight, midnight + 86_400, zone)).filter(|o| o.all_day && !o.cancelled).map(|o| o.summary).collect();
    let whole: Vec<&str> = whole.iter().map(String::as_str).collect();
    let mine: Vec<String> = config.every_account().filter_map(|a| a.name.clone()).chain([tr().text("holder-you", None)]).collect();
    // The other parent by every name they go by: their person's (docs/persons.md), and the agenda's word.
    let persons = sioul_core::persons::Persons::cached();
    let others: Vec<String> = persons.get(&agenda.parent_person).map(|p| vec![p.name.clone(), p.full_name.clone()]).unwrap_or_default().into_iter().chain([agenda.parent.clone()]).filter(|n| !n.trim().is_empty()).collect();
    match holders::custody_of(&whole, &mine, &others) {
        Some(who) => (Some(who), say("holder-why-custody", &[("child", child_name(agenda)), ("who", who_name(who))])),
        None => (None, String::new()),
    }
}

/// The questions due now, as JSON (asked once, research/agendas.md §6,
/// recommendation 9): each child's coming event or open task that nobody
/// takes and that came since this device's first look, each proposed to you,
/// each the other parent said not this time to; with its line, what you may
/// answer and the proposal. The Porch may list them as notices; the Agenda
/// shows them under its head line (`questions`, `takeChild`).
pub(crate) fn questions() -> String {
    let config = load_config();
    let here = agendas_here(&config);
    if !here.iter().any(|(a, _)| a.whose == Whose::Child) {
        return "[]".into();
    }
    let zone = sioul_core::zone::current();
    let now = sioul_core::zone::now().timestamp().as_second();
    let own = sioul_core::agenda::own_addresses();
    let asked = Asked::load(&Asked::path());
    let mut out: Vec<(i64, serde_json::Value)> = Vec::new();
    for ChildRead { path, text, agenda, .. } in child_items(&here).into_iter().filter(|c| !c.unknown) {
        let Some(uid) = holders::uid_of(&text) else { continue };
        let Some(asking) = holders::asking(&text, &own, &zone) else { continue };
        if !asked.due(&uid, asking, now) || config_stand_in(&config, &text) {
            continue;
        }
        // A coming event, or an open task.
        let task = text.contains("BEGIN:VTODO");
        let start = sioul_core::agenda::first_times(&text, &zone).map(|(s, _)| s).unwrap_or_default();
        if task && sioul_core::lines::unfold(&text).iter().any(|l| sioul_core::lines::name(l) == "STATUS" && matches!(sioul_core::lines::value(l).trim().to_ascii_uppercase().as_str(), "COMPLETED" | "CANCELLED")) {
            continue;
        }
        if !task && (start <= now || sioul_core::agenda::first_times(&text, &zone).is_none()) {
            continue;
        }
        let (title, when) = named(&text);
        let pairs = [("child", child_name(&agenda)), ("title", title), ("when", when), ("who", agenda.parent.clone())];
        let line = say(
            match asking {
                Asking::Nobody => "holder-question",
                Asking::Proposed => "holder-proposed",
                Asking::Reopened => "holder-reopened",
            },
            &pairs,
        );
        let kind = holders::kind_of(&text);
        let (proposal, why) = proposal(&agenda, &text, kind, &config, &zone);
        let busy = sioul_core::agenda::first_times(&text, &zone).map(|(s, e)| busy_then(&here, &agenda.parent, s, e)).unwrap_or_default();
        let why = [why, busy].into_iter().filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" ");
        out.push((
            if task { i64::MAX } else { start },
            serde_json::json!({ "key": path.display().to_string(), "uid": uid, "start": 0, "asking": asking, "line": line, "other": agenda.parent, "proposal": proposal.map(Who::id).unwrap_or_default(), "why": why }),
        ));
    }
    out.sort_by_key(|(at, _)| *at);
    json(&out.into_iter().map(|(_, v)| v).collect::<Vec<_>>())
}

// ---------------------------------------------------------------- who does what (part 4)

/// Who does what, one week of the children's agendas (part 4;
/// research/agendas.md §6, recommendation 11): each child's events and
/// tasks of that week, day by day, each with who takes it ("You", "Sam",
/// "Sam, asked", "Nobody yet"), and nothing more: no total per parent, no
/// history of who did more, since records invite score-keeping (Clark
/// 1984, §2.2). `from`: a day of the week ("2026-11-04"; "" this week);
/// `child`: one child's agenda by its place, "" every child; `mine`: what
/// you take alone, across children. Open tasks without a date come last.
pub(crate) fn who_does_what(from: &str, child: &str, mine: bool) -> String {
    who_does_what_as(from, child, mine, &sioul_core::agenda::own_addresses())
}

/// `who_does_what`, you being `own`.
fn who_does_what_as(from: &str, child: &str, mine: bool, own: &[String]) -> String {
    let config = load_config();
    let here = agendas_here(&config);
    let zone = sioul_core::zone::current();
    let today = sioul_core::zone::now().date();
    let day = from.parse::<jiff::civil::Date>().unwrap_or(today);
    let monday = day.checked_sub(jiff::Span::new().days(i64::from(day.weekday().to_monday_zero_offset()))).unwrap_or(day);
    let midnight = |d: jiff::civil::Date| d.to_zoned(zone.clone()).map_or(0, |z| z.timestamp().as_second());
    let (start, end) = (midnight(monday), monday.checked_add(jiff::Span::new().days(7)).map_or(midnight(monday) + 7 * 86_400, midnight));
    let children: Vec<&(Agenda, Collection)> = here.iter().filter(|(a, _)| a.whose == Whose::Child).collect();
    let who_of = |taken: &Taken| -> (String, bool) {
        match &taken.to {
            Some(h) if h.among(own) && h.takes() => (tr().text("holder-you-take", None), true),
            Some(h) if h.answer == Answer::Declined => (tr().text("holder-nobody-yet", None), false),
            Some(h) if h.among(own) => (tr().text("holder-you-asked", None), false),
            Some(h) => {
                let name = if h.name.is_empty() { h.address.clone() } else { h.name.clone() };
                (if h.takes() { name } else { say("holder-asked-short", &[("who", name)]) }, false)
            }
            None => (tr().text("holder-nobody-yet", None), false),
        }
    };
    // (its day's midnight, its start, the row); undated tasks at the end.
    let mut rows: Vec<(i64, i64, serde_json::Value)> = Vec::new();
    for (agenda, collection) in children.iter().filter(|(a, _)| child.is_empty() || a.id == child).map(|pair| (&pair.0, &pair.1)) {
        let name = child_name(agenda);
        let store = sioul_core::meeting_lines::load(&sioul_core::meeting_lines::default_path());
        for path in collection.items() {
            let Some((text, unknown)) = read_with(&path, &store) else { continue };
            // Who takes it kept on another device (R1): "Not known here", never "Nobody yet".
            let who_of = |taken: &Taken| if unknown { (tr().text("holder-not-known", None), false) } else { who_of(taken) };
            if text.contains("BEGIN:VTODO") {
                let lines = sioul_core::lines::unfold(&text);
                if lines.iter().any(|l| sioul_core::lines::name(l) == "STATUS" && matches!(sioul_core::lines::value(l).trim().to_ascii_uppercase().as_str(), "COMPLETED" | "CANCELLED")) {
                    continue;
                }
                let due = lines.iter().find(|l| sioul_core::lines::name(l) == "DUE").and_then(|l| sioul_core::lines::value(l).trim().get(..8).and_then(|d| jiff::civil::Date::strptime("%Y%m%d", d).ok()));
                let at = match due {
                    Some(d) if midnight(d) >= start && midnight(d) < end => midnight(d),
                    Some(_) => continue,
                    None => i64::MAX,
                };
                let taken = holders::taken(&text, None, &zone);
                let (who, yours) = who_of(&taken);
                if mine && !yours {
                    continue;
                }
                let (title, _) = named(&text);
                let uid = holders::uid_of(&text).unwrap_or_default();
                rows.push((at, at, serde_json::json!({ "kind": "task", "uri": sioul_core::links::task_uri(&uid), "key": path.display().to_string(), "start": 0, "title": title, "time": "", "child": name, "who": who, "yours": yours, "nobody": taken.to.is_none() })));
                continue;
            }
            for o in sioul_core::agenda::file_occurrences(&path, collection, start, end, &zone).into_iter().filter(|o| !o.cancelled) {
                let taken = holders::taken(&text, o.recurring.then_some(o.start), &zone);
                let (who, yours) = who_of(&taken);
                if mine && !yours {
                    continue;
                }
                let at = jiff::Timestamp::from_second(o.start).map(|t| t.to_zoned(zone.clone())).ok();
                let day = at.as_ref().map_or(start, |z| midnight(z.date()));
                let time = if o.all_day { String::new() } else { at.map(|z| z.strftime("%H:%M").to_string()).unwrap_or_default() };
                let title = if o.summary.trim().is_empty() { tr().text("agenda-untitled", None) } else { o.summary.clone() };
                rows.push((day, o.start, serde_json::json!({ "kind": "event", "uri": "", "key": o.key, "start": o.start, "title": title, "time": time, "child": name, "who": who, "yours": yours, "nobody": taken.to.is_none() })));
            }
        }
    }
    rows.sort_by_key(|(day, at, _)| (*day, *at));
    let mut days: Vec<serde_json::Value> = Vec::new();
    let mut current: Option<i64> = None;
    for (day, _, row) in rows {
        if current != Some(day) {
            let label = if day == i64::MAX {
                tr().text("holder-no-date", None)
            } else {
                jiff::Timestamp::from_second(day).map(|t| tr().day(t.to_zoned(zone.clone()).date())).unwrap_or_default()
            };
            days.push(serde_json::json!({ "label": label, "items": [] }));
            current = Some(day);
        }
        if let Some(items) = days.last_mut().and_then(|d| d["items"].as_array_mut()) {
            items.push(row);
        }
    }
    let shift = |n: i64| monday.checked_add(jiff::Span::new().days(n)).map(|d| d.to_string()).unwrap_or_default();
    json(&serde_json::json!({
        "from": monday.to_string(),
        "previous": shift(-7),
        "next": shift(7),
        "week": say("holder-week-of", &[("day", tr().day(monday))]),
        "children": children.iter().map(|(a, _)| serde_json::json!({ "id": a.id, "name": child_name(a) })).collect::<Vec<_>>(),
        "child": child,
        "mine": mine,
        "days": days,
    }))
}

/// "Sam is busy then.", when the other parent's agenda for availability
/// (another parent's or a partner's of that name) has a busy time over it:
/// consulted for who takes it, never said as a notice (research/agendas.md
/// §6, recommendation 14).
fn busy_then(here: &[(Agenda, Collection)], parent: &str, start: i64, end: i64) -> String {
    if parent.trim().is_empty() || end <= start {
        return String::new();
    }
    let zone = sioul_core::zone::current();
    let theirs = here.iter().filter(|(a, _)| a.busy() && matches!(a.whose, Whose::Parent | Whose::Partner) && a.person.eq_ignore_ascii_case(parent.trim()));
    let busy = theirs.into_iter().any(|(_, c)| c.items().iter().any(|p| sioul_core::agenda::file_occurrences(p, c, start, end, &zone).iter().any(|o| !o.cancelled && !o.transparent && o.start < end && o.end > start)));
    if busy { say("holder-busy", &[("who", parent.to_string())]) } else { String::new() }
}

// ---------------------------------------------------------------- an answer by mail

/// Whether a message comes from the address it shows, as the Porch judges
/// its sender (porch.rs, `checks_with`): not forged (`trust::judge_sender`,
/// a list excused as the Porch excuses it) and authenticated
/// (`trust::authenticated`, with the account's sealers): its results read
/// by the account's trusted ids (`trusted_ids`), your domains its own.
pub(crate) fn proven_sender(card: &sioul_core::card::Card, trusted_ids: &[String], own_domains: &[String]) -> bool {
    use sioul_core::trust;
    let auth = trust::read_auth_results(&card.headers, trusted_ids);
    let sealers = trust::sealers(trusted_ids, own_domains);
    let excused = card.is_list.then(|| trust::list_excused(auth.as_ref(), &card.headers, &sealers)).flatten();
    let (judged, _) = trust::judge_sender(auth.as_ref(), excused.as_ref(), card.sender_domain());
    judged != trust::Trust::Forged && trust::authenticated(auth.as_ref(), &card.headers, &sealers)
}

/// A REPLY in your mail to a child's event's invitation (RFC 5546 §3.2.3):
/// the holder line it names gets its answer and when it was given, once,
/// when the message is from the address it answers for and the Porch would
/// take it as coming from there (`proven_sender`): a From is easy to forge,
/// and a forged "Sam takes it" would leave a child's appointment with
/// nobody. Unproven, nothing is written, and the reader says so. Returns
/// the line the reader says ("Sam takes Tom to “Dentist”, Tuesday 10
/// November at 10:00."), or none for a REPLY of no child's event.
pub(crate) fn replied(reply: &str, message: &sioul_core::card::Card, trusted_ids: &[String], own_domains: &[String]) -> Option<String> {
    let (uid, address, answer, _) = holders::reply_of(reply)?;
    if !message.from_address.as_deref().is_some_and(|from| address.eq_ignore_ascii_case(from.trim())) {
        return None;
    }
    let config = load_config();
    let here = agendas_here(&config);
    let zone = sioul_core::zone::current();
    let ChildRead { path, text, agenda, unknown } = child_items(&here).into_iter().find(|c| holders::uid_of(&c.text).as_deref() == Some(uid.as_str()))?;
    let (title, when) = named(&text);
    let who = holders::taken(&text, None, &zone).to.filter(|h| h.is(&address)).map(|h| if h.name.is_empty() { h.address } else { h.name }).unwrap_or(address);
    if !proven_sender(message, trusted_ids, own_domains) {
        return Some(say("holder-replied-unproven", &[("who", who), ("child", child_name(&agenda)), ("title", title)]));
    }
    // Who takes it known here: the answer written, into the store for a meeting (`write_child`).
    if !unknown && let Some(written) = holders::replied(&text, reply, &zone) {
        write_child(&path, &text, &written).ok()?;
    }
    let id = if answer == Answer::Accepted { "holder-replied-yes" } else if answer == Answer::Declined { "holder-replied-no" } else { "holder-replied-maybe" };
    Some(say(id, &[("who", who), ("child", child_name(&agenda)), ("title", title), ("when", when)]))
}

/// The file of a child's item, by its UID ("" none): the child event a stand-in stands for.
pub(crate) fn child_key(uid: &str) -> Option<String> {
    if uid.is_empty() {
        return None;
    }
    let config = load_config();
    child_items(&agendas_here(&config)).into_iter().find(|c| holders::uid_of(&c.text).as_deref() == Some(uid)).map(|c| c.path.display().to_string())
}


#[cfg(test)]
mod tests {
    use super::*;

    /// Part 3 on this device's own files (stand-in folders, never the
    /// person's): a child's event nobody takes; taken ("Me"), its holder line
    /// is written and your stand-in comes into the agenda its kind's go to,
    /// its file named after its derived UID, said with what Undo puts back;
    /// asked of the other parent, who answers in their own Sioul, the
    /// stand-in goes and no draft is made; "Later" writes this device's
    /// question alone; the card says where it stands at each step.
    #[test]
    fn a_child_s_event_taken_then_asked_of_the_other_parent() {
        sioul_core::config::stand_ins_only();
        let config = crate::backend::config_path();
        let tom = vdir::create(vdir::Kind::Calendars, vdir::LOCAL, "Tom, part 3", None, &["VEVENT", "VTODO"]).unwrap();
        let mine = vdir::create(vdir::Kind::Calendars, vdir::LOCAL, "Mine, part 3", None, &["VEVENT"]).unwrap();
        let place = agendas::place;
        for (field, value) in [("whose", "child"), ("name", "Tom"), ("parent", "Sam"), ("parent_address", "sam@example.org"), ("parent_answers", "sioul")] {
            agendas::set_field(&config, &place(&tom), field, value).unwrap();
        }
        agendas::set_stand_in_agenda(&config, Kind::Dentist, Some(&place(&mine))).unwrap();
        // The configuration just written is read at once (`agendas::read_config` compares its bytes).
        let start = sioul_core::zone::now().timestamp().as_second() / 60 * 60 + 5 * 86_400;
        let utc = sioul_core::moves::utc;
        let child = format!(
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:part3-dentist\r\nDTSTAMP:{}\r\nDTSTART:{}\r\nDTEND:{}\r\nSUMMARY:Dentist\r\nX-SIOUL-CONCEPT;VALUE=URI:tag:aurelienpierre.com,2026:sioul/child-kind/dentist\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
            utc(start - 86_400),
            utc(start),
            utc(start + 1_800)
        );
        let file = tom.dir.join("part3-dentist.ics");
        vdir::write_item(&file, &child).unwrap();
        let key = file.display().to_string();
        let me = Me { name: "Alex".into(), address: "alex@example.org".into(), own: vec!["alex@example.org".into()] };
        let state = || card_as(&file, 0, &me.own).map(|c| c["state"].as_str().unwrap_or_default().to_string()).unwrap_or_default();
        assert_eq!(state(), "nobody");
        // Taken: the line, the stand-in in the dentist's agenda, both said with Undo.
        let (files, line) = take_files(&key, 0, "me", &me).unwrap().unwrap();
        let stand_in = mine.dir.join(format!("{}.ics", holders::stand_in_uid("part3-dentist", &place(&mine))));
        assert!(stand_in.exists(), "{files:?}");
        assert_eq!(files.iter().map(|f| (f.path.clone(), f.before.is_some(), f.after.is_some())).collect::<Vec<_>>(), [(file.clone(), true, true), (stand_in.clone(), false, true)]);
        assert!(line.contains("Tom") && line.contains("Dentist"), "{line}");
        let held = std::fs::read_to_string(&file).unwrap();
        assert!(sioul_core::lines::unfold(&held).iter().any(|l| l.starts_with("X-SIOUL-TAKEN-BY;CN=Alex;PARTSTAT=ACCEPTED;SINCE=") && l.ends_with(":mailto:alex@example.org")), "{held}");
        assert_eq!(state(), "yours");
        let made = std::fs::read_to_string(&stand_in).unwrap();
        assert_eq!(holders::stands_for(&made).as_deref(), Some("part3-dentist"));
        // Who does what (part 4): its week lists it, "You", and "What I take" keeps it.
        let day = jiff::Timestamp::from_second(start).unwrap().to_zoned(sioul_core::zone::current()).date().to_string();
        let week = |mine: bool| serde_json::from_str::<serde_json::Value>(&who_does_what_as(&day, &place(&tom), mine, &me.own)).unwrap();
        let rows = |w: &serde_json::Value| w["days"].as_array().unwrap().iter().flat_map(|d| d["items"].as_array().unwrap().clone()).collect::<Vec<_>>();
        let listed = rows(&week(false));
        assert!(listed.iter().any(|r| r["title"] == "Dentist" && r["child"] == "Tom" && r["yours"] == true), "{listed:?}");
        assert_eq!(rows(&week(true)).len(), listed.len(), "what I take: all of it, here");
        // Taken again: the same stand-in, never a second one.
        take_files(&key, 0, "me", &me).unwrap();
        assert_eq!(mine.items().len(), 1);
        // Asked of Sam, who answers in their own Sioul: the stand-in goes, no draft.
        // Counted for this event alone: other tests draft beside this one (the meetings you organize).
        let drafts_here = || sioul_core::compose::Draft::all().into_iter().filter(|d| d.calendar.as_deref().is_some_and(|ics| ics.contains("part3-dentist")) || d.to.iter().any(|t| t.contains("sam@example.org"))).count();
        let drafts_before = drafts_here();
        let (files, _) = take_files(&key, 0, "other", &me).unwrap().unwrap();
        assert!(!stand_in.exists() && files.iter().any(|f| f.path == stand_in && f.after.is_none()), "{files:?}");
        assert_eq!(drafts_here(), drafts_before);
        assert_eq!(state(), "asked");
        let after: Vec<serde_json::Value> = serde_json::from_str::<serde_json::Value>(&who_does_what_as(&day, &place(&tom), true, &me.own)).unwrap()["days"].as_array().unwrap().iter().flat_map(|d| d["items"].as_array().unwrap().clone()).collect();
        assert!(after.is_empty(), "asked of Sam: no longer what I take: {after:?}");
        // "Later" writes this device's question alone.
        let asked = std::fs::read_to_string(&file).unwrap();
        assert!(take_files(&key, 0, "later", &me).unwrap().is_none());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), asked);
        assert!(Asked::load(&Asked::path()).later.contains_key("part3-dentist"));
        // Without an address of yours, nothing is written.
        let nobody = Me { name: String::new(), address: String::new(), own: Vec::new() };
        assert!(take_files(&key, 0, "me", &nobody).is_err());
        for c in [tom, mine] {
            let _ = std::fs::remove_dir_all(&c.dir);
        }
        let _ = agendas::set_stand_in_agenda(&config, Kind::Dentist, None);
    }

    /// A child's event a guest was invited to (a meeting, its file as its
    /// guests read it, Sioul's lines of it kept apart: second review of the
    /// organizer's door, N6): who takes it and its kind are read from
    /// Sioul's own store of them, and taking it writes there, never into
    /// the meeting, whose server would send them to its guests; the stand-in
    /// comes as for any child's event, and Undo puts both back.
    #[test]
    fn a_child_s_meeting_keeps_who_takes_it_apart() {
        sioul_core::config::stand_ins_only();
        let config = crate::backend::config_path();
        let tom = vdir::create(vdir::Kind::Calendars, vdir::LOCAL, "Tom, a meeting", None, &["VEVENT", "VTODO"]).unwrap();
        let mine = vdir::create(vdir::Kind::Calendars, vdir::LOCAL, "Mine, a meeting", None, &["VEVENT"]).unwrap();
        let place = agendas::place;
        for (field, value) in [("whose", "child"), ("name", "Tom"), ("parent", "Sam"), ("parent_address", "sam@example.org"), ("parent_answers", "sioul")] {
            agendas::set_field(&config, &place(&tom), field, value).unwrap();
        }
        agendas::set_stand_in_agenda(&config, Kind::Activity, Some(&place(&mine))).unwrap();
        let start = sioul_core::zone::now().timestamp().as_second() / 60 * 60 + 6 * 86_400;
        let utc = sioul_core::moves::utc;
        let child = format!(
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:n6-child-meeting\r\nDTSTAMP:{}\r\nDTSTART:{}\r\nDTEND:{}\r\nSUMMARY:Swimming\r\nORGANIZER:mailto:alex@example.org\r\nATTENDEE;SCHEDULE-AGENT=CLIENT:mailto:coach@example.net\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
            utc(start - 86_400),
            utc(start),
            utc(start + 1_800)
        );
        let file = tom.dir.join("n6-child-meeting.ics");
        vdir::store_item(&file, &child).unwrap();
        // This test's own store of those lines: other tests write theirs beside it.
        let store = sioul_core::config::data_dir().join(format!("meeting-lines-children-{}.toml", std::process::id()));
        sioul_core::meeting_lines::path_here(Some(&store));
        assert_eq!(sioul_core::meeting_lines::default_path(), store);
        sioul_core::meeting_lines::add(&store, "n6-child-meeting", &[(String::new(), sioul_core::tasks::concept_line(&Kind::Activity.concept()))]).unwrap();
        let key = file.display().to_string();
        let me = Me { name: "Alex".into(), address: "alex@example.org".into(), own: vec!["alex@example.org".into()] };
        let card = |what: &str| card_as(&file, 0, &me.own).map(|c| c[what].clone()).unwrap_or_default();
        assert_eq!((card("state"), card("kind")), (serde_json::json!("nobody"), serde_json::json!("activity")), "its kind read from the store");
        let (files, _) = take_files(&key, 0, "me", &me).unwrap().unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), child, "the meeting untouched");
        let kept = sioul_core::meeting_lines::load(&store).events.get("n6-child-meeting").cloned().unwrap_or_default();
        assert!(kept.times.get("").is_some_and(|l| l.iter().any(|l| l.starts_with("X-SIOUL-TAKEN-BY;CN=Alex;PARTSTAT=ACCEPTED"))), "{kept:?}");
        assert_eq!(card("state"), serde_json::json!("yours"));
        let stand_in = mine.dir.join(format!("{}.ics", holders::stand_in_uid("n6-child-meeting", &place(&mine))));
        assert!(stand_in.exists() && files.iter().any(|f| f.path == store) && files.iter().any(|f| f.path == stand_in), "{files:?}");
        // Undo: the store and the stand-in as they were.
        assert!(sioul_core::agentwrites::Said { at: 0, line: String::new(), files }.put_back());
        assert!(!stand_in.exists() && card("state") == serde_json::json!("nobody"));
        sioul_core::meeting_lines::path_here(None);
        let _ = std::fs::remove_file(&store);
        for c in [tom, mine] {
            let _ = std::fs::remove_dir_all(&c.dir);
        }
        let _ = agendas::set_stand_in_agenda(&config, Kind::Activity, None);
    }

    /// The third review of the organizer's door, R1 (its two probes
    /// turned). Tom's psychiatrist, which Alex takes: its stand-in in Alex's
    /// agenda for work says "Away". A guest invited to it on another device:
    /// its file here holds neither its kind nor who takes it, and this device
    /// keeps nothing of them, so who takes it is unknown here: the upkeep
    /// leaves the stand-in exactly as it is, never unmasked; the card says it
    /// and offers nothing; taking it is refused; nothing asks of it; the week
    /// says "Not known here"; taking its last guest off here is refused.
    /// This device keeping them (the one that invited the guest): the
    /// stand-in stays masked; taking the last guest off puts them back into
    /// the file and forgets them, the stand-in still masked. A child's event
    /// the door makes a meeting here is known here, even with nothing kept.
    #[test]
    fn a_child_s_meeting_kept_on_another_device_stays_as_it_is() {
        sioul_core::config::stand_ins_only();
        let dir = sioul_core::config::data_dir().join(format!("r1-children-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let config = dir.join("config.toml");
        std::fs::write(&config, "language = \"en\"\n\n[[account]]\nid = \"r1-alex\"\nkind = \"imap\"\naddress = \"alex@example.org\"\nname = \"Alex\"\nhost = \"imap.example.org\"\n").unwrap();
        crate::backend::TEST_CONFIG.with(|own| *own.borrow_mut() = Some(config.clone()));
        sioul_core::agenda::own_addresses_here(Some(&["alex@example.org"]));
        sioul_core::agenda::open_organizer_door_here(true);
        let store = dir.join(sioul_core::meeting_lines::FILE);
        sioul_core::meeting_lines::path_here(Some(&store));
        let asked = dir.join("holder-questions.toml");
        let tom = vdir::create(vdir::Kind::Calendars, vdir::LOCAL, "Tom, R1", None, &["VEVENT", "VTODO"]).unwrap();
        let work = vdir::create(vdir::Kind::Calendars, vdir::LOCAL, "Work, R1", None, &["VEVENT"]).unwrap();
        let place = agendas::place;
        for (field, value) in [("whose", "child"), ("name", "Tom"), ("parent", "Sam"), ("parent_address", "sam@example.org"), ("parent_answers", "sioul")] {
            agendas::set_field(&config, &place(&tom), field, value).unwrap();
        }
        agendas::set_field(&config, &place(&work), "area", "work").unwrap();
        agendas::set_stand_in_agenda(&config, Kind::Health, Some(&place(&work))).unwrap();
        let zone = sioul_core::zone::current();
        let now = sioul_core::zone::now().timestamp().as_second();
        let start = now / 60 * 60 + 6 * 86_400;
        let utc = sioul_core::moves::utc;
        let child = format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:r1-psy\r\nDTSTAMP:{}\r\nDTSTART:{}\r\nDTEND:{}\r\nSUMMARY:Tom: psychiatrist\r\nLOCATION:Dr Martin\\, 12 rue des Lilas\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n", utc(start - 86_400), utc(start), utc(start + 1_800));
        let alex = Holder { name: "Alex".into(), address: "alex@example.org".into(), answer: Answer::Accepted, since: Some(now), back: false };
        let held = holders::with_kind(&holders::with_taken(&child, None, &Taken { to: Some(alex), back: None }, &zone).unwrap(), Some(Kind::Health)).unwrap();
        let file = tom.dir.join("r1-psy.ics");
        vdir::write_item(&file, &held).unwrap();
        let key = file.display().to_string();
        upkeep_at(&[], &asked);
        let stand_in = work.dir.join(format!("{}.ics", holders::stand_in_uid("r1-psy", &place(&work))));
        let masked = std::fs::read_to_string(&stand_in).unwrap();
        // Its title "Away" in the window's language (another test may have set it), its place out.
        assert!(masked.contains("CLASS:CONFIDENTIAL") && !masked.contains("psychiatrist") && !masked.contains("Lilas"), "{masked}");
        let events = [sioul_core::agenda::EventRef { key: stand_in.display().to_string(), uid: holders::stand_in_uid("r1-psy", &place(&work)), ..Default::default() }];
        // A guest invited on another device: the file as its guests read it, nothing kept here.
        let meeting = held.replace("END:VEVENT", "ORGANIZER;CN=Alex:mailto:alex@example.org\r\nATTENDEE;SCHEDULE-AGENT=CLIENT:mailto:office@example.net\r\nEND:VEVENT");
        let (there, aside) = sioul_core::organizer::set_aside(&meeting);
        vdir::store_item(&file, &there).unwrap();
        let there = std::fs::read_to_string(&file).unwrap();
        upkeep_at(&events, &asked);
        assert_eq!(std::fs::read_to_string(&stand_in).unwrap(), masked, "never unmasked, never rewritten");
        let own = vec!["alex@example.org".to_string()];
        let card = card_as(&file, 0, &own).unwrap();
        assert_eq!((card["state"].as_str(), card["line"].as_str(), card["writable"].as_bool()), (Some("unknown"), Some(tr().text("holder-line-unknown", None).as_str()), Some(false)));
        let me = Me { name: "Alex".into(), address: "alex@example.org".into(), own: own.clone() };
        assert_eq!(take_files(&key, 0, "me", &me).err(), Some(tr().text("holder-unknown", None)), "an answer here would be this device's alone");
        assert!(!questions().contains("r1-psy"), "never asked");
        let day = jiff::Timestamp::from_second(start).unwrap().to_zoned(zone.clone()).date().to_string();
        let week: serde_json::Value = serde_json::from_str(&who_does_what_as(&day, &place(&tom), false, &own)).unwrap();
        assert!(week["days"].as_array().unwrap().iter().flat_map(|d| d["items"].as_array().unwrap().clone()).any(|r| r["who"] == tr().text("holder-not-known", None)), "{week}");
        let off = crate::organized::plan(&file, Some(&there), &there, Some(&[]), &tom);
        assert_eq!(off.err(), Some(tr().text("invite-holder-elsewhere", None)), "its lines are on another device");
        // This device keeps them (it invited the guest): known, the stand-in masked still.
        sioul_core::meeting_lines::add(&store, "r1-psy", &aside.lines).unwrap();
        upkeep_at(&events, &asked);
        assert_eq!(std::fs::read_to_string(&stand_in).unwrap(), masked);
        assert_eq!(card_as(&file, 0, &own).unwrap()["state"], "yours");
        // Its last guest taken off here: its lines back into its file, forgotten in the store.
        let plan = crate::organized::plan(&file, Some(&there), &there, Some(&[]), &tom).unwrap();
        assert!(plan.lines_back && !sioul_core::agenda::with_others(&plan.text), "{}", plan.text);
        let written = crate::organized::write(&plan).unwrap();
        let back = std::fs::read_to_string(&file).unwrap();
        assert_eq!((holders::kind_of(&back), holders::taken(&back, None, &zone).to.map(|h| h.address)), (Some(Kind::Health), Some("alex@example.org".to_string())), "{back}");
        assert!(!sioul_core::meeting_lines::load(&store).events.contains_key("r1-psy"));
        // Undo, its cancellation unsent: the meeting back as its guests read it, its lines kept apart again.
        assert_eq!(crate::organized::put_back(&written), Ok(()));
        assert_eq!(std::fs::read_to_string(&file).unwrap(), there);
        assert!(sioul_core::meeting_lines::load(&store).events.get("r1-psy").is_some_and(|k| k.times[""].iter().any(|l| l.starts_with("X-SIOUL-TAKEN-BY"))), "kept apart again");
        let written = crate::organized::write(&crate::organized::plan(&file, Some(&there), &there, Some(&[]), &tom).unwrap()).unwrap();
        written.drafts.iter().filter_map(|id| sioul_core::compose::Draft::by_id(id)).for_each(|d| d.discard());
        assert!(!sioul_core::meeting_lines::load(&store).events.contains_key("r1-psy"));
        upkeep_at(&events, &asked);
        assert_eq!(std::fs::read_to_string(&stand_in).unwrap(), masked, "still masked");
        // A child's event made a meeting here, nothing of Sioul's in it: known here.
        let dentist = tom.dir.join("r1-dentist.ics");
        let plain = child.replace("UID:r1-psy", "UID:r1-dentist").replace("Tom: psychiatrist", "Dentist");
        vdir::write_item(&dentist, &plain).unwrap();
        let office = [sioul_core::organizer::Guest { address: "office@example.net".into(), name: String::new() }];
        let written = crate::organized::write(&crate::organized::plan(&dentist, Some(&plain), &plain, Some(&office), &tom).unwrap()).unwrap();
        written.drafts.iter().filter_map(|id| sioul_core::compose::Draft::by_id(id)).for_each(|d| d.discard());
        assert_eq!(sioul_core::meeting_lines::load(&store).events.get("r1-dentist"), Some(&sioul_core::meeting_lines::Kept::default()));
        assert_eq!(card_as(&dentist, 0, &own).unwrap()["state"], "nobody", "known here: nobody takes it yet");
        sioul_core::meeting_lines::path_here(None);
        sioul_core::agenda::open_organizer_door_here(false);
        sioul_core::agenda::own_addresses_here(None);
        crate::backend::TEST_CONFIG.with(|own| *own.borrow_mut() = None);
        for c in [tom, work] {
            let _ = std::fs::remove_dir_all(&c.dir);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A REPLY the Porch would not take as coming from its sender (SPF and
    /// DKIM failing, no DMARC pass, by the account's trusted results) changes
    /// nothing, and the reader says so; the same REPLY, proven, sets the
    /// answer at its DTSTAMP.
    #[test]
    fn an_unproven_reply_changes_nothing() {
        sioul_core::config::stand_ins_only();
        let config = crate::backend::config_path();
        let lea = vdir::create(vdir::Kind::Calendars, vdir::LOCAL, "Léa, a reply", None, &["VEVENT"]).unwrap();
        for (field, value) in [("whose", "child"), ("name", "Léa"), ("parent", "Sam"), ("parent_address", "sam@example.org")] {
            agendas::set_field(&config, &agendas::place(&lea), field, value).unwrap();
        }
        let asked = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:reply-dentist\r\nDTSTART:20991110T090000Z\r\nDTEND:20991110T100000Z\r\nSUMMARY:Dentist\r\nX-SIOUL-TAKEN-BY;CN=Sam;PARTSTAT=NEEDS-ACTION;SINCE=20991101T090000Z:mailto:sam@example.org\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let file = lea.dir.join("reply-dentist.ics");
        vdir::write_item(&file, asked).unwrap();
        let asked = std::fs::read_to_string(&file).unwrap();
        let reply = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nMETHOD:REPLY\r\nBEGIN:VEVENT\r\nUID:reply-dentist\r\nDTSTAMP:20991102T090000Z\r\nATTENDEE;PARTSTAT=ACCEPTED;CN=Sam:mailto:sam@example.org\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let message = |results: &str| {
            let raw = format!("From: Sam <sam@example.org>\r\nTo: alex@example.org\r\nSubject: Accepted: Dentist\r\nMessage-ID: <reply-1@example.org>\r\nAuthentication-Results: mx.example.net; {results}\r\n\r\nAccepted.\r\n");
            sioul_core::card::Card::from_bytes(raw.as_bytes()).unwrap()
        };
        let trusted = vec!["mx.example.net".to_string()];
        let failing = message("spf=fail smtp.mailfrom=example.org; dkim=fail header.d=example.org; dmarc=none header.from=example.org");
        let said = replied(reply, &failing, &trusted, &[]).unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), asked, "nothing written");
        assert_eq!(said, say("holder-replied-unproven", &[("who", "Sam".into()), ("child", "Léa".into()), ("title", "Dentist".into())]));
        // Proven (SPF passing, nothing forged): the answer is written, at its DTSTAMP.
        let passing = message("spf=pass smtp.mailfrom=example.org; dkim=fail header.d=example.org; dmarc=none header.from=example.org");
        replied(reply, &passing, &trusted, &[]).unwrap();
        let holder = holders::taken(&std::fs::read_to_string(&file).unwrap(), None, &sioul_core::zone::current()).to.unwrap();
        assert_eq!((holder.answer, holder.since), (Answer::Accepted, sioul_core::moves::read_utc("20991102T090000Z")));
        let _ = std::fs::remove_dir_all(&lea.dir);
    }
}
