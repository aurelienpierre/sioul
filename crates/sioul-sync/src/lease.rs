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

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// How long a claim lives after its last renewal.
pub const ALIVE: i64 = 5 * 60;
/// How long a computer keeps a part before acting on it.
pub const SETTLING: i64 = 90;

/// Who keeps a part, as the claims say.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rule {
    /// The computer used most recently: medicines' reminders.
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
    /// The last renewal heard from another live computer; 0 when none is alive.
    pub others_heard: i64,
    /// How many other computers claim it now.
    pub others: usize,
}

impl Keeper {
    /// The only computer: no sharing, or nobody else claims.
    pub fn alone(computer: &str) -> Keeper {
        Keeper { computer: computer.to_string(), name: host_name(), mine: true, settled: true, others_heard: 0, others: 0 }
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

fn read(folder: &Path, key: &[u8; 32], part: &str) -> Vec<Claim> {
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

/// The keeper among live claims, by the part's rule; ties go to the smaller id.
fn choose<'a>(claims: &'a [Claim], rule: Rule, now: i64) -> Option<&'a Claim> {
    let live = claims.iter().filter(|c| c.until > now);
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
    let mut claims = read(folder, key, part);
    let old = claims.iter().find(|c| c.computer == computer).cloned();
    // A claim that lapsed, or closed, starts again: its age counts from now.
    let since = old.as_ref().filter(|c| c.until > now && !c.closed).map_or(now, |c| c.since);
    let taken = if take { now } else { old.as_ref().map_or(0, |c| c.taken) };
    let claim = Claim { computer: computer.to_string(), name: host_name(), since, renewed: now, until: now + ALIVE, active: active.max(old.as_ref().map_or(0, |c| c.active)), taken, wrote, closed: false, pad: String::new() };
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

/// Who keeps `part`, as read now, without claiming it.
pub fn look(folder: &Path, key: &[u8; 32], part: &str, computer: &str, now: i64, rule: Rule) -> Keeper {
    keeper(&read(folder, key, part), part, computer, rule, now)
}

fn keeper(claims: &[Claim], part: &str, computer: &str, rule: Rule, now: i64) -> Keeper {
    let others: Vec<&Claim> = claims.iter().filter(|c| c.computer != computer && c.until > now).collect();
    let others_heard = others.iter().map(|c| c.renewed).max().unwrap_or(0);
let Some(chosen) = choose(claims, rule, now) else { return Keeper::alone(computer) };
    let mine = chosen.computer == computer;
    // Settled: this computer has kept it, as seen here, for the settling time.
    let settled = match KEPT.lock() {
        Ok(mut kept) => {
            let entry = kept.entry(part.to_string()).or_insert_with(|| (chosen.computer.clone(), now));
            if entry.0 != chosen.computer {
                *entry = (chosen.computer.clone(), now);
            }
            mine && (others.is_empty() || now - entry.1 >= SETTLING)
        }
        Err(_) => false,
    };
    Keeper { computer: chosen.computer.clone(), name: chosen.name.clone(), mine, settled, others_heard, others: others.len() }
}

/// This computer's claim on `part` closed: Sioul quits, or is put away on a
/// phone. It keeps nothing, and says how far it wrote its records, so the
/// others know everything it marked before it closed.
pub fn close(folder: &Path, key: &[u8; 32], part: &str, computer: &str, now: i64, wrote: Option<(u32, u64)>) -> Result<(), String> {
    let old = read(folder, key, part).into_iter().find(|c| c.computer == computer);
    let claim = Claim { computer: computer.to_string(), name: host_name(), since: now, renewed: now, until: now, active: old.as_ref().map_or(0, |c| c.active), taken: old.as_ref().map_or(0, |c| c.taken), wrote, closed: true, pad: String::new() };
    write(folder, key, part, computer, &claim)?;
    if let Ok(mut kept) = KEPT.lock() {
        kept.remove(part);
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
        // Claims not renewed die: nobody else keeps them then.
        assert!(look(&dir, &key, "invoices", "desktop", 1200 + ALIVE + 1, Rule::StaysPut).mine, "dead claims keep nothing");
        // Another key reads nothing: a stranger's claims do not count.
        assert!(look(&dir, &[1u8; 32], "invoices", "desktop", 1210, Rule::StaysPut).mine);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
