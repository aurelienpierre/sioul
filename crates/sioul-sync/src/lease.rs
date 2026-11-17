// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! One computer at a time for a part of Sioul's work (docs/database.md,
//! "Leases"): reminding medicines, numbering invoices. Two computers doing
//! either at once is a harm: a dose reminded again after it was taken, two
//! invoices with one number.
//!
//! Through the sharing folder: each computer writes its own claim on a part,
//! sealed like the records (`leases/<part>/<computer>.lease`), one writer per
//! file so the sync never makes conflicted copies; renewed each minute while
//! Sioul runs, alive five minutes after its last renewal, taken out when Sioul
//! quits, or is put away on a phone: then it says so (`close`) with how far
//! it wrote its records, so the others know it marks nothing until it is
//! back (docs/health.md, "Knowing"). Every computer reads the same claims the same way:
//! - a part that follows you (medicines) is kept by the live claim used most
//!   recently: the computer you are at;
//! - a part that stays put (invoices) is kept by the live claim taken last,
//!   on purpose ("make invoices here"), else the oldest.
//!
//! A computer acts on a part only once it has kept it for the settling time:
//! the others' claims have had time to come through the sync. A sync that
//! stopped can still hide a claim; what must never happen twice also wants
//! the others heard from lately (`Keeper::others_heard`).
//!
//! What a thing is for chooses among them (docs/attention.md, "Which device
//! tells"): the notices' claims are read per area, with what the person said
//! each device is for (`Dispatch`, `told_by`, `keeper_for`). Among the
//! devices a thing comes forward on now, the one used last tells it; no new
//! file, the same claims. A device whose Sioul does not follow what devices
//! are for tells by the rule before, the one used last of all.

use serde::{Deserialize, Serialize};
use sioul_core::areas::{Area, Here, Line, Time, Week};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// How long a claim lives after its last renewal.
pub const ALIVE: i64 = 5 * 60;
/// How long a computer keeps a part before acting on it.
pub const SETTLING: i64 = 90;
/// How often the window renews its claims (each minute): a claim not
/// renewed for two of these is no running window's.
pub const RENEWED_EVERY: i64 = 60;
/// How long a claim on a part that stays put (invoices) still holds it once
/// lapsed, unless it says it closed: a computer whose sync stopped, or whose
/// claims a phone's sync app brings half an hour late, may still be numbering
/// (review of 5 October 2026, F7); one gone without closing (a crash, a
/// computer given away) holds it a day, then no more.
pub const UNCLOSED: i64 = 24 * 3600;

/// Who keeps a part, as the claims say.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Rule {
    /// The computer used most recently: medicines' reminders.
    #[default]
    FollowsYou,
    /// The one that took it last, on purpose: invoices' numbers.
    StaysPut,
}

/// A computer's claim on a part, in seconds.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claim {
    pub computer: String,
    /// Its name, to say where: the host's.
    pub name: String,
    /// Since when it claims, without a break.
    pub since: i64,
    pub renewed: i64,
    pub until: i64,
    /// You were last at it then.
    pub active: i64,
    /// Taken on purpose then; 0 never.
    pub taken: i64,
    /// How far it had written its records when it renewed: its round and the
    /// number of the last (`share::written`). Read that far, everything it
    /// said until then is known; none from a Sioul that does not say.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wrote: Option<(u32, u64)>,
    /// For invoices: each series' last number this computer knows
    /// (`sioul_core::invoice::series::Journal::lasts`), so that a computer
    /// whose records have not come yet still never gives one of them again;
    /// none from a Sioul that does not say, nor for another part.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub series: BTreeMap<String, u64>,
    /// It closed then (quit, or put away): it marks nothing until it says otherwise.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub closed: bool,
    /// The file's size changed at each writing, so that every sync app brings
    /// it (`share::pad_for`): read by nobody.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub pad: String,
}

/// Who keeps a part now.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Keeper {
    pub computer: String,
    pub name: String,
    /// This computer keeps it.
    pub mine: bool,
    /// And has kept it long enough to act.
    pub settled: bool,
    /// The last renewal heard from another live computer; 0 when none is
    /// alive. For a part that stays put: the renewal of the one heard from
    /// least lately among those still holding it, lapsed but not closed too
    /// (`UNCLOSED`).
    pub others_heard: i64,
    /// How many other computers claim it now (for a part that stays put,
    /// those lapsed but not closed too).
    pub others: usize,
    /// Since when this keeper has kept it, as seen by this process (Unix
    /// seconds): alone, since this process began (`started`), never "always"
    /// (the sixth review's R2). A dose's first reminder is this computer's
    /// only if it kept the doses at the dose's time (docs/health.md, "What
    /// became of a reminder").
    pub since: i64,
}

/// When this process began, as the lease first asked (Unix seconds).
pub fn started() -> i64 {
    static STARTED: std::sync::OnceLock<i64> = std::sync::OnceLock::new();
    *STARTED.get_or_init(|| std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64))
}

impl Keeper {
    /// The only computer: no sharing, or nobody else claims.
    pub fn alone(computer: &str) -> Keeper {
        Keeper { computer: computer.to_string(), name: host_name(), mine: true, settled: true, others_heard: 0, others: 0, since: started() }
    }
}

/// This computer's name, as people call it: its host name, asked once. On a
/// Mac no file holds it and a program tells it: started at each claim (three
/// a minute), it also held for a moment a copy of every file Sioul had open,
/// the sharing's lock among them (`share::exchange_lock`).
pub fn host_name() -> String {
    static NAME: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    NAME.get_or_init(|| {
        let named = std::env::var("COMPUTERNAME").ok().or_else(|| std::fs::read_to_string("/etc/hostname").ok()).or_else(|| crate::command("hostname").output().ok().map(|o| String::from_utf8_lossy(&o.stdout).to_string()));
        named.map(|n| n.trim().to_string()).filter(|n| !n.is_empty()).unwrap_or_else(|| "?".into())
    })
    .clone()
}

fn folder_of(folder: &Path, part: &str) -> PathBuf {
    folder.join("leases").join(part)
}

fn bound(part: &str, computer: &str) -> String {
    format!("lease{}{part}{}{computer}", '\u{1f}', '\u{1f}')
}

/// Every claim on `part` that reads, live or not.
pub fn claims(folder: &Path, key: &[u8; 32], part: &str) -> Vec<Claim> {
    read(folder, key, part)
}

/// The claims on `part`: of one fetched from the server too
/// (`remote::overlay`), the later renewed of the two copies; the folder's on a tie.
fn read(folder: &Path, key: &[u8; 32], part: &str) -> Vec<Claim> {
    let mut by: BTreeMap<String, Claim> = BTreeMap::new();
    for dir in std::iter::once(folder.to_path_buf()).chain(crate::remote::overlay(folder)) {
        for claim in read_in(&dir, key, part) {
            match by.get(&claim.computer) {
                Some(kept) if (kept.renewed, kept.until) >= (claim.renewed, claim.until) => {}
                _ => {
                    by.insert(claim.computer.clone(), claim);
                }
            }
        }
    }
    by.into_values().collect()
}

fn read_in(folder: &Path, key: &[u8; 32], part: &str) -> Vec<Claim> {
    let Ok(entries) = std::fs::read_dir(folder_of(folder, part)) else { return Vec::new() };
    entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "lease"))
        .filter_map(|path| {
            let computer = path.file_stem()?.to_string_lossy().to_string();
            // A claim is a few hundred bytes: a larger file is no claim, never read whole.
            let text = crate::share::read_small(&path)?;
            let plain = crate::share::open(key, &bound(part, &computer), text.trim())?;
            let claim: Claim = serde_json::from_slice(&plain).ok()?;
            // A claim names the computer whose file it is, or it is no claim.
            (claim.computer == computer).then_some(claim)
        })
        .collect()
}

/// Whether a claim holds its part now: alive; for a part that stays put,
/// lapsed but not closed, within `UNCLOSED` of its last renewal too.
fn holds(claim: &Claim, rule: Rule, now: i64) -> bool {
    claim.until > now || (rule == Rule::StaysPut && !claim.closed && now - claim.renewed <= UNCLOSED)
}

/// Another computer's claim on a part that stays put (invoices), closed
/// with records it wrote not all read here yet (`read`: whether everything
/// a computer wrote up to its round and number is read here,
/// `share::Heard::complete`): what it numbered last may be in them, and
/// numbering here before they come could give one number twice. A pull
/// brings a claim ahead of a record it could not fetch (a round the server
/// holds locked: review of 3273f2d, R2), and a phone's sync app may too. A
/// day after its last renewal it no longer counts (`UNCLOSED`): records not
/// come by then were lost, never read.
pub fn closed_unread<'a>(claims: &'a [Claim], computer: &str, now: i64, read: impl Fn(&str, (u32, u64)) -> bool) -> Option<&'a Claim> {
    claims.iter().find(|c| c.computer != computer && c.closed && now - c.renewed <= UNCLOSED && c.wrote.is_some_and(|wrote| !read(&c.computer, wrote)))
}

/// The keeper among the claims that hold the part (`holds`), by the part's
/// rule; ties go to the smaller id.
fn choose<'a>(claims: &'a [Claim], rule: Rule, now: i64) -> Option<&'a Claim> {
    let live = claims.iter().filter(|c| holds(c, rule, now));
    match rule {
        Rule::FollowsYou => live.max_by(|a, b| a.active.cmp(&b.active).then_with(|| b.computer.cmp(&a.computer))),
        // The last taken on purpose; none taken: the oldest claim.
        Rule::StaysPut => live.max_by(|a, b| a.taken.cmp(&b.taken).then_with(|| b.since.cmp(&a.since)).then_with(|| b.computer.cmp(&a.computer))),
    }
}

/// Since when this computer keeps each part, as seen here: (part → (keeper, since)).
static KEPT: Mutex<BTreeMap<String, (String, i64)>> = Mutex::new(BTreeMap::new());

/// This computer's claim on `part`, renewed (made the first time): `active`
/// is when you were last at this computer, `take` takes the part on purpose,
/// `wrote` how far it wrote its records. Returns who keeps it now.
#[allow(clippy::too_many_arguments)]
pub fn renew(folder: &Path, key: &[u8; 32], part: &str, computer: &str, now: i64, active: i64, take: bool, rule: Rule, wrote: Option<(u32, u64)>) -> Result<Keeper, String> {
    renew_with(folder, key, part, computer, Renewal { now, active, take, rule, wrote, series: None })
}

/// What a renewal says, beside the part and the computer.
#[derive(Debug, Clone, Default)]
pub struct Renewal {
    pub now: i64,
    pub active: i64,
    pub take: bool,
    pub rule: Rule,
    pub wrote: Option<(u32, u64)>,
    /// For invoices: each series' last number known here; None keeps what the claim said.
    pub series: Option<BTreeMap<String, u64>>,
}

/// `renew`, with each series' last number for invoices: the claim carries
/// the larger of what it said and what is known now, never less.
pub fn renew_with(folder: &Path, key: &[u8; 32], part: &str, computer: &str, renewal: Renewal) -> Result<Keeper, String> {
    let Renewal { now, active, take, rule, wrote, series } = renewal;
    let mut claims = read(folder, key, part);
    let old = claims.iter().find(|c| c.computer == computer).cloned();
    // A claim that lapsed, or closed, starts again: its age counts from now.
    let since = old.as_ref().filter(|c| c.until > now && !c.closed).map_or(now, |c| c.since);
    let taken = if take { now } else { old.as_ref().map_or(0, |c| c.taken) };
    let mut kept = old.as_ref().map(|c| c.series.clone()).unwrap_or_default();
    for (sequence, last) in series.unwrap_or_default() {
        let at = kept.entry(sequence).or_insert(0);
        *at = (*at).max(last);
    }
    let claim = Claim { computer: computer.to_string(), name: host_name(), since, renewed: now, until: now + ALIVE, active: active.max(old.as_ref().map_or(0, |c| c.active)), taken, wrote, series: kept, closed: false, pad: String::new() };
    write(folder, key, part, computer, &claim)?;
    claims.retain(|c| c.computer != computer);
    claims.push(claim);
    Ok(keeper(&claims, part, computer, rule, now))
}

/// A claim sealed into its file, its size changed from the one there (`share::pad_for`).
fn write(folder: &Path, key: &[u8; 32], part: &str, computer: &str, claim: &Claim) -> Result<(), String> {
    let path = folder_of(folder, part).join(format!("{computer}.lease"));
    let sealed = |pad: usize| serde_json::to_vec(&Claim { pad: ".".repeat(pad), ..claim.clone() }).map(|plain| crate::share::seal(key, &bound(part, computer), &plain)).map_err(|e| e.to_string());
    let pad = crate::share::pad_for(&path, |pad| sealed(pad).map_or(0, |s| s.len() as u64));
    crate::share::write_atomically(&path, sealed(pad)?.as_bytes())
}

/// Each series' largest last number among every computer's claim on
/// `part`, live, lapsed or closed: a number one of them gave is never given
/// again, whichever keeps the part now.
pub fn claimed_series(claims: &[Claim]) -> BTreeMap<String, u64> {
    let mut out: BTreeMap<String, u64> = BTreeMap::new();
    for (sequence, last) in claims.iter().flat_map(|c| c.series.iter()) {
        let at = out.entry(sequence.clone()).or_insert(0);
        *at = (*at).max(*last);
    }
    out
}

/// Who keeps `part`, as read now, without claiming it.
pub fn look(folder: &Path, key: &[u8; 32], part: &str, computer: &str, now: i64, rule: Rule) -> Keeper {
    keeper(&read(folder, key, part), part, computer, rule, now)
}

fn keeper(claims: &[Claim], part: &str, computer: &str, rule: Rule, now: i64) -> Keeper {
    let others: Vec<&Claim> = claims.iter().filter(|c| c.computer != computer && holds(c, rule, now)).collect();
    let heard = others.iter().map(|c| c.renewed);
    let others_heard = match rule {
        Rule::FollowsYou => heard.max(),
        // Invoices: each other computer holding them heard from lately, the least lately one counts.
        Rule::StaysPut => heard.min(),
    }
    .unwrap_or(0);
let Some(chosen) = choose(claims, rule, now) else { return Keeper::alone(computer) };
    let mine = chosen.computer == computer;
    // Settled: this computer has kept it, as seen here, for the settling time.
    let (settled, since) = match KEPT.lock() {
        Ok(mut kept) => {
            let entry = kept.entry(part.to_string()).or_insert_with(|| (chosen.computer.clone(), now));
            if entry.0 != chosen.computer {
                *entry = (chosen.computer.clone(), now);
            }
            (mine && (others.is_empty() || now - entry.1 >= SETTLING), entry.1)
        }
        Err(_) => (false, now),
    };
    Keeper { computer: chosen.computer.clone(), name: chosen.name.clone(), mine, settled, others_heard, others: others.len(), since }
}

/// What the devices are for now, as the claims on a part that follows you
/// are read to say which device tells a thing (docs/attention.md, "Which
/// device tells"). The same on every device that reads the same settings.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Dispatch {
    /// What now is for, and the week's hours.
    pub time: Time,
    pub week: Week,
    /// The line of each device that counts and whose Sioul follows what
    /// devices are for (`devices::Entry::areas`), by its id, the device
    /// asking among them. A device that is not here tells by the rule
    /// before: when it is the one used last of all, everything. It counts
    /// for nothing among these: what only it could be for is held by no
    /// device, and comes on the others as the hours say, so that an older
    /// Sioul never leaves a thing told by nobody.
    pub lines: BTreeMap<String, Line>,
}

impl Dispatch {
    /// One device alone, nothing said of it: every answer as before.
    pub fn alone(computer: &str) -> Dispatch {
        Dispatch { lines: BTreeMap::from([(computer.to_string(), Line::default())]), ..Dispatch::default() }
    }

    /// What the lines of every device that counts say together (`Here::among`), read as none of them.
    fn together(&self) -> Here {
        let lines: Vec<Line> = self.lines.values().copied().collect();
        lines.split_first().map_or_else(Here::default, |(first, rest)| Here::among(*first, rest, self.week))
    }

    /// Whether something for `area` comes forward now on the device `id`,
    /// one of those that follow what devices are for.
    pub fn brings(&self, id: &str, area: Area) -> bool {
        self.lines.get(id).is_some_and(|line| self.together().as_device(*line).brings(area, self.time, self.week))
    }

    /// Whether something for `area` comes forward now on some device that counts.
    pub fn somewhere(&self, area: Area) -> bool {
        area.meets(self.together().anywhere(self.time, self.week))
    }
}

/// Which device tells a thing, as the claims and the devices' lines say.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Told<'a> {
    /// The device of this claim.
    By(&'a Claim),
    /// It comes forward on devices of which none runs now: it waits for
    /// one, which tells it when it next can.
    Waits,
    /// No claim is alive: the device asking is alone, as before.
    Alone,
}

/// Which device tells a thing for `area` (none: a thing no device narrows),
/// among the live claims on a part that follows you: a pure reading of the
/// claims and of what each device is for.
/// - The device used last of all (the keeper before) runs a Sioul that does
///   not follow what devices are for: it tells everything, as before, and
///   the others stay silent.
/// - Otherwise the devices that follow choose among themselves: of those
///   the thing comes forward on now, the one used last. One that does not
///   follow stays silent, since it is not the one used last of all.
/// - A thing with no area, or that comes forward on no device that counts
///   now (its row alone lets it come: `attention`), is told by the device
///   used last of all, as before: nothing waits for good.
pub fn told_by<'a>(claims: &'a [Claim], dispatch: &Dispatch, area: Option<Area>, now: i64) -> Told<'a> {
    let Some(last) = choose(claims, Rule::FollowsYou, now) else { return Told::Alone };
    let Some(area) = narrowed(claims, dispatch, area, now) else { return Told::By(last) };
    let for_it = claims.iter().filter(|c| holds(c, Rule::FollowsYou, now) && dispatch.brings(&c.computer, area));
    for_it.max_by(|a, b| a.active.cmp(&b.active).then_with(|| b.computer.cmp(&a.computer))).map_or(Told::Waits, Told::By)
}

/// The area the devices choose by for a thing, when they do: something is
/// said of some device, the device used last of all follows what devices
/// are for, the thing has an area, and it comes forward on some device that
/// counts now. None: the device used last of all tells it, as before; with
/// nothing said of any device, always.
fn narrowed(claims: &[Claim], dispatch: &Dispatch, area: Option<Area>, now: i64) -> Option<Area> {
    if dispatch.lines.values().all(|line| line.says_nothing()) {
        return None;
    }
    let last = choose(claims, Rule::FollowsYou, now)?;
    area.filter(|a| dispatch.lines.contains_key(&last.computer) && !a.is_empty() && dispatch.somewhere(*a))
}

/// Whether `computer` tells a thing for `area` now, as `told_by` reads the
/// claims: its own claim chosen; alone; or, when no device the thing comes
/// forward on runs, a device it comes forward on that asks all the same (a
/// phone's background step, whose claim is closed while it is put away):
/// it tells rather than nobody, as a device alone does.
pub fn tells(claims: &[Claim], computer: &str, dispatch: &Dispatch, area: Option<Area>, now: i64) -> bool {
    match told_by(claims, dispatch, area, now) {
        Told::By(claim) => claim.computer == computer,
        Told::Alone => true,
        Told::Waits => area.is_some_and(|area| dispatch.brings(computer, area)),
    }
}

/// The name a part is kept under for one area, as seen here (`KEPT`).
fn kept_for(part: &str, area: Option<Area>) -> String {
    match area {
        Some(area) if !area.is_empty() => format!("{part}{}{}", '\u{1f}', area.id()),
        _ => part.to_string(),
    }
}

/// Who tells a thing for `area` on a part that follows you (the notices),
/// as the claims read now and `dispatch` says what each device is for
/// (`told_by`), without claiming. The settling time is counted among the
/// devices the thing comes forward on: a device tells it once it has been
/// the one for the settling time, or at once when no other of them runs.
pub fn keeper_for(claims: &[Claim], part: &str, computer: &str, dispatch: &Dispatch, area: Option<Area>, now: i64) -> Keeper {
    let Some(area) = narrowed(claims, dispatch, area, now) else {
        // No device narrows it: the keeper as before, settled as the part is.
        return match told_by(claims, dispatch, area, now) {
            Told::By(_) => keeper(claims, part, computer, Rule::FollowsYou, now),
            _ => Keeper::alone(computer),
        };
    };
    let others: Vec<&Claim> = claims.iter().filter(|c| c.computer != computer && holds(c, Rule::FollowsYou, now) && dispatch.brings(&c.computer, area)).collect();
    let others_heard = others.iter().map(|c| c.renewed).max().unwrap_or(0);
    let chosen = match told_by(claims, dispatch, Some(area), now) {
        Told::By(claim) => Some((claim.computer.clone(), claim.name.clone())),
        // No device it comes forward on runs: this one, when it is for it.
        _ if dispatch.brings(computer, area) => Some((computer.to_string(), host_name())),
        _ => None,
    };
    let Some((chosen, name)) = chosen else {
        return Keeper { computer: String::new(), name: String::new(), mine: false, settled: false, others_heard, others: others.len(), since: now };
    };
    let mine = chosen == computer;
    let (settled, since) = match KEPT.lock() {
        Ok(mut kept) => {
            let entry = kept.entry(kept_for(part, Some(area))).or_insert_with(|| (chosen.clone(), now));
            if entry.0 != chosen {
                *entry = (chosen.clone(), now);
            }
            (mine && (others.is_empty() || now - entry.1 >= SETTLING), entry.1)
        }
        Err(_) => (false, now),
    };
    Keeper { computer: chosen, name, mine, settled, others_heard, others: others.len(), since }
}

/// Why invoices may not be numbered on a computer now, as the claims on
/// them say (`may_number`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Numbering {
    /// It may.
    Yes,
    /// Its own claim is closed, or not renewed for two periods: no window
    /// of Sioul runs there to keep the invoices.
    NotRunning,
    /// Another computer keeps them: its name.
    Elsewhere(String),
    /// Kept here, not long enough for the others to know it yet; or
    /// another computer closed with records not read here yet.
    Settling,
    /// Another computer holding them has not been heard from lately.
    OthersSilent,
}

/// Whether `computer` may number invoices now (a part that stays put), as
/// `claims` say, read at `now` (`read`: whether everything a computer
/// wrote up to its round and number is read here): its own claim alive and
/// renewed within two periods (`RENEWED_EVERY`), never a closed one; the
/// keeper; kept long enough for the others to know it, counted from when
/// it claimed without a break or took the invoices on purpose, whichever is
/// later (`SETTLING`); no other computer closed with records not read here
/// (`closed_unread`); every other computer holding them heard from in the
/// last three minutes. The window and the command line ask the same.
pub fn may_number(claims: &[Claim], computer: &str, now: i64, read: impl Fn(&str, (u32, u64)) -> bool) -> Numbering {
    let Some(own) = claims.iter().find(|c| c.computer == computer && !c.closed && c.until > now && now - c.renewed <= 2 * RENEWED_EVERY) else { return Numbering::NotRunning };
    let Some(chosen) = choose(claims, Rule::StaysPut, now) else { return Numbering::NotRunning };
    if chosen.computer != computer {
        return Numbering::Elsewhere(chosen.name.clone());
    }
    let others: Vec<&Claim> = claims.iter().filter(|c| c.computer != computer && holds(c, Rule::StaysPut, now)).collect();
    if !others.is_empty() && now - own.since.max(own.taken) < SETTLING {
        return Numbering::Settling;
    }
    if closed_unread(claims, computer, now, read).is_some() {
        return Numbering::Settling;
    }
    if others.iter().map(|c| c.renewed).min().is_some_and(|heard| now - heard > 3 * 60) {
        return Numbering::OthersSilent;
    }
    Numbering::Yes
}

/// This computer's claim on `part` closed: Sioul quits, or is put away on a
/// phone. It keeps nothing, and says how far it wrote its records, so the
/// others know everything it marked before it closed.
pub fn close(folder: &Path, key: &[u8; 32], part: &str, computer: &str, now: i64, wrote: Option<(u32, u64)>) -> Result<(), String> {
    close_with(folder, key, part, computer, now, wrote, None)
}

/// `close`, with each series' last number known now for invoices (the
/// journal's, which an issue just before may have raised): the closed claim
/// carries the larger of it and what it said.
pub fn close_with(folder: &Path, key: &[u8; 32], part: &str, computer: &str, now: i64, wrote: Option<(u32, u64)>, known: Option<BTreeMap<String, u64>>) -> Result<(), String> {
    let old = read(folder, key, part).into_iter().find(|c| c.computer == computer);
    let mut series = old.as_ref().map(|c| c.series.clone()).unwrap_or_default();
    for (sequence, last) in known.unwrap_or_default() {
        let at = series.entry(sequence).or_insert(0);
        *at = (*at).max(last);
    }
    let claim = Claim { computer: computer.to_string(), name: host_name(), since: now, renewed: now, until: now, active: old.as_ref().map_or(0, |c| c.active), taken: old.as_ref().map_or(0, |c| c.taken), wrote, series, closed: true, pad: String::new() };
    write(folder, key, part, computer, &claim)?;
    if let Ok(mut kept) = KEPT.lock() {
        // The part, and each area it was read for (`kept_for`).
        let per_area = format!("{part}{}", '\u{1f}');
        kept.retain(|name, _| name != part && !name.starts_with(&per_area));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sioul-lease-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn medicines_follow_you() {
        let dir = folder("follow");
        let key = [7u8; 32];
        // The desktop alone: it keeps them at once, nobody else to wait for.
        let alone = renew(&dir, &key, "health", "desktop", 1000, 1000, false, Rule::FollowsYou, None).unwrap();
        assert!(alone.mine && alone.settled);
        // The laptop opens and is used: it keeps them, but acts only once settled.
        let laptop = renew(&dir, &key, "health", "laptop", 1060, 1060, false, Rule::FollowsYou, None).unwrap();
        assert!(laptop.mine && !laptop.settled, "the desktop may not have seen it yet");
        let desktop = renew(&dir, &key, "health", "desktop", 1070, 1000, false, Rule::FollowsYou, None).unwrap();
        assert!(!desktop.mine, "the desktop steps back");
        assert_eq!(desktop.others, 1);
        let later = renew(&dir, &key, "health", "laptop", 1060 + SETTLING, 1060 + SETTLING, false, Rule::FollowsYou, None).unwrap();
        assert!(later.mine && later.settled);
        // The laptop closes: it says so, and how far it wrote; the desktop keeps them again.
        close(&dir, &key, "health", "laptop", 1190, Some((1, 12))).unwrap();
        let closed = claims(&dir, &key, "health").into_iter().find(|c| c.computer == "laptop").unwrap();
        assert!(closed.closed && closed.wrote == Some((1, 12)) && closed.until <= 1190);
        let back = renew(&dir, &key, "health", "desktop", 1200, 1200, false, Rule::FollowsYou, None).unwrap();
        assert!(back.mine);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn invoices_stay_put() {
        let dir = folder("stay");
        let key = [9u8; 32];
        let first = renew(&dir, &key, "invoices", "desktop", 1000, 1000, false, Rule::StaysPut, None).unwrap();
        assert!(first.mine);
        // Used later, the laptop does not take them: the oldest keeps them.
        let laptop = renew(&dir, &key, "invoices", "laptop", 1100, 1100, false, Rule::StaysPut, None).unwrap();
        assert!(!laptop.mine && laptop.name == host_name());
        // Taken on purpose: the laptop keeps them, once settled.
        let taken = renew(&dir, &key, "invoices", "laptop", 1200, 1200, true, Rule::StaysPut, None).unwrap();
        assert!(taken.mine && !taken.settled);
        assert!(!look(&dir, &key, "invoices", "desktop", 1210, Rule::StaysPut).mine);
        // A claim not renewed, not closed, still holds them a day (until 10 October 2026 it died
        // with its five minutes, and another computer numbered too: the review's F7).
        assert!(!look(&dir, &key, "invoices", "desktop", 1200 + ALIVE + 1, Rule::StaysPut).mine, "a lapsed claim, not closed, still holds them");
        assert!(look(&dir, &key, "invoices", "desktop", 1200 + UNCLOSED + 1, Rule::StaysPut).mine, "a day without news: it keeps nothing");
        // Another key reads nothing: a stranger's claims do not count.
        assert!(look(&dir, &[1u8; 32], "invoices", "desktop", 1210, Rule::StaysPut).mine);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Each claim carries the last number of each series its computer gave
    /// (the invoicing's phase 1): kept through a renewal that says nothing,
    /// never lowered, kept when it closes; every computer reads the largest.
    #[test]
    fn a_claim_carries_the_last_numbers() {
        let dir = folder("series");
        let key = [4u8; 32];
        let said = |n: u64| Some(BTreeMap::from([("2026-{N:3}".to_string(), n)]));
        let renewal = |now: i64, series| Renewal { now, active: now, take: false, rule: Rule::StaysPut, wrote: None, series };
        renew_with(&dir, &key, "invoices-series", "desk", renewal(1000, said(7))).unwrap();
        renew_with(&dir, &key, "invoices-series", "desk", renewal(1060, None)).unwrap();
        renew_with(&dir, &key, "invoices-series", "desk", renewal(1120, said(5))).unwrap();
        renew_with(&dir, &key, "invoices-series", "laptop", renewal(1130, said(3))).unwrap();
        close(&dir, &key, "invoices-series", "desk", 1200, None).unwrap();
        let claims = claims(&dir, &key, "invoices-series");
        assert_eq!(claims.iter().find(|c| c.computer == "desk").map(|c| c.series.clone()), said(7), "never lowered, kept when closed");
        assert_eq!(claimed_series(&claims), said(7).unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The review of 3273f2d, R2: the desk makes an invoice and quits; its
    /// closed claim comes to the laptop ahead of the record holding the
    /// invoice. The laptop keeps the invoices, settled, but numbers nothing
    /// until that record is read here, or a day went by.
    #[test]
    fn a_closed_claim_holds_the_numbers_until_its_records_are_read() {
        let dir = folder("closed-unread");
        let key = [3u8; 32];
        // A part of its own name: who keeps a part, as seen here, is kept by its name for every test (`KEPT`).
        renew(&dir, &key, "invoices-r2", "desk", 1000, 1000, true, Rule::StaysPut, Some((1, 12))).unwrap();
        close(&dir, &key, "invoices-r2", "desk", 1100, Some((1, 14))).unwrap();
        let laptop = renew(&dir, &key, "invoices-r2", "laptop", 1130, 1130, false, Rule::StaysPut, None).unwrap();
        assert!(laptop.mine && laptop.settled, "a closed claim keeps nothing");
        let claims = claims(&dir, &key, "invoices-r2");
        // The desk's records read here up to its record `n`.
        fn read_to(n: u64) -> impl Fn(&str, (u32, u64)) -> bool {
            move |computer, wrote| computer == "desk" && (1, n) >= wrote
        }
        assert_eq!(closed_unread(&claims, "laptop", 1130, read_to(13)).map(|c| c.computer.as_str()), Some("desk"), "the desk's last record not read here");
        assert!(closed_unread(&claims, "laptop", 1130, read_to(14)).is_none(), "read as far as it wrote");
        assert!(closed_unread(&claims, "desk", 1130, read_to(0)).is_none(), "its own claim");
        assert!(closed_unread(&claims, "laptop", 1100 + UNCLOSED + 1, read_to(13)).is_none(), "a day later: lost, never read");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The review of 5 October 2026, F7: the desktop numbers invoices; on the
    /// phone its claim lapsed (a sync app half an hour late, a sync stopped),
    /// not closed. The phone never numbers meanwhile: the desktop keeps them;
    /// taken on purpose, the phone still waits until the desktop is heard
    /// from, closes, or a day went by without its news.
    #[test]
    fn invoices_stay_with_a_computer_not_heard_from_until_it_closes() {
        let dir = folder("unclosed");
        let key = [5u8; 32];
        renew(&dir, &key, "invoices", "desktop", 1000, 1000, false, Rule::StaysPut, None).unwrap();
        // Ten minutes on, its claim not renewed here: the phone does not keep them.
        let phone = renew(&dir, &key, "invoices", "phone", 1600, 1600, false, Rule::StaysPut, None).unwrap();
        assert!(!phone.mine, "a computer not heard from is not a computer gone: {phone:?}");
        // Taken here on purpose: kept here, but the desktop, not heard from for ten minutes, holds the numbers back.
        let taken = renew(&dir, &key, "invoices", "phone", 1700, 1700, true, Rule::StaysPut, None).unwrap();
        assert!(taken.mine && taken.others == 1 && taken.others_heard == 1000, "{taken:?}");
        // The desktop closes: the phone alone.
        close(&dir, &key, "invoices", "desktop", 1800, None).unwrap();
        let alone = renew(&dir, &key, "invoices", "phone", 1900, 1900, false, Rule::StaysPut, None).unwrap();
        assert!(alone.mine && alone.others == 0, "{alone:?}");
        // A computer gone without closing holds them a day, then no more.
        renew(&dir, &key, "invoices", "laptop", 2000, 2000, true, Rule::StaysPut, None).unwrap();
        assert!(!look(&dir, &key, "invoices", "phone", 2000 + UNCLOSED, Rule::StaysPut).mine);
        assert!(look(&dir, &key, "invoices", "phone", 2001 + UNCLOSED, Rule::StaysPut).mine);
        // A part that follows you (medicines; its own name here, as `KEPT` is the process's): a lapsed claim keeps nothing, as before.
        renew(&dir, &key, "follows", "desktop", 1000, 1000, false, Rule::FollowsYou, None).unwrap();
        assert!(renew(&dir, &key, "follows", "phone", 1600, 1600, false, Rule::FollowsYou, None).unwrap().mine);
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn live(computer: &str, active: i64, now: i64) -> Claim {
        Claim { computer: computer.into(), name: computer.into(), since: 0, renewed: now - 10, until: now - 10 + ALIVE, active, ..Claim::default() }
    }

    fn week() -> Week {
        Week { work_hours: true, admin_hours: true, meals: true, sleep: true }
    }

    /// With nothing said of any device, the notices are read as before:
    /// whatever a thing is for, the device used last of all tells it.
    #[test]
    fn with_nothing_said_the_device_used_last_tells_as_before() {
        let now = 10_000;
        let claims = [live("desk", 9_000, now), live("laptop", 9_500, now), live("phone", 9_200, now)];
        let areas = [None, Some(Area::default()), Some(Area::WORK), Some(Area::ADMIN), Some(Area::LEISURE), Some(Area::PERSONAL), Some(Area::ALL)];
        let lines: BTreeMap<String, Line> = ["desk", "laptop", "phone"].iter().map(|id| (id.to_string(), Line::default())).collect();
        for time in Time::STATES.into_iter().chain([Time::Any, Time::Several(Area::MIXED)]) {
            let dispatch = Dispatch { time, week: week(), lines: lines.clone() };
            for area in areas {
                assert_eq!(told_by(&claims, &dispatch, area, now), Told::By(&claims[1]), "{time:?} {area:?}");
                assert_eq!(["desk", "laptop", "phone"].map(|d| tells(&claims, d, &dispatch, area, now)), [false, true, false]);
            }
        }
        // The part's own keeper, settled as it always was, whatever the thing is for: no reading per area.
        let dispatch = Dispatch { time: Time::Work, week: week(), lines: lines.clone() };
        let (plain, per_area) = (keeper(&claims, "notices-unsaid", "laptop", Rule::FollowsYou, now), keeper_for(&claims, "notices-unsaid", "laptop", &dispatch, Some(Area::WORK), now));
        assert_eq!(per_area, plain);
        assert!(per_area.mine && per_area.others == 2);
        // A claim of a device this one does not count yet (its entry not learned): as before, the one used last of all.
        let unknown = [claims[0].clone(), live("new", 9_900, now)];
        assert_eq!(told_by(&unknown, &dispatch, Some(Area::WORK), now), Told::By(&unknown[1]));
        // No claim alive: alone, as before; a device alone with nothing said, the same.
        assert_eq!(told_by(&[], &Dispatch::alone("desk"), Some(Area::WORK), now), Told::Alone);
        assert!(tells(&[], "desk", &Dispatch::alone("desk"), Some(Area::LEISURE), now));
    }

    /// Among the devices a thing comes forward on now, the one used last
    /// tells it: the work computer used last, leisure's mail is told by the
    /// phone; with the phone put away, it waits there until the phone asks.
    #[test]
    fn the_devices_a_thing_is_for_choose_among_themselves() {
        let now = 10_000;
        let lines: BTreeMap<String, Line> = [("desk", Line::for_area(Area::WORK)), ("phone", Line::for_area(Area::PERSONAL)), ("tablet", Line::default())].into_iter().map(|(id, line)| (id.to_string(), line)).collect();
        let at = |time: Time| Dispatch { time, week: week(), lines: lines.clone() };
        // Everything runs; the desk was used last, then the phone, then the tablet.
        let claims = [live("desk", 9_900, now), live("phone", 9_500, now), live("tablet", 9_000, now)];
        let by = |time: Time, area: Area| match told_by(&claims, &at(time), Some(area), now) {
            Told::By(claim) => claim.computer.clone(),
            other => format!("{other:?}"),
        };
        assert_eq!((by(Time::Work, Area::WORK), by(Time::Leisure, Area::LEISURE), by(Time::Admin, Area::ADMIN)), ("desk".into(), "phone".into(), "phone".into()));
        assert_eq!(by(Time::Admin, Area::MIXED), "phone", "a thing for two areas, where one of them comes forward");
        // What comes forward on no device now (work's mail in the evening, which a row alone let come): the one used last of all.
        assert_eq!(by(Time::Leisure, Area::WORK), "desk");
        assert_eq!(told_by(&claims, &at(Time::Leisure), None, now), Told::By(&claims[0]), "a thing no device narrows");
        // The phone put away (its claim closed): leisure's is the tablet's, which nothing was said of.
        let closed = [claims[0].clone(), Claim { closed: true, until: now - 60, ..claims[1].clone() }, claims[2].clone()];
        assert_eq!(told_by(&closed, &at(Time::Leisure), Some(Area::LEISURE), now), Told::By(&closed[2]));
        // The tablet closed too: it waits; the phone's background step, asking, tells it; the desk never.
        let alone = [claims[0].clone(), Claim { closed: true, until: now - 60, ..claims[1].clone() }];
        assert_eq!(told_by(&alone, &at(Time::Leisure), Some(Area::LEISURE), now), Told::Waits);
        assert_eq!(["desk", "phone", "tablet"].map(|d| tells(&alone, d, &at(Time::Leisure), Some(Area::LEISURE), now)), [false, true, true]);
        // A cell the person changed: friends reach the phone during work; the desk, used last, stays silent for them.
        let mut friends = at(Time::Work);
        friends.lines.insert("phone".into(), Line::for_area(Area::PERSONAL).with_cell(Time::Work, Some(Area::LEISURE)));
        friends.lines.remove("tablet");
        assert_eq!(told_by(&claims[..2], &friends, Some(Area::LEISURE), now), Told::By(&claims[1]));
        // The settling time is counted among the devices the thing is for: the phone alone for leisure tells at once,
        // though the desk runs too; two devices for it, the one used last waits for the settling time.
        let two = Dispatch { lines: lines.iter().filter(|(id, _)| id.as_str() != "tablet").map(|(id, line)| (id.clone(), *line)).collect(), ..at(Time::Leisure) };
        let phone = keeper_for(&claims[..2], "notices-areas-a", "phone", &two, Some(Area::LEISURE), now);
        assert!(phone.mine && phone.settled && phone.others == 0, "{phone:?}");
        let desk = keeper_for(&claims[..2], "notices-areas-a", "desk", &two, Some(Area::LEISURE), now);
        assert!(!desk.mine && !desk.settled && desk.computer == "phone" && desk.others == 1, "{desk:?}");
        let phone = keeper_for(&claims, "notices-areas-b", "phone", &at(Time::Leisure), Some(Area::LEISURE), now);
        assert!(phone.mine && !phone.settled && phone.others == 1, "the tablet may not have seen it yet: {phone:?}");
        let later: Vec<Claim> = claims.iter().map(|c| Claim { renewed: c.renewed + SETTLING, until: c.until + SETTLING, ..c.clone() }).collect();
        assert!(keeper_for(&later, "notices-areas-b", "phone", &at(Time::Leisure), Some(Area::LEISURE), now + SETTLING).settled);
        // A thing no device narrows: the part's own keeper, settled as before.
        let plain = keeper_for(&claims, "notices-areas-c", "desk", &at(Time::Leisure), None, now);
        assert!(plain.mine && plain.computer == "desk" && plain.others == 2);
        // Asked for a thing that comes forward on no running device, by a device that is not for it: not its to tell.
        let waits = keeper_for(&alone, "notices-areas-d", "desk", &at(Time::Leisure), Some(Area::LEISURE), now);
        assert!(!waits.mine && !waits.settled && waits.computer.is_empty(), "{waits:?}");
    }

    /// An older Sioul, a work computer and a personal phone together: each
    /// thing is told exactly once, whichever was used last, or waits for a
    /// device that is for it. The older one tells by the rule before (the
    /// one used last of all tells everything); the two others read what each
    /// device is for.
    #[test]
    fn an_older_device_a_work_computer_and_a_personal_phone_tell_each_thing_once() {
        let now = 10_000;
        let lines: BTreeMap<String, Line> = [("desk", Line::for_area(Area::WORK)), ("phone", Line::for_area(Area::PERSONAL))].into_iter().map(|(id, line)| (id.to_string(), line)).collect();
        let devices = ["old", "desk", "phone"];
        let things = [None, Some(Area::WORK), Some(Area::ADMIN), Some(Area::LEISURE), Some(Area::PERSONAL), Some(Area::MIXED), Some(Area::ALL)];
        let mut walked = 0;
        // Each order of use, and each device running or not (its claim closed).
        for order in [[3, 2, 1], [3, 1, 2], [2, 3, 1], [1, 3, 2], [2, 1, 3], [1, 2, 3]] {
            for running in 0..8u8 {
                let claims: Vec<Claim> = devices.iter().zip(order).enumerate().map(|(i, (device, used))| if running & (1 << i) != 0 { live(device, 9_000 + used, now) } else { Claim { closed: true, until: now - 60, ..live(device, 9_000 + used, now) } }).collect();
                let last = choose(&claims, Rule::FollowsYou, now).map(|c| c.computer.clone());
                for time in Time::STATES.into_iter().chain([Time::Several(Area::MIXED)]) {
                    let dispatch = Dispatch { time, week: week(), lines: lines.clone() };
                    for thing in things {
                        // What each device would do of a thing its own rules let it tell now.
                        let tellers: Vec<&str> = devices
                            .iter()
                            .copied()
                            .filter(|device| running & (1 << devices.iter().position(|d| d == device).unwrap()) != 0)
                            .filter(|device| match *device {
                                // The older Sioul: the device used last of all tells, whatever the thing is for.
                                "old" => last.as_deref() == Some("old"),
                                // The others: only what comes forward on them, or what no device narrows; then the claims read per area.
                                device => {
                                    let comes = thing.is_none_or(|area| dispatch.brings(device, area) || !dispatch.somewhere(area));
                                    comes && tells(&claims, device, &dispatch, thing, now)
                                }
                            })
                            .collect();
                        assert!(tellers.len() <= 1, "told twice: {tellers:?} for {thing:?} at {time:?}, used {order:?}, running {running:03b}");
                        // Told by nobody only while no running device may tell it: the older one was not used last
                        // (it would tell everything), and no running device is one the thing comes forward on.
                        if tellers.is_empty() && running != 0 {
                            let runs = |device: &str| running & (1 << devices.iter().position(|d| *d == device).unwrap()) != 0;
                            let for_it = ["desk", "phone"].iter().any(|d| runs(d) && thing.is_none_or(|area| dispatch.brings(d, area) || !dispatch.somewhere(area)));
                            assert!(last.as_deref() != Some("old") && !for_it, "nobody tells {thing:?} at {time:?}, used {order:?}, running {running:03b}");
                            assert_eq!(told_by(&claims, &dispatch, thing, now), Told::Waits, "it waits for a device that is for it");
                        }
                        walked += 1;
                    }
                }
            }
        }
        assert_eq!(walked, 6 * 8 * 6 * 7);
        // In words, once: the three run, leisure's mail in the evening.
        let all = |order: [i64; 3]| -> Vec<Claim> { devices.iter().zip(order).map(|(device, used)| live(device, 9_000 + used, now)).collect() };
        let evening = Dispatch { time: Time::Leisure, week: week(), lines };
        let said = |claims: &[Claim], thing: Area| match told_by(claims, &evening, Some(thing), now) {
            Told::By(claim) => claim.computer.clone(),
            other => format!("{other:?}"),
        };
        assert_eq!(said(&all([3, 2, 1]), Area::LEISURE), "old", "the older one used last tells everything, the others stay silent");
        assert_eq!(said(&all([1, 3, 2]), Area::LEISURE), "phone", "the desk used last: the phone tells leisure's");
        assert_eq!(said(&all([1, 2, 3]), Area::LEISURE), "phone");
        assert_eq!(said(&all([2, 3, 1]), Area::WORK), "desk", "work's in the evening, let come by its row alone: the one used last of all");
    }

    /// The window and the command line ask the same (the second review,
    /// G10): a claim not renewed for two minutes, or closed, is no running
    /// window's; settled from when the invoices were taken on purpose, not
    /// from the claim's age; another computer's silence said.
    #[test]
    fn numbering_asks_the_claims_as_the_window_does() {
        let claim = |computer: &str, since: i64, renewed: i64, taken: i64| Claim { computer: computer.into(), name: computer.into(), since, renewed, until: renewed + ALIVE, taken, ..Claim::default() };
        let read_all = |_: &str, _: (u32, u64)| true;
        let now = 10_000;
        assert_eq!(may_number(&[claim("desk", 1000, now - 30, 0)], "desk", now, read_all), Numbering::Yes);
        assert_eq!(may_number(&[claim("desk", 1000, now - 3 * RENEWED_EVERY, 0)], "desk", now, read_all), Numbering::NotRunning, "no window renews it");
        assert_eq!(may_number(&[Claim { closed: true, ..claim("desk", 1000, now - 10, 0) }], "desk", now, read_all), Numbering::NotRunning, "closed");
        // Taken on purpose ten seconds ago, the laptop still holding them: settling, whatever the claim's age.
        let both = [claim("desk", 1000, now - 5, now - 10), claim("laptop", 2000, now - 20, 0)];
        assert_eq!(may_number(&both, "desk", now, read_all), Numbering::Settling);
        let silent = [claim("desk", 1000, now - 5, now - 200), claim("laptop", 2000, now - 250, 0)];
        assert_eq!(may_number(&silent, "desk", now, read_all), Numbering::OthersSilent, "the laptop not heard from lately");
        assert_eq!(may_number(&both, "laptop", now, read_all), Numbering::Elsewhere("desk".into()));
    }
}
