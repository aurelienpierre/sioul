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
//! - **The list** of calls declined, on the phone's Porch (`view`), with the
//!   voicemail Free mails linked to its call and played on demand (`act`).
//! - **Settings ▸ Calls** (`setup`, `setup_change`).

use crate::backend::{load_config, tr};
use jiff::Zoned;
use serde_json::json;
use sioul_core::calls::{self as rules, Held, Seen, Through};
use sioul_core::config::Config;
use sioul_core::everywhere::{self as switches, People, Switch};
use sioul_core::porch::{Senders, Standing};
use sioul_core::quiet::Overrides;
use sioul_core::reach::{Channel, Clock, Moment, Reach, Row, Who};
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
fn screens_here() -> bool {
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

/// The table: who each number is, the floors, "Let every call through", and
/// the frames from today's midnight, four days on (from midnight, so that a
/// table made a minute later says the same and is not written again).
fn make(config: &Config, switch: &Switch, now: &Zoned) -> rules::Table {
    let senders = Senders::load(config);
    let reach = Reach::load(config);
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
    // The do-not-disturb list lets its people through only while its setting says so.
    let people = if config.dnd.people { People::load(&People::default_path()) } else { People::default() };
    rules::table(rules::Made {
        made: now.timestamp().as_millisecond(),
        region: senders.region(),
        numbers: senders.numbers().into_iter().map(|(key, who)| (key, who.id().to_string())).collect(),
        prefixes: senders.prefixes().into_iter().map(|(prefix, who)| (prefix, who.id().to_string())).collect(),
        people: &people,
        through: rules::through_of(switch),
        frames: rules::frames(&reach, &clock, &from, &until),
    })
}

// ---------------------------------------------------------------- "Let every call through"

/// "Let every call through" as this device knows it: the shared switch, and
/// on the phone its notification's own press, the later of the two.
fn through_now(switch: &Switch) -> Option<Through> {
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

/// The Porch's list of calls declined (CallsSection.qml): {lines}; each line
/// shown at the times its caller may reach you, by phone or in writing.
pub(crate) fn view() -> String {
    if !phone() {
        return json!({ "lines": [] }).to_string();
    }
    let held = rules::read_held(&path(rules::HELD));
    if held.is_empty() {
        return json!({ "lines": [] }).to_string();
    }
    let config = load_config();
    let now = Zoned::now();
    let senders = Senders::load(&config);
    let reach = Reach::load(&config);
    let overrides = Overrides::load(&Overrides::default_path());
    let moment = Moment::of(&crate::hours::mode_at(&now), sioul_core::pause::nothing_now(&overrides, &config.free_time));
    let shows = |h: &Held| match who_now(&senders, h) {
        None => reach.allows_row(Channel::Calls, Row::Hidden, &moment) || reach.allows(Channel::Mail, Who::Stranger, &moment),
        Some(who) => reach.allows(Channel::Calls, who, &moment) || reach.allows(Channel::Mail, who, &moment),
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
    let seen = Seen::load(&path(rules::SEEN));
    let lister = rules::Lister { now: &now, tr: tr(), region: senders.region(), name_of: &name_of, shows: &shows };
    json!({ "lines": rules::lines(&held, &seen, &messages, &lister) }).to_string()
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

/// An action of the list (CallsSection.qml): "seen" {ids}, "add-contact"
/// {number}, "block" {number}, "listen" {path, sound}, "open-blocked".
/// Answers {said, url, shared} (`shared`: the lists changed, to share).
pub(crate) fn act(verb: &str, json: &str) -> String {
    let asked: serde_json::Value = serde_json::from_str(json).unwrap_or_default();
    let text = |key: &str| asked[key].as_str().unwrap_or_default().trim().to_string();
    let (said, url, shared) = match verb {
        "seen" => {
            let ids: Vec<String> = asked["ids"].as_array().map(|a| a.iter().filter_map(|i| i.as_str().map(str::to_string)).collect()).unwrap_or_default();
            let file = path(rules::SEEN);
            let mut seen = Seen::load(&file);
            seen.add(&ids, jiff::Timestamp::now().as_millisecond());
            (seen.save(&file).err().unwrap_or_default(), String::new(), false)
        }
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
    json!({ "said": said, "url": url, "shared": shared }).to_string()
}

// ---------------------------------------------------------------- Settings ▸ Calls

/// Settings ▸ Calls (CallsSetup.qml): {android, state, moment, people,
/// dnd_people}; the table made at once when the role was just given.
pub(crate) fn setup() -> String {
    forget_state();
    let state = state();
    if state["held"] == true && !path(rules::TABLE).is_file() {
        std::thread::spawn(|| refresh(true));
    }
    let config = load_config();
    json!({
        "android": cfg!(target_os = "android"),
        "state": state,
        "moment": moment(),
        "people": People::load(&People::default_path()).people.len(),
        "dnd_people": config.dnd.people,
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

    /// On a computer, without SIOUL_CALLS: nothing read or written (the
    /// calls' folder is never looked at), nothing asked of Java.
    #[test]
    fn nothing_is_written_nor_asked_on_a_computer() {
        if phone() {
            return;
        }
        refresh(true);
        assert_eq!(view(), json!({ "lines": [] }).to_string());
        assert_eq!(step(false), json!({}));
        assert_eq!(state(), serde_json::Value::Null);
        assert!(!screens_here());
    }
}
