// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Your availabilities on their servers, without the window
//! (design-availability-sharing §3; docs/client.md, "Sharing your
//! availability"), so that the phone's background steps keep them too:
//!
//! - **the mirrors** (`keep`): each calendar of busy blocks written here from
//!   the freshest view only (`availability::may_write`), what it wrote from
//!   noted in this device's sealed entry (`devices::Entry::mirrors`);
//! - **the servers' side** (`marks`): each share waiting for its server's yes
//!   made, each mirror marked transparent and described once a day, your
//!   working hours told to the servers you chose, the agendas of someone
//!   else's availability marked transparent; each step tried again at most
//!   every quarter of an hour, and nothing asked while nothing waits;
//! - **making and stopping a share** (`make_share`, `stop_share`), its
//!   server's own way first (`availability::choose`);
//! - **the phone's step** (`background_step`): one token check per calendar
//!   that counts, a pull and a rewrite only when one changed.
//!
//! What this device did on the servers is its own memory
//! (`state/availability.toml`), never shared: the settings say what is
//! shared, each device sees to it.
//!
//! **The busy tasks' times** (review of `reach`, B3 and B4, 19 November
//! 2026) are one record for all devices, the freshest winning
//! (`availability::BusyTimes`), carried in each device's sealed entry of the
//! sharing (`devices::Entry::busy_tasks`), the way a mirror's note travels:
//! never on any calendar's server. A device whose task lists are as fresh as
//! any other's (`availability::tasks_fresh`, against the stamps of the
//! others' entries, `devices::task_writers`) works them out
//! (`availability::busy_times`): a task's block or start time; the plan's
//! time before its date asked, a span once shared kept while its task is the
//! same, still ahead and no busy event took its place, else laid again from
//! this device's plan (`plan_seen`, the window's, never written to a file);
//! without a plan here (a phone's background step, the command line), the
//! shared spans of a task that is the same; where none is known anywhere,
//! nothing (the task's panel says so: `busy_line`). It keeps them in its own
//! entry. A device whose task lists are behind carries the record over as
//! found: it neither adds, changes nor removes a task's block. Each mirror's
//! note keeps the calendars' stamp (`Entry::mirrors`) and the task lists'
//! (`Entry::mirror_tasks`).

use crate::backend::{load_config, say, tr};
use serde::{Deserialize, Serialize};
use sioul_core::availability::{self as av, Audience, Availability, Choice, Mirror, Server, ServerKind, Share, Way};
use sioul_core::config::{Account, Config};
use sioul_core::vdir::{self, Collection, Kind, State};
use sioul_sync::SyncError;
use sioul_sync::calendars::{self, GoogleApi};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

/// What this device did on the servers, never shared (`state/availability.toml`).
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Memo {
    /// Each mirror's place, and the day its description last said ("2026-11-05").
    #[serde(default)]
    described: BTreeMap<String, String>,
    /// Each account, and a hash of the working hours last told it.
    #[serde(default)]
    hours: BTreeMap<String, String>,
    /// The agendas of someone else's availability marked transparent here, by place.
    #[serde(default)]
    transparent: BTreeSet<String>,
    /// What was tried and refused, by what it is ("share:`<id>`:`<way>`:`<who>`"), and when: tried again a quarter of an hour later.
    #[serde(default)]
    tried: BTreeMap<String, i64>,
    /// The notices said once ("moved:`<id>`", "stale:`<id>`:`<day>`").
    #[serde(default)]
    said: BTreeSet<String>,
    /// The availabilities the migration made here, to say once.
    #[serde(default)]
    moved: Vec<String>,
    /// Each mirror seen here without its calendar, by "`<availability>`:`<account>`", and since when (`keep`).
    #[serde(default)]
    missing: BTreeMap<String, i64>,
    /// Each mirror made again here, by "`<availability>`:`<account>`", until its calendar is on its server.
    #[serde(default)]
    remade: BTreeSet<String>,
    /// Why a mirror could not be made (again), by "`<availability>`:`<account>`": said on its shares' lines.
    #[serde(default)]
    problems: BTreeMap<String, String>,
    /// Shares made on a calendar another availability keeps, to take back there once (`marks`).
    #[serde(default)]
    take_back: Vec<TakeBack>,
}

/// A share made on a calendar another availability keeps (settings of before
/// 26 November 2026), its own calendar made since: taken back there, unless
/// that availability shares the same way with the same.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct TakeBack {
    account: String,
    /// The calendar's address on its server.
    url: String,
    way: String,
    #[serde(default)]
    who: String,
}

/// How long a mirror is seen without its calendar here before `keep` makes
/// it again, and only once its account was synced since (`synced_since`): a
/// removal on another device reaches this one by two ways, its calendar by
/// the sync and its availability's settings by the sharing, and the second
/// may come later. Making it again meanwhile would bring back what was removed.
const MISSING_FOR: i64 = 10 * 60;

fn memo_path() -> PathBuf {
    sioul_core::config::state_dir().join("availability.toml")
}

/// This device's plan's time for busy tasks (`availability::plan_spans`), as
/// the window last worked it out; none in a process without a window (a
/// phone's background service, the command line), which then uses the
/// shared record (`keep`). Never written to a file.
static PLAN: std::sync::Mutex<Option<av::Planned>> = std::sync::Mutex::new(None);

/// The plan's time for busy tasks, as the window just worked it out.
pub(crate) fn plan_seen(planned: av::Planned) {
    *PLAN.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(planned);
}

/// The busy tasks' times as `keep` last worked them out or carried them, for
/// the task's panel (`busy_line`).
static LAST: std::sync::Mutex<Option<av::BusyTimes>> = std::sync::Mutex::new(None);

impl Memo {
    fn load() -> Memo {
        crate::files::read_to_string(memo_path()).ok().and_then(|text| toml::from_str(&text).ok()).unwrap_or_default()
    }

    fn save(&self) {
        if let Ok(text) = toml::to_string(self) {
            let _ = sioul_sync::share::write_atomically(&memo_path(), text.as_bytes());
        }
    }

    /// Whether `key` may be tried now: never refused, or refused a quarter of an hour ago.
    fn due(&self, key: &str, now: i64) -> bool {
        self.tried.get(key).is_none_or(|at| now - at >= 15 * 60)
    }
}

/// Whether this device said a notice of `key` already (`Memo::said`): the
/// availabilities' notices, and the first busy stretch's (`everywhere`).
pub(crate) fn said_once(key: &str) -> bool {
    Memo::load().said.contains(key)
}

/// A notice of `key` said on this device: never said again here.
pub(crate) fn mark_said_once(key: &str) {
    with_memo(|memo| {
        memo.said.insert(key.to_string());
    });
}

/// A memo changed and kept, under one lock across threads.
fn with_memo<R>(change: impl FnOnce(&mut Memo) -> R) -> R {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _held = LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut memo = Memo::load();
    let before = memo.clone();
    let out = change(&mut memo);
    if memo != before {
        memo.save();
    }
    out
}

fn now() -> i64 {
    sioul_core::zone::now().timestamp().as_second()
}

fn today() -> jiff::civil::Date {
    sioul_core::zone::now().date()
}

// ---------------------------------------------------------------- the servers

/// The host of an address: "cloud.example.org".
fn host_of(url: &str) -> String {
    url.split("://").nth(1).and_then(|rest| rest.split(['/', ':']).next()).unwrap_or_default().to_ascii_lowercase()
}

/// What an account's server is: Google by its sign-in, a Nextcloud by where
/// its calendars live (`/remote.php/dav/`, Nextcloud's, ownCloud's and
/// Murena's), Fastmail by its host; any other server, as it is.
pub(crate) fn kind_of(account: &Account) -> ServerKind {
    if account.auth.as_deref() == Some("google") {
        return ServerKind::Google;
    }
    let home = sioul_sync::dav::Homes::load(&account.id).calendars.unwrap_or_default();
    if home.contains("/remote.php/dav") {
        ServerKind::Nextcloud
    } else if host_of(&home).ends_with("fastmail.com") {
        ServerKind::Fastmail
    } else {
        ServerKind::Other
    }
}

/// Every calendars account as the way chooser reads it (`availability::choose`).
pub(crate) fn servers(config: &Config) -> Vec<Server> {
    config
        .accounts
        .iter()
        .filter(|a| a.is_dav())
        .map(|a| {
            let kind = kind_of(a);
            let host = if kind == ServerKind::Google { a.address.clone().unwrap_or_default() } else { host_of(&sioul_sync::dav::Homes::load(&a.id).calendars.unwrap_or_default()) };
            let both = matches!(kind, ServerKind::Google | ServerKind::Nextcloud);
            Server { account: a.id.clone(), kind, host, shares: both, publishes: both }
        })
        .collect()
}

/// An account's server, as a person reads it: "Google", its host.
pub(crate) fn server_name(config: &Config, account: &str) -> String {
    servers(config).into_iter().find(|s| s.account == account).map(|s| if s.kind == ServerKind::Google { "Google".to_string() } else { s.host }).filter(|h| !h.is_empty()).unwrap_or_else(|| account.to_string())
}

fn collection(account: &str, id: &str) -> Option<Collection> {
    vdir::collections(Kind::Calendars).into_iter().find(|c| c.account == account && c.id == id)
}

fn by_place(place: &str) -> Option<Collection> {
    vdir::collections(Kind::Calendars).into_iter().find(|c| sioul_core::agendas::place(c) == place)
}

/// A calendar's address on its server, once it is there.
fn on_server(collection: &Collection) -> Option<String> {
    let state = State::load(&collection.state_path());
    (!state.url.is_empty() && !state.pending).then_some(state.url)
}

/// A mirror's name, what its reader sees: the person's name from the
/// account, then "busy times" ("Alex, busy times"); "Busy times" without one.
pub(crate) fn mirror_name(config: &Config, account: &str) -> String {
    match config.every_account().find(|a| a.id == account).and_then(|a| a.name.clone()).map(|n| n.trim().to_string()).filter(|n| !n.is_empty()) {
        Some(name) => say("availability-mirror-name", &[("name", name)]),
        None => tr().text("availability-calendar", None),
    }
}

// ---------------------------------------------------------------- a mirror made again

/// A mirror's key in this device's memo: "`<availability>`:`<account>`".
fn mirror_key(availability: &str, account: &str) -> String {
    format!("{availability}:{account}")
}

/// The accounts where `availability`'s mirror is a calendar an earlier
/// availability keeps (`availability::borrowed`).
fn borrowed_in(config: &Config, availability: &Availability) -> Vec<String> {
    av::borrowed(&config.agenda.availabilities, availability)
}

/// An availability's own calendar of blocks in an account, here: none when
/// it has no mirror there, when its calendar is gone, or when an earlier
/// availability keeps that calendar (`borrowed_in`).
fn own_mirror(config: &Config, availability: &Availability, account: &str) -> Option<Collection> {
    let mirror = availability.mirror(account)?;
    if borrowed_in(config, availability).iter().any(|a| a == account) {
        return None;
    }
    collection(&mirror.account, &mirror.calendar)
}

/// Whether this device synced an account's calendars since `at`: one of them
/// pulled whole and current then (`State::pulled`).
fn synced_since(account: &str, at: i64) -> bool {
    vdir::collections(Kind::Calendars).iter().filter(|c| c.account == account).any(|c| State::load(&c.state_path()).pulled.is_some_and(|p| p >= at))
}

/// An availability's mirror made (again) in an account, written into the
/// settings: its own folder (`availability::mirror_folder`, the same on
/// every device, so that two devices making it make one calendar), adopted
/// when that folder is here already (another device made it), its name the
/// mirror's; each of its shares there waiting for its server again, a link's
/// link with it, to be made on this calendar. A share made on a calendar
/// another availability keeps is taken back there (`TakeBack`, by `marks`).
/// Sent by the next sync. Returns the availability as written.
fn make_mirror(config: &Config, availability: &Availability, account: &str) -> Result<Availability, String> {
    let key = mirror_key(&availability.id, account);
    let was = availability.mirror(account).map(|m| m.calendar.clone());
    let borrowed = borrowed_in(config, availability).iter().any(|a| a == account);
    let folder = av::mirror_folder(&availability.id, was.as_deref());
    let made = match collection(account, &folder) {
        Some(here) => Ok(here),
        None => vdir::create_as(Kind::Calendars, account, &folder, &mirror_name(config, account), None, &["VEVENT"]),
    };
    let made = match made {
        Ok(made) => made,
        Err(problem) => {
            with_memo(|m| m.problems.insert(key.clone(), problem.clone()));
            return Err(problem);
        }
    };
    let old_url = was.as_deref().filter(|_| borrowed).and_then(|c| collection(account, c)).and_then(|c| on_server(&c));
    let mut written = availability.clone();
    written.mirrors.retain(|m| m.account != account);
    written.mirrors.push(Mirror { account: account.to_string(), calendar: made.id.clone(), transparent: false });
    let mut back = Vec::new();
    for share in written.shares.iter_mut().filter(|s| s.account == account && s.way().is_some_and(Way::mirrored)) {
        if let Some(url) = old_url.as_ref().filter(|_| !share.made.is_empty()) {
            back.push(TakeBack { account: account.to_string(), url: url.clone(), way: share.way.clone(), who: share.who.clone() });
        }
        share.made.clear();
        if share.way() == Some(Way::MirrorLink) {
            share.link.clear();
        }
    }
    sioul_core::agendas::put_availability(&crate::backend::config_path(), &written)?;
    with_memo(|m| {
        m.problems.remove(&key);
        m.missing.remove(&key);
        m.remade.insert(key.clone());
        m.take_back.extend(back);
    });
    Ok(written)
}

/// Whether the sharing caught up since `at`: not set up here, or an exchange
/// that went well since (`share::last_exchange`). A removal on another
/// device reaches this one by the sharing's settings as well as by the
/// calendar's sync.
fn sharing_since(at: i64) -> bool {
    !crate::share::configured() || crate::share::last_exchange().is_some_and(|(when, well)| well && when >= at)
}

/// Each mirror a share of `availability` needs that is not its own calendar
/// here made (again) by `make_mirror`: at once when it has no entry, or when
/// an earlier availability keeps its calendar (`borrowed_in`); when its
/// calendar is gone, once seen gone for `MISSING_FOR`, its account synced
/// since (`synced_since`) and the sharing caught up since (`sharing`, which
/// `keep` gives as `sharing_since`): a removal on another device arrives by
/// both, and the calendar's sync alone would bring back a share you just
/// stopped. Its account gone from Sioul: said on its shares' lines, nothing
/// made. Returns the availability as it now is.
fn made_again(config: &Config, availability: &Availability, now: i64, sharing: &dyn Fn(i64) -> bool) -> Availability {
    let mut current = availability.clone();
    let accounts: BTreeSet<String> = current.shares.iter().filter(|s| s.way().is_some_and(Way::mirrored)).map(|s| s.account.clone()).collect();
    for account in accounts {
        let key = mirror_key(&current.id, &account);
        if !config.accounts.iter().any(|a| a.id == account && a.is_dav()) {
            with_memo(|m| m.problems.insert(key.clone(), tr().text("availability-mirror-no-account", None)));
            continue;
        }
        let borrowed = borrowed_in(config, &current).contains(&account);
        let entry = current.mirror(&account).cloned();
        let here = entry.as_ref().and_then(|m| collection(&m.account, &m.calendar));
        let due = match (&entry, &here) {
            (None, _) => true,
            (Some(_), _) if borrowed => true,
            (Some(_), Some(calendar)) => {
                // There: no longer missing; made again and on its server, said no more.
                let on = on_server(calendar).is_some();
                with_memo(|m| {
                    m.missing.remove(&key);
                    m.problems.remove(&key);
                    if on {
                        m.remade.remove(&key);
                    }
                });
                false
            }
            (Some(_), None) => {
                let since = with_memo(|m| *m.missing.entry(key.clone()).or_insert(now));
                now - since >= MISSING_FOR && synced_since(&account, since) && sharing(since)
            }
        };
        if !due {
            continue;
        }
        match make_mirror(config, &current, &account) {
            Ok(written) => current = written,
            Err(problem) => eprintln!("availability: {key}: {problem}"),
        }
    }
    current
}

/// What a mirrored share's line says of its calendar while it is not there
/// (`servers::share_line`): why it could not be made again; that it is
/// being made again, while it is gone, borrowed from another availability,
/// or made again here and not on its server yet. None while it is there,
/// and for a share no mirror carries.
pub(crate) fn mirror_line(config: &Config, availability: &Availability, share: &Share) -> Option<String> {
    if !share.way().is_some_and(Way::mirrored) {
        return None;
    }
    let key = mirror_key(&availability.id, &share.account);
    let memo = Memo::load();
    if let Some(problem) = memo.problems.get(&key) {
        return Some(say("availability-share-remake-failed", &[("why", problem.clone())]));
    }
    let remaking = || say("availability-share-remaking", &[("server", server_name(config, &share.account))]);
    match own_mirror(config, availability, &share.account) {
        Some(here) if on_server(&here).is_none() && memo.remade.contains(&key) => Some(remaking()),
        Some(_) => None,
        None => Some(remaking()),
    }
}

// ---------------------------------------------------------------- the mirrors, from the freshest view

/// When this device last knew a source agenda whole and current
/// (`State::pulled`); one on this computer always is. None for one never
/// pulled whole here.
fn pulled_of(collection: &Collection, now: i64) -> Option<i64> {
    if collection.account == vdir::LOCAL {
        return Some(now);
    }
    State::load(&collection.state_path()).pulled
}

/// This device's view of an availability's sources (`availability::view`):
/// those it has here, a source gone from it no longer counted.
fn view_here(availability: &Availability, now: i64) -> Option<(i64, i64)> {
    let pulled: Vec<Option<i64>> = availability.agendas.iter().filter_map(|place| by_place(place)).map(|c| pulled_of(&c, now)).collect();
    av::view(&pulled)
}

/// A mirror's files made what the blocks say: whether anything was written
/// (through the one door of every writer, `vdir::write_item`, `remove_item`).
fn write_files(collection: &Collection, files: &BTreeMap<String, String>) -> bool {
    let mut changed = false;
    for (name, text) in files {
        let path = collection.dir.join(name);
        if crate::files::read_to_string(&path).ok().as_deref() != Some(text.as_str()) && vdir::write_item(&path, text).is_ok() {
            changed = true;
        }
    }
    for path in collection.items() {
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        if !files.contains_key(&name) && vdir::remove_item(&path).is_ok() {
            changed = true;
        }
    }
    changed
}

/// What this device wrote a mirror from, in its sealed entry
/// (`devices::Entry::mirrors`, the calendars' view; `mirror_tasks`, the
/// task lists' its tasks' part came from).
fn note_written(id: &str, as_of: i64, tasks_as_of: i64) {
    let Some((_, vault)) = crate::share::vault() else { return };
    let path = sioul_sync::devices::own_path(&sioul_core::config::state_dir());
    let _ = sioul_sync::devices::change(&path, vault.as_ref().map(|(folder, key)| (folder.as_path(), key)), |entry| {
        entry.mirrors.insert(id.to_string(), as_of);
        if tasks_as_of > 0 {
            entry.mirror_tasks.insert(id.to_string(), tasks_as_of);
        }
    });
}

/// How far ahead the days off are laid into the busy times: past any availability's window.
const DAYS_OFF_AHEAD: i64 = 400 * 86_400;

/// Your days off within `window`, busy all day for others (decision 3 of
/// design-calendars), from the one function that says them
/// (`vacations::DaysOff`); never for do-not-disturb (`busy::stretches`).
/// Laid into the busy tasks' times where they are worked out
/// (`busy_times_now`), so that they follow the same rule of which device
/// writes: one sealed record, the freshest device's (`BusyTimes`, the review
/// of `reach`, B3); a device whose view is behind carries over what it found,
/// and two devices that know different days never write the mirror in turn
/// (the calendars' review, F43).
fn days_off(config: &Config, window: (i64, i64), zone: &jiff::tz::TimeZone) -> Vec<(i64, i64)> {
    av::days_off_spans(&sioul_core::vacations::DaysOff::of(config), window, zone)
}

/// The busy tasks' times worked out here kept in this device's own entry,
/// when they changed (`devices::Entry::busy_tasks`).
fn keep_times(times: &av::BusyTimes) {
    let path = sioul_sync::devices::own_path(&sioul_core::config::state_dir());
    if sioul_sync::devices::own(&path).busy_tasks == *times {
        return;
    }
    let vault = crate::share::vault().and_then(|(_, vault)| vault);
    let _ = sioul_sync::devices::change(&path, vault.as_ref().map(|(folder, key)| (folder.as_path(), key)), |entry| {
        entry.busy_tasks = times.clone();
    });
}

/// This device's view of its task lists (`availability::view`): when it
/// last knew each of them whole and current.
fn tasks_view(now: i64) -> Option<(i64, i64)> {
    let pulled: Vec<Option<i64>> = sioul_core::tasks::lists().iter().map(|c| pulled_of(c, now)).collect();
    av::view(&pulled)
}

/// The busy tasks' times for `availability`'s mirror at `now`: worked out
/// here when this device's task lists are as fresh as any other's, kept in
/// its entry then; otherwise the freshest shared record, carried over
/// (module's words). With the stamp of the task lists it came from.
fn busy_times_now(config: &Config, availability: Option<&str>, tasks: &[sioul_core::tasks::Task], events: &[sioul_core::agenda::Occurrence], now: i64) -> av::BusyTimes {
    let own_path = sioul_sync::devices::own_path(&sioul_core::config::state_dir());
    let own = sioul_sync::devices::own(&own_path);
    let entries = crate::devices::entries();
    let others: Vec<&sioul_sync::devices::Entry> = entries.as_ref().map(|(id, all)| all.iter().filter(|e| e.id != *id && !e.left).collect()).unwrap_or_default();
    // This device's own record first: among equals, it stays.
    let shared = av::BusyTimes::freshest(std::iter::once(&own.busy_tasks).chain(others.iter().map(|e| &e.busy_tasks).filter(|b| b.as_of <= now + 600)));
    let stamps: Vec<i64> = match (&entries, availability) {
        (Some((id, all)), Some(availability)) => sioul_sync::devices::task_writers(all, id, availability, now),
        (Some((id, all)), None) => all.iter().filter(|e| e.id != *id && !e.left && e.busy_tasks.as_of > 0 && e.busy_tasks.as_of <= now + 600).map(|e| e.busy_tasks.as_of).collect(),
        _ => Vec::new(),
    };
    let view = tasks_view(now);
    let fresh = av::tasks_fresh(view, stamps);
    let zone = sioul_core::zone::current();
    let estimate = config.tasks.estimate.unwrap_or(sioul_core::plan::Settings::default().default_estimate);
    let plan = PLAN.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
    let mut times = av::tasks_part(fresh, tasks, events, plan.as_ref(), &shared, estimate, &zone, now, view.map_or(0, av::as_of));
    if fresh {
        // Your days off, laid into the record where it is worked out (F43); a device
        // behind carries them over with the rest, as it found them.
        // From yesterday's midnight: the same spans all day long, the record written again only when they change.
        let midnight = sioul_core::timeline::midnight(sioul_core::zone::now().date(), &zone) - 86_400;
        av::lay_days_off(&mut times, days_off(config, (midnight, midnight + DAYS_OFF_AHEAD), &zone));
        keep_times(&times);
    }
    *LAST.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(times.clone());
    times
}

/// Every mirror written from this device's view, when it is the freshest
/// (`availability::may_write`, against the other devices' sealed entries):
/// the coming days' blocks with their margins, health never named
/// (`availability::health_events`); what it wrote from noted. Nothing is
/// asked of any server. Returns the accounts whose mirror changed here,
/// for their sync to send.
pub(crate) fn keep(config: &Config, now: i64) -> Vec<String> {
    let list: Vec<&Availability> = config.agenda.availabilities.iter().filter(|a| !a.mirrors.is_empty()).collect();
    if list.is_empty() {
        return Vec::new();
    }
    let entries = crate::devices::entries();
    let fresh: Vec<(&Availability, Option<(i64, i64)>)> = list
        .into_iter()
        .map(|a| (a, view_here(a, now)))
        .filter(|(a, view)| av::may_write(*view, entries.as_ref().map(|(own, all)| sioul_sync::devices::mirror_writers(all, own, &a.id, now)).unwrap_or_default()))
        .collect();
    if fresh.is_empty() {
        return Vec::new();
    }
    let widest = fresh.iter().map(|(a, _)| av::window(a, now)).fold((i64::MAX, i64::MIN), |(from, to), (a, b)| (from.min(a), to.max(b)));
    let events = sioul_core::agenda::occurrences(widest.0, widest.1);
    let zone = sioul_core::zone::current();
    let tasks = sioul_core::tasks::all(&zone);
    let recorded = sioul_core::health::HealthState::errand_uids(&sioul_core::health::HealthState::default_path()).unwrap_or_default();
    let health = av::health_events(&events, &av::errand_tasks(&tasks, &recorded));
    let (busy, not_known) = (tr().text("availability-busy", None), tr().text("availability-not-known", None));
    let words = av::Words { busy: &busy, not_known: &not_known };
    let mut changed: Vec<String> = Vec::new();
    for (availability, view) in fresh {
        let availability = &made_again(config, availability, now, &sharing_since);
        // Only what is marked busy: the busy tasks' times, worked out here or carried over (module's words).
        let times = busy_times_now(config, Some(&availability.id), &tasks, &events, now);
        let task_time = times.time();
        let files = av::mirror_files(&events, availability, &words, &health, &task_time, now);
        let mut wrote = false;
        for mirror in &availability.mirrors {
            // Its own calendar alone: never one another availability keeps (one mirror calendar each).
            let Some(collection) = own_mirror(config, availability, &mirror.account) else { continue };
            if write_files(&collection, &files) {
                wrote = true;
                changed.push(mirror.account.clone());
            }
        }
        if wrote && let Some(view) = view {
            note_written(&availability.id, av::as_of(view), times.as_of);
        }
    }
    changed.sort();
    changed.dedup();
    changed
}

/// What the availability you share holds for a task (`uid`), in a sentence
/// for its panel under its Busy switch (review B4): the spans to come of it
/// and its open steps, as `keep` last worked them out or carried them (else
/// the freshest shared record); or that its time is not known yet. Empty
/// while no availability keeps a mirror, or the task gives no busy time
/// (`availability::busy_for_timer`).
pub(crate) fn busy_line(config: &Config, tasks: &[sioul_core::tasks::Task], uid: &str) -> String {
    if !config.agenda.availabilities.iter().any(|a| !a.mirrors.is_empty()) || !av::busy_for_timer(tasks, uid) {
        return String::new();
    }
    let times = LAST.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone().unwrap_or_else(|| {
        let own = sioul_sync::devices::own(&sioul_sync::devices::own_path(&sioul_core::config::state_dir())).busy_tasks;
        let others: Vec<av::BusyTimes> = crate::devices::entries().map(|(id, all)| all.into_iter().filter(|e| e.id != id && !e.left).map(|e| e.busy_tasks).collect()).unwrap_or_default();
        av::BusyTimes::freshest(std::iter::once(&own).chain(others.iter()))
    });
    // It and its open steps, however deep.
    let mut ours: BTreeSet<&str> = BTreeSet::from([uid]);
    loop {
        let more: Vec<&str> = tasks.iter().filter(|t| t.status.is_open() && t.parent().is_some_and(|p| ours.contains(p)) && !ours.contains(t.uid.as_str())).map(|t| t.uid.as_str()).collect();
        if more.is_empty() {
            break;
        }
        ours.extend(more);
    }
    let now = now();
    let mut spans: Vec<(i64, i64)> = times.tasks.iter().filter(|(id, _)| ours.contains(id.as_str())).flat_map(|(_, t)| t.spans.iter().copied()).filter(|(_, end)| *end > now).collect();
    spans.sort_unstable();
    if spans.is_empty() {
        return tr().text("busy-time-not-known", None);
    }
    let zone = sioul_core::zone::current();
    let today = today();
    let said: Vec<String> = spans
        .iter()
        .take(3)
        .map(|(start, end)| {
            let at = |t: i64| jiff::Timestamp::from_second(t).map(|t| t.to_zoned(zone.clone())).ok();
            let day = at(*start).map(|z| tr().day_in(z.date(), today)).unwrap_or_default();
            format!("{day}, {}–{}", sioul_core::timeline::clock(*start, &zone), sioul_core::timeline::clock(*end, &zone))
        })
        .collect();
    let mut args = sioul_core::i18n::args();
    args.set("spans", said.join("; "));
    args.set("more", spans.len().saturating_sub(3));
    tr().text("busy-time-holds", Some(&args))
}

/// The phone's background step (`steps`): for each account an availability
/// reads or keeps a mirror in, one token check per calendar that counts
/// (`dav::check_named`: its sources there and its mirrors), a pull only when
/// one changed; then the mirrors from the freshest view (`keep`), and those
/// that changed sent at once. No other calendar is asked, no event read
/// while nothing changed.
pub(crate) fn background_step(config: &Config) {
    let mut named: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for availability in config.agenda.availabilities.iter().filter(|a| !a.mirrors.is_empty()) {
        for place in &availability.agendas {
            if let Some((account, id)) = place.split_once('/')
                && account != vdir::LOCAL
            {
                named.entry(account.to_string()).or_default().insert(id.to_string());
            }
        }
        for mirror in &availability.mirrors {
            named.entry(mirror.account.clone()).or_default().insert(mirror.calendar.clone());
        }
    }
    for (account, ids) in &named {
        let Some(account) = config.accounts.iter().find(|a| a.id == *account && a.is_dav()) else { continue };
        let ids: Vec<String> = ids.iter().cloned().collect();
        if let Err(e) = sioul_sync::dav::check_named(account, &ids) {
            eprintln!("availability: {}: {e:?}", account.id);
        }
    }
    for account in keep(config, now()) {
        let Some(account) = config.accounts.iter().find(|a| a.id == account) else { continue };
        let mirrors: Vec<String> = config.agenda.availabilities.iter().flat_map(|a| a.mirrors.iter()).filter(|m| m.account == account.id).map(|m| m.calendar.clone()).collect();
        if let Err(e) = sioul_sync::dav::check_named(account, &mirrors) {
            eprintln!("availability: {}: {e:?}", account.id);
        }
    }
}

// ---------------------------------------------------------------- a share made and stopped

/// A server's refusal, said (`servers::server_said`).
fn said(error: &SyncError, account: &str) -> String {
    crate::servers::server_said(error, account)
}

/// A Google calendar's id, from an agenda's place.
fn google_calendar(place: &str) -> Option<String> {
    let collection = by_place(place)?;
    calendars::google_id(&State::load(&collection.state_path()).url)
}

/// The mirror of an availability in an account, its own calendar
/// (`own_mirror`: never one another availability keeps), and its address on
/// its server once there: where a share of it is made.
fn mirror_url(config: &Config, availability: &Availability, account: &str) -> Result<String, String> {
    let collection = own_mirror(config, availability, account).ok_or_else(|| tr().text("availability-mirror-missing", None))?;
    on_server(&collection).ok_or_else(|| tr().text("availability-mirror-not-yet", None))
}

/// The calendar a share of it was made on, as its entry names it, and its
/// address on its server: where a share is taken back.
fn made_on(availability: &Availability, account: &str) -> Option<String> {
    let mirror = availability.mirror(account)?;
    on_server(&collection(&mirror.account, &mirror.calendar)?)
}

/// Whether a share can be asked of its server now: a mirror's once its own
/// calendar is there; a link you paste once it is pasted; the rest at once.
pub(crate) fn ready(config: &Config, availability: &Availability, share: &Share) -> bool {
    match share.way() {
        Some(way) if way.mirrored() => mirror_url(config, availability, &share.account).is_ok(),
        Some(Way::FastmailLink | Way::Appointments | Way::GoogleSchedule) => !share.link.trim().is_empty(),
        Some(_) => true,
        None => false,
    }
}

/// A share asked of its server (its own way: `Way`); its link, when the way
/// has one. Nothing leaves this computer in the demo profile.
pub(crate) fn make_share(config: &Config, availability: &Availability, share: &Share) -> Result<String, String> {
    let account = config.every_account().find(|a| a.id == share.account).cloned().ok_or_else(|| tr().text("agenda-new-no-account", None))?;
    let way = share.way().ok_or_else(|| tr().text("availability-way-none", None))?;
    if crate::backend::offline() {
        return Ok(match way {
            Way::MirrorLink | Way::GooglePublic => format!("https://cloud.example.org/remote.php/dav/public-calendars/{}?export", availability.id.chars().take(8).collect::<String>()),
            _ => share.link.clone(),
        });
    }
    let who = share.who.trim();
    let e = |error: SyncError| said(&error, &account.id);
    let google = || GoogleApi::new(account.address.as_deref().unwrap_or_default()).map_err(e);
    match way {
        Way::GoogleFreeBusy => {
            let api = google()?;
            for calendar in availability.agendas.iter().filter(|p| p.starts_with(&format!("{}/", account.id))).filter_map(|p| google_calendar(p)) {
                api.grant(&calendar, "user", who, "freeBusyReader").map_err(e)?;
            }
            Ok(String::new())
        }
        Way::GooglePublic => {
            let calendar = availability.agendas.first().and_then(|p| google_calendar(p)).ok_or_else(|| tr().text("agenda-sharing-not-yet", None))?;
            match calendars::google_public_busy(&google()?, &calendar, true) {
                Ok(link) => Ok(link.unwrap_or_default()),
                Err(error) if calendars::public_with_details(&error) => Err(tr().text("availability-google-public-already", None)),
                Err(error) => Err(e(error)),
            }
        }
        Way::MirrorShare | Way::MirrorFederated | Way::MirrorLink => {
            let url = mirror_url(config, availability, &account.id)?;
            if account.auth.as_deref() == Some("google") {
                let api = google()?;
                let calendar = calendars::google_id(&url).ok_or_else(|| tr().text("agenda-sharing-not-yet", None))?;
                return match way {
                    Way::MirrorLink => {
                        api.grant(&calendar, "default", "", "reader").map_err(e)?;
                        Ok(calendars::google_public_ical(&calendar))
                    }
                    _ => api.grant(&calendar, "user", who, "freeBusyReader").map(|()| String::new()).map_err(e),
                };
            }
            let client = sioul_sync::dav::client_for(&account).map_err(e)?;
            match way {
                Way::MirrorLink => {
                    calendars::dav_publish(&client, &url, true).map_err(e)?;
                    Ok(calendars::dav_sharing(&client, &url).map_err(e)?.link.unwrap_or_default())
                }
                Way::MirrorFederated => {
                    let principal = calendars::federated_principal(who).ok_or_else(|| tr().text("agenda-share-elsewhere", None))?;
                    calendars::dav_share(&client, &url, &principal, false).map(|()| String::new()).map_err(e)
                }
                _ => {
                    let (name, group) = who.strip_prefix("group:").map_or((who, false), |n| (n.trim(), true));
                    let principal = calendars::principal(name, group).ok_or_else(|| tr().text("agenda-share-elsewhere", None))?;
                    calendars::dav_share(&client, &url, &principal, false).map(|()| String::new()).map_err(e)
                }
            }
        }
        // Made on its own server's page, its link pasted here: nothing to ask.
        Way::FastmailLink | Way::Appointments | Way::GoogleSchedule => Ok(share.link.clone()),
    }
}

/// A share taken back on its server, as it was made (`make_share`); a mirror
/// not on its server yet, or a link you pasted, has nothing to take back there.
pub(crate) fn stop_share(config: &Config, availability: &Availability, share: &Share) -> Result<(), String> {
    if crate::backend::offline() || share.made.is_empty() {
        return Ok(());
    }
    let account = config.every_account().find(|a| a.id == share.account).cloned().ok_or_else(|| tr().text("agenda-gone", None))?;
    let Some(way) = share.way() else { return Ok(()) };
    let who = share.who.trim();
    let e = |error: SyncError| said(&error, &account.id);
    let google = || GoogleApi::new(account.address.as_deref().unwrap_or_default()).map_err(e);
    match way {
        Way::GoogleFreeBusy => {
            let api = google()?;
            for calendar in availability.agendas.iter().filter(|p| p.starts_with(&format!("{}/", account.id))).filter_map(|p| google_calendar(p)) {
                api.revoke(&calendar, &format!("user:{who}")).map_err(e)?;
            }
            Ok(())
        }
        Way::GooglePublic => {
            let Some(calendar) = availability.agendas.first().and_then(|p| google_calendar(p)) else { return Ok(()) };
            calendars::google_public_busy(&google()?, &calendar, false).map(|_| ()).map_err(e)
        }
        Way::MirrorShare | Way::MirrorFederated | Way::MirrorLink => {
            let Some(url) = made_on(availability, &account.id) else { return Ok(()) };
            stop_on(&account, &url, way, who)
        }
        Way::FastmailLink | Way::Appointments | Way::GoogleSchedule => Ok(()),
    }
}

/// A mirror's share taken back on the calendar at `url`, as it was made.
fn stop_on(account: &Account, url: &str, way: Way, who: &str) -> Result<(), String> {
    let e = |error: SyncError| said(&error, &account.id);
    if account.auth.as_deref() == Some("google") {
        let api = GoogleApi::new(account.address.as_deref().unwrap_or_default()).map_err(e)?;
        let Some(calendar) = calendars::google_id(url) else { return Ok(()) };
        let rule = if way == Way::MirrorLink { "default".to_string() } else { format!("user:{who}") };
        return api.revoke(&calendar, &rule).map_err(e);
    }
    let client = sioul_sync::dav::client_for(account).map_err(e)?;
    match way {
        Way::MirrorLink => calendars::dav_publish(&client, url, false).map_err(e),
        Way::MirrorFederated => calendars::federated_principal(who).map_or(Ok(()), |p| calendars::dav_unshare(&client, url, &p).map_err(e)),
        _ => {
            let (name, group) = who.strip_prefix("group:").map_or((who, false), |n| (n.trim(), true));
            calendars::principal(name, group).map_or(Ok(()), |p| calendars::dav_unshare(&client, url, &p).map_err(e))
        }
    }
}

/// The shares made on a calendar another availability keeps taken back
/// there (`TakeBack`), once each, unless an availability whose own calendar
/// it is shares it the same way with the same; a refusal tried again a
/// quarter of an hour later.
fn take_back(config: &Config, now: i64) {
    let waiting = with_memo(|m| m.take_back.clone());
    for back in waiting {
        let key = format!("back:{}:{}:{}", back.url, back.way, back.who);
        if !with_memo(|m| m.due(&key, now)) {
            continue;
        }
        let kept_there = config.agenda.availabilities.iter().any(|a| {
            a.shares.iter().any(|s| s.account == back.account && s.way == back.way && s.who.trim().eq_ignore_ascii_case(back.who.trim()) && !s.made.is_empty())
                && own_mirror(config, a, &back.account).and_then(|c| on_server(&c)).is_some_and(|u| u == back.url)
        });
        let done = match (config.every_account().find(|a| a.id == back.account), Way::parse(&back.way)) {
            _ if kept_there => Ok(()),
            (Some(account), Some(way)) => stop_on(account, &back.url, way, back.who.trim()),
            _ => Ok(()),
        };
        with_memo(|m| match done {
            Ok(()) => {
                m.take_back.retain(|b| *b != back);
                m.tried.remove(&key);
            }
            Err(_) => {
                m.tried.insert(key.clone(), now);
            }
        });
    }
}

/// A share added to an availability, here: its mirror made in that account
/// when its way needs one and it has no calendar of its own there (none yet,
/// its calendar gone, or one another availability keeps: `make_mirror`,
/// sent by the next sync), the availability's coarseness and horizon its
/// first audience's when it had no share yet (`availability::defaults`).
/// The share waits for its server (`made` empty); `marks` asks it. Returns
/// the availability as written.
pub(crate) fn add_share(config: &Config, availability: &Availability, choice: &Choice, audience: Audience, link: &str) -> Result<Availability, String> {
    let mut written = availability.clone();
    if written.shares.is_empty() {
        let d = av::defaults(audience);
        (written.rounding, written.horizon, written.closing) = (d.rounding, d.horizon, d.closing);
    }
    let mut share = choice.share(audience);
    share.link = link.trim().to_string();
    if !written.shares.iter().any(|s| s.same(&share)) {
        written.shares.push(share);
    }
    if choice.way.mirrored() && own_mirror(config, &written, &choice.account).is_none() {
        // Made (again) with the share in it, so that the share waits for this calendar.
        return make_mirror(config, &written, &choice.account);
    }
    sioul_core::agendas::put_availability(&crate::backend::config_path(), &written)?;
    Ok(written)
}

/// A share taken out here, once its server took it back: the last one of
/// its availability takes the availability and its mirrors with it (each a
/// dated copy first, as any removal), a mirror no share needs any more too;
/// never a calendar another availability still names as its mirror (settings
/// of before 26 November 2026, two availabilities on one calendar). Returns
/// the availability as it was, for Undo.
pub(crate) fn drop_share(availability: &Availability, share: &Share) -> Result<Availability, String> {
    let path = crate::backend::config_path();
    let config = load_config();
    let named_elsewhere = |mirror: &Mirror| config.agenda.availabilities.iter().any(|a| a.id != availability.id && a.mirrors.iter().any(|m| m.account == mirror.account && m.calendar == mirror.calendar));
    let left: Vec<&Share> = availability.shares.iter().filter(|s| !s.same(share)).collect();
    if left.is_empty() {
        for mirror in availability.mirrors.iter().filter(|m| !named_elsewhere(m)) {
            if let Some(collection) = collection(&mirror.account, &mirror.calendar) {
                vdir::remove_collection(&collection)?;
            }
        }
        sioul_core::agendas::remove_availability(&path, &availability.id)?;
        return Ok(availability.clone());
    }
    sioul_core::agendas::remove_share(&path, &availability.id, share)?;
    // A mirror no share needs any more: gone too.
    for mirror in availability.mirrors.iter().filter(|m| !left.iter().any(|s| s.account == m.account && s.way().is_some_and(Way::mirrored))) {
        if let Some(collection) = collection(&mirror.account, &mirror.calendar).filter(|_| !named_elsewhere(mirror)) {
            vdir::remove_collection(&collection)?;
        }
        sioul_core::agendas::remove_mirror(&path, &availability.id, &mirror.account)?;
    }
    Ok(availability.clone())
}

// ---------------------------------------------------------------- the servers' side, as it waits

/// What waits on the servers, asked now (in a thread: it may take a while):
/// each share waiting for its server's yes, once it can be asked (`ready`);
/// each mirror on its server marked transparent and described once a day;
/// your working hours told to the servers you chose, taken back from the
/// others; each agenda of someone else's availability marked transparent.
/// A refusal is tried again a quarter of an hour later. Nothing is asked
/// while nothing waits. Returns what to say.
pub(crate) fn marks(config: &Config) -> Vec<String> {
    if crate::backend::offline() {
        return demo_marks(config);
    }
    let (now, day) = (now(), today());
    let path = crate::backend::config_path();
    let mut lines = Vec::new();
    for availability in &config.agenda.availabilities {
        for share in availability.shares.iter().filter(|s| s.made.is_empty() && ready(config, availability, s)) {
            let key = format!("share:{}:{}:{}:{}", availability.id, share.way, share.account, share.who);
            if !with_memo(|m| m.due(&key, now)) {
                continue;
            }
            match make_share(config, availability, share) {
                Ok(link) => {
                    let made = Share { made: day.to_string(), link: if link.is_empty() { share.link.clone() } else { link }, ..share.clone() };
                    if sioul_core::agendas::put_share(&path, &availability.id, &made).is_ok() {
                        lines.push(share_made_line(availability, &made));
                    }
                    with_memo(|m| m.tried.remove(&key));
                }
                Err(problem) => {
                    eprintln!("availability: {key}: {problem}");
                    with_memo(|m| m.tried.insert(key.clone(), now));
                }
            }
        }
        for mirror in &availability.mirrors {
            let Some(account) = config.every_account().find(|a| a.id == mirror.account) else { continue };
            if account.auth.as_deref() == Some("google") {
                continue;
            }
            let Some(url) = collection(&mirror.account, &mirror.calendar).and_then(|c| on_server(&c)) else { continue };
            let place = format!("{}/{}", mirror.account, mirror.calendar);
            let described = with_memo(|m| m.described.get(&place).cloned()).is_some_and(|d| d == day.to_string());
            let key = format!("marks:{place}");
            if (mirror.transparent && described) || !with_memo(|m| m.due(&key, now)) {
                continue;
            }
            let Ok(client) = sioul_sync::dav::client_for(account) else { continue };
            let mut refused = false;
            if !mirror.transparent {
                match calendars::set_transparent(&client, &url, true) {
                    Ok(()) => {
                        let _ = sioul_core::agendas::put_mirror(&path, &availability.id, &Mirror { transparent: true, ..mirror.clone() });
                    }
                    Err(_) => refused = true,
                }
            }
            if !described {
                match calendars::set_description(&client, &url, &av::mirror_description(tr(), day)) {
                    Ok(()) => with_memo(|m| {
                        m.described.insert(place.clone(), day.to_string());
                    }),
                    Err(_) => refused = true,
                }
            }
            with_memo(|m| if refused { m.tried.insert(key.clone(), now) } else { m.tried.remove(&key) });
        }
    }
    take_back(config, now);
    working_hours(config, now);
    others_transparent(config, now);
    lines
}

/// The demo profile asks no server: each share waiting is said made, its link a stand-in's.
fn demo_marks(config: &Config) -> Vec<String> {
    let path = crate::backend::config_path();
    let mut lines = Vec::new();
    for availability in &config.agenda.availabilities {
        for share in availability.shares.iter().filter(|s| s.made.is_empty()) {
            let link = make_share(config, availability, share).unwrap_or_default();
            let made = Share { made: today().to_string(), link: if link.is_empty() { share.link.clone() } else { link }, ..share.clone() };
            if sioul_core::agendas::put_share(&path, &availability.id, &made).is_ok() {
                lines.push(share_made_line(availability, &made));
            }
        }
    }
    lines
}

/// "Your busy times are shared with Sam.", "…by a link.", once its server said yes.
pub(crate) fn share_made_line(_availability: &Availability, share: &Share) -> String {
    let who = if share.who.is_empty() { tr().text("availability-by-link", None) } else { share.who.trim_start_matches("group:").to_string() };
    say("availability-shared", &[("who", who)])
}

/// Your working hours told to each server you chose (`[agenda] send_hours`),
/// again when they change; taken back from a server no longer chosen.
fn working_hours(config: &Config, now: i64) {
    let zone = sioul_core::zone::current();
    let name = zone.iana_name().unwrap_or("UTC").to_string();
    let text = av::working_hours(&config.windows, &name, &zone);
    // What was told, as a mark of this device's own (a hash; never shared).
    let hash = text.as_deref().map(|t| {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::hash::DefaultHasher::new();
        t.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }).unwrap_or_default();
    let told = with_memo(|m| m.hours.clone());
    let mut wanted: BTreeMap<String, String> = config.agenda.send_hours.iter().filter(|a| config.accounts.iter().any(|x| x.id == **a && kind_of(x) == ServerKind::Nextcloud)).map(|a| (a.clone(), hash.clone())).collect();
    // Taken back: "" says none told there.
    let gone: Vec<String> = told.keys().filter(|a| !wanted.contains_key(*a)).cloned().collect();
    for account in gone {
        wanted.insert(account, String::new());
    }
    for (account_id, hash) in wanted {
        if told.get(&account_id).is_some_and(|h| *h == hash) || (hash.is_empty() && !told.contains_key(&account_id)) {
            continue;
        }
        let key = format!("hours:{account_id}");
        if !with_memo(|m| m.due(&key, now)) {
            continue;
        }
        let Some(account) = config.accounts.iter().find(|a| a.id == account_id) else {
            with_memo(|m| m.hours.remove(&account_id));
            continue;
        };
        let Some(home) = sioul_sync::dav::Homes::load(&account.id).calendars else { continue };
        let done = sioul_sync::dav::client_for(account).and_then(|client| {
            let inbox = calendars::schedule_inbox(&client, &home)?;
            calendars::set_working_hours(&client, &inbox, if hash.is_empty() { None } else { text.as_deref() })
        });
        with_memo(|m| match done {
            Ok(()) if hash.is_empty() => {
                m.hours.remove(&account_id);
                m.tried.remove(&key);
            }
            Ok(()) => {
                m.hours.insert(account_id.clone(), hash.clone());
                m.tried.remove(&key);
            }
            Err(_) => {
                m.tried.insert(key.clone(), now);
            }
        });
    }
}

/// Each agenda of someone else's availability on a Nextcloud marked
/// transparent there, once: otherwise its server counts it as your own busy
/// time when others invite you (research/availability-sharing.md §5.3).
fn others_transparent(config: &Config, now: i64) {
    let done = with_memo(|m| m.transparent.clone());
    // Never one you said counts as your busy time (`busytime`): its server keeps your word.
    for agenda in sioul_core::agendas::all(config).into_iter().filter(|a| a.whose != sioul_core::agendas::Whose::Yours && a.busy() && !a.busy_time.counts && !done.contains(&a.id)) {
        let Some(account) = config.accounts.iter().find(|x| x.id == agenda.account && kind_of(x) == ServerKind::Nextcloud) else { continue };
        let Some(url) = by_place(&agenda.id).and_then(|c| on_server(&c)) else { continue };
        let key = format!("transparent:{}", agenda.id);
        if !with_memo(|m| m.due(&key, now)) {
            continue;
        }
        let marked = sioul_sync::dav::client_for(account).and_then(|client| calendars::set_transparent(&client, &url, true));
        with_memo(|m| {
            if marked.is_ok() {
                m.transparent.insert(agenda.id.clone());
                m.tried.remove(&key);
            } else {
                m.tried.insert(key.clone(), now);
            }
        });
    }
}

// ---------------------------------------------------------------- what is said, once

/// The single share of before read as an availability, once, at the start
/// (`config::migrate_availability`): its notice kept to say at the next admin window.
pub(crate) fn migrate_at_start() {
    match sioul_core::config::migrate_availability(&crate::backend::config_path(), tr(), today()) {
        Ok(Some(id)) => with_memo(|m| m.moved.push(id)),
        Ok(None) => {}
        Err(e) => eprintln!("availability: {e}"),
    }
}

/// What to say now, once, at an admin window (never as an alert): the
/// migration's word ("Your availability for Sam is now in Agenda ⚙ ▸ Your
/// availability…"), and a mirror last written more than a day ago, once a day.
pub(crate) fn notices(config: &Config, admin: bool) -> Vec<String> {
    if !admin {
        return Vec::new();
    }
    let now = now();
    let day = today().to_string();
    let entries = crate::devices::entries();
    let mut lines = Vec::new();
    with_memo(|m| {
        for id in std::mem::take(&mut m.moved) {
            let Some(availability) = config.agenda.availabilities.iter().find(|a| a.id == id) else { continue };
            if m.said.insert(format!("moved:{id}")) {
                let who = availability.shares.first().map(|s| s.who.clone()).filter(|w| !w.is_empty()).unwrap_or_else(|| tr().text("availability-by-link", None));
                lines.push(say("availability-moved", &[("who", who)]));
            }
        }
        for availability in config.agenda.availabilities.iter().filter(|a| a.shares.iter().any(|s| s.way().is_some_and(Way::mirrored) && !s.made.is_empty())) {
            let Some((at, device)) = last_write(entries.as_ref(), &availability.id) else { continue };
            if now - at > 24 * 3600 && m.said.insert(format!("stale:{}:{day}", availability.id)) {
                let who = availability.shares.iter().map(|s| s.who.clone()).find(|w| !w.is_empty()).unwrap_or_else(|| tr().text("availability-by-link", None));
                lines.push(say("availability-stale", &[("who", who), ("when", when(at, now)), ("device", device)]));
            }
        }
    });
    lines
}

/// The last write of an availability's mirror by any device, and which, in
/// words ("the computer", "the phone"): from the devices' sealed entries.
fn last_write(entries: Option<&(String, Vec<sioul_sync::devices::Entry>)>, id: &str) -> Option<(i64, String)> {
    let (_, all) = entries?;
    all.iter().filter_map(|e| e.mirrors.get(id).map(|at| (*at, e))).max_by_key(|(at, _)| *at).map(|(at, e)| (at, tr().text(match e.form_or_kind() { sioul_sync::devices::PHONE => "availability-device-phone", sioul_sync::devices::TABLET => "availability-device-tablet", _ => "availability-device-computer" }, None)))
}

/// A time as Mail's rows say it: "14:05" today, "yesterday at 18:10", else its date.
pub(crate) fn when(at: i64, now: i64) -> String {
    let zone = sioul_core::zone::current();
    let (Ok(then), Ok(today)) = (jiff::Timestamp::from_second(at).map(|t| t.to_zoned(zone.clone())), jiff::Timestamp::from_second(now).map(|t| t.to_zoned(zone))) else { return String::new() };
    let time = then.strftime("%H:%M").to_string();
    if then.date() == today.date() {
        say("availability-today-at", &[("time", time)])
    } else if today.date().yesterday().is_ok_and(|y| y == then.date()) {
        say("availability-yesterday-at", &[("time", time)])
    } else {
        tr().date(&then, true)
    }
}

/// Who keeps a share up to date, in one sentence (design §3): its server,
/// for a way of its own; else your devices, the phone in the background
/// when one wrote it lately; with when it was last written once that is
/// more than an hour ago.
pub(crate) fn keeper_line(availability: &Availability, share: &Share, entries: Option<&(String, Vec<sioul_sync::devices::Entry>)>, now: i64) -> String {
    if !share.way().is_some_and(Way::mirrored) {
        return tr().text("availability-kept-by-server", None);
    }
    let phone_keeps = entries.is_some_and(|(_, all)| all.iter().any(|e| e.kind == sioul_sync::devices::PHONE && !e.left && e.mirrors.get(&availability.id).is_some_and(|at| now - at < 24 * 3600)));
    let here_phone = cfg!(target_os = "android") && load_config().dnd.background;
    let base = tr().text(if phone_keeps || here_phone { "availability-kept-by-devices" } else { "availability-kept-while-open" }, None);
    match last_write(entries, &availability.id) {
        Some((at, device)) if now - at > 3600 => format!("{base} {}", say("availability-kept-last", &[("when", when(at, now)), ("device", device)])),
        _ => base,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Settings of their own for this test's thread, with one calendars account.
    fn settings(name: &str, account: &str) -> PathBuf {
        sioul_core::config::stand_ins_only();
        let dir = sioul_core::config::data_dir().join(format!("{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let config = dir.join("config.toml");
        std::fs::write(&config, format!("language = \"en\"\n\n[[account]]\nid = \"{account}\"\nkind = \"dav\"\nhost = \"cloud.example.invalid\"\naddress = \"sam@example.invalid\"\n")).unwrap();
        crate::backend::TEST_CONFIG.with(|own| *own.borrow_mut() = Some(config.clone()));
        dir
    }

    /// A calendar as its server has it: sent, its address known, yours.
    fn on_its_server(collection: &Collection) {
        let mut state = State::load(&collection.state_path());
        state.pending = false;
        state.url = format!("https://cloud.example.invalid/remote.php/dav/calendars/sam/{}/", collection.id);
        state.owned = Some(true);
        state.save(&collection.state_path()).unwrap();
    }

    /// A calendar gone from its server, as the sync forgets it (`dav::forget_gone`).
    fn gone(account: &str, id: &str) {
        let collection = collection(account, id).unwrap();
        std::fs::remove_dir_all(&collection.dir).unwrap();
        std::fs::remove_file(collection.state_path()).unwrap();
    }

    fn availability(config: &Config, id: &str) -> Availability {
        config.agenda.availabilities.iter().find(|a| a.id == id).cloned().unwrap()
    }

    /// A mirror whose calendar went (removed on another device, or on its
    /// server) and is shared again: its calendar made again at once, in a
    /// folder of its own, never the one of before; its share of before
    /// waiting again with its link, to be made on the new calendar; its line
    /// says it is being made again until the calendar is on its server. One
    /// gone with nothing shared again: made again by `keep` alone, once seen
    /// gone for ten minutes and its account synced since.
    #[test]
    fn a_mirror_removed_then_shared_again_is_made_again() {
        let account = "mirror-remade";
        let dir = settings("mirror-remade", account);
        let config = load_config();
        let first = Availability::new("By link", vec![format!("{account}/personal")], Audience::Link);
        let link = Choice { way: Way::MirrorLink, account: account.into(), who: String::new(), why: "" };
        let written = add_share(&config, &first, &link, Audience::Link, "").unwrap();
        let folder = av::mirror_folder(&first.id, None);
        assert_eq!(written.mirror(account).map(|m| m.calendar.as_str()), Some(folder.as_str()), "made in its own folder");
        let made = collection(account, &folder).unwrap();
        assert_eq!(made.name, mirror_name(&config, account));
        on_its_server(&made);
        let day = Share { made: "2026-11-25".into(), link: "https://cloud.example.invalid/p/old".into(), ..written.shares[0].clone() };
        sioul_core::agendas::put_share(&crate::backend::config_path(), &first.id, &day).unwrap();
        let config = load_config();
        let shared = availability(&config, &first.id);
        assert!(ready(&config, &shared, &shared.shares[0]) && mirror_line(&config, &shared, &shared.shares[0]).is_none(), "there: nothing more said");
        // Gone, then shared once more with a person.
        gone(account, &folder);
        let remaking = say("availability-share-remaking", &[("server", server_name(&config, account))]);
        assert_eq!(mirror_line(&config, &shared, &shared.shares[0]).as_ref(), Some(&remaking), "said, never silent");
        let sam = Choice { way: Way::MirrorShare, account: account.into(), who: "sam".into(), why: "" };
        let again = add_share(&config, &shared, &sam, Audience::Person, "").unwrap();
        let second = format!("{folder}-2");
        assert_eq!(again.mirror(account).map(|m| m.calendar.as_str()), Some(second.as_str()), "made again, never in the folder of before");
        assert!(collection(account, &second).is_some());
        assert!(again.shares.iter().all(|s| s.made.is_empty()) && again.shares.iter().all(|s| s.link.is_empty()), "every share waits for the new calendar: {:?}", again.shares);
        let config = load_config();
        let kept = availability(&config, &first.id);
        assert_eq!((kept.shares.len(), kept.mirrors.len()), (2, 1));
        assert!(!ready(&config, &kept, &kept.shares[0]), "not on its server yet");
        assert_eq!(mirror_line(&config, &kept, &kept.shares[0]), Some(remaking), "until it is there");
        on_its_server(&collection(account, &second).unwrap());
        assert!(ready(&config, &kept, &kept.shares[0]));
        let now = now();
        let _ = made_again(&config, &kept, now, &|_| true);
        assert!(mirror_line(&config, &kept, &kept.shares[0]).is_none(), "there: said no more");
        // Gone again, nothing shared again: `keep`'s step waits ten minutes, and a sync of its account since.
        gone(account, &second);
        let personal = vdir::create_as(Kind::Calendars, account, "personal", "Personal", None, &["VEVENT"]).unwrap();
        on_its_server(&personal);
        assert_eq!(made_again(&config, &kept, now, &|_| true).mirror(account).map(|m| m.calendar.clone()), Some(second.clone()), "just seen gone: waits");
        assert_eq!(made_again(&config, &kept, now + MISSING_FOR, &|_| true).mirror(account).map(|m| m.calendar.clone()), Some(second.clone()), "its account not synced since: waits");
        let mut state = State::load(&personal.state_path());
        state.pulled = Some(now + 60);
        state.save(&personal.state_path()).unwrap();
        assert_eq!(made_again(&config, &kept, now + MISSING_FOR, &|_| false).mirror(account).map(|m| m.calendar.clone()), Some(second.clone()), "the sharing not caught up since: waits (a removal on another device may still be on its way)");
        let third = made_again(&config, &kept, now + MISSING_FOR, &|_| true);
        assert_eq!(third.mirror(account).map(|m| m.calendar.clone()), Some(format!("{folder}-3")), "made again by keep");
        assert_eq!(availability(&load_config(), &first.id).mirror(account).map(|m| m.calendar.clone()), Some(format!("{folder}-3")), "written");
        crate::backend::TEST_CONFIG.with(|own| *own.borrow_mut() = None);
        for c in vdir::collections(Kind::Calendars).into_iter().filter(|c| c.account == account) {
            let _ = std::fs::remove_dir_all(&c.dir);
            let _ = std::fs::remove_file(c.state_path());
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Two availabilities on one account, both naming one calendar (made by
    /// two devices before 26 November 2026): read without error, the first
    /// keeps it, nothing of the second written there nor shared from it; the
    /// second given its own at its next turn, its share then made there; a
    /// share of either stopped never removes the calendar the other names.
    /// A new availability never reuses another's calendar.
    #[test]
    fn two_availabilities_on_one_account_get_a_calendar_each() {
        let account = "mirror-two";
        let dir = settings("mirror-two", account);
        let path = crate::backend::config_path();
        let calendar = vdir::create_as(Kind::Calendars, account, "old-busy-times", "Old busy times", None, &["VEVENT"]).unwrap();
        on_its_server(&calendar);
        let link = Share { audience: "link".into(), way: "mirror-link".into(), account: account.into(), ..Share::default() };
        let both = |id: &str| Availability { id: id.into(), name: "For the team".into(), agendas: vec![format!("{account}/personal")], mirrors: vec![Mirror { account: account.into(), calendar: "old-busy-times".into(), transparent: false }], shares: vec![link.clone()], ..Availability::default() };
        sioul_core::agendas::put_availability(&path, &both("a1b2c3d4-one")).unwrap();
        sioul_core::agendas::put_availability(&path, &both("e5f6a7b8-two")).unwrap();
        let config = load_config();
        assert_eq!(config.agenda.availabilities.len(), 2, "read without error");
        let (one, two) = (availability(&config, "a1b2c3d4-one"), availability(&config, "e5f6a7b8-two"));
        assert!(own_mirror(&config, &one, account).is_some() && own_mirror(&config, &two, account).is_none(), "the first keeps it");
        assert!(ready(&config, &one, &one.shares[0]) && !ready(&config, &two, &two.shares[0]), "nothing of the second shared from it");
        assert!(mirror_line(&config, &two, &two.shares[0]).is_some());
        // The second's share stopped: its availability goes, the calendar the first names stays.
        drop_share(&two, &two.shares[0]).unwrap();
        assert!(collection(account, "old-busy-times").is_some(), "the first still names it");
        sioul_core::agendas::put_availability(&path, &two).unwrap();
        // The second's turn: its own calendar, the first's left as it was.
        let config = load_config();
        let two = availability(&config, "e5f6a7b8-two");
        let given = made_again(&config, &two, now(), &|_| false);
        assert_eq!(given.mirror(account).map(|m| m.calendar.as_str()), Some("busy-e5f6a7b8"));
        let config = load_config();
        assert_eq!(availability(&config, "a1b2c3d4-one").mirror(account).map(|m| m.calendar.as_str()), Some("old-busy-times"));
        assert_eq!(availability(&config, "e5f6a7b8-two").mirror(account).map(|m| m.calendar.as_str()), Some("busy-e5f6a7b8"));
        assert!(av::borrowed(&config.agenda.availabilities, &availability(&config, "e5f6a7b8-two")).is_empty());
        on_its_server(&collection(account, "busy-e5f6a7b8").unwrap());
        let two = availability(&config, "e5f6a7b8-two");
        assert!(ready(&config, &two, &two.shares[0]), "its share made on its own calendar");
        // A third, new: never one of theirs.
        let third = Availability::new("For Sam", vec![format!("{account}/personal")], Audience::Person);
        let sam = Choice { way: Way::MirrorShare, account: account.into(), who: "sam".into(), why: "" };
        let made = add_share(&config, &third, &sam, Audience::Person, "").unwrap();
        let folder = made.mirror(account).map(|m| m.calendar.clone()).unwrap();
        assert!(folder != "old-busy-times" && folder != "busy-e5f6a7b8", "{folder}");
        crate::backend::TEST_CONFIG.with(|own| *own.borrow_mut() = None);
        for c in vdir::collections(Kind::Calendars).into_iter().filter(|c| c.account == account) {
            let _ = std::fs::remove_dir_all(&c.dir);
            let _ = std::fs::remove_file(c.state_path());
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
