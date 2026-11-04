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
fn named(text: &str) -> (String, String) {
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

/// The children's items here: each file's text, with its agenda.
fn child_items(here: &[(Agenda, Collection)]) -> Vec<(PathBuf, String, Agenda)> {
    here.iter()
        .filter(|(a, _)| a.whose == Whose::Child)
        .flat_map(|(a, c)| c.items().into_iter().filter_map(move |path| std::fs::read_to_string(&path).ok().map(|text| (path, text, a.clone()))))
        .collect()
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
    let items = child_items(&here);
    let uids: BTreeSet<String> = items.iter().filter_map(|(_, t, _)| holders::uid_of(t)).collect();
    // Your agendas' events that may stand in for a child's.
    let places: BTreeMap<PathBuf, (String, bool)> = here.iter().filter(|(a, _)| a.whose == Whose::Yours && a.events).map(|(a, c)| (c.dir.clone(), (a.id.clone(), a.for_work()))).collect();
    let derived: BTreeSet<String> = uids.iter().flat_map(|u| places.values().map(move |(place, _)| holders::stand_in_uid(u, place))).collect();
    let candidates: Vec<(PathBuf, String, String, bool)> = events
        .iter()
        .filter(|e| derived.contains(&e.uid) || e.related.iter().any(|r| r.kind.eq_ignore_ascii_case("PARENT") && uids.contains(&r.uid)))
        .filter_map(|e| {
            let path = PathBuf::from(&e.key);
            let (place, work) = path.parent().and_then(|d| places.get(d)).cloned()?;
            let text = std::fs::read_to_string(&path).ok()?;
            Some((path, text, place, work))
        })
        .collect();
    let children: Vec<holders::ChildItem> = items.iter().map(|(path, text, agenda)| holders::ChildItem { path, text, agenda }).collect();
    let mine: Vec<holders::Mine> = candidates.iter().map(|(path, text, place, work)| holders::Mine { path, text, place, work: *work }).collect();
    let target = |kind: Option<Kind>| target_of(&config, &here, kind);
    let follow_up = |agenda: &Agenda, kind: Kind| follow_up_of(&config, &here, agenda, kind);
    let away = tr().text("holder-away", None);
    let cx = holders::Context { own: &own, target: &target, details: &details_of, away: &away, follow_up: &follow_up, now, zone: &zone };
    let acts = holders::upkeep(&children, &mine, &cx);
    let named_child = |uid: &str| items.iter().find(|(_, t, _)| holders::uid_of(t).as_deref() == Some(uid)).map(|(_, t, a)| (child_name(a), named(t)));
    for act in acts {
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
    for (_, text, agenda) in &items {
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
    let path = Asked::path();
    let mut asked = Asked::load(&path);
    let mut changed = asked.look(&uids);
    for (_, text, agenda) in &items {
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

/// The child's item a card or a question names, with its agenda: its file, text, agenda.
fn child_item(key: &str) -> Result<(PathBuf, String, Agenda, Collection), String> {
    let path = crate::pim::ours(key).ok_or_else(|| tr().text("agenda-gone", None))?;
    let text = std::fs::read_to_string(&path).map_err(|_| tr().text("agenda-gone", None))?;
    let collection = crate::pim::collection_of(&path).ok_or_else(|| tr().text("agenda-gone", None))?;
    let agenda = agendas::agenda_of(&collection, &load_config());
    if agenda.whose != Whose::Child {
        return Err(tr().text("holder-not-child", None));
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
        vdir::write_item(&path, &written)?;
        let (title, when) = named(&written);
        let line = say("holder-said-back", &[("child", child_name(&agenda)), ("title", title), ("when", when)]);
        return Ok(Some((vec![sioul_core::agentwrites::FileChange { path, before: Some(text), after: Some(written) }], line)));
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
    sioul_core::agenda::guard(&path, sioul_core::agenda::Change::Write(&written)).map_err(|r| sioul_core::agenda::said(r, &sioul_core::agenda::Change::Write(&written)))?;
    // The other parent asked by mail: the invitation made first, nothing written without it.
    let mut sent = read_sent();
    let sequence = sent.invitations.get(&uid).map_or(0, |i| i.sequence + 1);
    let draft = if who == "other" && agenda.parent_answers == Answers::Mail { Some(draft_invitation(&config, &agenda, &written, "REQUEST", sequence)?) } else { None };
    let mut files = vec![sioul_core::agentwrites::FileChange { path: path.clone(), before: Some(text.clone()), after: Some(written.clone()) }];
    vdir::write_item(&path, &written)?;
    // Your stand-in at once, as the next plan would make or take it out.
    let here = agendas_here(&config);
    let candidates: Vec<(PathBuf, String, String, bool)> = here
        .iter()
        .filter(|(a, _)| a.whose == Whose::Yours && a.events)
        .filter_map(|(a, c)| {
            let file = c.dir.join(format!("{}.ics", holders::stand_in_uid(&uid, &a.id)));
            std::fs::read_to_string(&file).ok().map(|t| (file, t, a.id.clone(), a.for_work()))
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
            vdir::write_item(&path, &written)?;
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
    let text = std::fs::read_to_string(path).ok()?;
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
    for (path, text, agenda) in child_items(&here) {
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
        for path in collection.items() {
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
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
fn proven_sender(card: &sioul_core::card::Card, trusted_ids: &[String], own_domains: &[String]) -> bool {
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
    let (path, text, agenda) = child_items(&here).into_iter().find(|(_, t, _)| holders::uid_of(t).as_deref() == Some(uid.as_str()))?;
    let (title, when) = named(&text);
    let who = holders::taken(&text, None, &zone).to.filter(|h| h.is(&address)).map(|h| if h.name.is_empty() { h.address } else { h.name }).unwrap_or(address);
    if !proven_sender(message, trusted_ids, own_domains) {
        return Some(say("holder-replied-unproven", &[("who", who), ("child", child_name(&agenda)), ("title", title)]));
    }
    if let Some(written) = holders::replied(&text, reply, &zone) {
        vdir::write_item(&path, &written).ok()?;
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
    child_items(&agendas_here(&config)).into_iter().find(|(_, t, _)| holders::uid_of(t).as_deref() == Some(uid)).map(|(p, _, _)| p.display().to_string())
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
        let drafts_before = sioul_core::compose::Draft::all().len();
        let (files, _) = take_files(&key, 0, "other", &me).unwrap().unwrap();
        assert!(!stand_in.exists() && files.iter().any(|f| f.path == stand_in && f.after.is_none()), "{files:?}");
        assert_eq!(sioul_core::compose::Draft::all().len(), drafts_before);
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
