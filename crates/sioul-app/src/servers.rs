// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Agendas on their servers (several agendas, part 5; docs/client.md,
//! "Agendas on their servers"), from Agenda ⚙ ▸ Your agendas: made with
//! their role, renamed, recoloured, removed (a dated copy first) or left;
//! their sharing read and changed; your availability shared with one person.
//! What is made, renamed, recoloured, removed or left is written here and
//! sent by the next sync (`dav::sync`); sharing is asked of the server at
//! once, in a thread of its own, its answer the `agenda_shared` signal. Only
//! the window does any of this: agents never make, remove or share an agenda.

use crate::backend::{QtThread, Shared, json, load_config, say, tell, tr};
use serde::Deserialize;
use sioul_core::agendas::{self, Agenda, Whose};
use sioul_core::availability::Availability;
use sioul_core::config::{Account, Config};
use sioul_core::vdir::{self, Collection, Kind, State};
use sioul_sync::calendars::{self, Access, GoogleApi, Sharer, Sharing};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

/// How each account's server shared when last asked: what a removal says of
/// its trash bin (Nextcloud's), by account.
static SHARERS: Mutex<BTreeMap<String, Sharer>> = Mutex::new(BTreeMap::new());

/// Where an agenda lives: "local" (this computer), "google", "caldav".
fn server_of(config: &Config, collection: &Collection) -> &'static str {
    if collection.account == vdir::LOCAL {
        "local"
    } else if config.every_account().find(|a| a.id == collection.account).is_some_and(|a| a.auth.as_deref() == Some("google")) {
        "google"
    } else {
        "caldav"
    }
}

/// The server's name as a person reads it: "Google", "this computer", its host.
fn server_name(config: &Config, collection: &Collection) -> String {
    match server_of(config, collection) {
        "local" => tr().text("agenda-server-here", None),
        "google" => "Google".into(),
        _ => {
            let url = State::load(&collection.state_path()).url;
            let host = url.split("://").nth(1).and_then(|r| r.split(['/', ':']).next()).unwrap_or_default().to_string();
            if host.is_empty() { collection.account.clone() } else { host }
        }
    }
}

/// A Google account's main calendar (its id the account's address), which
/// Google never deletes: `calendars.delete` takes secondary calendars only
/// (Calendar API v3, calendars, read 4 November 2026).
fn google_primary(config: &Config, collection: &Collection) -> bool {
    let Some(account) = config.every_account().find(|a| a.id == collection.account && a.auth.as_deref() == Some("google")) else { return false };
    let address = account.address.as_deref().unwrap_or_default();
    !address.is_empty() && calendars::google_id(&State::load(&collection.state_path()).url).is_some_and(|id| id.eq_ignore_ascii_case(address))
}

/// An agenda and its collection, by place.
fn find(id: &str) -> Option<(Collection, Agenda, Config)> {
    let config = load_config();
    let collection = vdir::collections(Kind::Calendars).into_iter().find(|c| agendas::place(c) == id)?;
    let agenda = agendas::agenda_of(&collection, &config);
    Some((collection, agenda, config))
}

/// Whether an agenda is yours to remove: yours, and one its server lets you write.
/// Whether an agenda is yours to rename, recolour, share and remove: its
/// server says you own it (`State::owned`, read at each listing), or you made
/// it here and it is not sent yet, or it is on this computer; and you may
/// write it. Its role never says it: a child's agenda on your own server is
/// yours, a calendar shared with you is not, whatever you call it.
fn own(agenda: &Agenda, collection: &Collection) -> bool {
    let state = State::load(&collection.state_path());
    let yours = collection.account == vdir::LOCAL || state.pending || state.owned == Some(true);
    yours && !agenda.server_read_only && !collection.read_only
}

/// Whether its server says the calendar is someone else's: the only one a
/// Stop showing leaves on the server; any other is hidden on this device alone.
fn someone_elses(collection: &Collection) -> bool {
    State::load(&collection.state_path()).owned == Some(false)
}

/// What Agenda ⚙ shows of an agenda's server: where it lives, whether it is
/// yours to rename and remove, or someone else's to stop showing.
pub(crate) fn line_of(config: &Config, collection: &Collection, agenda: &Agenda) -> serde_json::Value {
    serde_json::json!({
        "server": server_of(config, collection),
        "server_name": server_name(config, collection),
        "own": own(agenda, collection),
    })
}

/// The accounts a new agenda can go into: calendars-and-contacts accounts, and this computer.
pub(crate) fn accounts(config: &Config) -> Vec<serde_json::Value> {
    let mut out: Vec<serde_json::Value> = config
        .accounts
        .iter()
        .filter(|a| a.is_dav())
        .map(|a| serde_json::json!({ "id": a.id, "name": a.address.clone().unwrap_or_else(|| a.id.clone()), "google": a.auth.as_deref() == Some("google") }))
        .collect();
    out.push(serde_json::json!({ "id": vdir::LOCAL, "name": tr().text("agenda-server-here", None), "google": false }));
    out
}

// ---------------------------------------------------------------- made, renamed, recoloured, removed

#[derive(Deserialize)]
struct NewAgenda {
    account: String,
    name: String,
    #[serde(default)]
    colour: String,
    #[serde(default = "yes")]
    events: bool,
    #[serde(default)]
    tasks: bool,
    #[serde(default)]
    whose: String,
    #[serde(default)]
    person: String,
    #[serde(default)]
    area: String,
    #[serde(default)]
    reaches: String,
}

fn yes() -> bool {
    true
}

/// A new agenda (`text`, JSON: account, name, colour, events, tasks, whose,
/// person, area, reaches), made here with its role, and on its server by the
/// next sync (MKCALENDAR, RFC 4791 §5.3.1; Google's `calendars.insert`, its
/// events only). Returns its place, or what went wrong.
pub(crate) fn make(qt: &QtThread, shared: &Arc<Shared>, text: &str) -> Result<String, String> {
    let asked: NewAgenda = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let config = load_config();
    let name = asked.name.trim();
    if name.is_empty() {
        return Err(tr().text("agenda-new-needs-name", None));
    }
    let google = config.every_account().find(|a| a.id == asked.account).is_some_and(|a| a.auth.as_deref() == Some("google"));
    if asked.account != vdir::LOCAL && !config.accounts.iter().any(|a| a.id == asked.account && a.is_dav()) {
        return Err(tr().text("agenda-new-no-account", None));
    }
    // Google's calendars hold events; its task lists are Google Tasks'.
    let components: Vec<&str> = match (asked.events, asked.tasks) {
        (true, true) if google => vec!["VEVENT"],
        (true, true) => vec!["VEVENT", "VTODO"],
        (false, true) => vec!["VTODO"],
        _ => vec!["VEVENT"],
    };
    let colour = Some(asked.colour.trim()).filter(|c| c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|x| x.is_ascii_hexdigit())).map(str::to_ascii_lowercase);
    let made = vdir::create(Kind::Calendars, &asked.account, name, colour.as_deref(), &components)?;
    let place = agendas::place(&made);
    let path = crate::backend::config_path();
    for (field, value) in [("whose", asked.whose.as_str()), ("name", asked.person.as_str()), ("area", asked.area.as_str()), ("reaches", asked.reaches.as_str())] {
        if !value.trim().is_empty() && !(field == "whose" && value == "yours") {
            agendas::set_field(&path, &place, field, value)?;
        }
    }
    // A child's, a partner's, another parent's: their person found, or made (docs/persons.md).
    let problem = crate::persons::tie_role(&place);
    if !problem.is_empty() {
        tell(qt, shared, problem);
    }
    crate::pim::nudge(shared, &asked.account);
    crate::agenda::show_agenda(qt, shared);
    crate::work::show_work(qt, shared);
    Ok(place)
}

/// An agenda renamed here, sent by the next sync; with Undo.
pub(crate) fn rename(qt: &QtThread, shared: &Arc<Shared>, id: &str, name: &str) -> String {
    let Some((collection, agenda, _)) = find(id) else { return tr().text("agenda-gone", None) };
    if !own(&agenda, &collection) {
        return tr().text("agenda-not-yours-to-change", None);
    }
    let was = collection.name.clone();
    if let Err(e) = vdir::rename_collection(&collection, name) {
        return e;
    }
    crate::pim::nudge(shared, &collection.account);
    crate::agenda::show_agenda(qt, shared);
    let id = id.to_string();
    crate::mail::offer_back(qt, shared, say("agenda-renamed", &[("name", name.trim().to_string())]), move |qt, shared| match find(&id) {
        Some((collection, _, _)) => {
            let back = vdir::rename_collection(&collection, &was).err().unwrap_or_default();
            crate::pim::nudge(shared, &collection.account);
            crate::agenda::show_agenda(qt, shared);
            back
        }
        None => tr().text("undo-too-late", None),
    });
    String::new()
}

/// An agenda's colour changed here ("#rrggbb"), sent by the next sync; with Undo.
pub(crate) fn recolour(qt: &QtThread, shared: &Arc<Shared>, id: &str, colour: &str) -> String {
    let Some((collection, agenda, _)) = find(id) else { return tr().text("agenda-gone", None) };
    if !own(&agenda, &collection) {
        return tr().text("agenda-not-yours-to-change", None);
    }
    let was = collection.color.clone().unwrap_or_default();
    if let Err(e) = vdir::recolour_collection(&collection, colour) {
        return e;
    }
    crate::pim::nudge(shared, &collection.account);
    crate::agenda::show_agenda(qt, shared);
    crate::work::show_work(qt, shared);
    let id = id.to_string();
    crate::mail::offer_back(qt, shared, tr().text("agenda-recoloured", None), move |qt, shared| match find(&id) {
        Some((collection, _, _)) => {
            let back = vdir::recolour_collection(&collection, &was).err().unwrap_or_default();
            crate::pim::nudge(shared, &collection.account);
            crate::agenda::show_agenda(qt, shared);
            back
        }
        None => tr().text("undo-too-late", None),
    });
    String::new()
}

/// What removing an agenda would do, asked before (JSON): `act` "remove"
/// (yours: "Remove “Swimming”, 42 events, from cloud.example.org?"), "leave"
/// (someone else's: "Stop showing “Swimming”? It stays its owner's."), or
/// "none"; `note`, what the server keeps of it (Nextcloud's trash bin, 30
/// days by default; Google keeps nothing).
pub(crate) fn removal(id: &str) -> String {
    let Some((collection, agenda, config)) = find(id) else { return json(&serde_json::json!({ "act": "none", "sentence": tr().text("agenda-gone", None) })) };
    let server = server_of(&config, &collection);
    if !own(&agenda, &collection) {
        if server == "local" {
            return json(&serde_json::json!({ "act": "none", "sentence": tr().text("agenda-not-yours-to-change", None) }));
        }
        if someone_elses(&collection) {
            return json(&serde_json::json!({ "act": "leave", "sentence": say("agenda-leave-question", &[("name", collection.name.clone())]), "note": tr().text("agenda-leave-note", None) }));
        }
        // Yours but read only, or whose it is unknown: hidden on this device, nothing sent.
        return json(&serde_json::json!({ "act": "hide", "sentence": say("agenda-hide-question", &[("name", collection.name.clone())]), "note": tr().text("agenda-hide-note", None) }));
    }
    if google_primary(&config, &collection) {
        return json(&serde_json::json!({ "act": "none", "sentence": tr().text("agenda-remove-google-primary", None) }));
    }
    let items = collection.items().len();
    let what = if collection.components.iter().all(|c| c.eq_ignore_ascii_case("VTODO")) { "tasks" } else { "events" };
    let mut args = sioul_core::i18n::args();
    args.set("name", collection.name.clone());
    args.set("count", items as i64);
    args.set("what", what);
    args.set("server", server_name(&config, &collection));
    let sentence = tr().text("agenda-remove-question", Some(&args));
    let known = SHARERS.lock().ok().and_then(|s| s.get(&collection.account).copied());
    let note = match (server, known) {
        ("local", _) => String::new(),
        ("google", _) => tr().text("agenda-remove-google-final", None),
        (_, Some(Sharer::Nextcloud)) => tr().text("agenda-remove-nextcloud-trash", None),
        _ => tr().text("agenda-remove-server-may-keep", None),
    };
    json(&serde_json::json!({ "act": "remove", "sentence": sentence, "note": note }))
}

/// An agenda removed (yours: a dated copy first, then deleted on its server
/// by the next sync) or left (someone else's: no longer shown, the share left
/// where the server lets you). No Undo: the copy is the way back. Returns what
/// the status line says.
pub(crate) fn remove(qt: &QtThread, shared: &Arc<Shared>, id: &str) -> String {
    let Some((collection, agenda, config)) = find(id) else { return tr().text("agenda-gone", None) };
    let said = if own(&agenda, &collection) {
        if google_primary(&config, &collection) {
            return tr().text("agenda-remove-google-primary", None);
        }
        match vdir::remove_collection(&collection) {
            Ok(copy) => say("agenda-removed", &[("name", collection.name.clone()), ("copy", copy.display().to_string())]),
            Err(e) => return e,
        }
    } else if server_of(&config, &collection) == "local" {
        return tr().text("agenda-not-yours-to-change", None);
    } else if someone_elses(&collection) {
        match vdir::stop_showing(&collection) {
            Ok(()) => say("agenda-left", &[("name", collection.name.clone())]),
            Err(e) => return e,
        }
    } else {
        // Its server does not say it is someone else's: kept hidden here, as "Keep hidden" does.
        let problem = crate::agenda::keep_hidden(&agendas::place(&collection), true);
        if !problem.is_empty() {
            return problem;
        }
        say("agenda-hidden-here", &[("name", collection.name.clone())])
    };
    crate::pim::nudge(shared, &collection.account);
    crate::agenda::show_agenda(qt, shared);
    crate::work::show_work(qt, shared);
    tell(qt, shared, said.clone());
    said
}

// ---------------------------------------------------------------- sharing

/// A change of an agenda's sharing.
pub(crate) enum Change {
    Read,
    /// With a user of the same Nextcloud (`group:<name>` a group of it), or an address (Google).
    Share { who: String, access: Access },
    /// A share taken back, by its id (`Sharee::id`).
    Unshare(String),
    /// The public link on or off.
    Link(bool),
}

/// The account and the address of an agenda's calendar, for its server.
fn reach(id: &str) -> Result<(Collection, Agenda, Account, String), String> {
    let (collection, agenda, config) = find(id).ok_or_else(|| tr().text("agenda-gone", None))?;
    let account = config.every_account().find(|a| a.id == collection.account).cloned().ok_or_else(|| tr().text("agenda-sharing-here", None))?;
    let url = State::load(&collection.state_path()).url;
    if url.is_empty() {
        return Err(tr().text("agenda-sharing-not-yet", None));
    }
    Ok((collection, agenda, account, url))
}

/// A server's refusal, said; Google's Calendar API off in the key's project
/// said with what to turn on.
fn server_said(error: &sioul_sync::SyncError, account: &str) -> String {
    if calendars::api_off(error) { tr().text("agenda-google-api-off", None) } else { error.sentence(tr(), account) }
}

/// One change of sharing asked of the server, then the sharing read again.
fn ask(id: &str, change: &Change) -> Result<(Agenda, Sharing), String> {
    if crate::backend::offline() {
        return demo_sharing(id).ok_or_else(|| tr().text("agenda-gone", None));
    }
    let (_, agenda, account, url) = reach(id)?;
    let e = |e: sioul_sync::SyncError| server_said(&e, &account.id);
    let sharing = if account.auth.as_deref() == Some("google") {
        let address = account.address.clone().unwrap_or_default();
        let api = GoogleApi::new(&address).map_err(e)?;
        let calendar = calendars::google_id(&url).ok_or_else(|| tr().text("agenda-sharing-not-yet", None))?;
        match change {
            Change::Read => {}
            Change::Share { who, access } => {
                if !who.contains('@') {
                    return Err(tr().text("agenda-share-google-address", None));
                }
                api.grant(&calendar, "user", who.trim(), calendars::google_role(*access)).map_err(e)?;
            }
            Change::Unshare(rule) => api.revoke(&calendar, rule).map_err(e)?,
            Change::Link(true) => api.grant(&calendar, "default", "", "reader").map_err(e)?,
            Change::Link(false) => api.revoke(&calendar, "default").map_err(e)?,
        }
        calendars::google_sharing(&api, &calendar, &address).map_err(e)?
    } else {
        let client = sioul_sync::dav::client_for(&account).map_err(e)?;
        match change {
            Change::Read => {}
            Change::Share { who, access } => {
                let (name, group) = who.strip_prefix("group:").map_or((who.as_str(), false), |name| (name, true));
                let Some(principal) = calendars::principal(name, group) else { return Err(tr().text("agenda-share-elsewhere", None)) };
                calendars::dav_share(&client, &url, &principal, *access == Access::Write).map_err(e)?;
            }
            Change::Unshare(who) => calendars::dav_unshare(&client, &url, who).map_err(e)?,
            Change::Link(on) => calendars::dav_publish(&client, &url, *on).map_err(e)?,
        }
        calendars::dav_sharing(&client, &url).map_err(e)?
    };
    if let Ok(mut known) = SHARERS.lock() {
        known.insert(account.id.clone(), sharing.sharer);
    }
    Ok((agenda, sharing))
}

/// Who sees an agenda and how, in one line: "Only you.", "Sam can read
/// it.", "Sam, the other parent, can change it.", "Anyone with the link can
/// read it."; "This server does not share from Sioul."
pub(crate) fn sharing_line(agenda: &Agenda, sharing: &Sharing) -> String {
    if sharing.sharer == Sharer::None {
        return tr().text("agenda-sharing-none", None);
    }
    let parent = |name: &str| matches!(agenda.whose, Whose::Child | Whose::Parent) && ((!agenda.parent.is_empty() && name.eq_ignore_ascii_case(&agenda.parent)) || (!agenda.parent_address.is_empty() && name.eq_ignore_ascii_case(&agenda.parent_address)) || (!agenda.person.is_empty() && agenda.whose == Whose::Parent && name.eq_ignore_ascii_case(&agenda.person)));
    let mut out: Vec<String> = sharing
        .people
        .iter()
        .map(|p| {
            let who = if parent(&p.name) { say("agenda-sharing-the-other-parent", &[("who", p.name.clone())]) } else { p.name.clone() };
            say(
                match p.access {
                    Access::Write => "agenda-sharing-write",
                    Access::Read => "agenda-sharing-read",
                    Access::Busy => "agenda-sharing-busy",
                },
                &[("who", who)],
            )
        })
        .collect();
    if sharing.link.is_some() {
        out.push(tr().text("agenda-sharing-link", None));
    }
    if out.is_empty() {
        return tr().text("agenda-sharing-only-you", None);
    }
    out.join(" ")
}

/// The demo profile (`SIOUL_DEMO`) asks no server: a Nextcloud's answer stood
/// in, nothing shared but a child's agenda, with its other parent, and your
/// busy times' calendar by its link when they are shared so.
fn demo_sharing(id: &str) -> Option<(Agenda, Sharing)> {
    let (collection, agenda, config) = find(id)?;
    let people = if agenda.whose == Whose::Child && !agenda.parent.is_empty() {
        vec![calendars::Sharee { id: format!("principal:principals/users/{}", agenda.parent.to_lowercase()), name: agenda.parent.clone(), access: Access::Write, group: false }]
    } else {
        Vec::new()
    };
    let busy = config.agenda.availability.as_ref().is_some_and(|a| a.link && a.account == collection.account && a.calendar == collection.id);
    let link = busy.then(|| calendars::subscription(&format!("https://cloud.example.org/remote.php/dav/public-calendars/{}", collection.id)));
    Some((agenda, Sharing { sharer: Sharer::Nextcloud, people, link, links: true }))
}

/// An agenda's sharing as JSON for its line: the sentence, the people, the
/// link, what its server lets Sioul do, and what those who read it see of
/// confidential or private events.
fn sharing_json(agenda: &Agenda, sharing: &Sharing, said: &str) -> String {
    let note = match sharing.sharer {
        Sharer::Nextcloud => tr().text("agenda-sharing-nextcloud-note", None),
        Sharer::Google => tr().text("agenda-sharing-google-note", None),
        Sharer::None => String::new(),
    };
    json(&serde_json::json!({ "line": sharing_line(agenda, sharing), "sharer": sharing.sharer, "people": sharing.people, "link": sharing.link, "links": sharing.links, "note": note, "said": said, "problem": "" }))
}

/// Reads or changes an agenda's sharing in a thread of its own; its answer
/// comes as `agenda_shared(id, JSON)`. Taking a share back is said with Undo
/// (it shares again); a public link turned on says once what anyone with it sees.
pub(crate) fn sharing(qt: &QtThread, shared: &Arc<Shared>, id: &str, change: Change) {
    let (qt, shared, id) = (qt.clone(), Arc::clone(shared), id.to_string());
    std::thread::spawn(move || {
        // Who it was shared with, and how, read first, to share it again on Undo.
        let undo = match &change {
            Change::Unshare(who) => {
                let before = ask(&id, &Change::Read).ok().and_then(|(_, s)| s.people.into_iter().find(|p| p.id == *who));
                let (access, group) = before.map_or((Access::Read, false), |p| (p.access, p.group));
                let name = who.rsplit(['/', ':']).next().unwrap_or(who).to_string();
                Some((if group { format!("group:{name}") } else { name }, access))
            }
            _ => None,
        };
        let answer = match ask(&id, &change) {
            Ok((agenda, now)) => {
                let said = match &change {
                    Change::Link(true) => tr().text(if now.sharer == Sharer::Google { "agenda-link-google-sees" } else { "agenda-link-nextcloud-sees" }, None),
                    _ => String::new(),
                };
                if let Some((who, access)) = undo {
                    let back_id = id.clone();
                    let name = who.strip_prefix("group:").unwrap_or(&who).to_string();
                    crate::mail::offer_back(&qt, &shared, say("agenda-unshared", &[("who", name)]), move |qt, shared| {
                        sharing(qt, shared, &back_id, Change::Share { who: who.clone(), access });
                        String::new()
                    });
                }
                sharing_json(&agenda, &now, &said)
            }
            Err(problem) => json(&serde_json::json!({ "problem": problem })),
        };
        let _ = qt.queue(move |mut sioul| sioul.as_mut().agenda_shared(cxx_qt_lib::QString::from(&id), cxx_qt_lib::QString::from(&answer)));
    });
}

// ---------------------------------------------------------------- your availability

#[derive(Deserialize)]
struct Asked {
    account: String,
    agendas: Vec<String>,
    #[serde(default)]
    with: String,
    #[serde(default)]
    link: bool,
    #[serde(default)]
    words: BTreeMap<String, String>,
}

/// What Agenda ⚙ shows of your availability, as JSON: what is shared and
/// with whom, the accounts it can be shared from, your agendas to pick;
/// `place`, the calendar of busy blocks Sioul keeps, whose sharing (its
/// link) the window asks of its server as for any agenda.
pub(crate) fn availability_json() -> String {
    let config = load_config();
    let current = config.agenda.availability.clone();
    let line = current.as_ref().map(|a| availability_line(a)).unwrap_or_default();
    let blocks = current.as_ref().filter(|a| !a.calendar.is_empty()).map(|a| format!("{}/{}", a.account, a.calendar)).unwrap_or_default();
    let agendas: Vec<serde_json::Value> = agendas::all(&config).iter().filter(|a| a.whose == Whose::Yours && a.events && !a.busy() && a.id != blocks).map(|a| serde_json::json!({ "id": a.id, "name": a.name, "account": a.account })).collect();
    let accounts: Vec<serde_json::Value> = config.accounts.iter().filter(|a| a.is_dav()).map(|a| serde_json::json!({ "id": a.id, "name": a.address.clone().unwrap_or_else(|| a.id.clone()), "google": a.auth.as_deref() == Some("google") })).collect();
    json(&serde_json::json!({ "current": current, "line": line, "place": blocks, "agendas": agendas, "accounts": accounts }))
}

/// "Your busy times are shared with Sam.", "…by a link.", "…will be shared with Sam once its calendar is on the server."
fn availability_line(availability: &Availability) -> String {
    let who = if availability.link && availability.with.is_empty() { tr().text("availability-by-link", None) } else { availability.with.clone() };
    say(if availability.shared { "availability-shared" } else { "availability-pending" }, &[("who", who)])
}

/// Your availability shared (`text`, JSON: account, agendas, with, link,
/// words). On Google, the agendas picked (its calendars) shared busy only
/// with the address, nothing copied; elsewhere a calendar of busy blocks made
/// in the account, filled with the plan's upkeep and shared once its server
/// holds it. Returns what went wrong, else "".
pub(crate) fn share_availability(qt: &QtThread, shared: &Arc<Shared>, text: &str) -> String {
    let result = (|| -> Result<(), String> {
        let asked: Asked = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let config = load_config();
        if config.agenda.availability.is_some() {
            return Err(tr().text("availability-already", None));
        }
        let account = config.accounts.iter().find(|a| a.id == asked.account && a.is_dav()).cloned().ok_or_else(|| tr().text("agenda-new-no-account", None))?;
        if asked.agendas.is_empty() {
            return Err(tr().text("availability-pick", None));
        }
        let google = account.auth.as_deref() == Some("google");
        let with = asked.with.trim().to_string();
        if google && !with.contains('@') {
            return Err(tr().text("agenda-share-google-address", None));
        }
        if !google && !asked.link && calendars::principal(&with, false).is_none() {
            return Err(tr().text("agenda-share-elsewhere", None));
        }
        let mut availability = Availability { account: account.id.clone(), agendas: asked.agendas.clone(), with, link: asked.link && !google, words: asked.words.into_iter().filter(|(_, w)| !w.trim().is_empty()).collect(), ..Availability::default() };
        if google {
            // Its own calendars, shared busy only: nothing copied.
            availability.agendas.retain(|place| place.starts_with(&format!("{}/", account.id)));
            if availability.agendas.is_empty() {
                return Err(tr().text("availability-google-own", None));
            }
            agendas::set_availability(&crate::backend::config_path(), Some(&availability))?;
            google_availability(qt, shared, true);
            return Ok(());
        }
        let calendar = vdir::create(Kind::Calendars, &account.id, &tr().text("availability-calendar", None), None, &["VEVENT"])?;
        availability.calendar = calendar.id.clone();
        // The demo profile asks no server: shared at once, its link stood in (`demo_sharing`).
        availability.shared = crate::backend::offline();
        agendas::set_availability(&crate::backend::config_path(), Some(&availability))?;
        // Kept to yourself in Sioul: the calendar of blocks is no agenda of yours to show.
        let _ = crate::agenda::keep_hidden(&agendas::place(&calendar), true);
        crate::pim::nudge(shared, &account.id);
        Ok(())
    })();
    match result {
        Ok(()) => {
            crate::agenda::show_agenda(qt, shared);
            String::new()
        }
        Err(e) => e,
    }
}

/// Google's busy-only shares made (`on`) or taken back, in a thread; the
/// configuration then says so, and `agenda_shared("availability", …)` answers.
fn google_availability(qt: &QtThread, shared: &Arc<Shared>, on: bool) {
    let (qt, shared) = (qt.clone(), Arc::clone(shared));
    std::thread::spawn(move || {
        let config = load_config();
        let Some(mut availability) = config.agenda.availability.clone() else { return };
        let result = (|| -> Result<(), String> {
            if crate::backend::offline() {
                // The demo profile asks no server.
                return Ok(());
            }
            let account = config.every_account().find(|a| a.id == availability.account).cloned().ok_or_else(|| tr().text("agenda-gone", None))?;
            let api = GoogleApi::new(account.address.as_deref().unwrap_or_default()).map_err(|e| server_said(&e, &account.id))?;
            for place in &availability.agendas {
                let Some(collection) = vdir::collections(Kind::Calendars).into_iter().find(|c| agendas::place(c) == *place) else { continue };
                let Some(calendar) = calendars::google_id(&State::load(&collection.state_path()).url) else { continue };
                if on {
                    api.grant(&calendar, "user", &availability.with, "freeBusyReader").map_err(|e| server_said(&e, &account.id))?;
                } else {
                    api.revoke(&calendar, &format!("user:{}", availability.with)).map_err(|e| server_said(&e, &account.id))?;
                }
            }
            Ok(())
        })();
        let path = crate::backend::config_path();
        let said = match result {
            Ok(()) if on => {
                availability.shared = true;
                let _ = agendas::set_availability(&path, Some(&availability));
                availability_line(&availability)
            }
            Ok(()) => {
                let _ = agendas::set_availability(&path, None);
                tr().text("availability-stopped", None)
            }
            Err(e) => e,
        };
        tell(&qt, &shared, said);
        let answer = availability_json();
        let _ = qt.queue(move |mut sioul| sioul.as_mut().agenda_shared(cxx_qt_lib::QString::from("availability"), cxx_qt_lib::QString::from(&answer)));
    });
}

/// Your availability no longer shared: the share taken back (Google's rules;
/// Nextcloud's share or link), and the calendar Sioul made removed. Returns what went wrong, else "".
pub(crate) fn stop_availability(qt: &QtThread, shared: &Arc<Shared>) -> String {
    let config = load_config();
    let Some(availability) = config.agenda.availability.clone() else { return String::new() };
    if config.every_account().find(|a| a.id == availability.account).is_some_and(|a| a.auth.as_deref() == Some("google")) {
        google_availability(qt, shared, false);
        return String::new();
    }
    let (qt, shared) = (qt.clone(), Arc::clone(shared));
    std::thread::spawn(move || {
        let place = format!("{}/{}", availability.account, availability.calendar);
        // The share first, where it was made; the calendar then goes, its share with it.
        if availability.shared {
            let change = if availability.with.is_empty() { Change::Link(false) } else { Change::Unshare(calendars::principal(&availability.with, false).unwrap_or_default()) };
            let _ = ask(&place, &change);
        }
        if let Some(collection) = vdir::collections(Kind::Calendars).into_iter().chain(vdir::every_collection(Kind::Calendars)).find(|c| agendas::place(c) == place) {
            let _ = vdir::remove_collection(&collection);
            crate::pim::nudge(&shared, &collection.account);
        }
        let _ = agendas::set_availability(&crate::backend::config_path(), None);
        let _ = crate::agenda::keep_hidden(&place, false);
        tell(&qt, &shared, tr().text("availability-stopped", None));
        let answer = availability_json();
        let _ = qt.queue(move |mut sioul| sioul.as_mut().agenda_shared(cxx_qt_lib::QString::from("availability"), cxx_qt_lib::QString::from(&answer)));
    });
    String::new()
}

/// The calendar of busy blocks kept up to date, from the plan's run (the
/// children's upkeep's moment): the blocks of the coming two months written,
/// those no longer wanted taken out; once its server holds it, shared once.
pub(crate) fn availability_upkeep(qt: &QtThread, shared: &Arc<Shared>) {
    let config = load_config();
    let Some(availability) = config.agenda.availability.clone().filter(|a| !a.calendar.is_empty()) else { return };
    let Some(collection) = vdir::collections(Kind::Calendars).into_iter().find(|c| c.account == availability.account && c.id == availability.calendar) else { return };
    let changed = keep_blocks(&collection, &availability);
    if changed {
        crate::pim::nudge(shared, &collection.account);
    }
    let state = State::load(&collection.state_path());
    if !availability.shared && !state.url.is_empty() && !state.pending {
        let (qt, shared, place) = (qt.clone(), Arc::clone(shared), agendas::place(&collection));
        std::thread::spawn(move || {
            let change = if availability.with.is_empty() { Change::Link(true) } else { Change::Share { who: availability.with.clone(), access: Access::Read } };
            match ask(&place, &change) {
                Ok((agenda, now)) => {
                    let done = Availability { shared: true, ..availability };
                    let _ = agendas::set_availability(&crate::backend::config_path(), Some(&done));
                    tell(&qt, &shared, availability_line(&done));
                    // Agenda ⚙ open says so at once: the line, and the link the server gave.
                    let (sharing, answer) = (sharing_json(&agenda, &now, ""), availability_json());
                    let _ = qt.queue(move |mut sioul| {
                        sioul.as_mut().agenda_shared(cxx_qt_lib::QString::from(&place), cxx_qt_lib::QString::from(&sharing));
                        sioul.as_mut().agenda_shared(cxx_qt_lib::QString::from("availability"), cxx_qt_lib::QString::from(&answer));
                    });
                }
                Err(e) => tell(&qt, &shared, e),
            }
        });
    }
}

/// The calendar's files made what the blocks say (`availability::wanted`):
/// whether anything was written.
fn keep_blocks(collection: &Collection, availability: &Availability) -> bool {
    let now = sioul_core::zone::now().timestamp().as_second();
    let events = sioul_core::agenda::occurrences(now - 86_400, now + 61 * 86_400);
    let wanted = sioul_core::availability::wanted(&events, availability, &tr().text("availability-busy", None));
    let mut changed = false;
    for (name, text) in &wanted {
        let path = collection.dir.join(name);
        if std::fs::read_to_string(&path).ok().as_deref() != Some(text.as_str()) && vdir::write_item(&path, text).is_ok() {
            changed = true;
        }
    }
    for path in collection.items() {
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        if !wanted.contains_key(&name) && vdir::remove_item(&path).is_ok() {
            changed = true;
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Whose an agenda is comes from its server, never its role: a child's
    /// agenda you own offers Remove (its copy, its question), never a leave;
    /// a calendar shared with you, whatever its role, Stop showing, which
    /// leaves it on its server; one whose owner its server does not say is
    /// hidden on this device alone.
    #[test]
    fn ownership_is_the_servers_word_never_the_role() {
        sioul_core::config::stand_ins_only();
        let config = crate::backend::config_path();
        let make = |name: &str, owned: Option<bool>| {
            let made = vdir::create(Kind::Calendars, "p5-own", name, None, &["VEVENT"]).unwrap();
            let mut state = State::load(&made.state_path());
            state.pending = false;
            state.url = format!("https://cloud.example.invalid/remote.php/dav/calendars/jane/{}/", made.id);
            state.owned = owned;
            state.save(&made.state_path()).unwrap();
            agendas::place(&made)
        };
        let tom = make("Tom, owned", Some(true));
        agendas::set_field(&config, &tom, "whose", "child").unwrap();
        let sams = make("Sam's, shared with you", Some(false));
        let unknown = make("Somewhere", None);
        let act = |id: &str| serde_json::from_str::<serde_json::Value>(&removal(id)).unwrap()["act"].as_str().unwrap_or_default().to_string();
        assert_eq!(act(&tom), "remove", "a child's agenda you own");
        assert_eq!(act(&sams), "leave", "shared with you: left on its server");
        assert_eq!(act(&unknown), "hide", "whose it is unknown: hidden here only");
        let (tom_c, tom_a, _) = find(&tom).unwrap();
        assert!(own(&tom_a, &tom_c) && vdir::stop_showing(&tom_c).is_err(), "yours: never a leave");
        let (unknown_c, unknown_a, _) = find(&unknown).unwrap();
        assert!(!own(&unknown_a, &unknown_c) && vdir::stop_showing(&unknown_c).is_err() && vdir::remove_collection(&unknown_c).is_err(), "unknown: neither left nor removed on its server");
        let (sams_c, _, _) = find(&sams).unwrap();
        vdir::stop_showing(&sams_c).unwrap();
        assert!(State::load(&sams_c.state_path()).leave, "someone else's: its leave goes to the server");
        for c in vdir::every_collection(Kind::Calendars).into_iter().filter(|c| c.account == "p5-own") {
            let _ = std::fs::remove_dir_all(&c.dir);
            let _ = std::fs::remove_file(c.state_path());
        }
    }

    /// Your availability by a link: Agenda ⚙ gets its calendar's place, whose
    /// sharing it asks as any agenda's (its link); the demo profile's stand-in
    /// gives that calendar alone a link, the feed an agenda app subscribes to.
    #[test]
    fn availability_by_link_names_its_calendar() {
        sioul_core::config::stand_ins_only();
        let dir = sioul_core::config::data_dir().join(format!("p5-busy-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let config = dir.join("config.toml");
        std::fs::write(&config, "").unwrap();
        crate::backend::TEST_CONFIG.with(|own| *own.borrow_mut() = Some(config.clone()));
        let busy = vdir::create(Kind::Calendars, "p5-busy", "Busy times", None, &["VEVENT"]).unwrap();
        let home = vdir::create(Kind::Calendars, "p5-busy", "Home", None, &["VEVENT"]).unwrap();
        let shared = Availability { account: "p5-busy".into(), agendas: vec![agendas::place(&home)], link: true, calendar: busy.id.clone(), shared: true, ..Availability::default() };
        let ((), said) = sioul_core::changes::heard(|| agendas::set_availability(&config, Some(&shared)).unwrap());
        assert_eq!(said, [sioul_core::changes::Kind::Agendas], "Agenda ⚙ hears it");
        let shown: serde_json::Value = serde_json::from_str(&availability_json()).unwrap();
        assert_eq!(shown["place"].as_str(), Some(agendas::place(&busy).as_str()));
        let link = demo_sharing(&agendas::place(&busy)).unwrap().1.link.unwrap_or_default();
        assert!(link.starts_with("https://cloud.example.org/") && link.ends_with("?export"), "{link}");
        assert_eq!(demo_sharing(&agendas::place(&home)).unwrap().1.link, None, "no other calendar's");
        crate::backend::TEST_CONFIG.with(|own| *own.borrow_mut() = None);
        for c in vdir::every_collection(Kind::Calendars).into_iter().filter(|c| c.account == "p5-busy") {
            let _ = std::fs::remove_dir_all(&c.dir);
            let _ = std::fs::remove_file(c.state_path());
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The line of an agenda's sharing: only you; each person by what they
    /// may do, the other parent named so on a child's agenda; the link; a
    /// server that does not share says so.
    #[test]
    fn the_sharing_line_says_who_sees_it() {
        let collection = Collection { kind: Kind::Calendars, account: "cloud".into(), id: "tom".into(), dir: std::path::PathBuf::from("/nowhere"), name: "Tom".into(), color: None, read_only: false, components: vec!["VEVENT".into()] };
        let said = agendas::Said { whose: Some("child".into()), name: Some("Tom".into()), parent: Some("Sam".into()), parent_address: Some("sam@example.org".into()), ..Default::default() };
        let tom = agendas::Agenda::of(&collection, &said, &Config::default(), false, &|| true);
        let person = |name: &str, access| calendars::Sharee { id: format!("principal:principals/users/{name}"), name: name.into(), access, group: false };
        let none = Sharing { sharer: Sharer::Nextcloud, people: Vec::new(), link: None, links: true };
        assert_eq!(sharing_line(&tom, &none), tr().text("agenda-sharing-only-you", None));
        let some = Sharing { people: vec![person("Sam", Access::Write), person("lea", Access::Read)], link: Some("https://cloud.example.org/p/x".into()), ..none.clone() };
        let line = sharing_line(&tom, &some);
        let parent = say("agenda-sharing-the-other-parent", &[("who", "Sam".into())]);
        for part in [say("agenda-sharing-write", &[("who", parent)]), say("agenda-sharing-read", &[("who", "lea".into())]), tr().text("agenda-sharing-link", None)] {
            assert!(line.contains(&part), "{part}: {line}");
        }
        assert_eq!(sharing_line(&tom, &Sharing { sharer: Sharer::None, ..none }), tr().text("agenda-sharing-none", None));
    }
}
