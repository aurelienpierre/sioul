// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! An office's opening hours on its card, in the window (OpeningHours.qml):
//! what is true now, the hours in words, where they come from; changed by
//! hand; looked up when the person asks, from the administration's
//! directory and OpenStreetMap (`sioul_core::hourslookup`); "Call back…",
//! a task that follows them. The card is written only through the
//! contacts' own writer (`contacts::with_hours`), then sent, and the tasks
//! that involve the office are planned again (`work::show_work`).

use crate::backend::{QtThread, Shared, load_config, offline, say, tell, tr};
use crate::pim;
use cxx_qt_lib::QString;
use serde::{Deserialize, Serialize};
use sioul_core::contacts::{self, Contact};
use sioul_core::hourslookup::{self, Asked, Found};
use sioul_core::opening::{self, CardHours, Hours};
use sioul_core::vdir;
use std::sync::Arc;

/// One set of hours, as the card's section says it.
#[derive(Serialize, Default)]
struct HoursView {
    /// As the card keeps them.
    value: String,
    /// One line per run of days: "Monday to Friday: 9:00–12:00".
    lines: Vec<String>,
    /// Closed on public holidays.
    holidays_off: bool,
    /// Sioul counts none of it: shown as written.
    unread: bool,
    /// What it could not count, as written.
    rest: Vec<String>,
    /// "Typed by you on …", "From … checked on …".
    source: String,
    /// Checked more than a year ago.
    stale: bool,
    /// What is true now: "Open until 12:00"; "" when it counts no hours.
    now: String,
}

fn hours_view(hours: &Hours, holidays: &std::collections::BTreeSet<jiff::civil::Date>, now: &jiff::Zoned) -> HoursView {
    let parsed = hours.read();
    let followed = opening::Followed { windows: parsed.windows.clone(), holidays_off: parsed.holidays_off, whose: opening::Whose::Usual };
    HoursView {
        value: hours.value.clone(),
        lines: opening::day_lines(&parsed.windows, tr()),
        holidays_off: parsed.holidays_off,
        unread: parsed.windows.is_empty(),
        rest: parsed.rest,
        source: opening::source_line(hours, tr()),
        stale: opening::stale(hours, now.date()),
        now: opening::now_line(&followed, holidays, now, None, tr()),
    }
}

/// The public holidays around now, as the tasks read them (`quiet::Situation::now`).
fn holidays_now(now: &jiff::Zoned) -> std::collections::BTreeSet<jiff::civil::Date> {
    let from = now.date().yesterday().unwrap_or(now.date());
    let until = now.date().checked_add(jiff::Span::new().days(20)).unwrap_or(now.date());
    opening::public_holidays(&load_config(), now.time_zone(), from, until)
}

/// The card of a contact, read now, with its book.
fn card(key: &str) -> Option<(std::path::PathBuf, Contact)> {
    let path = pim::ours(key)?;
    let book = pim::collection_of(&path)?;
    let contact = contacts::read(&path, &book)?;
    Some((path, contact))
}

/// A card's hours section: {name, card ("org", "person", ""), read_only,
/// open, phone (each a `HoursView` or null), note, numbers, asked (what a
/// lookup would send, null for a person's card), sources (who it would be
/// sent to, in words)}. "" for a card that is gone. Read each time the
/// section shows and each minute after (what is true now changes).
pub(crate) fn contact_hours(key: &str) -> String {
    let Some((_, contact)) = card(key) else { return String::new() };
    let now = sioul_core::zone::now();
    let holidays = holidays_now(&now);
    let asked = Asked::of(&contact, crate::eventplace::service() == sioul_sync::suggest::Service::Ban);
    let sources = asked.as_ref().map(sources_named).unwrap_or_default();
    serde_json::json!({
        "name": contact.name,
        "card": contact.card,
        "read_only": contact.read_only,
        "open": contact.hours.open.as_ref().map(|h| hours_view(h, &holidays, &now)),
        "phone": contact.hours.phone.as_ref().map(|h| hours_view(h, &holidays, &now)),
        "note": contact.hours.note,
        "numbers": contact.phones.iter().map(|p| p.value.clone()).collect::<Vec<_>>(),
        "asked": asked,
        "sources": sources,
        "offline": offline(),
    })
    .to_string()
}

/// "the administration's directory (service-public.gouv.fr) and OpenStreetMap", or the map alone.
fn sources_named(asked: &Asked) -> String {
    let map = tr().text("hours-source-name-map", None);
    if asked.france { say("hours-sources-both", &[("directory", tr().text("hours-source-name-directory", None)), ("map", map)]) } else { map }
}

/// What the form typed reads as: {written, lines, days (seven, Monday
/// first, each "09:00-12:00, 14:00-17:00" or "" when closed), rest,
/// holidays_off}, for its line "Reads as…" and its rows day by day.
pub(crate) fn read_hours(text: &str) -> String {
    let parsed = opening::parse(text);
    let days: Vec<String> = (0..7)
        .map(|d| {
            let mut ranges: Vec<String> = parsed.windows.iter().filter(|w| w.weekday().is_some_and(|wd| wd.to_monday_zero_offset() as usize == d)).map(|w| format!("{}-{}", w.start, w.end_text())).collect();
            ranges.sort();
            ranges.join(", ")
        })
        .collect();
    serde_json::json!({
        "written": opening::written(&parsed.windows, parsed.holidays_off),
        "lines": opening::day_lines(&parsed.windows, tr()),
        "days": days,
        "rest": parsed.rest,
        "holidays_off": parsed.holidays_off,
    })
    .to_string()
}

/// What the form gives back: the hours typed (the short form or the
/// map's syntax), the phone's ("" when they do not differ), the free line.
#[derive(Deserialize)]
struct Typed {
    #[serde(default)]
    open: String,
    #[serde(default)]
    phone: String,
    #[serde(default)]
    note: String,
}

/// Hours as typed: what Sioul counts, written in the map's syntax, typed by
/// you today; unchanged from the card's, they keep their source and day.
/// What it cannot count goes to the free line (`rest`).
fn typed_hours(text: &str, was: Option<&Hours>, today: jiff::civil::Date, rest: &mut Vec<String>) -> Option<Hours> {
    let (written, left) = opening::tidied(text);
    if !left.is_empty() {
        rest.push(left);
    }
    if written.is_empty() {
        return None;
    }
    match was {
        Some(was) if opening::tidied(&was.value).0 == written => Some(was.clone()),
        _ => Some(Hours { value: written, source: opening::TYPED.into(), checked: today.to_string() }),
    }
}

/// Writes a card's hours through the contacts' writer, sends it, and plans
/// the office's tasks again. Nothing written when nothing changes.
fn write(qt: &QtThread, shared: &Arc<Shared>, path: &std::path::Path, hours: &CardHours) -> Result<bool, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let Some(written) = contacts::with_hours(&text, hours) else { return Ok(false) };
    vdir::write_item(path, &written)?;
    if let Some(account) = pim::account_of(path) {
        pim::nudge(shared, &account);
    }
    pim::show_pim(qt, shared);
    // The tasks that involve the office follow its hours: the plan read again.
    crate::work::show_work(qt, shared);
    Ok(true)
}

/// Saves the hours the form gives on a card. Returns {"ok"} or {"error"}.
pub(crate) fn save_contact_hours(qt: &QtThread, shared: &Arc<Shared>, key: &str, typed: &str) -> String {
    let fail = |e: String| serde_json::json!({ "error": e }).to_string();
    let Ok(typed) = serde_json::from_str::<Typed>(typed) else { return fail("?".into()) };
    let Some((path, contact)) = card(key) else { return fail(tr().text("contact-gone", None)) };
    if contact.read_only {
        return fail(tr().text("hours-read-only", None));
    }
    let today = sioul_core::zone::now().date();
    let mut rest = Vec::new();
    let open = typed_hours(&typed.open, contact.hours.open.as_ref(), today, &mut rest);
    let phone = typed_hours(&typed.phone, contact.hours.phone.as_ref(), today, &mut rest);
    let mut note = typed.note.trim().to_string();
    for part in rest {
        if !note.contains(&part) {
            note = if note.is_empty() { part } else { format!("{note}; {part}") };
        }
    }
    match write(qt, shared, &path, &CardHours { open, phone, note }) {
        Ok(changed) => {
            if changed {
                tell(qt, shared, tr().text("hours-saved", None));
            }
            serde_json::json!({ "ok": true }).to_string()
        }
        Err(e) => fail(e),
    }
}

/// "Call back…" on an office's card: the task "Call ‹name›", a call, that
/// office attached, needing it open, in the usual list: proposed from its
/// next opening. One press, no form. Returns {"uid"} or {"error"}.
pub(crate) fn call_back(qt: &QtThread, shared: &Arc<Shared>, key: &str) -> String {
    let fail = |e: String| serde_json::json!({ "error": e }).to_string();
    let Some((_, contact)) = card(key) else { return fail(tr().text("contact-gone", None)) };
    let title = say("hours-call-back-title", &[("name", contact.name.clone())]);
    let edit = sioul_core::tasks::TaskEdit {
        title: title.clone(),
        kind: "call".into(),
        office_hours: true,
        contacts: vec![sioul_core::tasks::ContactRef { name: contact.name.clone(), uri: sioul_core::links::contact_uri(contact.uid.trim()) }],
        ..sioul_core::tasks::TaskEdit::default()
    };
    match crate::work::create_task(qt, shared, &edit, "") {
        Ok(uid) => {
            tell(qt, shared, say("hours-call-back-made", &[("title", title)]));
            serde_json::json!({ "uid": uid }).to_string()
        }
        Err(e) => fail(e),
    }
}

/// What a lookup answers the window (`hours_found`): the offices found,
/// each with its hours in words; the sources that did not answer, said.
#[derive(Serialize)]
struct FoundView {
    #[serde(flatten)]
    found: Found,
    /// Its hours in words, one line per run of days.
    lines: Vec<String>,
    /// Its source, named.
    source_name: String,
}

/// Looks the hours of a card up, because the person pressed Look up: what
/// `asked` says (the words they saw), to the directory when the office is
/// in France and to OpenStreetMap, one request each, on a thread; the
/// answer comes in `hours_found`. Never for a person's card; never in the
/// demo, which answers from its examples, stays off the network and says so.
pub(crate) fn look_up(qt: &QtThread, key: &str, asked: &str) {
    let key = key.to_string();
    let answer = |qt: &QtThread, key: String, value: serde_json::Value| {
        let _ = qt.queue(move |mut sioul| sioul.as_mut().hours_found(QString::from(&key), QString::from(&value.to_string())));
    };
    let Some((_, contact)) = card(&key) else { return answer(qt, key, serde_json::json!({ "found": [], "failed": [tr().text("contact-gone", None)] })) };
    let (Some(_), Ok(asked)) = (Asked::of(&contact, false), serde_json::from_str::<Asked>(asked)) else { return answer(qt, key, serde_json::json!({ "found": [], "failed": [tr().text("hours-lookup-person", None)] })) };
    let qt = qt.clone();
    std::thread::spawn(move || {
        let language = tr().text("qt-locale", None).replace('_', "-");
        let mut found: Vec<Found> = Vec::new();
        let mut failed: Vec<String> = Vec::new();
        let mut said = |source: &str, result: Result<Vec<Found>, sioul_sync::SyncError>| match result {
            Ok(more) => found.extend(more),
            Err(e) => failed.push(say("hours-lookup-failed", &[("source", source.to_string()), ("error", e.sentence(tr(), source))])),
        };
        if offline() {
            // The demo's examples, read as a real answer is.
            if asked.france {
                said("service-public.gouv.fr", hourslookup::directory_answer(include_str!("../../sioul-core/tests/fixtures/hours-directory.json")).map_err(sioul_sync::SyncError::Server));
            }
            said("OpenStreetMap", hourslookup::map_answer(include_str!("../../sioul-core/tests/fixtures/hours-nominatim.json")).map_err(sioul_sync::SyncError::Server));
        } else {
            if asked.france {
                said("service-public.gouv.fr", sioul_sync::hours::directory(&asked));
            }
            said("OpenStreetMap", sioul_sync::hours::map(&asked, &language));
        }
        let views: Vec<FoundView> = found
            .into_iter()
            .map(|f| {
                let lines = opening::day_lines(&opening::parse(&f.hours).windows, tr());
                let source_name = tr().text(if f.source == opening::DIRECTORY { "hours-source-label-directory" } else { "hours-source-name-map" }, None);
                FoundView { found: f, lines, source_name }
            })
            .collect();
        answer(&qt, key, serde_json::json!({ "found": views, "failed": failed, "offline": offline() }));
    });
}

/// One office of the list picked: its hours on the card, with their source
/// and today (`hourslookup::picked`: nothing there emptied, the free line
/// added to). Returns {"ok", "offers": {"phone", "address"}} (what the card
/// lacks, offered, never written by itself) or {"error"}.
pub(crate) fn pick(qt: &QtThread, shared: &Arc<Shared>, key: &str, found: &str) -> String {
    #[derive(Deserialize)]
    struct Picked {
        source: String,
        #[serde(default)]
        hours: String,
        #[serde(default)]
        note: String,
        #[serde(default)]
        phone: String,
        #[serde(default)]
        address: String,
    }
    let fail = |e: String| serde_json::json!({ "error": e }).to_string();
    let Ok(picked) = serde_json::from_str::<Picked>(found) else { return fail("?".into()) };
    let Some((path, contact)) = card(key) else { return fail(tr().text("contact-gone", None)) };
    if contact.read_only {
        return fail(tr().text("hours-read-only", None));
    }
    let source = if picked.source == opening::DIRECTORY { opening::DIRECTORY } else { opening::MAP };
    let found = Found { source, hours: opening::tidied(&picked.hours).0, note: picked.note.clone(), ..Found::default() };
    let hours = hourslookup::picked(&contact.hours, &found, sioul_core::zone::now().date());
    if let Err(e) = write(qt, shared, &path, &hours) {
        return fail(e);
    }
    tell(qt, shared, tr().text("hours-saved", None));
    // The same number written otherwise ("+33 1 23…", "01 23…"): its last nine digits.
    let tail = |p: &str| {
        let digits: Vec<char> = p.chars().filter(char::is_ascii_digit).collect();
        digits[digits.len().saturating_sub(9)..].iter().collect::<String>()
    };
    let has_number = tail(&picked.phone).is_empty() || contact.phones.iter().any(|p| tail(&p.value) == tail(&picked.phone));
    let offers = serde_json::json!({
        "phone": if has_number { String::new() } else { picked.phone.trim().to_string() },
        "address": if contact.addresses.is_empty() { picked.address.trim().to_string() } else { String::new() },
    });
    serde_json::json!({ "ok": true, "offers": offers }).to_string()
}

/// A number or an address a lookup found, added to the card because the
/// person pressed its offer (`what`: "phone" or "address"). Returns {"ok"} or {"error"}.
pub(crate) fn add_to_card(qt: &QtThread, shared: &Arc<Shared>, key: &str, what: &str, value: &str) -> String {
    let fail = |e: String| serde_json::json!({ "error": e }).to_string();
    let Some((path, contact)) = card(key) else { return fail(tr().text("contact-gone", None)) };
    if contact.read_only {
        return fail(tr().text("hours-read-only", None));
    }
    let Ok(text) = std::fs::read_to_string(&path) else { return fail(tr().text("contact-gone", None)) };
    let property = if what == "address" { "ADR" } else { "TEL" };
    let Some(written) = contacts::with_value(&text, property, value) else { return serde_json::json!({ "ok": true }).to_string() };
    if let Err(e) = vdir::write_item(&path, &written) {
        return fail(e);
    }
    if let Some(account) = pim::account_of(&path) {
        pim::nudge(shared, &account);
    }
    tell(qt, shared, tr().text("contact-saved", None));
    pim::show_pim(qt, shared);
    serde_json::json!({ "ok": true }).to_string()
}
