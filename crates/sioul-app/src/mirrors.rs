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
    /// What was tried and refused, by what it is ("share:<id>:<way>:<who>"), and when: tried again a quarter of an hour later.
    #[serde(default)]
    tried: BTreeMap<String, i64>,
    /// The notices said once ("moved:<id>", "stale:<id>:<day>").
    #[serde(default)]
    said: BTreeSet<String>,
    /// The availabilities the migration made here, to say once.
    #[serde(default)]
    moved: Vec<String>,
}

fn memo_path() -> PathBuf {
    sioul_core::config::state_dir().join("availability.toml")
}

impl Memo {
    fn load() -> Memo {
        std::fs::read_to_string(memo_path()).ok().and_then(|text| toml::from_str(&text).ok()).unwrap_or_default()
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
        if std::fs::read_to_string(&path).ok().as_deref() != Some(text.as_str()) && vdir::write_item(&path, text).is_ok() {
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

/// What this device wrote a mirror from, in its sealed entry (`devices::Entry::mirrors`).
fn note_written(id: &str, as_of: i64) {
    let Some((_, vault)) = crate::share::vault() else { return };
    let path = sioul_sync::devices::own_path(&sioul_core::config::state_dir());
    let _ = sioul_sync::devices::change(&path, vault.as_ref().map(|(folder, key)| (folder.as_path(), key)), |entry| {
        entry.mirrors.insert(id.to_string(), as_of);
    });
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
    let tasks = sioul_core::tasks::all(&sioul_core::zone::current());
    let recorded = sioul_core::health::HealthState::errand_uids(&sioul_core::health::HealthState::default_path()).unwrap_or_default();
    let health = av::health_events(&events, &av::errand_tasks(&tasks, &recorded));
    let (busy, not_known) = (tr().text("availability-busy", None), tr().text("availability-not-known", None));
    let words = av::Words { busy: &busy, not_known: &not_known };
    let mut changed: Vec<String> = Vec::new();
    for (availability, view) in fresh {
        let files = av::mirror_files(&events, availability, &words, &health, now);
        let mut wrote = false;
        for mirror in &availability.mirrors {
            let Some(collection) = collection(&mirror.account, &mirror.calendar) else { continue };
            if write_files(&collection, &files) {
                wrote = true;
                changed.push(mirror.account.clone());
            }
        }
        if wrote && let Some(view) = view {
            note_written(&availability.id, av::as_of(view));
        }
    }
    changed.sort();
    changed.dedup();
    changed
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

/// The mirror of an availability in an account, and its address on its server once there.
fn mirror_url(availability: &Availability, account: &str) -> Result<String, String> {
    let mirror = availability.mirror(account).ok_or_else(|| tr().text("availability-mirror-missing", None))?;
    let collection = collection(&mirror.account, &mirror.calendar).ok_or_else(|| tr().text("availability-mirror-missing", None))?;
    on_server(&collection).ok_or_else(|| tr().text("availability-mirror-not-yet", None))
}

/// Whether a share can be asked of its server now: a mirror's once its
/// calendar is there; a link you paste once it is pasted; the rest at once.
pub(crate) fn ready(availability: &Availability, share: &Share) -> bool {
    match share.way() {
        Some(way) if way.mirrored() => mirror_url(availability, &share.account).is_ok(),
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
            let url = mirror_url(availability, &account.id)?;
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
            let Ok(url) = mirror_url(availability, &account.id) else { return Ok(()) };
            if account.auth.as_deref() == Some("google") {
                let api = google()?;
                let Some(calendar) = calendars::google_id(&url) else { return Ok(()) };
                let rule = if way == Way::MirrorLink { "default".to_string() } else { format!("user:{who}") };
                return api.revoke(&calendar, &rule).map_err(e);
            }
            let client = sioul_sync::dav::client_for(&account).map_err(e)?;
            match way {
                Way::MirrorLink => calendars::dav_publish(&client, &url, false).map_err(e),
                Way::MirrorFederated => calendars::federated_principal(who).map_or(Ok(()), |p| calendars::dav_unshare(&client, &url, &p).map_err(e)),
                _ => {
                    let (name, group) = who.strip_prefix("group:").map_or((who, false), |n| (n.trim(), true));
                    calendars::principal(name, group).map_or(Ok(()), |p| calendars::dav_unshare(&client, &url, &p).map_err(e))
                }
            }
        }
        Way::FastmailLink | Way::Appointments | Way::GoogleSchedule => Ok(()),
    }
}

/// A share added to an availability, here: its mirror made in that account
/// when its way needs one and there is none (sent by the next sync), the
/// availability's coarseness and horizon its first audience's when it had
/// no share yet (`availability::defaults`). The share waits for its server
/// (`made` empty); `marks` asks it. Returns the availability as written.
pub(crate) fn add_share(config: &Config, availability: &Availability, choice: &Choice, audience: Audience, link: &str) -> Result<Availability, String> {
    let mut written = availability.clone();
    if written.shares.is_empty() {
        let d = av::defaults(audience);
        (written.rounding, written.horizon, written.closing) = (d.rounding, d.horizon, d.closing);
    }
    if choice.way.mirrored() && written.mirror(&choice.account).is_none() {
        let made = vdir::create(Kind::Calendars, &choice.account, &mirror_name(config, &choice.account), None, &["VEVENT"])?;
        written.mirrors.push(Mirror { account: choice.account.clone(), calendar: made.id.clone(), transparent: false });
    }
    let mut share = choice.share(audience);
    share.link = link.trim().to_string();
    if !written.shares.iter().any(|s| s.same(&share)) {
        written.shares.push(share);
    }
    sioul_core::agendas::put_availability(&crate::backend::config_path(), &written)?;
    Ok(written)
}

/// A share taken out here, once its server took it back: the last one of
/// its availability takes the availability and its mirrors with it (each a
/// dated copy first, as any removal). Returns the availability as it was, for Undo.
pub(crate) fn drop_share(availability: &Availability, share: &Share) -> Result<Availability, String> {
    let path = crate::backend::config_path();
    let left: Vec<&Share> = availability.shares.iter().filter(|s| !s.same(share)).collect();
    if left.is_empty() {
        for mirror in &availability.mirrors {
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
        if let Some(collection) = collection(&mirror.account, &mirror.calendar) {
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
        for share in availability.shares.iter().filter(|s| s.made.is_empty() && ready(availability, s)) {
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
    for agenda in sioul_core::agendas::all(config).into_iter().filter(|a| a.whose != sioul_core::agendas::Whose::Yours && a.busy() && !done.contains(&a.id)) {
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
    all.iter().filter_map(|e| e.mirrors.get(id).map(|at| (*at, e))).max_by_key(|(at, _)| *at).map(|(at, e)| (at, tr().text(if e.kind == sioul_sync::devices::PHONE { "availability-device-phone" } else { "availability-device-computer" }, None)))
}

/// A time as Mail's rows say it: "14:05" today, "yesterday at 18:10", else its date.
fn when(at: i64, now: i64) -> String {
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
