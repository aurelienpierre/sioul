// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Calls screened on a phone, as this device writes and says them
//! (docs/android.md, "Calls"; the rules are `sioul_core::calls`, the
//! decision Java's: android/…/Calls.java).
//!
//! - **The table** Java decides from (`refresh`): written again when what it
//!   is made of changed (the settings, the lists, the do-not-disturb list,
//!   "Let every call through", the pauses and the times, Health), and at
//!   least every ten minutes while something of Sioul's runs: the window's
//!   minute and each do-not-disturb applied (`everywhere::apply`), the
//!   background service's step, Android's daily look for the events'
//!   reminders. Its frames reach four days ahead: Sioul closed that long, the
//!   calls ring.
//! - **Let every call through** (`press`), from the window on any device, or
//!   from the phone's notification (Java keeps that press; it is carried into
//!   the shared switch here, `carry`); said in the status line (`moment`) and
//!   the notification (`step`).
//! - **The log**: on a phone, Java's calls copied into this phone's own log,
//!   which the sharing carries to your other devices (`before_exchange`); on
//!   every device, its own lines taken out after their month.
//! - **The list** of calls declined, on the Porch of every device (`view`):
//!   this phone's own, and every phone's while `[porch] calls` says so (the
//!   default); with the voicemail Free mails linked to its call and played on
//!   demand; Seen on any device, gone from all (`act`).
//! - **A person's calls** of the month, on their sheet (`history`).
//! - **Settings ▸ Calls** (`setup`, `setup_change`).

use crate::backend::{load_config, tr};
use jiff::Zoned;
use serde_json::json;
use sioul_core::calls::{self as rules, Held, Through};
use sioul_core::config::Config;
use sioul_core::everywhere::{self as switches, People, Switch};
use sioul_core::porch::{Senders, Standing};
use sioul_core::attention::{Attention, Person};
use sioul_core::quiet::Overrides;
use sioul_core::reach::{Clock, Who};
use sioul_core::voicemail::{self, Voicemail};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

/// The table is made again at least this often while something runs (seconds), nothing changed.
const REMAKE_EVERY: i64 = 10 * 60;
/// Java's answer about the role, kept this long (seconds): the status line asks it each minute.
const STATE_KEPT: i64 = 30;
/// The voicemails' sounds kept to play, the newest.
const SOUNDS_KEPT: usize = 8;

/// This device's name in the sharing (share/here.toml), made once.
fn here_id() -> String {
    sioul_sync::share::Here::load(&sioul_core::config::state_dir()).id
}

/// Whether this device has calls to screen: a phone. SIOUL_CALLS makes a
/// computer write its table and read its list too, to look at them.
fn phone() -> bool {
    cfg!(target_os = "android") || std::env::var_os("SIOUL_CALLS").is_some()
}

fn path(name: &str) -> PathBuf {
    rules::folder().join(name)
}

// ---------------------------------------------------------------- what Android allows

static STATE: Mutex<Option<(i64, serde_json::Value)>> = Mutex::new(None);

/// What Android allows this phone (Calls.state): {api, available, held,
/// contacts, table, emergency_at}; null elsewhere. Kept half a minute.
fn state() -> serde_json::Value {
    let now = jiff::Timestamp::now().as_second();
    if let Ok(kept) = STATE.lock()
        && let Some((at, state)) = kept.as_ref()
        && (0..STATE_KEPT).contains(&(now - at))
    {
        return state.clone();
    }
    let state = crate::steps::java("calls-state", "{}");
    if let Ok(mut kept) = STATE.lock() {
        *kept = Some((now, state.clone()));
    }
    state
}

/// The role asked or changed: Android's answer read again next time.
fn forget_state() {
    if let Ok(mut kept) = STATE.lock() {
        *kept = None;
    }
}

/// Whether this phone screens calls now: Sioul holds the role.
pub(crate) fn screens_here() -> bool {
    cfg!(target_os = "android") && state()["held"] == true
}

// ---------------------------------------------------------------- the table

/// What the table is made of, as the files' times say it, with the day: the
/// same, nothing to make again before ten minutes.
type Inputs = (jiff::civil::Date, Vec<Option<SystemTime>>);

fn changed(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn inputs(config: &Config, now: &Zoned) -> Inputs {
    let files = [
        crate::backend::config_path(),
        config.safe_senders_path(),
        config.neutral_senders_path(),
        config.restricted_senders_path(),
        config.blocked_senders_path(),
        config.known_senders_path(),
        People::default_path(),
        Switch::default_path(),
        Overrides::default_path(),
        sioul_core::health::Health::default_path(),
        sioul_core::needs::Days::default_path(),
        sioul_core::attention::Slots::default_path(),
        // A focus session: do-not-disturb while it counts, when set so (a layer of the frames).
        sioul_core::timelog::folder().join("running.toml"),
        path(rules::THROUGH_HERE),
    ];
    (now.date(), files.iter().map(|f| changed(f)).collect())
}

/// What was made last in this process: its inputs, when (seconds), what it said.
static MADE: Mutex<Option<(Inputs, i64, String)>> = Mutex::new(None);
static BUSY: Mutex<()> = Mutex::new(());

/// The table written again when what it is made of changed, ten minutes went
/// by, or `now` says so (a press, a number blocked, the role given). On a
/// phone only; never two at once (one asked meanwhile, unforced, is let go:
/// the next look makes it).
pub(crate) fn refresh(now: bool) {
    if !phone() {
        return;
    }
    let busy = if now { Some(BUSY.lock().unwrap_or_else(std::sync::PoisonError::into_inner)) } else { crate::backend::one_at_a_time(&BUSY) };
    let Some(_busy) = busy else { return };
    if cfg!(target_os = "android") && !screens_here() {
        // Not screening: no table to keep; your other devices told so, once.
        let here = here_id();
        if Switch::load(&Switch::default_path()).device.get(&here).is_some_and(|d| rules::part_of(d).screens) {
            carry();
        }
        return;
    }
    let config = load_config();
    let at = Zoned::now();
    let stamp = at.timestamp().as_second();
    let there = path(rules::TABLE).is_file();
    let before = inputs(&config, &at);
    let fresh = MADE.lock().ok().and_then(|made| made.as_ref().map(|(inputs, when, _)| *inputs == before && (0..REMAKE_EVERY).contains(&(stamp - when)))).unwrap_or(false);
    if !now && there && fresh {
        return;
    }
    let switch = carry();
    let table = make(&config, &switch, &at);
    let content = rules::content(&table);
    let same = there && MADE.lock().ok().and_then(|made| made.as_ref().map(|(_, _, said)| *said == content)).unwrap_or(false);
    if !same && let Err(e) = rules::write(&path(rules::TABLE), &table) {
        eprintln!("sioul: calls: {e}");
        return;
    }
    // After carrying, which may have written the switch's file.
    if let Ok(mut made) = MADE.lock() {
        *made = Some((inputs(&config, &at), stamp, content));
    }
}

/// This phone's press on its notification (Java's file) carried into the
/// shared switch once, and whether it screens calls said there, for your
/// other devices: their window offers "Let every call through" then.
fn carry() -> Switch {
    let path = Switch::default_path();
    let here = here_id();
    if !cfg!(target_os = "android") || here.is_empty() {
        return Switch::load(&path);
    }
    let local: Option<Through> = rules::read_json(&rules::folder().join(rules::THROUGH_HERE));
    let screens = screens_here();
    let result = switches::change_own(&path, &here, |switch| {
        let mut changed = local.as_ref().is_some_and(|l| rules::merge_local(switch, &here, l));
        changed |= rules::set_screens(switch, &here, screens);
        if changed {
            named(switch, &here);
        }
        changed
    });
    result.unwrap_or_else(|e| {
        eprintln!("sioul: calls: {e}");
        Switch::load(&path)
    })
}

/// This device's own table in the switch's file, its kind and name set.
fn named(switch: &mut Switch, here: &str) {
    let own = switch.device.entry(here.to_string()).or_default();
    own.kind = crate::devices::kind().to_string();
    own.name = if own.kind == "phone" { String::new() } else { crate::devices::name() };
}

/// The table: who each number is (the Always through list's as rows of their
/// own, never a blocked one), the floors, "Let every call through", and the
/// frames from today's midnight, four days on (from midnight, so that a
/// table made a minute later says the same and is not written again), with
/// the matrix's layers: today's slots of time for you, do-not-disturb.
fn make(config: &Config, switch: &Switch, now: &Zoned) -> rules::Table {
    let senders = Senders::load(config);
    let attention = Attention::of(config);
    let clock = if crate::steps::in_service() {
        // The background service reads no calendar (a phone's battery): the meals
        // are not pushed past the day's events there; the window's next look does it.
        let needs = sioul_core::health::Health::load(&sioul_core::health::Health::default_path()).needs;
        let days = sioul_core::needs::Days::load(&sioul_core::needs::Days::default_path());
        Clock::new(config, Overrides::load(&Overrides::default_path()), needs, days, Vec::new())
    } else {
        Clock::load(config, now)
    };
    let from = now.date().to_zoned(now.time_zone().clone()).unwrap_or_else(|_| now.clone());
    let until = from.checked_add(jiff::Span::new().days(rules::TABLE_DAYS)).unwrap_or_else(|_| now.clone());
    // Always through: its people's numbers, read against who each number is; blocked beats it.
    let people = People::load(&People::default_path());
    let always = rules::always_numbers(&people, senders.region(), &|key| senders.judge_number(key).who);
    rules::table(rules::Made {
        made: now.timestamp().as_millisecond(),
        region: senders.region(),
        numbers: senders.numbers().into_iter().map(|(key, who)| (key, who.id().to_string())).collect(),
        prefixes: senders.prefixes().into_iter().map(|(prefix, who)| (prefix, who.id().to_string())).collect(),
        always,
        through: rules::through_of(switch),
        frames: rules::frames(&attention, &clock, &layers(), &from, &until),
    })
}

/// What holds on top of the times, ahead: today's slots of time for you
/// (`state/slots.toml`), and do-not-disturb from its switch or a focus
/// session, from when, until when (none: until turned off; the table is
/// written again at each change, `everywhere::apply`).
fn layers() -> rules::Layers {
    let since = jiff::Timestamp::now().as_second();
    rules::Layers { slots: crate::hours::slots().slots, dnd: crate::everywhere::gate().map(|until| (since, until)) }
}

// ---------------------------------------------------------------- "Let every call through"

/// "Let every call through" as this device knows it: the shared switch, and
/// on the phone its notification's own press, the later of the two.
pub(crate) fn through_now(switch: &Switch) -> Option<Through> {
    let shared = rules::through_of(switch);
    let local: Option<Through> = if cfg!(target_os = "android") { rules::read_json(&path(rules::THROUGH_HERE)) } else { None };
    match (shared, local) {
        (Some(shared), Some(local)) => Some(if local.pressed > shared.pressed { local } else { shared }),
        (shared, local) => shared.or(local),
    }
}

/// "15:30", "tomorrow at 09:31".
fn words_at(ms: i64, now: &Zoned) -> String {
    jiff::Timestamp::from_millisecond(ms).map(|t| sioul_core::quiet::until_text(tr(), &t.to_zoned(now.time_zone().clone()), now)).unwrap_or_default()
}

/// Pressed in the window, on any device: every call rings for `minutes` (0:
/// until turned off), or calls are screened again; on every device once the
/// sharing carries it. The caller writes the phone's table next (`refresh`,
/// off the window's thread).
pub(crate) fn press(on: bool, minutes: i32) -> Result<(), String> {
    let here = here_id();
    let now = jiff::Timestamp::now().as_millisecond();
    let until = if on && minutes > 0 { now + i64::from(minutes) * 60_000 } else { 0 };
    switches::change_own(&Switch::default_path(), &here, |switch| {
        rules::press(switch, &here, on, until, now);
        named(switch, &here);
        true
    })?;
    Ok(())
}

/// The status line's part (main.qml, CallsApplet.qml): {shown (a phone of
/// yours screens calls), through, line, until_at}.
pub(crate) fn moment() -> serde_json::Value {
    let switch = Switch::load(&Switch::default_path());
    let shown = screens_here() || rules::screens_anywhere(&switch);
    if !shown {
        return json!({ "shown": false, "through": false, "line": "", "until_at": 0 });
    }
    let now = Zoned::now();
    let now_ms = now.timestamp().as_millisecond();
    let through = through_now(&switch).filter(|t| t.holds(now_ms));
    let emergency = if cfg!(target_os = "android") { state()["emergency_at"].as_i64().unwrap_or(0) } else { 0 };
    let line = rules::through_line(tr(), through.as_ref(), emergency, now_ms, &|ms| words_at(ms, &now));
    json!({ "shown": true, "through": through.is_some(), "line": line, "until_at": through.map_or(0, |t| t.until) })
}

/// At each step of the background service: the table made again if need be
/// (at once after a press on the notification), and the notification's words
/// for the calls (StepService): {screening, through, line, hour, off, again};
/// nothing while this phone does not screen calls.
pub(crate) fn step(pressed: bool) -> serde_json::Value {
    if pressed {
        forget_state();
    }
    refresh(pressed);
    if !screens_here() {
        return json!({});
    }
    let said = moment();
    json!({
        "screening": true,
        "through": said["through"],
        "line": said["line"],
        "hour": tr().text("calls-note-hour", None),
        "off": tr().text("calls-note-off", None),
        "again": tr().text("calls-note-again", None),
    })
}

// ---------------------------------------------------------------- the log

/// When this device last took its old lines out (ms, its clock): once a day.
static TRIMMED: Mutex<i64> = Mutex::new(0);
/// Java's file as last copied in this process: its size and time.
static COPIED: Mutex<Option<(u64, Option<SystemTime>)>> = Mutex::new(None);

/// Before an exchange (the window's, a quick one: the background step, a
/// dose's alarm) and before the list is read: on a phone, Java's new calls
/// copied into its own log, which the sharing carries (`calls::carry_into`);
/// on every device, once a day, its own lines taken out after their month,
/// and the others' long after theirs (a phone gone for good never takes its
/// own out), and a phone's older seen file made a line of its seen log. Cheap
/// when nothing changed: Java's file is read again only when it changed.
pub(crate) fn before_exchange() {
    let here = here_id();
    if here.is_empty() {
        return;
    }
    let root = rules::folder();
    let now = jiff::Timestamp::now().as_millisecond();
    let due = TRIMMED.lock().map(|mut last| a_day_since(&mut last, now)).unwrap_or(false);
    if due {
        let done = rules::adopt_older_seen(&root, &here, now).and_then(|()| rules::trim_own(&root, &here, now)).and_then(|_| rules::trim_others(&root, &here, now));
        if let Err(e) = done {
            eprintln!("sioul: calls: {e}");
        }
    }
    if !phone() {
        return;
    }
    let java = path(rules::HELD);
    let Some(stamp) = std::fs::metadata(&java).ok().map(|m| (m.len(), m.modified().ok())) else { return };
    let mut copied = COPIED.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if *copied == Some(stamp) {
        return;
    }
    let region = sioul_core::reach::region(&load_config());
    match rules::carry_into(&rules::own_file(&root, rules::LOG, &here), &rules::read_held(&java), region, now) {
        Ok(_) => *copied = Some(stamp),
        Err(e) => eprintln!("sioul: calls: {e}"),
    }
}

/// Whether a day went by since `last` (ms); when it did, `last` becomes
/// `now`, so that the next comes a day after this one.
fn a_day_since(last: &mut i64, now: i64) -> bool {
    let due = now - *last > 86_400_000;
    if due {
        *last = now;
    }
    due
}

// ---------------------------------------------------------------- the list

/// Who a call's caller is now, as your lists and address books say (a number
/// only in the phone's contacts: neutral); none for a hidden number.
fn who_now(senders: &Senders, held: &Held) -> Option<Who> {
    if held.hidden {
        return None;
    }
    let who = senders.judge_number(&held.key).who;
    Some(if who == Who::Stranger && !held.name.trim().is_empty() { Who::Neutral } else { who })
}

/// The calls a device lists, every phone's of the month (`everywhere`:
/// `[porch] calls`, the default), or this device's own alone.
fn listed(root: &Path, here: &str, everywhere: bool, now_ms: i64) -> Vec<Held> {
    let mut calls = rules::read_logs(root, now_ms);
    if !everywhere {
        calls.retain(|h| h.device == here);
    }
    calls
}

/// Your phones in the sharing, each with its name (a phone's model), for the
/// list's words ("your phone (GS290)"); read again five minutes on at most.
fn phone_names() -> Vec<(String, String)> {
    type Kept = Option<(i64, Vec<(String, String)>)>;
    static KEPT: Mutex<Kept> = Mutex::new(None);
    let now = jiff::Timestamp::now().as_second();
    if let Ok(kept) = KEPT.lock()
        && let Some((at, names)) = kept.as_ref()
        && (0..300).contains(&(now - at))
    {
        return names.clone();
    }
    let names: Vec<(String, String)> = match crate::share::vault() {
        Some((_, Some((folder, key)))) => sioul_sync::devices::all(&folder, &key).0.into_iter().filter(|d| d.kind == sioul_sync::devices::PHONE).map(|d| (d.id, d.name)).collect(),
        _ => Vec::new(),
    };
    if let Ok(mut kept) = KEPT.lock() {
        *kept = Some((now, names.clone()));
    }
    names
}

/// Whether this device has apps for `tel:` and `sms:` links: a phone always;
/// a computer when its system names one (Linux: `xdg-mime`, KDE Connect,
/// which hands them to your phone, or a softphone). Looked up once.
fn link_apps() -> (bool, bool) {
    static FOUND: std::sync::OnceLock<(bool, bool)> = std::sync::OnceLock::new();
    *FOUND.get_or_init(|| {
        if cfg!(target_os = "android") {
            return (true, true);
        }
        let has = |scheme: &str| {
            cfg!(target_os = "linux")
                && std::process::Command::new("xdg-mime")
                    .args(["query", "default", &format!("x-scheme-handler/{scheme}")])
                    .output()
                    .ok()
                    .filter(|out| out.status.success())
                    .is_some_and(|out| !String::from_utf8_lossy(&out.stdout).trim().is_empty())
        };
        (has("tel"), has("sms"))
    })
}

/// The Porch's list of calls declined (CallsSection.qml): {lines, phone (this
/// device is one), links: {tel, sms}}; each line shown at the times its
/// caller's Calls row lets them through (a row that rings at no time at all,
/// in work and admin time: `Attention::listed`), on every device: this
/// phone's own calls, and every phone's while `[porch] calls` says so.
pub(crate) fn view() -> String {
    let empty = || json!({ "lines": [] }).to_string();
    let root = rules::folder();
    // No phone ever shared a call with this computer: nothing to read.
    if !phone() && !root.join(rules::LOG).is_dir() {
        return empty();
    }
    if phone() {
        before_exchange();
    }
    let config = load_config();
    let here = here_id();
    let now = Zoned::now();
    let held = listed(&root, &here, config.porch.calls, now.timestamp().as_millisecond());
    if held.is_empty() {
        return empty();
    }
    let senders = Senders::load(&config);
    let attention = Attention::of(&config);
    let moment = crate::hours::attention_now();
    let always = People::load(&People::default_path());
    let shows = |h: &Held| match who_now(&senders, h) {
        None => attention.listed(Person::Hidden, false, &moment),
        Some(who) => attention.listed(Person::of(who), always.admits_number(&h.key, senders.region()), &moment),
    };
    let name_of = |key: &str| {
        let judged = senders.judge_number(key);
        (!judged.card.trim().is_empty()).then_some(judged.card)
    };
    let mails = voicemails(&config, senders.region());
    let messages: BTreeMap<String, rules::Message> = voicemail::link(&held, &mails)
        .into_iter()
        .map(|(id, i)| (id, rules::Message { text: rules::message_words(tr(), mails[i].seconds), path: mails[i].path.display().to_string(), sound: mails[i].sound }))
        .collect();
    let seen = rules::read_seen(&root);
    let phones = phone_names();
    let words = |device: &str| rules::phone_words(tr(), device, &here, &phones, cfg!(target_os = "android"));
    let lister = rules::Lister { now: &now, tr: tr(), region: senders.region(), name_of: &name_of, shows: &shows, phone: &words };
    let (tel, sms) = link_apps();
    json!({ "lines": rules::lines(&held, &seen, &messages, &lister), "phone": cfg!(target_os = "android"), "links": { "tel": tel, "sms": sms } }).to_string()
}

/// A person's calls of the last month (`numbers`: theirs, as written), for
/// their sheet (`reaches::person`): a sentence each, newest first, from every
/// phone, rang or declined; none where no phone shares its calls, and never a
/// blocked caller's (their calls never leave their phone).
pub(crate) fn history(numbers: &[String]) -> Vec<String> {
    let root = rules::folder();
    if numbers.is_empty() || !root.join(rules::LOG).is_dir() {
        return Vec::new();
    }
    let config = load_config();
    let region = sioul_core::reach::region(&config);
    let keys: std::collections::BTreeSet<String> = numbers.iter().map(|n| sioul_core::phones::key(n.trim().trim_start_matches(sioul_core::porch::TEL), region)).filter(|k| sioul_core::phones::is_whole(k)).collect();
    if keys.is_empty() {
        return Vec::new();
    }
    let here = here_id();
    let now = Zoned::now();
    let calls = rules::read_logs(&root, now.timestamp().as_millisecond());
    let phones = phone_names();
    let words = |device: &str| rules::phone_words(tr(), device, &here, &phones, cfg!(target_os = "android"));
    let (none, all) = (|_: &str| None, |_: &Held| true);
    let lister = rules::Lister { now: &now, tr: tr(), region, name_of: &none, shows: &all, phone: &words };
    rules::history(&calls, &keys, &lister)
}

/// The voicemails Free mailed lately, from every account's inbox: each folder
/// listed again only when it changed (a letter came, or was read), each
/// letter read once (by its file's name); never the sound itself, kept in memory.
fn voicemails(config: &Config, region: Option<&'static sioul_core::phones::Region>) -> Vec<Voicemail> {
    type Folder = (Option<SystemTime>, Vec<Voicemail>);
    static FOLDERS: Mutex<BTreeMap<PathBuf, Folder>> = Mutex::new(BTreeMap::new());
    static LETTERS: Mutex<BTreeMap<String, Option<Voicemail>>> = Mutex::new(BTreeMap::new());
    let zone = jiff::tz::TimeZone::system();
    let oldest = SystemTime::now().checked_sub(Duration::from_secs(u64::try_from((rules::LISTED_DAYS + 1) * 86_400).unwrap_or(0))).unwrap_or(SystemTime::UNIX_EPOCH);
    let mut folders = FOLDERS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut letters = LETTERS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut found = Vec::new();
    for source in config.mail_sources() {
        for folder in [source.folder.join("new"), source.folder.join("cur")] {
            let stamp = changed(&folder);
            if let Some((known, kept)) = folders.get(&folder)
                && *known == stamp
                && stamp.is_some()
            {
                found.extend(kept.iter().cloned());
                continue;
            }
            let mut here = Vec::new();
            if let Ok(entries) = std::fs::read_dir(&folder) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with('.') || entry.metadata().ok().filter(|m| m.is_file()).and_then(|m| m.modified().ok()).is_none_or(|m| m < oldest) {
                        continue;
                    }
                    let file = entry.path();
                    let unique = format!("{}/{}", source.account.as_deref().unwrap_or(""), sioul_core::maildir::unique_part(&name));
                    let read = letters.entry(unique).or_insert_with(|| read_voicemail(&file, &source.trusted_ids, region, &zone));
                    if let Some(v) = read {
                        here.push(Voicemail { path: file.clone(), ..v.clone() });
                    }
                }
            }
            found.extend(here.iter().cloned());
            folders.insert(folder, (stamp, here));
        }
    }
    // Letters older than the list forgotten.
    let kept: std::collections::BTreeSet<PathBuf> = found.iter().map(|v| v.path.clone()).collect();
    if letters.len() > 4096 {
        letters.retain(|_, v| v.as_ref().is_some_and(|v| kept.contains(&v.path)));
    }
    found
}

fn read_voicemail(file: &Path, trusted: &[String], region: Option<&'static sioul_core::phones::Region>, zone: &jiff::tz::TimeZone) -> Option<Voicemail> {
    use std::io::Read;
    // The head first: most letters are not Free's, and some are large.
    let mut head = Vec::with_capacity(16 * 1024);
    std::fs::File::open(file).ok()?.take(16 * 1024).read_to_end(&mut head).ok()?;
    if !voicemail::may_be(&head) {
        return None;
    }
    let raw = std::fs::read(file).ok()?;
    voicemail::read(&raw, file, region, zone, trusted)
}

/// A voicemail's sound written where the player reads it: its address
/// ("file://…"). Only one found among your mail: never another file.
fn listen(mail: &str, sound: u32) -> Result<String, String> {
    let config = load_config();
    let file = PathBuf::from(mail);
    let region = sioul_core::reach::region(&config);
    if !voicemails(&config, region).iter().any(|v| v.path == file && v.sound == Some(sound)) {
        return Err(tr().text("calls-listen-gone", None));
    }
    let (_, bytes) = sioul_core::reading::attachment(&file, sound).ok_or_else(|| tr().text("calls-listen-gone", None))?;
    let dir = sioul_core::config::cache_dir().join(rules::FOLDER);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    // A name of its own per message: a player keeps what it read at an address.
    let name = format!("{}-{sound}.wav", sioul_core::maildir::unique_part(&file.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()).replace(['/', '\\', ':'], "_"));
    let out = dir.join(name);
    std::fs::write(&out, bytes).map_err(|e| e.to_string())?;
    // The older sounds go: those of the newest messages stay.
    if let Ok(entries) = std::fs::read_dir(&dir) {
        let mut sounds: Vec<(SystemTime, PathBuf)> = entries.flatten().filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path()))).collect();
        sounds.sort();
        let extra = sounds.len().saturating_sub(SOUNDS_KEPT);
        for (_, old) in sounds.into_iter().take(extra) {
            let _ = std::fs::remove_file(old);
        }
    }
    Ok(format!("file://{}", out.display()))
}

/// An action of the list (CallsSection.qml): "seen" {ids}, "unseen" {at}
/// (Seen undone, within its ten seconds), "add-contact" {number} (Android's
/// form; a computer opens Sioul's own), "block" {number}, "listen" {path,
/// sound}, "open-blocked". Answers {said, url, shared, at} (`shared`: what
/// travels changed, to share now; `at`: the Seen to undo).
pub(crate) fn act(verb: &str, json: &str) -> String {
    let asked: serde_json::Value = serde_json::from_str(json).unwrap_or_default();
    let text = |key: &str| asked[key].as_str().unwrap_or_default().trim().to_string();
    let mut at = 0;
    let (said, url, shared) = match verb {
        // In this device's own seen log, which the sharing carries: gone from every device's Porch.
        "seen" => {
            let ids: Vec<String> = asked["ids"].as_array().map(|a| a.iter().filter_map(|i| i.as_str().map(str::to_string)).collect()).unwrap_or_default();
            match rules::mark_seen(&rules::folder(), &here_id(), &ids, jiff::Timestamp::now().as_millisecond()) {
                Ok(pressed) => {
                    at = pressed;
                    (tr().text("calls-seen-said", None), String::new(), true)
                }
                Err(e) => (e, String::new(), false),
            }
        }
        // This device's own Seen undone; another device's stays.
        "unseen" => match rules::unmark_seen(&rules::folder(), &here_id(), asked["at"].as_i64().unwrap_or(0)) {
            Ok(gone) => (String::new(), String::new(), gone),
            Err(e) => (e, String::new(), false),
        },
        // Android's own form for a new contact, the number filled in: saved by you, or not.
        "add-contact" => {
            crate::steps::java("add-contact", &json!({ "name": "", "phones": [text("number")], "emails": [] }).to_string());
            (String::new(), String::new(), false)
        }
        // On your blocked list, as a number (`tel:`): refused whenever they call, on every device's list.
        "block" => {
            let number = text("number");
            if number.is_empty() {
                (String::new(), String::new(), false)
            } else {
                // The table written again by the caller, off the window's thread.
                match sioul_core::porch::set_standing(&load_config(), &format!("{}{number}", sioul_core::porch::TEL), Standing::Blocked) {
                    Ok(()) => (tr().text("calls-blocked-said", None), String::new(), true),
                    Err(e) => (e, String::new(), false),
                }
            }
        }
        "listen" => match listen(&text("path"), u32::try_from(asked["sound"].as_u64().unwrap_or(0)).unwrap_or(0)) {
            Ok(url) => (String::new(), url, false),
            Err(e) => (e, String::new(), false),
        },
        "open-blocked" => {
            crate::steps::java("calls-open-blocked", "{}");
            (String::new(), String::new(), false)
        }
        _ => (String::new(), String::new(), false),
    };
    json!({ "said": said, "url": url, "shared": shared, "at": at }).to_string()
}

// ---------------------------------------------------------------- Settings ▸ Calls

/// Settings ▸ Calls (CallsSetup.qml): {android, state, moment}; the table
/// made at once when the role was just given. Who rings when is What
/// reaches you's (its Calls rows, Always through).
pub(crate) fn setup() -> String {
    forget_state();
    let state = state();
    if state["held"] == true && !path(rules::TABLE).is_file() {
        std::thread::spawn(|| refresh(true));
    }
    json!({
        "android": cfg!(target_os = "android"),
        "state": state,
        "moment": moment(),
    })
    .to_string()
}

/// An action of Settings ▸ Calls: "ask-role" (Android's question), "open-roles"
/// (its default apps, to change or stop), "allow-contacts", "open-blocked",
/// "dial" {number} (the phone app with a code typed in, never called). The tab again.
pub(crate) fn setup_change(verb: &str, json: &str) -> String {
    let asked: serde_json::Value = serde_json::from_str(json).unwrap_or_default();
    match verb {
        "ask-role" => {
            crate::steps::java("calls-ask-role", "{}");
        }
        "open-roles" => {
            crate::steps::java("calls-open-roles", "{}");
        }
        "allow-contacts" => crate::steps::ask_contacts(),
        "open-blocked" => {
            crate::steps::java("calls-open-blocked", "{}");
        }
        // Only the codes that read where calls go: Sioul dials nothing else.
        "dial" => {
            let number = asked["number"].as_str().unwrap_or_default();
            if ["*#61#", "*#62#", "*#67#"].contains(&number) {
                crate::steps::java("calls-dial", &json!({ "number": number }).to_string());
            }
        }
        _ => {}
    }
    setup()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// On a computer, without SIOUL_CALLS and no phone's log here: nothing
    /// listed, no table written, nothing asked of Java; nothing read but
    /// whether the phones' logs are there.
    #[test]
    fn nothing_is_written_nor_asked_on_a_computer() {
        if phone() || rules::folder().join(rules::LOG).is_dir() {
            return;
        }
        refresh(true);
        assert_eq!(view(), json!({ "lines": [] }).to_string());
        assert!(history(&["+33199001234".to_string()]).is_empty());
        assert_eq!(step(false), json!({}));
        assert_eq!(state(), serde_json::Value::Null);
        assert!(!screens_here());
        assert!(!path(rules::TABLE).exists());
    }

    /// A device lists every phone's calls while `[porch] calls` says so (the
    /// default); else only its own: a phone its own, a computer none.
    #[test]
    fn a_device_lists_the_phones_calls_as_the_setting_says() {
        let root = std::env::temp_dir().join(format!("sioul-app-calls-listed-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let now = jiff::Timestamp::now().as_millisecond();
        let call = |at: i64, key: &str| Held { at, key: key.into(), number: key.into(), who: "stranger".into(), column: "sleep".into(), why: "matrix".into(), ..Held::default() };
        rules::carry_into(&rules::own_file(&root, rules::LOG, "phone-a"), &[call(now - 3_600_000, "+33199001234")], None, now).unwrap();
        rules::carry_into(&rules::own_file(&root, rules::LOG, "phone-b"), &[call(now - 1_800_000, "+33465710042")], None, now).unwrap();
        assert_eq!(listed(&root, "desk", true, now).iter().map(|h| h.device.as_str()).collect::<Vec<_>>(), ["phone-a", "phone-b"]);
        assert!(listed(&root, "desk", false, now).is_empty(), "the setting off: a computer lists none");
        assert_eq!(listed(&root, "phone-a", false, now).len(), 1, "a phone its own");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Old lines taken out once a day, whatever the exchanges in between.
    #[test]
    fn once_a_day() {
        let mut last = 0;
        let start = 1_791_346_200_000;
        assert!(a_day_since(&mut last, start), "the first time");
        assert!(!a_day_since(&mut last, start + 60_000) && !a_day_since(&mut last, start + 23 * 3_600_000), "not again the same day");
        assert!(a_day_since(&mut last, start + 86_400_001), "a day after the last");
        assert!(!a_day_since(&mut last, start + 86_400_001 + 60_000));
    }
}
