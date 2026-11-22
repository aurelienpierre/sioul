// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Extensions in the window (docs/extensions.md): Settings ▸ Extensions,
//! each extension's runs, and the sign-in page it asks for.
//!
//! - **The page** (`page`): what the signed manifest offers, each with what
//!   it brings, the sites it talks to and whether it keeps a sign-in, before
//!   anything is enabled; then Enable, Sign in…, its state and last problem
//!   in one sentence, Disable, Remove (keeping its agendas or not). Opening
//!   it reads the manifest; so does a day passed while one is enabled
//!   (`tick`); nothing else does.
//! - **An update** that asks for no more than the version in use (no other
//!   host, no new sign-in) is taken when the manifest is read; one that asks
//!   for more waits for the person's yes (`accept`).
//! - **The runs**: an extension enabled here and signed in refreshes when
//!   Sioul starts, then every three hours while the window runs (`tick`),
//!   and in the phone's background step (`background_step`); never two of
//!   the same at once, in one process (`RUNNING`) or across a phone's two
//!   (the window's and the background service's: `run_lock`); never on the
//!   window's thread.
//! - **The sign-in page** (`show_page`): on a computer the window's
//!   SignInWindow.qml, on Android SignIn.java; the address returned comes
//!   back through `returned`.
//! - **The agendas** an extension brings (a pupil's timetable each) go into
//!   calendars of the person's own, one each, chosen on this page
//!   (`choose_calendar`, kept in the shared settings: `[extensions.calendars]`),
//!   through the agendas' door (`sioul_core::extensions::door`: the agenda's
//!   own door for events, its events marked as the extension's); the
//!   calendar's server carries them to every device and to whom it is shared
//!   with. A device whose fetch is older than another's writes nothing
//!   (`availability::may_write`, `devices::mirror_writers`).

use crate::backend::{QtThread, load_config, say, tr};
use cxx_qt_lib::QString;
use serde_json::{Value, json};
use sioul_core::extensions::manifest::{CredField, Manifest, Offer};
use sioul_core::extensions::door;
use sioul_core::extensions::reply::{self, AgendaReply, ProblemKind, RefreshReply, SignInReply};
use sioul_core::extensions::state::{self, Brought, Installed, State};
use sioul_sync::extensions::{self as store, Call, Real, SignInPage, engine::Outcome};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;
use std::time::SystemTime;

/// Every three hours.
const EVERY: i64 = 3 * 3600;
/// The manifest read once a day while an extension is enabled.
const DAILY: i64 = 24 * 3600;

/// The manifest last read here, and where from; or why it was not.
struct Listed {
    address: String,
    manifest: Option<Manifest>,
    problem: Option<String>,
}

static LISTED: Mutex<Option<Listed>> = Mutex::new(None);
/// The extensions running now, by id: never two calls of one at once.
static RUNNING: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());
/// The extensions refreshed since this process started ("when Sioul starts").
static STARTED: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());
/// When each extension last ran in this process, whether or not its record
/// could be written: a record that cannot be written (a full disk) never
/// makes it run again before three hours.
static ATTEMPTED: Mutex<BTreeMap<String, i64>> = Mutex::new(BTreeMap::new());
/// The manifest being read.
static READING: Mutex<bool> = Mutex::new(false);
/// When the window's minute last tried to read the manifest.
static TRIED: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);

fn now() -> i64 {
    jiff::Timestamp::now().as_second()
}

fn french() -> bool {
    tr().language().starts_with("fr")
}

// ---------------------------------------------------------------- the agendas' door

/// The agendas' door an extension's lessons go through: into the calendar
/// the person chose for each agenda, through the agenda's own door for
/// events (`door::take`), and out of every calendar on Remove (`door::take_out`).
/// A trait so that the runs' rules are tested without calendars.
pub(crate) trait AgendaDoor {
    /// An agenda's lessons written into the calendar at `place` ("account/id").
    fn take(&self, extension: &str, agenda: &AgendaReply, place: &str) -> Result<door::Done, door::Problem>;
    /// Every lesson of `extension` taken out of the person's calendars: how many, and how many refused.
    fn take_out(&self, extension: &str) -> (usize, usize);
}

/// The agendas' door itself.
pub(crate) struct Door;

impl AgendaDoor for Door {
    fn take(&self, extension: &str, agenda: &AgendaReply, place: &str) -> Result<door::Done, door::Problem> {
        let Some(calendar) = door::calendar_at(place) else { return Err(door::Problem::CalendarGone) };
        let cancelled = |title: &str| say("extension-event-cancelled", &[("title", title.to_string())]);
        let words = door::Words { cancelled: &cancelled, from: say("extension-event-from-line", &[("name", sioul_core::extensions::category(extension))]) };
        // The person's per-agenda choice, or the extension's suggestion when they made none.
        let busy = crate::backend::config_read().map_or(agenda.busy, |c| c.extensions.busy_for(extension, &agenda.id, agenda.busy));
        door::take(extension, agenda, &calendar, &words, busy)
    }

    fn take_out(&self, extension: &str) -> (usize, usize) {
        door::take_out(extension)
    }
}

fn door() -> Door {
    Door
}

/// What the freshest-view rule knows an extension's agendas by, in each device's sealed entry.
fn writer_key(id: &str) -> String {
    format!("extension:{id}")
}

/// Whether this device may write the agendas of `id` from a fetch made at
/// `fetched`: when no other device last wrote them from a later one.
fn may_write(id: &str, fetched: i64) -> bool {
    let others = crate::devices::entries().map(|(own, all)| sioul_sync::devices::mirror_writers(&all, &own, &writer_key(id), now())).unwrap_or_default();
    sioul_core::availability::may_write(Some((fetched, fetched)), others)
}

/// What this device wrote the agendas of `id` from, in its sealed entry.
fn note_written(id: &str, fetched: i64) {
    let Some((_, vault)) = crate::share::vault() else { return };
    let path = sioul_sync::devices::own_path(&sioul_core::config::state_dir());
    let _ = sioul_sync::devices::change(&path, vault.as_ref().map(|(folder, key)| (folder.as_path(), key)), |entry| {
        entry.mirrors.insert(writer_key(id), fetched);
    });
}

/// A refresh's agendas taken in through `door` when `write` (this device's
/// fetch is the freshest), each into the calendar `chosen` names for it,
/// none written while no calendar is chosen. What each agenda is now (its
/// lessons, its calendar, when read: only when written here, R3; its
/// problem: "choose", or the door's word), and the log's lines, fixed words
/// and ids alone.
pub(crate) fn take_in(door: &dyn AgendaDoor, extension: &str, reply: &RefreshReply, write: bool, chosen: &dyn Fn(&str) -> Option<String>, fetched: i64) -> (Vec<Brought>, Vec<String>) {
    let (mut brought, mut log) = (Vec::new(), Vec::new());
    for agenda in &reply.agendas {
        let mut now = Brought { id: agenda.id.clone(), name: agenda.name.clone(), events: agenda.events, busy: agenda.busy, ..Brought::default() };
        match chosen(&agenda.id) {
            None => {
                now.problem = Some("choose".into());
                log.push(format!("agenda {}: {} lessons, waiting for a calendar to be chosen", agenda.id, agenda.events));
            }
            Some(place) if !write => now.calendar = place,
            Some(place) => {
                match door.take(extension, agenda, &place) {
                    Ok(done) => {
                        now.events = done.lessons;
                        now.read = fetched;
                        if done.refused > 0 || done.clash > 0 {
                            now.problem = Some("refused".into());
                        }
                        log.push(format!("agenda {}: {} lessons; {} written, {} taken out, {} unchanged, {} refused, {} names taken", agenda.id, done.lessons, done.written, done.removed, done.unchanged, done.refused, done.clash));
                    }
                    Err(problem) => {
                        now.problem = Some(problem.word().into());
                        log.push(format!("agenda {}: nothing written: {}", agenda.id, problem.word()));
                    }
                }
                now.calendar = place;
            }
        }
        brought.push(now);
    }
    (brought, log)
}

/// The record's agendas once `now` is taken in: each agenda named now
/// updated (when it was read and where it went kept from before when not
/// written now), those named before and not now kept as they were (a pupil
/// gone stays listed), `state::MOST_KEPT` at most.
fn merged(before: &[Brought], now: Vec<Brought>) -> Vec<Brought> {
    let mut out: Vec<Brought> = now
        .into_iter()
        .map(|mut agenda| {
            if let Some(old) = before.iter().find(|b| b.id == agenda.id) {
                if agenda.read == 0 {
                    agenda.read = old.read;
                }
                if agenda.calendar.is_empty() && agenda.problem.as_deref() != Some("choose") {
                    agenda.calendar = old.calendar.clone();
                }
            }
            agenda
        })
        .collect();
    for old in before {
        if !out.iter().any(|b| b.id == old.id) {
            out.push(old.clone());
        }
    }
    out.truncate(state::MOST_KEPT);
    out
}

/// The calendar each agenda of `extension` goes into, as the shared settings
/// say (`[extensions.calendars]`); none for any when the settings do not
/// read (absence proves nothing: nothing is written then).
fn chosen_calendars(extension: &str) -> Option<BTreeMap<String, String>> {
    let config = crate::backend::config_read()?;
    let prefix = format!("{extension}/");
    Some(config.extensions.calendars.iter().filter_map(|(key, place)| Some((key.strip_prefix(&prefix)?.to_string(), place.clone()))).collect())
}

// ---------------------------------------------------------------- the page

/// The words of a problem kept (`Installed::problem`), in one sentence.
fn problem_line(word: &str, name: &str) -> String {
    let id = match word {
        "sign-in" => "extension-problem-sign-in",
        "unreachable" => "extension-problem-unreachable",
        "refused" => "extension-problem-refused",
        "changed" => "extension-problem-changed",
        "stopped-time" => "extension-stopped-time",
        "stopped-memory" => "extension-stopped-memory",
        "stopped-error" => "extension-stopped-error",
        "stopped-nonsense" => "extension-stopped-nonsense",
        "stopped-asks-more" => "extension-stopped-asks-more",
        "stopped-interface" => "extension-stopped-interface",
        "stopped-unread" => "extension-stopped-unread",
        "stopped-removed" => "extension-stopped-removed",
        "settings" => "extension-problem-settings",
        "keyring" => "extension-problem-keyring",
        "record" => "extension-problem-record",
        "withdrawn" => "extension-problem-withdrawn",
        _ => "extension-problem-other",
    };
    say(id, &[("name", name.to_string())])
}

fn when(at: i64) -> String {
    jiff::Timestamp::from_second(at).map(|t| tr().date(&t.to_zoned(sioul_core::zone::current()), true)).unwrap_or_default()
}

/// An offer, and this device's record of it, for ExtensionsTab.qml.
fn offer_json(offer: &Offer, installed: Option<&Installed>, running: bool, calendars: &Calendars) -> Value {
    let fr = french();
    let name = offer.name.said(fr).to_string();
    let hosts: Vec<&str> = offer.hosts.iter().map(String::as_str).collect();
    let mut line = String::new();
    let mut agendas = Vec::new();
    if let Some(installed) = installed {
        line = if installed.last_run == 0 {
            say("extension-never-ran", &[])
        } else if installed.agendas.is_empty() {
            say("extension-ran", &[("when", when(installed.last_run))])
        } else {
            let events: usize = installed.agendas.iter().map(|a| a.events).sum();
            say("extension-brought", &[("when", when(installed.last_brought.max(installed.last_run))), ("agendas", installed.agendas.len().to_string()), ("events", events.to_string())])
        };
        agendas = installed.agendas.iter().map(|a| agenda_json(&offer.id, a, calendars)).collect();
    }
    let waiting = installed.and_then(|i| i.waiting.as_ref()).map(|w| {
        let added: Vec<&str> = w.hosts.iter().filter(|h| !installed.is_some_and(|i| i.offer.hosts.contains(h))).map(String::as_str).collect();
        // A redirect sign-in newly asked for, or a credential field not accepted before.
        let had = installed.map(|i| &i.offer);
        let new_sign_in = w.sign_in.is_some() && had.is_some_and(|o| o.sign_in != w.sign_in);
        let new_credentials = !w.credentials.iter().all(|c| had.is_some_and(|o| o.credentials.contains(c)));
        let sign_in = new_sign_in || new_credentials;
        say("extension-update-asks", &[("version", w.version.clone()), ("sites", if added.is_empty() { "none".to_string() } else { added.join(", ") }), ("signin", if sign_in { "yes" } else { "no" }.to_string())])
    });
    let needs_sign_in = offer.sign_in.is_some() || !offer.credentials.is_empty();
    json!({
        "id": offer.id,
        "name": name,
        "about": offer.about.said(fr),
        "version": installed.map_or(offer.version.clone(), |i| i.offer.version.clone()),
        "brings": say("extension-brings-agenda", &[]),
        "sites": say("extension-sites", &[("count", hosts.len().to_string()), ("sites", hosts.join(", "))]),
        "signIn": if !offer.credentials.is_empty() { say("extension-keeps-credentials", &[]) } else if offer.sign_in.is_some() { say("extension-keeps-sign-in", &[]) } else { say("extension-keeps-no-sign-in", &[]) },
        "needsSignIn": needs_sign_in,
        "usable": offer.usable(),
        "unusable": if offer.usable() { String::new() } else { say("extension-needs-newer-sioul", &[]) },
        "installed": installed.is_some(),
        "enabled": installed.is_some_and(|i| i.enabled),
        "signedIn": installed.is_some_and(|i| i.signed_in),
        "running": running,
        "line": line,
        "problem": installed.and_then(|i| i.problem.as_deref()).map(|p| problem_line(p, &name)).unwrap_or_default(),
        "waiting": waiting.unwrap_or_default(),
        "agendas": agendas,
    })
}

/// The sentence of a list that did not read, in Sioul's language alone (P3):
/// its signature refused, an older list than the one accepted here, or a
/// list that could not be fetched or read. The detail, in English, goes to the
/// terminal only (`read_list`).
fn list_problem(detail: &str) -> &'static str {
    if detail.starts_with("the manifest's signature") {
        "extensions-list-signature"
    } else if detail.starts_with("an older manifest") || detail.starts_with("another manifest under serial") {
        "extensions-list-older"
    } else {
        "extensions-list-unread"
    }
}

/// The page's view: the offers of the manifest last read, then those this
/// device has that it no longer lists; where the list is read and how it went.
fn page_json() -> String {
    let state = state::load(&state::path());
    let calendars = Calendars::now();
    let running = RUNNING.lock().map(|r| r.clone()).unwrap_or_default();
    let listed = LISTED.lock().ok();
    let listed = listed.as_ref().and_then(|l| l.as_ref());
    let mut offers: Vec<Value> = Vec::new();
    let mut seen = BTreeSet::new();
    if let Some(manifest) = listed.and_then(|l| l.manifest.as_ref()) {
        for offer in &manifest.offers {
            seen.insert(offer.id.clone());
            offers.push(offer_json(offer, state.get(&offer.id).filter(|i| !i.leaving), running.contains(&offer.id), &calendars));
        }
    }
    // Those removed here are gone from the page, even while their sign-in is still to forget.
    for installed in state.extensions.iter().filter(|i| !seen.contains(&i.offer.id) && !i.leaving) {
        offers.push(offer_json(&installed.offer, Some(installed), running.contains(&installed.offer.id), &calendars));
    }
    let address = listed.map(|l| l.address.clone()).unwrap_or_else(|| load_config().extensions.manifest_address().to_string());
    json!({
        "reading": READING.lock().map(|r| *r).unwrap_or(false),
        "where": say("extensions-where", &[("address", address)]),
        "problem": listed.and_then(|l| l.problem.as_deref()).map(|p| say(list_problem(p), &[])).unwrap_or_default(),
        "offers": offers,
        "calendars": calendars.choices,
    })
    .to_string()
}

/// The calendars an agenda's lessons may go into, as the page offers them:
/// a new one on each account that keeps calendars (named after the pupil),
/// a new one on this device, or one of the person's own that they may write
/// (`door::writable`); and what the shared settings chose for each agenda.
struct Calendars {
    /// {choice, label}: `new:<account>` or `place:<account/id>`.
    choices: Vec<Value>,
    /// The person's calendars that may receive lessons, by place: their label.
    labels: BTreeMap<String, String>,
    /// `[extensions.calendars]`, as the settings read now (none when they do not).
    chosen: Option<BTreeMap<String, String>>,
    /// `[extensions.busy]`, the per-agenda busy choices (none when the settings do not read).
    busy: Option<BTreeMap<String, bool>>,
}

impl Calendars {
    fn now() -> Self {
        let config = load_config();
        // First a new calendar on each account that keeps calendars (to share from its server), then
        // the person's own, then a new one on this device alone (for someone with no server).
        let accounts = crate::servers::accounts(&config);
        let id_of = |account: &Value| account["id"].as_str().unwrap_or_default().to_string();
        let mut choices: Vec<Value> = accounts
            .iter()
            .filter(|account| id_of(account) != sioul_core::vdir::LOCAL)
            .map(|account| json!({ "choice": format!("new:{}", id_of(account)), "label": say("extension-calendar-new", &[("account", account["name"].as_str().unwrap_or_default().to_string())]) }))
            .collect();
        let mut labels = BTreeMap::new();
        for calendar in sioul_core::vdir::collections(sioul_core::vdir::Kind::Calendars).into_iter().filter(door::writable) {
            let place = sioul_core::agendas::place(&calendar);
            let label = calendar.label(&config, tr());
            choices.push(json!({ "choice": format!("place:{place}"), "label": label }));
            labels.insert(place, label);
        }
        choices.push(json!({ "choice": format!("new:{}", sioul_core::vdir::LOCAL), "label": say("extension-calendar-new-here", &[]) }));
        let settings = crate::backend::config_read().map(|c| c.extensions);
        Self { choices, labels, chosen: settings.as_ref().map(|e| e.calendars.clone()), busy: settings.map(|e| e.busy) }
    }
}

/// An agenda an extension brought, for the page: its name, where its lessons
/// go and how it went, in one line; the calendar chosen ("" none).
fn agenda_json(extension: &str, agenda: &Brought, calendars: &Calendars) -> Value {
    let place = calendars.chosen.as_ref().and_then(|c| c.get(&sioul_core::config::extension_calendar_key(extension, &agenda.id))).cloned().unwrap_or_default();
    let label = calendars.labels.get(&place).cloned();
    let (line, warn) = match (place.is_empty(), &label, agenda.problem.as_deref()) {
        (true, ..) => (say("extension-agenda-choose", &[]), true),
        (false, None, _) | (false, _, Some("calendar-gone")) => (say("extension-agenda-gone", &[]), true),
        (false, _, Some("too-large")) => (say("extension-agenda-too-large", &[]), true),
        (false, _, Some("unreadable")) => (say("extension-agenda-unreadable", &[]), true),
        (false, Some(label), Some("refused")) => (say("extension-agenda-refused", &[("calendar", label.clone())]), true),
        (false, Some(label), _) if agenda.read > 0 && agenda.calendar == place => {
            (say("extension-agenda-read", &[("calendar", label.clone()), ("lessons", agenda.events.to_string()), ("when", when(agenda.read))]), false)
        }
        (false, Some(label), _) => (say("extension-agenda-next", &[("calendar", label.clone())]), false),
    };
    // Whether its lessons count as the person's own busy time: their choice, or the extension's suggestion.
    let busy = calendars.busy.as_ref().and_then(|b| b.get(&sioul_core::config::extension_calendar_key(extension, &agenda.id)).copied()).unwrap_or(agenda.busy);
    json!({ "id": agenda.id, "name": agenda.name, "line": line, "warn": warn, "place": place, "busy": busy })
}

/// The page's view set on the window's thread.
fn show(qt: &QtThread) {
    let view = page_json();
    let _ = qt.queue(move |mut sioul| sioul.as_mut().set_extensions(QString::from(&view)));
}

/// Settings ▸ Extensions shown: its view at once, then the manifest read.
pub(crate) fn page(qt: &QtThread) {
    show(qt);
    read_list(Some(qt.clone()));
}

// ---------------------------------------------------------------- the manifest

/// The manifest read, off the window's thread; an update taken or set
/// waiting; the page told when there is one.
fn read_list(qt: Option<QtThread>) {
    {
        let Ok(mut reading) = READING.lock() else { return };
        if *reading {
            return;
        }
        *reading = true;
    }
    if let Some(qt) = &qt {
        show(qt);
    }
    std::thread::spawn(move || {
        let address = load_config().extensions.manifest_address().to_string();
        let read = store::read_manifest(&address, SystemTime::now()).and_then(|manifest| apply(&manifest, &address).map(|()| manifest));
        let (manifest, problem) = match read {
            Ok(manifest) => (Some(manifest), None),
            Err(problem) => {
                eprintln!("sioul: extensions: the list: {problem}");
                (None, Some(problem))
            }
        };
        if let Ok(mut listed) = LISTED.lock() {
            let kept = listed.take().and_then(|l| l.manifest);
            // A list that did not read leaves the last one shown, with why.
            *listed = Some(Listed { address, manifest: manifest.or(kept), problem });
        }
        if let Ok(mut reading) = READING.lock() {
            *reading = false;
        }
        if let Some(qt) = &qt {
            show(qt);
        }
    });
}

/// What a manifest whose signature held changes in this device's record.
#[derive(Debug, Default, PartialEq, Eq)]
struct Plan {
    /// Installed here and no longer listed: they stop, and say so.
    withdrawn: Vec<String>,
    /// Listed again after they were withdrawn: they may be enabled again.
    listed_again: Vec<String>,
    /// What an extension is given narrowed at once to what the manifest lists
    /// now (its hosts, its sign-in), its file unchanged: no question asked.
    narrowed: Vec<Offer>,
    /// A newer offer that asks for no more: taken now (its file downloaded when it changed).
    taken: Vec<Offer>,
    /// A newer offer that asks for more: waiting for the person's yes.
    waiting: Vec<Offer>,
    /// An update waiting that the manifest no longer offers: dropped.
    unwaited: Vec<String>,
}

/// Versions compared part by part (`0.10.0` after `0.9.1`), numbers as
/// numbers, any other part as text.
fn version_order(a: &str, b: &str) -> std::cmp::Ordering {
    let parts = |v: &str| v.split(['.', '-', '+']).map(|p| p.parse::<u64>().map_or_else(|_| (1, 0, p.to_string()), |n| (0, n, String::new()))).collect::<Vec<_>>();
    parts(a).cmp(&parts(b))
}

/// What the offer accepted here may still be given under `listed`: its hosts
/// that `listed` names too, its sign-in only while `listed` has one.
fn narrowed(accepted: &Offer, listed: &Offer) -> Offer {
    let hosts: Vec<String> = accepted.hosts.iter().filter(|h| listed.hosts.iter().any(|l| l.eq_ignore_ascii_case(h))).cloned().collect();
    let sign_in = if listed.sign_in.is_some() { accepted.sign_in.clone() } else { None };
    // Only the credential fields the list still names: no field is asked for that the list dropped.
    let credentials: Vec<CredField> = accepted.credentials.iter().filter(|c| listed.credentials.contains(c)).copied().collect();
    Offer { hosts, sign_in, credentials, ..accepted.clone() }
}

/// The changes `manifest` brings to `state` (M1): an installed extension it
/// no longer lists stops; one it lists is given at once no more than the
/// manifest says; a newer offer is taken when it asks for no more, else waits.
/// Never an older version than the one in use (M3); an update waiting that
/// the manifest no longer offers is dropped; and nothing is fetched for an
/// extension disabled here (M4): it takes its update when enabled again.
fn plan(state: &State, manifest: &Manifest) -> Plan {
    let mut plan = Plan::default();
    for installed in state.extensions.iter().filter(|i| !i.leaving) {
        let id = &installed.offer.id;
        let Some(offer) = manifest.offers.iter().find(|o| o.id == *id) else {
            if installed.enabled || installed.problem.as_deref() != Some("withdrawn") || installed.waiting.is_some() {
                plan.withdrawn.push(id.clone());
            }
            continue;
        };
        if installed.problem.as_deref() == Some("withdrawn") {
            plan.listed_again.push(id.clone());
        }
        if installed.waiting.as_ref().is_some_and(|w| w.sha256 != offer.sha256 || !offer.grants().asks_more_than(&installed.offer.grants())) {
            plan.unwaited.push(id.clone());
        }
        if offer.sha256 == installed.offer.sha256 && !offer.grants().asks_more_than(&installed.offer.grants()) {
            // The same file, asking no more: the manifest's offer as it is now.
            if *offer != installed.offer {
                plan.narrowed.push(offer.clone());
            }
            continue;
        }
        let less = narrowed(&installed.offer, offer);
        if less != installed.offer {
            plan.narrowed.push(less);
        }
        if !offer.usable() || !installed.enabled || version_order(&offer.version, &installed.offer.version) == std::cmp::Ordering::Less {
            continue;
        }
        if offer.grants().asks_more_than(&installed.offer.grants()) {
            plan.waiting.push(offer.clone());
        } else {
            plan.taken.push(offer.clone());
        }
    }
    plan
}

/// A manifest whose signature held taken in this device's state: its serial
/// first (never lower than the last accepted here, or the whole manifest is
/// refused), then `plan`'s changes: withdrawn extensions stopped, what each
/// is given narrowed at once, a newer version taken when it asks for no
/// more, else set waiting for the person's yes. Files are downloaded outside
/// the state's lock.
fn apply(manifest: &Manifest, address: &str) -> Result<(), String> {
    let path = state::path();
    state::change(&path, |state| {
        state.accept(manifest.serial, &manifest.sha256)?;
        state.checked = now();
        Ok::<(), String>(())
    })??;
    let plan = plan(&state::load(&path), manifest);
    let mut taken = Vec::new();
    for offer in &plan.taken {
        let installed = state::load(&path).get(&offer.id).map(|i| i.offer.clone());
        let Some(old) = installed else { continue };
        if old.sha256 == offer.sha256 {
            taken.push((old, offer.clone()));
            continue;
        }
        match store::install(address, offer) {
            Ok(_) => taken.push((old, offer.clone())),
            Err(problem) => eprintln!("sioul: extensions: {}: an update: {problem}", offer.id),
        }
    }
    for id in &plan.withdrawn {
        let _ = state::append_log(&state::log_path(id), &[format!("{} sioul: no longer in the extensions' list: it no longer runs here", jiff::Timestamp::now())]);
    }
    state::change(&path, |state| {
        for id in &plan.withdrawn {
            if let Some(installed) = state.get_mut(id) {
                installed.enabled = false;
                if let Some(waiting) = installed.waiting.take() {
                    store::discard_file(&waiting);
                }
                installed.problem = Some("withdrawn".into());
            }
        }
        for id in &plan.listed_again {
            if let Some(installed) = state.get_mut(id)
                && installed.problem.as_deref() == Some("withdrawn")
            {
                installed.problem = None;
            }
        }
        for id in &plan.unwaited {
            if let Some(installed) = state.get_mut(id) {
                installed.waiting = None;
            }
        }
        for offer in &plan.narrowed {
            if let Some(installed) = state.get_mut(&offer.id)
                && installed.offer.sha256 == offer.sha256
            {
                installed.offer = offer.clone();
            }
        }
        for (old, offer) in &taken {
            if let Some(installed) = state.get_mut(&offer.id) {
                if old.sha256 != offer.sha256 {
                    store::discard_file(old);
                }
                installed.offer = offer.clone();
                installed.waiting = None;
            }
        }
        for offer in &plan.waiting {
            if let Some(installed) = state.get_mut(&offer.id) {
                installed.waiting = Some(offer.clone());
            }
        }
    })
}

/// The offer of `id` in the manifest last read.
fn offered(id: &str) -> Option<(Offer, String)> {
    let listed = LISTED.lock().ok()?;
    let listed = listed.as_ref()?;
    Some((listed.manifest.as_ref()?.offers.iter().find(|o| o.id == id)?.clone(), listed.address.clone()))
}

// ---------------------------------------------------------------- the person's acts

/// Enable: the file downloaded, checked against the signed manifest and
/// kept; then a sign-in asked for, or a first refresh.
pub(crate) fn enable(qt: &QtThread, id: &str) {
    let (qt, id) = (qt.clone(), id.to_string());
    std::thread::spawn(move || {
        let installed = state::load(&state::path()).get(&id).cloned();
        let withdrawn = installed.as_ref().is_some_and(|i| i.problem.as_deref() == Some("withdrawn"));
        let enabled = installed.map(|i| i.offer).filter(|_| !withdrawn);
        let outcome = match enabled {
            // Installed before, disabled since: enabled again as it was accepted.
            Some(offer) if store::checked_file(&offer).is_ok() => state::change(&state::path(), |s| {
                if let Some(i) = s.get_mut(&id) {
                    i.enabled = true;
                }
            }),
            _ if withdrawn => Err(say("extension-not-listed", &[])),
            _ => match offered(&id) {
                None => Err(say("extension-not-listed", &[])),
                Some((offer, _)) if !offer.usable() => Err(say("extension-needs-newer-sioul", &[])),
                Some((offer, address)) => store::install(&address, &offer).and_then(|_| {
                    state::change(&state::path(), |s| {
                        s.extensions.retain(|i| i.offer.id != offer.id);
                        s.extensions.push(Installed::new(offer.clone()));
                    })
                }),
            },
        };
        let name = name_of(&id);
        match outcome {
            Ok(()) => {
                crate::backend::set_status(&qt, say("extension-enabled", &[("name", name)]));
                if state::load(&state::path()).get(&id).is_some_and(|i| i.offer.sign_in.is_none()) {
                    refresh(Some(&qt), &id);
                }
            }
            Err(problem) => {
                eprintln!("sioul: extensions: {id}: enabling: {problem}");
                crate::backend::set_status(&qt, say("extension-not-enabled", &[("name", name)]));
            }
        }
        show(&qt);
    });
}

/// Disable: it no longer runs here; its sign-in and its lessons stay.
pub(crate) fn disable(qt: &QtThread, id: &str) {
    let _ = state::change(&state::path(), |s| {
        if let Some(i) = s.get_mut(id) {
            i.enabled = false;
        }
    });
    crate::backend::set_status(qt, say("extension-disabled", &[("name", name_of(id))]));
    show(qt);
}

/// How long Remove waits for a call under way to end: it ends at its next
/// ask or slice once the extension is marked as leaving; a web request may
/// still take its 30 seconds.
const ENDING: std::time::Duration = std::time::Duration::from_secs(45);
/// A sign-in the keyring refused to forget is tried again this often.
const FORGET_AGAIN: i64 = 6 * 3600;
/// When this process last tried to forget the sign-ins of extensions removed.
static FORGOT: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);

/// Remove (K1): the extension marked as leaving (nothing schedules it, a call
/// under way ends at its next ask, never writing its secret back), its
/// sign-in page closed, the call waited for; then its sign-in forgotten, its
/// files and log taken out, its lessons taken out of the person's calendars
/// when `keep` is false (the calendars stay), and its record gone. While the keyring refuses, the record stays, marked, and the
/// sign-in is tried again later (`forget_leaving`).
pub(crate) fn remove(qt: &QtThread, id: &str, keep: bool) {
    let (qt, id) = (qt.clone(), id.to_string());
    std::thread::spawn(move || {
        let name = name_of(&id);
        let _ = state::change(&state::path(), |s| {
            if let Some(i) = s.get_mut(&id) {
                i.leaving = true;
                i.enabled = false;
            }
        });
        cancel_page(&id);
        cancel_form(&id);
        // The call under way, here or in another process, ends: its lock is ours once it has.
        let started = std::time::Instant::now();
        let held = loop {
            if let Some(held) = run_lock(&id) {
                break held;
            }
            if started.elapsed() > ENDING {
                break None;
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        };
        let installed = state::load(&state::path()).get(&id).cloned();
        let mut taken = true;
        if let Some(installed) = &installed {
            store::discard_file(&installed.offer);
            if let Some(waiting) = &installed.waiting {
                store::discard_file(waiting);
            }
        }
        if !keep {
            // Every lesson of it, whatever calendar and agenda, by the form of its UID (never
            // the last reply's list, R2 d); the calendars themselves stay.
            let (_, refused) = door().take_out(&id);
            taken = refused == 0;
            // The calendars chosen for it leave the shared settings: another device that
            // still runs it writes nothing until one is chosen again there (R2 c);
            // its busy choices leave with them.
            if let Err(why) = sioul_core::config::forget_extension(&crate::backend::config_path(), &id) {
                eprintln!("sioul: extensions: {id}: its calendars in the settings: {why}");
            }
        }
        let forgotten = forget(&id);
        drop(held);
        let line = if !forgotten {
            say("extension-removed-keyring", &[("name", name)])
        } else if !taken {
            say("extension-removed-agendas-stay", &[("name", name)])
        } else if keep {
            say("extension-removed-kept", &[("name", name)])
        } else {
            say("extension-removed", &[("name", name)])
        };
        crate::backend::set_status(&qt, line);
        show(&qt);
    });
}

/// The sign-in of `id` forgotten: then its record, its log and its lock go;
/// else its record stays, marked as leaving. Whether it was.
fn forget(id: &str) -> bool {
    if store::forget_secret(id).is_err() {
        return false;
    }
    let gone = state::change(&state::path(), |s| s.extensions.retain(|i| !(i.offer.id == id && i.leaving)));
    let _ = std::fs::remove_file(state::log_path(id));
    let _ = std::fs::remove_file(run_lock_path(id));
    gone.is_ok()
}

/// The sign-ins of extensions removed while the keyring refused, tried again
/// at the first minute of the window and every six hours (never each minute:
/// a locked keyring may ask for its password).
fn forget_leaving(now: i64) {
    if now - FORGOT.load(std::sync::atomic::Ordering::Relaxed) < FORGET_AGAIN {
        return;
    }
    FORGOT.store(now, std::sync::atomic::Ordering::Relaxed);
    let leaving: Vec<String> = state::load(&state::path()).extensions.iter().filter(|i| i.leaving).map(|i| i.offer.id.clone()).collect();
    for id in leaving {
        if run_lock(&id).is_some() && forget(&id) {
            eprintln!("sioul: extensions: {id}: its sign-in forgotten, at last");
        }
    }
}

/// The calendar an agenda's lessons go into, chosen on the page. `choice`
/// is `new:<account>`, a new calendar there named after the pupil (made as
/// Agenda ⚙'s New agenda makes one, on its server by the next sync), or
/// "place:<account/id>", one of the person's own that may be written
/// (`door::writable`). Kept in the shared settings, so that every device
/// writes into it; the agenda's lessons in the calendar chosen before are
/// taken out of it; then a refresh writes them into the new one.
pub(crate) fn choose_calendar(qt: &QtThread, shared: &std::sync::Arc<crate::backend::Shared>, id: &str, agenda: &str, choice: &str) {
    let Some(installed) = state::load(&state::path()).get(id).cloned() else { return };
    let Some(brought) = installed.agendas.iter().find(|a| a.id == agenda).cloned() else { return };
    let place = if let Some(account) = choice.strip_prefix("new:") {
        match crate::servers::make(qt, shared, &json!({ "account": account, "name": brought.name }).to_string()) {
            Ok(place) => place,
            Err(why) => {
                crate::backend::set_status(qt, why);
                return;
            }
        }
    } else if let Some(place) = choice.strip_prefix("place:").filter(|p| door::calendar_at(p).is_some_and(|c| door::writable(&c))) {
        place.to_string()
    } else {
        crate::backend::set_status(qt, say("extension-agenda-gone", &[]));
        show(qt);
        return;
    };
    let before = chosen_calendars(id).and_then(|c| c.get(agenda).cloned());
    if let Err(why) = sioul_core::config::set_extension_calendar(&crate::backend::config_path(), id, agenda, &place) {
        eprintln!("sioul: extensions: {id}: a calendar chosen: {why}");
        crate::backend::set_status(qt, say("extension-calendar-not-kept", &[]));
        return;
    }
    let _ = state::change(&state::path(), |s| {
        if let Some(a) = s.get_mut(id).and_then(|i| i.agendas.iter_mut().find(|a| a.id == agenda)) {
            a.problem = None;
        }
    });
    crate::backend::set_status(qt, say("extension-calendar-chosen", &[("name", brought.name.clone())]));
    show(qt);
    let (qt, id, agenda) = (qt.clone(), id.to_string(), agenda.to_string());
    std::thread::spawn(move || {
        // Its lessons leave the calendar chosen before: they go into this one now.
        if let Some(old) = before.filter(|old| *old != place).and_then(|old| door::calendar_at(&old)) {
            let (removed, refused) = door::take_out_of(&old, &id, &agenda);
            let _ = state::append_log(&state::log_path(&id), &[format!("{} sioul: agenda {agenda}: another calendar chosen; {removed} lessons taken out of the one before, {refused} refused", jiff::Timestamp::now())]);
        }
        if installed.enabled && (installed.signed_in || installed.offer.sign_in.is_none()) {
            refresh(Some(&qt), &id);
        }
    });
}

/// Whether an agenda's lessons count as the person's own busy time, chosen on
/// the page (the toggle beside the calendar). Kept in the shared settings
/// (`[extensions.busy]`), so every device writes the same transparency; then
/// a refresh writes the lessons again with it (`TRANSP:OPAQUE` or
/// `TRANSPARENT`). A shared setting like the calendar choice, not a secret.
pub(crate) fn choose_busy(qt: &QtThread, id: &str, agenda: &str, counts: bool) {
    let Some(installed) = state::load(&state::path()).get(id).cloned() else { return };
    if let Err(why) = sioul_core::config::set_extension_busy(&crate::backend::config_path(), id, agenda, counts) {
        eprintln!("sioul: extensions: {id}: a busy choice: {why}");
        crate::backend::set_status(qt, say("extension-calendar-not-kept", &[]));
        return;
    }
    // The calendar its lessons are in now, to take them out of so the refresh writes them fresh.
    let place = chosen_calendars(id).and_then(|c| c.get(agenda).cloned());
    show(qt);
    let (qt, id, agenda) = (qt.clone(), id.to_string(), agenda.to_string());
    std::thread::spawn(move || {
        let _ = state::append_log(&state::log_path(&id), &[format!("{} sioul: agenda {agenda}: busy time set to {counts}", jiff::Timestamp::now())]);
        // The busy mark is not part of the lesson's hash, so a refresh alone would
        // leave the events already written as they are: they are taken out first,
        // then the refresh writes them again with the transparency now chosen (the
        // way a calendar change relocates them). A device whose fetch is not the
        // freshest writes nothing; another device's next refresh restores them.
        if let Some(calendar) = place.and_then(|p| door::calendar_at(&p)) {
            let (removed, refused) = door::take_out_of(&calendar, &id, &agenda);
            let _ = state::append_log(&state::log_path(&id), &[format!("{} sioul: agenda {agenda}: {removed} lessons taken out to rewrite their busy state, {refused} refused", jiff::Timestamp::now())]);
        }
        if installed.enabled && (installed.signed_in || installed.offer.sign_in.is_none()) {
            refresh(Some(&qt), &id);
        }
    });
}

/// Yes to an update that asks for more: installed, what it is given now its own.
pub(crate) fn accept(qt: &QtThread, id: &str) {
    let (qt, id) = (qt.clone(), id.to_string());
    std::thread::spawn(move || {
        let address = load_config().extensions.manifest_address().to_string();
        let waiting = state::load(&state::path()).get(&id).and_then(|i| i.waiting.clone());
        if let Some(offer) = waiting {
            match store::install(&address, &offer) {
                Ok(_) => {
                    let _ = state::change(&state::path(), |s| {
                        if let Some(i) = s.get_mut(&id) {
                            store::discard_file(&i.offer);
                            // A new sign-in asked for: the old one no longer counts.
                            if i.offer.sign_in != offer.sign_in {
                                i.signed_in = false;
                            }
                            i.offer = offer.clone();
                            i.waiting = None;
                        }
                    });
                    crate::backend::set_status(&qt, say("extension-updated", &[("name", name_of(&id)), ("version", offer.version.clone())]));
                }
                Err(problem) => {
                    eprintln!("sioul: extensions: {id}: an update: {problem}");
                    crate::backend::set_status(&qt, say("extension-not-enabled", &[("name", name_of(&id))]));
                }
            }
        }
        show(&qt);
    });
}

fn name_of(id: &str) -> String {
    let fr = french();
    state::load(&state::path()).get(id).map(|i| i.offer.name.said(fr).to_string()).or_else(|| offered(id).map(|(o, _)| o.name.said(fr).to_string())).unwrap_or_else(|| id.to_string())
}

// ---------------------------------------------------------------- the runs

/// The log's lines of one call written, under a line saying which.
fn keep_log(id: &str, what: &str, outcome: &Outcome) {
    let mut lines = vec![format!("{} {what}", jiff::Timestamp::now())];
    lines.extend(outcome.log.iter().cloned());
    if let Err(problem) = state::append_log(&state::log_path(id), &lines) {
        eprintln!("sioul: extensions: {id}: its log: {problem}");
    }
}

/// The mark of a call running (`RUNNING`), taken away when it is dropped:
/// at the call's end, or as its thread unwinds from a panic (R5), so that a
/// run that panicked never stays "running" until Sioul is closed.
struct Running(String);

impl Drop for Running {
    fn drop(&mut self) {
        if let Ok(mut running) = RUNNING.lock() {
            running.remove(&self.0);
        }
    }
}

/// Marks `id` running; none when it runs already.
fn start(id: &str) -> Option<Running> {
    RUNNING.lock().ok().and_then(|mut r| r.insert(id.to_string()).then(|| Running(id.to_string())))
}

/// Where the lock of one call of `id` is, beside its log (this device's state).
fn run_lock_path(id: &str) -> std::path::PathBuf {
    state::log_path(id).with_extension("run")
}

/// The lock of one call of `id` across processes (a phone's window and its
/// background service share the extension's sign-in): taken without
/// waiting, held for the whole call, let go when the file is dropped. None
/// when another process holds it. A lock that cannot be taken here at all (a
/// file system without locks) lets the call run: `RUNNING` still holds within the process.
#[must_use]
fn run_lock(id: &str) -> Option<Option<std::fs::File>> {
    let path = run_lock_path(id);
    if let Some(folder) = path.parent() {
        let _ = std::fs::create_dir_all(folder);
    }
    let Ok(file) = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(&path) else { return Some(None) };
    match file.try_lock() {
        Ok(()) => Some(Some(file)),
        Err(std::fs::TryLockError::WouldBlock) => None,
        Err(_) => Some(None),
    }
}

/// One refresh of `id`, here and now (off the window's thread): its agendas
/// taken in when this device's fetch is the freshest, its state noted.
pub(crate) fn refresh(qt: Option<&QtThread>, id: &str) {
    refresh_since(qt, id, None);
}

/// A refresh decided from a record whose last run was `seen` (the minute's
/// tick, the phone's step): none when another process ran it meanwhile.
fn refresh_since(qt: Option<&QtThread>, id: &str, seen: Option<i64>) {
    let Some(running) = start(id) else { return };
    // One call of an extension at a time across processes: the other process runs it.
    let Some(_held) = run_lock(id) else { return };
    let Some(installed) = state::load(&state::path()).get(id).cloned().filter(|i| i.enabled) else { return };
    if seen.is_some_and(|seen| ran_meanwhile(seen, installed.last_run)) {
        return;
    }
    if let Some(qt) = qt {
        show(qt);
    }
    if let Ok(mut started) = STARTED.lock() {
        started.insert(id.to_string());
    }
    let started = now();
    note_attempt(id, started);
    let (outcome, _) = store::run(&installed.offer, Call::Refresh, french(), Real::new(id, None).removable());
    keep_log(id, "refresh", &outcome);
    // The data's age: when its last answer came, not when the run began (R2 b).
    let fetched = outcome.answered.unwrap_or_else(now);
    let mut brought: Option<Vec<Brought>> = None;
    let mut wrote = false;
    let problem: Option<String> = match &outcome.reply {
        Err(stop) => Some(stop.word().to_string()),
        Ok(text) => match reply::refresh_reply(text) {
            Err(nonsense) => {
                let _ = state::append_log(&state::log_path(id), &[format!("{} sioul: stopped: it replied nonsense: {}", jiff::Timestamp::now(), nonsense.0)]);
                Some("stopped-nonsense".into())
            }
            // Removed meanwhile: nothing of it is taken in.
            Ok(_) if state::load(&state::path()).get(id).is_none_or(|i| i.leaving) => Some("stopped-removed".into()),
            Ok(reply) => match chosen_calendars(id) {
                // Settings that do not read change nothing: no lesson written, none taken out.
                None => {
                    let _ = state::append_log(&state::log_path(id), &[format!("{} sioul: the settings file does not read: nothing written", jiff::Timestamp::now())]);
                    Some("settings".into())
                }
                Some(chosen) => {
                    let write = !reply.agendas.is_empty() && may_write(id, fetched);
                    let (taken, lines) = take_in(&door(), id, &reply, write, &|agenda| chosen.get(agenda).cloned(), fetched);
                    let at = jiff::Timestamp::now();
                    let _ = state::append_log(&state::log_path(id), &lines.iter().map(|line| format!("{at} sioul: {line}")).collect::<Vec<_>>());
                    wrote = taken.iter().any(|b| b.read == fetched);
                    if wrote {
                        note_written(id, fetched);
                    }
                    brought = Some(taken);
                    reply.problem.as_ref().map(|p| p.kind.word().to_string())
                }
            },
        },
    };
    let signed_out = problem.as_deref() == Some(ProblemKind::SignIn.word());
    let noted = state::change(&state::path(), |s| {
        if let Some(i) = s.get_mut(id) {
            i.last_run = started;
            if let Some(brought) = &brought {
                i.agendas = merged(&i.agendas, brought.clone());
                if wrote {
                    i.last_brought = fetched;
                }
            }
            if signed_out {
                i.signed_in = false;
            }
            i.problem = problem.clone();
        }
    });
    // Not noted (a full disk, a folder read only): the run waits three hours
    // all the same (`ATTEMPTED`), and the problem is said.
    let problem = match noted {
        Ok(()) => problem,
        Err(why) => {
            eprintln!("sioul: extensions: {id}: its record: {why}");
            let _ = state::append_log(&state::log_path(id), &[format!("{} sioul: this run could not be noted on this device; the next one waits three hours", jiff::Timestamp::now())]);
            Some("record".to_string())
        }
    };
    drop(running);
    if let Some(qt) = qt {
        if let Some(problem) = &problem {
            crate::backend::set_status(qt, problem_line(problem, &name_of(id)));
        } else if brought.iter().flatten().any(|b| b.problem.as_deref() == Some("choose")) {
            // Its agendas are known now: where their lessons go is the person's to choose.
            crate::backend::set_status(qt, say("extension-agendas-wait", &[("name", name_of(id))]));
        }
        show(qt);
    }
}

/// A run of `id` begun at `at`, kept in this process whatever its record says.
fn note_attempt(id: &str, at: i64) {
    if let Ok(mut attempted) = ATTEMPTED.lock() {
        attempted.insert(id.to_string(), at);
    }
}

fn attempted() -> BTreeMap<String, i64> {
    ATTEMPTED.lock().map(|a| a.clone()).unwrap_or_default()
}

/// Sign in…: the extension's sign-in call, its page shown by the window; a
/// choice asked of the person first when the extension asks one
/// (`extension_choices`), then a first refresh once signed in.
pub(crate) fn sign_in(qt: &QtThread, id: &str, choice: Option<String>) {
    let (qt, id) = (qt.clone(), id.to_string());
    std::thread::spawn(move || {
        let Some(installed) = state::load(&state::path()).get(&id).cloned().filter(|i| i.enabled) else { return };
        let wants_credentials = installed.offer.credentials.clone();
        // One sign-in at a time: another extension's page or form is open.
        if page_shown() || form_shown() {
            crate::backend::set_status(&qt, say("extension-sign-in-busy", &[]));
            return;
        }
        // A credentials sign-in collects the fields through Sioul's own form
        // before the call; Cancel runs nothing.
        let collected: Vec<(CredField, String)> = if wants_credentials.is_empty() {
            Vec::new()
        } else {
            let name = name_of(&id);
            match show_cred_form(&qt, &id, &name, &wants_credentials) {
                Some(values) => values,
                None => {
                    crate::backend::set_status(&qt, say("extension-sign-in-closed", &[("name", name)]));
                    return;
                }
            }
        };
        let Some(running) = start(&id) else { return };
        // A refresh of the background service holds it: the sign-in waits for none.
        let Some(held) = run_lock(&id) else { return };
        // A redirect sign-in shows a web page during the call; a credentials
        // sign-in shows none (its form was shown already).
        let shower: Option<store::SignInShower> = if wants_credentials.is_empty() {
            show(&qt);
            let (shower_qt, shower_id) = (qt.clone(), id.clone());
            Some(Box::new(move |page| show_page(&shower_qt, &shower_id, page)))
        } else {
            None
        };
        let call = Call::SignIn { choice: choice.as_deref(), credentials: &collected };
        let (outcome, _) = store::run(&installed.offer, call, french(), Real::new(&id, shower).removable());
        keep_log(&id, "sign-in", &outcome);
        drop(held);
        drop(running);
        let name = name_of(&id);
        let mut signed = false;
        let problem: Option<String> = match &outcome.reply {
            Err(stop) => Some(stop.word().to_string()),
            Ok(text) => match reply::sign_in_reply(text) {
                Err(_) => Some("stopped-nonsense".into()),
                Ok(SignInReply::SignedIn) => {
                    signed = true;
                    None
                }
                Ok(SignInReply::Choose(choices)) => {
                    let list = json!(choices.iter().map(|(id, name)| json!({ "id": id, "name": name })).collect::<Vec<_>>()).to_string();
                    let id_q = id.clone();
                    let _ = qt.queue(move |mut sioul| sioul.as_mut().extension_choices(QString::from(&id_q), QString::from(&list)));
                    None
                }
                Ok(SignInReply::Cancelled) => {
                    crate::backend::set_status(&qt, say("extension-sign-in-closed", &[("name", name.clone())]));
                    None
                }
                Ok(SignInReply::Problem(p)) => Some(p.kind.word().to_string()),
            },
        };
        let _ = state::change(&state::path(), |s| {
            if let Some(i) = s.get_mut(&id) {
                if signed {
                    i.signed_in = true;
                    i.problem = None;
                } else if problem.is_some() {
                    i.problem = problem.clone();
                }
            }
        });
        if signed {
            crate::backend::set_status(&qt, say("extension-signed-in", &[("name", name)]));
            refresh(Some(&qt), &id);
        } else if let Some(problem) = &problem {
            crate::backend::set_status(&qt, problem_line(problem, &name));
        }
        show(&qt);
    });
}

/// Each minute of the window: the extensions enabled and signed in here
/// refreshed when Sioul started or three hours passed, and the manifest read
/// once a day while one is enabled.
pub(crate) fn tick(qt: &QtThread) {
    let state = state::load(&state::path());
    let now = now();
    if state.extensions.iter().any(|i| i.leaving) {
        std::thread::spawn(move || forget_leaving(now));
    }
    if state.extensions.iter().all(|i| !i.enabled) {
        return;
    }
    // Once a day; after a failed read, again an hour later at the soonest.
    if now - state.checked >= DAILY && now - TRIED.load(std::sync::atomic::Ordering::Relaxed) >= 3600 {
        TRIED.store(now, std::sync::atomic::Ordering::Relaxed);
        read_list(Some(qt.clone()));
    }
    let started = started_here(&state, STARTED.lock().map(|s| s.clone()).unwrap_or_default());
    for installed in due(&state, now, &started, &attempted()) {
        let seen = state.get(&installed).map(|i| i.last_run);
        let (qt, id) = (qt.clone(), installed);
        std::thread::spawn(move || refresh_since(Some(&qt), &id, seen));
    }
}

/// The extensions due now: enabled here, signed in when they sign in, not
/// running, and not refreshed since Sioul started or for three hours. The
/// last run is the later of the record's and of this process's `attempted`.
fn due(state: &State, now: i64, started: &BTreeSet<String>, attempted: &BTreeMap<String, i64>) -> Vec<String> {
    let running = RUNNING.lock().map(|r| r.clone()).unwrap_or_default();
    state
        .extensions
        .iter()
        .filter(|i| i.enabled && (i.signed_in || i.offer.sign_in.is_none()) && !running.contains(&i.offer.id))
        .filter(|i| {
            let last = i.last_run.max(attempted.get(&i.offer.id).copied().unwrap_or(0));
            !started.contains(&i.offer.id) || now - last >= EVERY
        })
        .map(|i| i.offer.id.clone())
        .collect()
}

/// The phone's background step: the extensions due refreshed here, in the
/// service's process (no sign-in page there); never waited for long.
pub(crate) fn background_step() {
    let state = state::load(&state::path());
    let now = now();
    // In the service's process "started" means nothing: three hours, or never run.
    let attempted = attempted();
    let ids: Vec<String> = due(&state, now, &BTreeSet::new(), &attempted)
        .into_iter()
        .filter(|id| state.get(id).is_some_and(|i| now - i.last_run.max(attempted.get(id).copied().unwrap_or(0)) >= EVERY))
        .collect();
    for id in ids {
        let seen = state.get(&id).map(|i| i.last_run);
        refresh_since(None, &id, seen);
    }
}

/// What counts as refreshed since Sioul started. On a phone the window's
/// process ends and starts again many times a day: an opening is no start
/// there, and the three hours alone decide (R4); on a computer, `started`.
fn started_here(state: &State, started: BTreeSet<String>) -> BTreeSet<String> {
    if cfg!(target_os = "android") { state.extensions.iter().map(|i| i.offer.id.clone()).collect() } else { started }
}

/// Whether another process ran an extension since its record said `seen`.
fn ran_meanwhile(seen: i64, now_recorded: i64) -> bool {
    now_recorded > seen
}

// ---------------------------------------------------------------- the sign-in page

/// A sign-in page waited for: its own slot (never cleared by another's end),
/// the extension that asked it, where its answer goes, and what it watches for.
struct Page {
    slot: u64,
    id: String,
    said: std::sync::mpsc::Sender<Option<String>>,
    watch: String,
}

/// The sign-in page shown now: one at a time (the window has room for one).
static PAGE: Mutex<Option<Page>> = Mutex::new(None);
static SLOTS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// Whether a sign-in page is shown now.
fn page_shown() -> bool {
    PAGE.lock().map(|p| p.is_some()).unwrap_or(false)
}

/// A sign-in page shown by the window (a computer's SignInWindow.qml, a
/// phone's SignIn.java), its answer waited for here,
/// `page.wait` at most. Refused while another extension's page is shown:
/// each page has its own slot, and its end clears its own alone.
fn show_page(qt: &QtThread, id: &str, page: &SignInPage) -> Result<Option<String>, &'static str> {
    let (said, answer) = std::sync::mpsc::channel();
    let slot = SLOTS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    {
        let Ok(mut waiting) = PAGE.lock() else { return Err("not-now") };
        if waiting.is_some() {
            return Err("not-now");
        }
        *waiting = Some(Page { slot, id: id.to_string(), said, watch: page.watch.clone() });
    }
    let clear = || {
        if let Ok(mut waiting) = PAGE.lock()
            && waiting.as_ref().is_some_and(|p| p.slot == slot)
        {
            *waiting = None;
        }
    };
    // The screen's words in Sioul's language, not the phone's.
    let asked = json!({ "open": page.open, "watch": page.watch, "host": page.host, "french": french() }).to_string();
    #[cfg(target_os = "android")]
    {
        let _ = qt;
        if crate::steps::java("sign-in-open", &asked)["asked"] != true {
            clear();
            return Err("not-now");
        }
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = qt.queue(move |mut sioul| sioul.as_mut().set_sign_in_page(QString::from(&asked)));
    }
    let returned = answer.recv_timeout(page.wait).unwrap_or(None);
    clear();
    #[cfg(target_os = "android")]
    crate::steps::java("sign-in-close", "{}");
    #[cfg(not(target_os = "android"))]
    {
        let _ = qt.queue(|mut sioul| sioul.as_mut().set_sign_in_page(QString::default()));
    }
    Ok(returned)
}

/// The sign-in page of `id` closed, when it is the one shown (Remove): its call is told "cancelled".
fn cancel_page(id: &str) {
    let waiting = PAGE.lock().ok().and_then(|mut w| if w.as_ref().is_some_and(|p| p.id == id) { w.take() } else { None });
    if let Some(page) = waiting {
        let _ = page.said.send(None);
    }
}

// ---------------------------------------------------------------- the credentials form

/// A credentials form waited for: the extension that asked it, the fields it
/// asked (so Sioul reads back only those, in that order), and where the
/// values go. One at a time, as the web page is (`page_shown`/`form_shown`).
struct CredForm {
    id: String,
    fields: Vec<CredField>,
    said: std::sync::mpsc::Sender<Option<Vec<(CredField, String)>>>,
}

static CRED_FORM: Mutex<Option<CredForm>> = Mutex::new(None);

/// The most time the person has at a credentials form (the web page's own).
const FORM_WAIT: std::time::Duration = std::time::Duration::from_secs(15 * 60);

/// Whether a credentials form is shown now.
fn form_shown() -> bool {
    CRED_FORM.lock().map(|f| f.is_some()).unwrap_or(false)
}

/// The fixed label Sioul shows for a credential field (its own words, never
/// the extension's): one fluent sentence a kind.
fn cred_label(field: CredField) -> &'static str {
    match field {
        CredField::Email => "extension-cred-email",
        CredField::Username => "extension-cred-username",
        CredField::Password => "extension-cred-password",
        CredField::Server => "extension-cred-server",
        CredField::OneTimeCode => "extension-cred-one-time-code",
    }
}

/// Sioul's own credentials form shown (ExtensionSignIn.qml, a plain dialog in
/// Sioul's process on every device — no web view), its values waited for
/// here, `FORM_WAIT` at most. Framed entirely by Sioul: the only text from
/// the extension is its name; every field's label is Sioul's fixed word for
/// that kind. The values are returned to the caller and kept nowhere else.
/// None when the person cancelled or the form could not be shown.
fn show_cred_form(qt: &QtThread, id: &str, name: &str, fields: &[CredField]) -> Option<Vec<(CredField, String)>> {
    let (said, answer) = std::sync::mpsc::channel();
    {
        let Ok(mut waiting) = CRED_FORM.lock() else { return None };
        if waiting.is_some() {
            return None;
        }
        *waiting = Some(CredForm { id: id.to_string(), fields: fields.to_vec(), said });
    }
    let clear = || {
        if let Ok(mut waiting) = CRED_FORM.lock()
            && waiting.as_ref().is_some_and(|f| f.id == id)
        {
            *waiting = None;
        }
    };
    let shown: Vec<Value> = fields.iter().map(|f| json!({ "kind": f.tag(), "label": say(cred_label(*f), &[]), "masked": f.masked() })).collect();
    let asked = json!({ "id": id, "name": name, "fields": shown, "french": french() }).to_string();
    let _ = qt.queue(move |mut sioul| sioul.as_mut().set_extension_sign_in_form(QString::from(&asked)));
    let values = answer.recv_timeout(FORM_WAIT).unwrap_or(None);
    clear();
    let _ = qt.queue(|mut sioul| sioul.as_mut().set_extension_sign_in_form(QString::default()));
    values
}

/// The credentials form of `id` closed, when it is the one shown (Remove):
/// its sign-in is told the person cancelled.
fn cancel_form(id: &str) {
    let waiting = CRED_FORM.lock().ok().and_then(|mut f| if f.as_ref().is_some_and(|c| c.id == id) { f.take() } else { None });
    if let Some(form) = waiting {
        let _ = form.said.send(None);
    }
}

/// ExtensionSignIn.qml's word: the values typed, as a JSON object keyed by
/// the fields' kinds ({"email":…,"password":…}), or "" when cancelled. Sioul
/// reads back only the fields it asked, in the order it asked them, so the
/// form cannot add a field; the values reach the one sign-in that waits.
pub(crate) fn credentials_entered(id: &str, values: &str) {
    let waiting = CRED_FORM.lock().ok().and_then(|mut f| if f.as_ref().is_some_and(|c| c.id == id) { f.take() } else { None });
    let Some(form) = waiting else { return };
    let collected = match serde_json::from_str::<Value>(values) {
        Ok(Value::Object(map)) => Some(form.fields.iter().map(|field| (*field, map.get(field.tag()).and_then(Value::as_str).unwrap_or("").to_string())).collect()),
        _ => None,
    };
    let _ = form.said.send(collected);
}

/// The page's word: the address it returned to, or "" when the person
/// closed it. Only the address watched for is given back.
pub(crate) fn returned(address: &str) {
    let waiting = PAGE.lock().ok().and_then(|mut w| w.take());
    if let Some(page) = waiting {
        let _ = page.said.send(sioul_core::extensions::manifest::returns_to(address, &page.watch).then(|| address.to_string()));
    }
}

/// SignIn.java's word (android/main.cpp): {returned} or {cancelled}.
///
/// # Safety
/// `json` is null, or a zero-terminated UTF-8 text valid for the call.
#[cfg(target_os = "android")]
// SAFETY: no other symbol of the program has this name.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sioul_sign_in_returned(json: *const std::ffi::c_char) {
    // SAFETY: as the caller promises.
    let text = if json.is_null() { String::new() } else { unsafe { std::ffi::CStr::from_ptr(json) }.to_string_lossy().into_owned() };
    let _ = std::panic::catch_unwind(|| {
        let answer: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
        returned(answer["returned"].as_str().unwrap_or(""));
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use sioul_core::extensions::reply::refresh_reply;
    use std::cell::RefCell;

    /// A door in memory: what it was handed, by agenda, with the calendar named.
    #[derive(Default)]
    struct Memory(RefCell<BTreeMap<String, (String, AgendaReply)>>);

    impl AgendaDoor for Memory {
        fn take(&self, _: &str, agenda: &AgendaReply, place: &str) -> Result<door::Done, door::Problem> {
            if place == "gone/calendar" {
                return Err(door::Problem::CalendarGone);
            }
            self.0.borrow_mut().insert(agenda.id.clone(), (place.to_string(), agenda.clone()));
            Ok(door::Done { lessons: agenda.events, written: agenda.events, ..door::Done::default() })
        }
        fn take_out(&self, _: &str) -> (usize, usize) {
            let taken = self.0.borrow().len();
            self.0.borrow_mut().clear();
            (taken, 0)
        }
    }

    fn calendar(events: &[(&str, &str)]) -> String {
        let body: String = events.iter().map(|(uid, start)| format!("BEGIN:VEVENT\r\nUID:{uid}\r\nDTSTAMP:{start}\r\nDTSTART:{start}\r\nDTEND:{start}\r\nSUMMARY:Lesson\r\nEND:VEVENT\r\n")).collect();
        format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//t//EN\r\n{body}END:VCALENDAR\r\n")
    }

    fn refreshed(events: &[(&str, &str)], unread: &[(&str, &str)]) -> RefreshReply {
        let unread: Vec<Value> = unread.iter().map(|(from, until)| json!({ "from": from, "until": until })).collect();
        refresh_reply(&json!({ "agendas": [{ "id": "pupil-1", "name": "Timetable: Alex", "calendar": calendar(events), "unread": unread }] }).to_string()).unwrap()
    }

    /// An agenda goes to the door whole, read and counted, into the calendar
    /// chosen for it; with none chosen, or the one chosen gone, nothing is
    /// written and the record says why; a device whose fetch is not the
    /// freshest hands nothing and says nothing brought (R3); the record keeps
    /// a pupil no longer named and when each was last read; the log holds
    /// fixed words and ids, never a name the extension gave.
    #[test]
    fn an_agenda_goes_into_the_calendar_chosen_for_it() {
        let door = Memory::default();
        let first = refreshed(&[("A", "20261119T080000Z"), ("B", "20261120T080000Z"), ("C", "20261123T080000Z")], &[]);
        let (brought, log) = take_in(&door, "timetable", &first, true, &|_| None, 100);
        assert_eq!(brought[0].problem.as_deref(), Some("choose"));
        assert_eq!((brought[0].read, brought[0].events), (0, 3));
        assert!(door.0.borrow().is_empty(), "no calendar chosen: nothing written");
        let kids = |agenda: &str| (agenda == "pupil-1").then(|| "local/kids".to_string());
        let (brought, log_written) = take_in(&door, "timetable", &first, true, &kids, 100);
        assert_eq!(brought, vec![Brought { id: "pupil-1".into(), name: "Timetable: Alex".into(), events: 3, calendar: "local/kids".into(), read: 100, problem: None, busy: false }]);
        let (place, handed) = door.0.borrow()["pupil-1"].clone();
        assert_eq!((place.as_str(), handed.events, handed.calendar.matches("BEGIN:VEVENT").count()), ("local/kids", 3, 3));
        // The 20th answered 500: the door is told, to leave B as it is.
        let second = refreshed(&[("A", "20261119T090000Z")], &[("2026-11-19T23:00:00Z", "2026-11-20T23:00:00Z")]);
        take_in(&door, "timetable", &second, true, &kids, 200);
        assert_eq!(door.0.borrow()["pupil-1"].1.unread, vec![(1_795_129_200, 1_795_215_600)]);
        // The calendar chosen is gone: said, nothing written.
        let (gone, log_gone) = take_in(&door, "timetable", &first, true, &|_| Some("gone/calendar".into()), 300);
        assert_eq!((gone[0].problem.as_deref(), gone[0].read), (Some("calendar-gone"), 0));
        // Another device's fetch is the fresher: nothing handed to the door, nothing said brought (R3).
        door.0.borrow_mut().clear();
        let (later, _) = take_in(&door, "timetable", &first, false, &kids, 400);
        assert!(door.0.borrow().is_empty());
        assert_eq!((later[0].read, later[0].calendar.as_str()), (0, "local/kids"));
        // The record: when each was read kept, a pupil no longer named kept with where its lessons went.
        let before = vec![Brought { id: "pupil-0".into(), name: "Timetable: Sam".into(), events: 9, calendar: "local/sam".into(), read: 50, problem: None, busy: false }, brought[0].clone()];
        let after = merged(&before, later);
        assert_eq!(after.iter().map(|b| (b.id.as_str(), b.read)).collect::<Vec<_>>(), vec![("pupil-1", 100), ("pupil-0", 50)]);
        // The log: ids and fixed words, never the agenda's name.
        for line in log.iter().chain(&log_written).chain(&log_gone) {
            assert!(!line.contains("Alex") && line.starts_with("agenda pupil-1: "), "{line}");
        }
    }

    #[test]
    fn a_refresh_is_due_at_start_then_every_three_hours() {
        let offer = sioul_core::extensions::manifest::parse(&format!(
            "format = 1\nserial = 1\n[[extension]]\nid = \"t\"\nversion = \"1\"\ninterface = 1\nname = {{ en = \"T\", fr = \"T\" }}\nabout = {{ en = \"T\", fr = \"T\" }}\nbrings = [\"agenda\"]\nhosts = [\"api.example.org\"]\nsign_in = \"app://cb\"\nfile = \"t.wasm\"\nsha256 = \"{}\"\n",
            "0".repeat(64)
        ))
        .unwrap()
        .offers
        .remove(0);
        let mut installed = Installed::new(offer);
        installed.last_run = 1000;
        let mut state = State { extensions: vec![installed], ..State::default() };
        let (none, never) = (BTreeSet::new(), BTreeMap::new());
        assert!(due(&state, 2000, &none, &never).is_empty(), "not signed in: nothing to refresh");
        state.extensions[0].signed_in = true;
        assert_eq!(due(&state, 2000, &none, &never), vec!["t".to_string()], "Sioul started");
        let started: BTreeSet<String> = ["t".to_string()].into();
        assert!(due(&state, 2000, &started, &never).is_empty());
        assert_eq!(due(&state, 1000 + EVERY, &started, &never), vec!["t".to_string()]);
        // R1: it ran at 1000 + EVERY, and its record could not be written (a full disk):
        // the record still says 1000, yet a minute later it is not due, nor for three hours.
        let attempted: BTreeMap<String, i64> = [("t".to_string(), 1000 + EVERY)].into();
        assert!(due(&state, 1000 + EVERY + 60, &started, &attempted).is_empty(), "never a run every minute");
        assert!(due(&state, 1000 + 2 * EVERY - 1, &started, &attempted).is_empty());
        assert_eq!(due(&state, 1000 + 2 * EVERY, &started, &attempted), vec!["t".to_string()]);
        state.extensions[0].enabled = false;
        assert!(due(&state, 1000 + EVERY, &started, &never).is_empty());
    }

    fn manifest_of(offers: &[(&str, &str, &[&str], Option<&str>)]) -> Manifest {
        let mut text = String::from("format = 1\nserial = 2\n");
        for (id, sha, hosts, sign_in) in offers {
            let hosts: Vec<String> = hosts.iter().map(|h| format!("\"{h}\"")).collect();
            let sign_in = sign_in.map(|s| format!("sign_in = \"{s}\"\n")).unwrap_or_default();
            text.push_str(&format!("[[extension]]\nid = \"{id}\"\nversion = \"1\"\ninterface = 1\nname = {{ en = \"T\", fr = \"T\" }}\nabout = {{ en = \"T\", fr = \"T\" }}\nbrings = [\"agenda\"]\nhosts = [{}]\n{sign_in}file = \"t.wasm\"\nsha256 = \"{}\"\n", hosts.join(", "), sha.repeat(64)));
        }
        sioul_core::extensions::manifest::parse(&text).unwrap()
    }

    /// M1: a newer signed manifest takes away as well as it adds. An
    /// extension it no longer lists stops; a narrower list of hosts, or no
    /// sign-in, applies at once, its file unchanged, without asking; more
    /// still waits for the person's yes.
    #[test]
    fn a_newer_manifest_withdraws_and_narrows_at_once() {
        let before = manifest_of(&[("a", "1", &["api.example.org", "sso.example.org"], Some("app://cb")), ("b", "2", &["b.example.org"], None)]);
        let state = State { extensions: before.offers.iter().cloned().map(Installed::new).collect(), ..State::default() };
        // b withdrawn; a keeps its file but loses a host.
        let after = manifest_of(&[("a", "1", &["api.example.org"], Some("app://cb"))]);
        let planned = plan(&state, &after);
        assert_eq!(planned.withdrawn, vec!["b".to_string()]);
        assert_eq!(planned.narrowed.len(), 1);
        assert_eq!(planned.narrowed[0].hosts, vec!["api.example.org".to_string()], "the host taken away is no longer given");
        assert!(planned.taken.is_empty() && planned.waiting.is_empty(), "{planned:?}");
        // A new file asking for a host more, and no sign-in: the sign-in goes at once, the host waits.
        let wider = manifest_of(&[("a", "9", &["api.example.org", "sso.example.org", "new.example.org"], None), ("b", "2", &["b.example.org"], None)]);
        let planned = plan(&state, &wider);
        assert!(planned.withdrawn.is_empty());
        assert_eq!(planned.narrowed.len(), 1);
        assert_eq!(planned.narrowed[0].sign_in, None, "no sign-in given once the list keeps none");
        assert_eq!(planned.narrowed[0].sha256, "1".repeat(64), "the file in use stays until the update is taken");
        assert_eq!(planned.waiting.iter().map(|o| o.id.as_str()).collect::<Vec<_>>(), vec!["a"]);
        // Withdrawn already and still not listed: nothing more to do; listed again, it may be enabled again.
        let mut gone = state.clone();
        gone.extensions[1].enabled = false;
        gone.extensions[1].problem = Some("withdrawn".into());
        assert!(plan(&gone, &after).withdrawn.is_empty());
        assert_eq!(plan(&gone, &before).listed_again, vec!["b".to_string()]);
    }

    /// M3, M4: never an older version; a waiting update the manifest no
    /// longer offers is dropped; nothing is fetched for one disabled here.
    #[test]
    fn versions_are_ordered_and_a_stale_wait_is_dropped() {
        assert_eq!(version_order("0.10.0", "0.9.1"), std::cmp::Ordering::Greater);
        assert_eq!(version_order("1.0.0", "1.0.0"), std::cmp::Ordering::Equal);
        let installed = |version: &str| {
            let mut offer = manifest_of(&[("a", "1", &["api.example.org"], None)]).offers.remove(0);
            offer.version = version.into();
            State { extensions: vec![Installed::new(offer)], ..State::default() }
        };
        let mut older = manifest_of(&[("a", "2", &["api.example.org"], None)]);
        older.offers[0].version = "0.0.1".into();
        let planned = plan(&installed("1"), &older);
        assert!(planned.taken.is_empty() && planned.waiting.is_empty(), "an older version never replaces the one in use: {planned:?}");
        // An update asking for more waits; the next manifest goes back to the file in use: the wait is dropped.
        let mut waiting = installed("1");
        waiting.extensions[0].waiting = Some(manifest_of(&[("a", "3", &["api.example.org", "more.example.org"], None)]).offers.remove(0));
        assert_eq!(plan(&waiting, &manifest_of(&[("a", "1", &["api.example.org"], None)])).unwaited, vec!["a".to_string()]);
        // Disabled here: no download.
        let mut disabled = installed("1");
        disabled.extensions[0].enabled = false;
        let mut newer = manifest_of(&[("a", "4", &["api.example.org"], None)]);
        newer.offers[0].version = "2".into();
        assert!(plan(&disabled, &newer).taken.is_empty());
        assert_eq!(plan(&installed("1"), &newer).taken.len(), 1);
    }

    /// K2: a phone's window and its background service never run one
    /// extension at once; the second finds the lock held and lets the first
    /// run, then sees that it ran.
    #[test]
    fn one_call_of_an_extension_at_a_time_across_processes() {
        let id = "k2-lock-test";
        let first = run_lock(id).expect("free").expect("a lock file");
        assert!(run_lock(id).is_none(), "held by the other process (another open file here)");
        drop(first);
        let again = run_lock(id).expect("free again");
        drop(again);
        assert!(!ran_meanwhile(1000, 1000));
        assert!(ran_meanwhile(1000, 1000 + EVERY), "the other process ran it meanwhile: no second run");
        let _ = std::fs::remove_file(run_lock_path(id));
    }

    /// P3: the list's problem said in one of Sioul's sentences, never with the English detail.
    #[test]
    fn a_list_that_does_not_read_is_said_in_sioul_s_words() {
        assert_eq!(list_problem("the manifest's signature: signed by another key"), "extensions-list-signature");
        assert_eq!(list_problem("an older manifest (serial 3, 5 at least)"), "extensions-list-older");
        assert_eq!(list_problem("another manifest under serial 5"), "extensions-list-older");
        assert_eq!(list_problem("https://example.org/manifest.toml: the server answered 404"), "extensions-list-unread");
    }

    /// R5: a run whose thread panics is not left "running".
    #[test]
    fn a_run_that_panics_is_not_left_running() {
        let ran = std::thread::spawn(|| {
            let _running = start("r5-panics").expect("not running yet");
            assert!(start("r5-panics").is_none(), "never two at once");
            panic!("a run that panics");
        })
        .join();
        assert!(ran.is_err());
        assert!(!RUNNING.lock().unwrap().contains("r5-panics"));
        assert!(start("r5-panics").is_some(), "free again");
    }

    /// K1: Remove closes the sign-in page of the extension removed, and no other's.
    #[test]
    fn remove_closes_its_own_sign_in_page() {
        let (said, answer) = std::sync::mpsc::channel();
        *PAGE.lock().unwrap() = Some(Page { slot: 7, id: "other".into(), said, watch: "app://cb".into() });
        cancel_page("k1-removed");
        assert!(PAGE.lock().unwrap().is_some(), "another extension's page stays");
        cancel_page("other");
        assert_eq!(answer.recv().unwrap(), None, "its call is told cancelled");
        assert!(PAGE.lock().unwrap().is_none());
    }

    #[test]
    fn a_sign_in_page_gives_back_only_what_it_watched_for() {
        let page = |said| Some(Page { slot: 0, id: "t".into(), said, watch: "app://cb".into() });
        let (said, answer) = std::sync::mpsc::channel();
        *PAGE.lock().unwrap() = page(said);
        returned("https://elsewhere.example.org/?code=x");
        assert_eq!(answer.recv().unwrap(), None);
        // D3: what begins alike but names another host is not the address watched for.
        let (said, answer) = std::sync::mpsc::channel();
        *PAGE.lock().unwrap() = page(said);
        returned("app://cb.other.example/?code=x");
        assert_eq!(answer.recv().unwrap(), None);
        let (said, answer) = std::sync::mpsc::channel();
        *PAGE.lock().unwrap() = page(said);
        returned("app://cb?code=x");
        assert_eq!(answer.recv().unwrap().as_deref(), Some("app://cb?code=x"));
        returned("app://cb?code=again");
        assert!(PAGE.lock().unwrap().is_none(), "once");
    }
}
