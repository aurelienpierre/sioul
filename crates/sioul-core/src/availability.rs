// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! "Share my availability…" (several agendas, part 5; docs/client.md,
//! "Sharing your availability"; research/agendas.md §6, recommendation 15):
//! your busy times, given to one person, never what fills them.
//!
//! On Google, your calendars are shared with them for their busy times alone
//! (the ACL's `freeBusyReader`): nothing is copied. Elsewhere (Nextcloud),
//! Sioul keeps a calendar of its own, "Busy times", of plain blocks made from
//! the agendas you pick, each "Busy" unless you give an agenda a word ("Work",
//! for a partner, who reads a bare "busy" as secrecy: Thayer et al. 2012,
//! research/agendas.md §2.3), kept up to date with the plan, and shared read
//! only with one person of the server, or by its link. Stopping takes the
//! share back, and the calendar Sioul made.

use crate::agenda::Occurrence;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// What you share (`[agenda.availability]`, travelling with the settings).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Availability {
    /// The account it is shared from.
    pub account: String,
    /// The agendas it is made from, by place; on Google, the calendars shared busy only.
    #[serde(default)]
    pub agendas: Vec<String>,
    /// Whom: a user of the same Nextcloud, or an address (Google); "" by link alone.
    #[serde(default)]
    pub with: String,
    /// Shared by a read-only public link (Nextcloud).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub link: bool,
    /// A word for an agenda's blocks instead of "Busy", by place.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub words: BTreeMap<String, String>,
    /// The calendar Sioul made for the blocks, its folder's id; "" on Google.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub calendar: String,
    /// The share was made on the server.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub shared: bool,
}

/// The calendar of busy blocks Sioul keeps, as (account, folder id), while
/// one is: derived from your own events, it is no agenda of yours to show,
/// remind, plan around or meet in a conflict, so the agenda's events leave it
/// out (`agenda::occurrences`); Agenda ⚙ ▸ Your agendas still lists it.
pub fn blocks_place() -> Option<(String, String)> {
    crate::agendas::read_config().agenda.availability.as_ref().filter(|a| !a.calendar.is_empty()).map(|a| (a.account.clone(), a.calendar.clone()))
}

/// One busy block: from, until (Unix seconds), what it says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub start: i64,
    pub end: i64,
    pub word: String,
}

/// The blocks of `events` (those of the agendas picked, that take time:
/// not cancelled, free or declined, never a whole day), each saying its
/// agenda's word or `busy`; blocks that touch or overlap with the same word
/// made one, so that nothing says how many things fill a time.
pub fn blocks(events: &[Occurrence], availability: &Availability, busy: &str) -> Vec<Block> {
    let mut found: Vec<Block> = events
        .iter()
        .filter(|e| availability.agendas.iter().any(|a| *a == e.agenda) && !e.cancelled && !e.transparent && !e.declined && !e.all_day && e.end > e.start)
        .map(|e| Block { start: e.start, end: e.end, word: availability.words.get(&e.agenda).map(|w| w.trim().to_string()).filter(|w| !w.is_empty()).unwrap_or_else(|| busy.to_string()) })
        .collect();
    found.sort_by(|a, b| (&a.word, a.start, a.end).cmp(&(&b.word, b.start, b.end)));
    let mut merged: Vec<Block> = Vec::new();
    for block in found {
        match merged.last_mut() {
            Some(last) if last.word == block.word && block.start <= last.end => last.end = last.end.max(block.end),
            _ => merged.push(block),
        }
    }
    merged.sort_by_key(|b| (b.start, b.end));
    merged
}

/// A block's UID, the same on every device (a name-based UUID, `holders::uuid_v5`).
pub fn block_uid(block: &Block) -> String {
    crate::holders::uuid_v5(&crate::holders::NAMESPACE, &format!("busy\n{}\n{}\n{}", block.start, block.end, block.word))
}

/// A block's file: its time in UTC, its word, busy, public; its stamp its start, so that two devices write the same bytes.
pub fn block_text(block: &Block) -> String {
    let utc = crate::moves::utc;
    crate::lines::fold(&[
        "BEGIN:VCALENDAR".to_string(),
        "VERSION:2.0".to_string(),
        "PRODID:-//Sioul//Sioul//EN".to_string(),
        "BEGIN:VEVENT".to_string(),
        format!("UID:{}", block_uid(block)),
        format!("DTSTAMP:{}", utc(block.start)),
        format!("DTSTART:{}", utc(block.start)),
        format!("DTEND:{}", utc(block.end)),
        format!("SUMMARY:{}", crate::lines::escape(&block.word)),
        "TRANSP:OPAQUE".to_string(),
        "CLASS:PUBLIC".to_string(),
        "END:VEVENT".to_string(),
        "END:VCALENDAR".to_string(),
    ])
}

/// The files a calendar of blocks holds, by name ("<uid>.ics"): what it should hold now.
pub fn wanted(events: &[Occurrence], availability: &Availability, busy: &str) -> BTreeMap<String, String> {
    blocks(events, availability, busy).iter().map(|b| (format!("{}.ics", block_uid(b)), block_text(b))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The blocks: the agendas picked alone, what takes time alone, each
    /// "Busy" or its agenda's word, those of one word that touch made one;
    /// their files the same on every device, and nothing of what fills them.
    #[test]
    fn busy_blocks_say_nothing_of_what_fills_them() {
        let event = |agenda: &str, start: i64, end: i64, title: &str| Occurrence { agenda: agenda.into(), start, end, summary: title.into(), location: "Rue Blanche".into(), ..Occurrence::default() };
        let events = [
            event("me/personal", 1_000, 2_000, "Dentist"),
            event("me/personal", 2_000, 3_000, "Lunch with Tom"),
            event("work/main", 2_500, 4_000, "Board"),
            Occurrence { cancelled: true, ..event("me/personal", 5_000, 6_000, "Cancelled") },
            Occurrence { transparent: true, ..event("me/personal", 7_000, 8_000, "Free") },
            Occurrence { all_day: true, ..event("me/personal", 0, 86_400, "Holiday") },
            event("me/hidden", 9_000, 9_500, "Not picked"),
        ];
        let availability = Availability { agendas: vec!["me/personal".into(), "work/main".into()], words: [("work/main".to_string(), "Work".to_string())].into(), ..Availability::default() };
        let found = blocks(&events, &availability, "Busy");
        assert_eq!(found, [Block { start: 1_000, end: 3_000, word: "Busy".into() }, Block { start: 2_500, end: 4_000, word: "Work".into() }]);
        let files = wanted(&events, &availability, "Busy");
        assert_eq!(files.len(), 2);
        let all: String = files.values().cloned().collect();
        for kept in ["Dentist", "Lunch", "Board", "Rue Blanche"] {
            assert!(!all.contains(kept), "{kept}: {all}");
        }
        assert!(all.contains("SUMMARY:Busy") && all.contains("SUMMARY:Work") && all.contains("CLASS:PUBLIC"));
        assert_eq!(wanted(&events, &availability, "Busy"), files, "the same on every device");
    }
}
