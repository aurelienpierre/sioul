// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! An office's opening hours on its card, in the window (OpeningHours.qml):
//! what is true now, the hours in words, where they come from; changed by
//! hand; looked up when the person asks, from the administration's
//! directory and the map's search service (`sioul_core::hourslookup`), only
//! on a card that says it is an organisation's or already holds hours, which
//! any other card can be made with one link ("This is an office, a shop or
//! a practice"); "Call back…", a task that follows them. The card is written
//! only through the contacts' own writers (`contacts::with_hours`,
//! `contacts::as_office`), then sent, and the tasks that involve the office
//! are planned again (`work::show_work`); when it gets hours for the first
//! time by the person's act, its open calls and outings are ticked
//! (`work::tick_for_office`). Each entry's work is a function of its own
//! that needs no window (`card_saved`, `pick_read`, `call_back_edit`…), what
//! the tests run; the window's part (notices, views shown again) is around it.

use crate::backend::{QtThread, Shared, load_config, offline, say, tell, tr};
use crate::pim;
use cxx_qt_lib::QString;
use serde::{Deserialize, Serialize};
use sioul_core::contacts::{self, Contact};
use sioul_core::hourslookup::{self, Asked, Found};
use sioul_core::opening::{self, CardHours, Hours};
use sioul_core::vdir;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// One set of hours, as the card's section says it.
#[derive(Serialize, Default)]
struct HoursView {
    /// As the card keeps them.
    value: String,
    /// The short form the form shows, in the window's words ("lu-ve 9:00-12:00").
    short: String,
    /// One line per run of days: "Monday to Friday: 9:00–12:00".
    lines: Vec<String>,
    /// Closed on public holidays (`Hours::holidays_off`).
    holidays_off: bool,
    /// Sioul counts none of it: shown as written.
    unread: bool,
    /// Why, when there is more to say ("Di" in doubt, too long); "" otherwise.
    doubt: String,
    /// What it could not count, as written (cut at `opening::MOST_HOURS`).
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
    let followed = opening::Followed { windows: parsed.windows.clone(), holidays_off: hours.holidays_off(), whose: opening::Whose::Usual };
    HoursView {
        value: opening::cut(&hours.value, opening::MOST_HOURS),
        short: opening::short_form(&parsed.windows, tr()),
        lines: opening::day_lines(&parsed.windows, tr()),
        holidays_off: hours.holidays_off(),
        unread: parsed.windows.is_empty(),
        doubt: if parsed.doubt.is_empty() { String::new() } else { tr().text(parsed.doubt, None) },
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

/// Whether Sioul knows any public holidays: a subscribed calendar of them (Settings ▸ Hours).
fn holidays_known() -> bool {
    let config = load_config();
    sioul_core::subscribed::holidays(&config, sioul_core::zone::now().time_zone()).iter().any(|h| h.holidays == sioul_core::subscribed::Holidays::Public)
}

/// The card of a contact, read now, with its book.
fn card(key: &str) -> Option<(PathBuf, Contact)> {
    let path = pim::ours(key)?;
    let book = pim::collection_of(&path)?;
    let contact = contacts::read(&path, &book)?;
    Some((path, contact))
}

/// The person's region is France: the administration's directory is asked
/// for an office with no address there, or whose postal code's line is last.
fn region_france() -> bool {
    crate::eventplace::service() == sioul_sync::suggest::Service::Ban
}

/// Why the map's search service is not asked now: "off" (None chosen in
/// Contacts ⚙), "waiting" (it refused today), or "".
fn map_stopped() -> &'static str {
    match sioul_sync::geocode::stopped_now() {
        Some(sioul_sync::SyncError::Said(sioul_sync::geocode::SAID_OFF, _)) => "off",
        Some(_) => "waiting",
        None => "",
    }
}

/// A card's hours section: {name, card ("org", "person", ""), read_only,
/// open, phone (each a `HoursView` or null), note (its second lines' text
/// added, review 20), numbers, asked (what a lookup would send; null on a
/// card that is neither an organisation's nor holds hours), mark (the link
/// "This is an office, a shop or a practice" offered), sources (who it would
/// be sent to, in words; "" when nobody can be asked), map ("off",
/// "waiting" or ""), map_name, holidays_known, offline}. "" for a card that is gone.
/// Read each time the section shows and each minute after (what is true
/// now changes).
pub(crate) fn contact_hours(key: &str) -> String {
    let Some((_, contact)) = card(key) else { return String::new() };
    let now = sioul_core::zone::now();
    let holidays = holidays_now(&now);
    let asked = Asked::of(&contact, region_france());
    let map = map_stopped();
    let sources = asked.as_ref().map(|a| sources_named(a, map)).unwrap_or_default();
    serde_json::json!({
        "name": contact.name,
        "card": contact.card,
        "read_only": contact.read_only,
        "open": contact.hours.open.as_ref().map(|h| hours_view(h, &holidays, &now)),
        "phone": contact.hours.phone.as_ref().map(|h| hours_view(h, &holidays, &now)),
        "note": contact.hours.clone().folded().note,
        "numbers": contact.phones.iter().map(|p| p.value.clone()).collect::<Vec<_>>(),
        "mark": asked.is_none() && contact.card != "org" && !contact.read_only,
        "asked": asked,
        "sources": sources,
        "map": map,
        "map_name": map_name(),
        "holidays_known": holidays_known(),
        "offline": offline(),
    })
    .to_string()
}

/// The map's search service, named: "OpenStreetMap", or "the map's search
/// service (geo.example.org)" for another address chosen in Contacts ⚙.
fn map_name() -> String {
    match sioul_core::mapsearch::service(&load_config().map) {
        sioul_core::mapsearch::Service::At(address) if address != sioul_core::mapsearch::DEFAULT => {
            let host = address.strip_prefix("https://").unwrap_or(&address).split(['/', '?', '#']).next().unwrap_or("").to_string();
            say("hours-source-name-map-at", &[("host", host)])
        }
        _ => tr().text("hours-source-name-map", None),
    }
}

/// "the administration's directory (service-public.gouv.fr) and
/// OpenStreetMap", one of them, or "" when neither can be asked: the
/// directory for an office in France whose name has words of its own, the
/// map unless it is stopped (`map`: "off", "waiting").
fn sources_named(asked: &Asked, map: &str) -> String {
    let directory = asked.asks_directory().then(|| tr().text("hours-source-name-directory", None));
    let map = map.is_empty().then(map_name);
    match (directory, map) {
        (Some(directory), Some(map)) => say("hours-sources-both", &[("directory", directory), ("map", map)]),
        (Some(one), None) | (None, Some(one)) => one,
        (None, None) => String::new(),
    }
}

/// What the form typed reads as: {written, short (in the window's words),
/// lines, days (seven, Monday first, each "09:00-12:00, 14:00-17:00" or ""
/// when closed), rest, holidays_off, holidays_said, doubt}, for its line
/// "Reads as…", its rows day by day and its tick for public holidays.
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
        "short": opening::short_form(&parsed.windows, tr()),
        "lines": opening::day_lines(&parsed.windows, tr()),
        "days": days,
        "rest": parsed.rest,
        "holidays_off": parsed.holidays_off,
        "holidays_said": parsed.holidays_said,
        "doubt": if parsed.doubt.is_empty() { String::new() } else { tr().text(parsed.doubt, None) },
    })
    .to_string()
}

/// What the form gives back: the hours typed (the short form or the map's
/// syntax), the phone's ("" when they do not differ), the free line, and
/// its tick "Open on public holidays".
#[derive(Deserialize, Default)]
struct Typed {
    #[serde(default)]
    open: String,
    #[serde(default)]
    phone: String,
    #[serde(default)]
    note: String,
    #[serde(default)]
    holidays_open: bool,
}

/// Hours as typed: what Sioul counts, written in the map's syntax (`PH off`
/// unless the value or the tick says public holidays are open), typed by
/// you today. Hours that count the same as the card's (the same days, the
/// same public holidays) are the card's own, kept as they are with their
/// source, their day and the part Sioul cannot count (review 20: a save that
/// changes nothing writes nothing). What it cannot count goes to the free
/// line (`rest`), the card's former uncounted part too when they change or
/// are emptied. An empty field takes the hours off.
fn typed_hours(text: &str, was: Option<&Hours>, holidays_tick: bool, today: jiff::civil::Date, rest: &mut Vec<String>) -> Option<Hours> {
    // The card's part Sioul cannot count, when it counts the rest: shown apart in the form, so
    // moved to the free line when the hours change. Hours it counts none of were in the field itself.
    let card_rest = was.map(Hours::read).filter(|p| !p.windows.is_empty()).map(|p| p.rest.join("; ")).unwrap_or_default();
    if text.trim().is_empty() {
        if !card_rest.is_empty() {
            rest.push(card_rest);
        }
        return None;
    }
    let parsed = opening::parse(text);
    let holidays_off = if parsed.holidays_said { parsed.holidays_off } else { !holidays_tick };
    let typed_rest = parsed.rest.join("; ");
    if let Some(was) = was
        && was.read().windows == parsed.windows
        && (parsed.windows.is_empty() || was.holidays_off() == holidays_off)
    {
        let kept = was.read().rest;
        if !typed_rest.is_empty() && !kept.iter().any(|k| typed_rest.contains(k.as_str())) {
            rest.push(typed_rest);
        }
        return Some(was.clone());
    }
    if !typed_rest.is_empty() {
        rest.push(typed_rest);
    }
    if !card_rest.is_empty() {
        rest.push(card_rest);
    }
    if parsed.windows.is_empty() {
        return None;
    }
    Some(Hours { value: opening::written(&parsed.windows, holidays_off), source: opening::TYPED.into(), checked: today.to_string(), holidays_open: !holidays_off })
}

/// The card's hours once the form is saved (`typed`, as JSON) on `today`:
/// each set as `typed_hours` reads it, what Sioul cannot count added to the
/// free line (nothing typed dropped), the text of second lines folded in.
fn saved_hours(card: &CardHours, typed: &Typed, today: jiff::civil::Date) -> CardHours {
    let mut rest = Vec::new();
    let open = typed_hours(&typed.open, card.open.as_ref(), typed.holidays_open, today, &mut rest);
    let phone = typed_hours(&typed.phone, card.phone.as_ref(), typed.holidays_open, today, &mut rest);
    let mut note = opening::cut(typed.note.trim(), opening::MOST_NOTE);
    for part in rest {
        if !note.contains(&part) {
            note = if note.is_empty() { part } else { format!("{note}; {part}") };
        }
    }
    CardHours { open, phone, note, extra: card.extra.clone() }.folded()
}

/// What writing a card's hours did.
#[derive(Debug, Default, PartialEq, Eq)]
struct Written {
    /// The card changed (nothing written when its hours say the same).
    changed: bool,
    /// Its open calls and outings ticked, the card having got hours for the first time.
    ticked: usize,
}

/// Writes a card's hours through the contacts' writer, by the person's act
/// on this device, and sends it; when it gets hours Sioul counts for the
/// first time, its open calls and outings are ticked (`work::tick_for_office`).
/// Nothing written when nothing changes. No window needed: the tests run it.
fn card_saved(shared: &Shared, path: &Path, contact: &Contact, hours: &CardHours) -> Result<Written, String> {
    let text = crate::files::read_to_string(path).map_err(|e| e.to_string())?;
    let Some(written) = contacts::with_hours(&text, hours) else { return Ok(Written::default()) };
    vdir::write_item(path, &written)?;
    if let Some(account) = pim::account_of(path) {
        pim::nudge(shared, &account);
    }
    if let Ok(mut cache) = shared.loaded.lock() {
        *cache = None;
    }
    let first = !contact.hours.counts() && hours.counts();
    let ticked = if first { crate::work::tick_for_office(shared, &contact.uid, hours) } else { 0 };
    Ok(Written { changed: true, ticked })
}

/// The window told: the contacts and the plan read again.
fn shown(qt: &QtThread, shared: &Arc<Shared>, written: &Written) {
    if written.changed {
        pim::show_pim(qt, shared);
        // The tasks that involve the office follow its hours: the plan read again.
        crate::work::show_work(qt, shared);
    }
}

/// Saves the hours the form gives on a card. Returns {"ok", "changed", "ticked"} or {"error"}.
pub(crate) fn save_contact_hours(qt: &QtThread, shared: &Arc<Shared>, key: &str, typed: &str) -> String {
    match saved(shared, key, typed) {
        Ok(written) => {
            if written.changed {
                tell(qt, shared, if written.ticked > 0 { say("hours-saved-ticked", &[("count", written.ticked.to_string())]) } else { tr().text("hours-saved", None) });
            }
            shown(qt, shared, &written);
            serde_json::json!({ "ok": true, "changed": written.changed, "ticked": written.ticked }).to_string()
        }
        Err(e) => serde_json::json!({ "error": e }).to_string(),
    }
}

fn saved(shared: &Shared, key: &str, typed: &str) -> Result<Written, String> {
    let typed = serde_json::from_str::<Typed>(typed).map_err(|e| e.to_string())?;
    let (path, contact) = card(key).ok_or_else(|| tr().text("contact-gone", None))?;
    if contact.read_only {
        return Err(tr().text("hours-read-only", None));
    }
    let hours = saved_hours(&contact.hours, &typed, sioul_core::zone::now().date());
    card_saved(shared, &path, &contact, &hours)
}

/// What the mark is called among the changes to cards kept to be undone
/// (`sioul_core::duplicates::Done::kind`).
pub(crate) const MARKED: &str = "office";

/// "This is an office, a shop or a practice": the card marked as an
/// organisation's through the contacts' writer (`contacts::as_office`), so
/// that a lookup is offered on it; said with Undo, as the contacts' other
/// changes are, the card as it was kept thirty days with them
/// (`duplicates::put_back`; Contacts ▸ Duplicates, "Done lately"), so that a
/// press by mistake on a person's card is taken back. Returns {"ok"} or {"error"}.
pub(crate) fn mark_office(qt: &QtThread, shared: &Arc<Shared>, key: &str) -> String {
    match marked(shared, key) {
        Ok(Some((id, name))) => {
            pim::show_pim(qt, shared);
            crate::mail::offer_back(qt, shared, say("hours-marked", &[("name", name)]), move |qt, shared| crate::duplicates::put_back(qt, shared, &id).unwrap_or_else(|e| e));
            serde_json::json!({ "ok": true }).to_string()
        }
        Ok(None) => serde_json::json!({ "ok": true }).to_string(),
        Err(e) => serde_json::json!({ "error": e }).to_string(),
    }
}

/// The card marked, kept as it was among the changes to cards
/// (`sioul_core::duplicates::apply`): what was done's id and the card's
/// name; None when it said so already.
fn marked(shared: &Shared, key: &str) -> Result<Option<(String, String)>, String> {
    let (path, contact) = card(key).ok_or_else(|| tr().text("contact-gone", None))?;
    if contact.read_only {
        return Err(tr().text("hours-read-only", None));
    }
    let text = crate::files::read_to_string(&path).map_err(|e| e.to_string())?;
    let Some(written) = contacts::as_office(&text) else { return Ok(None) };
    let change = sioul_core::duplicates::Change { path: path.clone(), before: Some(text), after: Some(written) };
    let done = sioul_core::duplicates::apply(&sioul_core::duplicates::undo_dir(), MARKED, vec![contact.name.clone()], vec![change], jiff::Timestamp::now().as_second())?;
    if let Some(account) = pim::account_of(&path) {
        pim::nudge(shared, &account);
    }
    if let Ok(mut cache) = shared.loaded.lock() {
        *cache = None;
    }
    Ok(Some((done.id, contact.name)))
}

/// The task "Call back…" makes for a card: "Call ‹name›", a call, that card
/// attached; ticked "Needs an open office" only when the card is an office's
/// (an organisation's, or one with hours Sioul counts): a friend's card makes
/// a plain call (review 12).
fn call_back_edit(contact: &Contact) -> sioul_core::tasks::TaskEdit {
    sioul_core::tasks::TaskEdit {
        title: say("hours-call-back-title", &[("name", contact.name.clone())]),
        kind: "call".into(),
        office_hours: contact.card == "org" || contact.hours.counts(),
        contacts: vec![sioul_core::tasks::ContactRef { name: contact.name.clone(), uri: sioul_core::links::contact_uri(contact.uid.trim()) }],
        ..sioul_core::tasks::TaskEdit::default()
    }
}

/// The last "Call back…" pressed: its card and when.
static LAST_CALL_BACK: std::sync::Mutex<Option<(String, std::time::Instant)>> = std::sync::Mutex::new(None);

/// Whether a press of "Call back…" on `key` makes its task: not a second
/// press within a second (two quick presses make one task).
fn call_back_now(key: &str) -> bool {
    let mut last = LAST_CALL_BACK.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let again = last.as_ref().is_some_and(|(k, at)| k == key && at.elapsed() < std::time::Duration::from_secs(1));
    if !again {
        *last = Some((key.to_string(), std::time::Instant::now()));
    }
    !again
}

/// "Call back…" on a card: the task "Call ‹name›" (`call_back_edit`), in
/// the usual list; for an office, proposed from its next opening. One press,
/// no form. Returns {"uid"}, {"again"} for a second press within a second,
/// or {"error"}.
pub(crate) fn call_back(qt: &QtThread, shared: &Arc<Shared>, key: &str) -> String {
    let fail = |e: String| serde_json::json!({ "error": e }).to_string();
    let Some((_, contact)) = card(key) else { return fail(tr().text("contact-gone", None)) };
    if !call_back_now(key) {
        return serde_json::json!({ "again": true }).to_string();
    }
    let edit = call_back_edit(&contact);
    match crate::work::create_task(qt, shared, &edit, "") {
        Ok(uid) => {
            tell(qt, shared, say(if edit.office_hours { "hours-call-back-made" } else { "hours-call-back-made-plain" }, &[("title", edit.title.clone())]));
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

/// The words the window sends with a press of Look up: what the person saw
/// (`Asked`), and the press's number, which its answer carries back.
#[derive(Deserialize)]
struct Press {
    #[serde(flatten)]
    asked: Asked,
    #[serde(default)]
    press: u64,
}

/// What a press may ask, on the card as it is now: refused for a card that
/// is neither an organisation's nor holds hours (`Asked::of`, in the core:
/// never a person's name), and for words that are not the press's.
fn asked_for(contact: &Contact, typed: &str) -> Result<(Asked, u64), String> {
    let refused = || tr().text("hours-lookup-person", None);
    let card = Asked::of(contact, region_france()).ok_or_else(refused)?;
    let press: Press = serde_json::from_str(typed).map_err(|e| e.to_string())?;
    // What may go to the directory is what the card says of where it is, as Sioul reads it.
    Ok((Asked { france: card.france && hourslookup::in_france(&press.asked.place, region_france()), ..press.asked }, press.press))
}

/// Looks the hours of a card up, because the person pressed Look up: what
/// they saw (the words, which they may have corrected), to the directory
/// when the office is in France and to the map's search service unless it
/// is stopped, both at once, one request each, on threads; the answer comes
/// in `hours_found`, with the press's number (an earlier press never fills a
/// later list). Never for a card that is neither an organisation's nor holds
/// hours; never in the demo, which answers from its examples, stays off the
/// network and says so.
pub(crate) fn look_up(qt: &QtThread, key: &str, typed: &str) {
    let key = key.to_string();
    let answer = |qt: &QtThread, key: String, value: serde_json::Value| {
        let _ = qt.queue(move |mut sioul| sioul.as_mut().hours_found(QString::from(&key), QString::from(&value.to_string())));
    };
    let press = serde_json::from_str::<serde_json::Value>(typed).ok().and_then(|v| v["press"].as_u64()).unwrap_or(0);
    let Some((_, contact)) = card(&key) else { return answer(qt, key, serde_json::json!({ "press": press, "found": [], "failed": [tr().text("contact-gone", None)] })) };
    let asked = match asked_for(&contact, typed) {
        Ok((asked, _)) => asked,
        Err(e) => return answer(qt, key, serde_json::json!({ "press": press, "found": [], "failed": [e] })),
    };
    let map_stopped = sioul_sync::geocode::stopped_now();
    let qt = qt.clone();
    std::thread::spawn(move || {
        let language = tr().text("qt-locale", None).replace('_', "-");
        let mut found: Vec<Found> = Vec::new();
        let mut failed: Vec<String> = Vec::new();
        let mut said = |source: &str, result: Result<Vec<Found>, sioul_sync::SyncError>| match result {
            Ok(more) => found.extend(more),
            // Sioul's own sentence (the map's service refused): said as it is.
            Err(sioul_sync::SyncError::Said(id, detail)) => failed.push(sioul_sync::SyncError::Said(id, detail).sentence(tr(), source)),
            Err(e) => failed.push(say("hours-lookup-failed", &[("source", source.to_string()), ("error", e.sentence(tr(), source))])),
        };
        if offline() {
            // The demo's examples, read as a real answer is.
            if asked.asks_directory() {
                said("service-public.gouv.fr", hourslookup::directory_answer(include_str!("../../sioul-core/tests/fixtures/hours-directory.json")).map_err(sioul_sync::SyncError::Server));
            }
            if map_stopped.is_none() {
                said("OpenStreetMap", hourslookup::map_answer(include_str!("../../sioul-core/tests/fixtures/hours-nominatim.json")).map_err(sioul_sync::SyncError::Server));
            }
        } else {
            // Both at once: the slower never waits for the other.
            let directory = asked.asks_directory().then(|| {
                let asked = asked.clone();
                std::thread::spawn(move || sioul_sync::hours::directory(&asked))
            });
            let map = map_stopped.is_none().then(|| {
                let asked = asked.clone();
                std::thread::spawn(move || sioul_sync::hours::map(&asked, &language))
            });
            if let Some(directory) = directory {
                said("service-public.gouv.fr", directory.join().unwrap_or_else(|_| Err(sioul_sync::SyncError::Server("?".into()))));
            }
            if let Some(map) = map {
                said("OpenStreetMap", map.join().unwrap_or_else(|_| Err(sioul_sync::SyncError::Server("?".into()))));
            }
        }
        let views: Vec<FoundView> = found
            .into_iter()
            .map(|f| {
                let lines = opening::day_lines(&opening::parse(&f.hours).windows, tr());
                let source_name = tr().text(if f.source == opening::DIRECTORY { "hours-source-label-directory" } else { "hours-source-name-map" }, None);
                FoundView { found: f, lines, source_name }
            })
            .collect();
        answer(&qt, key, serde_json::json!({ "press": press, "found": views, "failed": failed, "offline": offline() }));
    });
}

/// One office of the list as the window gives it back, read: its source
/// one of the two, its hours as Sioul writes them, the rest as given.
fn found_of(text: &str) -> Result<Found, String> {
    let mut found: Found = serde_json::from_str(text).map_err(|e| e.to_string())?;
    found.source = if found.source == opening::DIRECTORY { opening::DIRECTORY.into() } else { opening::MAP.into() };
    let parsed = opening::parse(&found.hours);
    found.holidays_open = found.holidays_open || (parsed.holidays_said && !parsed.holidays_off);
    found.hours = opening::written(&parsed.windows, !found.holidays_open);
    Ok(found)
}

/// What the card lacks that the office picked has: {phone, address}, each
/// "" when the card has it already (the same number written otherwise,
/// "+33 1 23…" and "01 23…", by its last nine digits).
fn offers(contact: &Contact, found: &Found) -> serde_json::Value {
    let tail = |p: &str| {
        let digits: Vec<char> = p.chars().filter(char::is_ascii_digit).collect();
        digits[digits.len().saturating_sub(9)..].iter().collect::<String>()
    };
    let has_number = tail(&found.phone).is_empty() || contact.phones.iter().any(|p| tail(&p.value) == tail(&found.phone));
    serde_json::json!({
        "phone": if has_number { String::new() } else { found.phone.trim().to_string() },
        "address": if contact.addresses.is_empty() { found.address.trim().to_string() } else { String::new() },
    })
}

/// One office of the list picked: its hours on the card, with their source
/// and today (`hourslookup::picked`: nothing there emptied, the free line
/// added to, the tick for public holidays as the source says). Returns
/// {"ok", "changed", "ticked", "offers": {"phone", "address"}} (what the card
/// lacks, offered, never written by itself) or {"error"}.
pub(crate) fn pick(qt: &QtThread, shared: &Arc<Shared>, key: &str, found: &str) -> String {
    match picked(shared, key, found) {
        Ok((written, offers)) => {
            // Said only when something was written (review 17).
            if written.changed {
                tell(qt, shared, if written.ticked > 0 { say("hours-saved-ticked", &[("count", written.ticked.to_string())]) } else { tr().text("hours-saved", None) });
            }
            shown(qt, shared, &written);
            serde_json::json!({ "ok": true, "changed": written.changed, "ticked": written.ticked, "offers": offers }).to_string()
        }
        Err(e) => serde_json::json!({ "error": e }).to_string(),
    }
}

fn picked(shared: &Shared, key: &str, found: &str) -> Result<(Written, serde_json::Value), String> {
    let found = found_of(found)?;
    let (path, contact) = card(key).ok_or_else(|| tr().text("contact-gone", None))?;
    if contact.read_only {
        return Err(tr().text("hours-read-only", None));
    }
    let hours = hourslookup::picked(&contact.hours, &found, sioul_core::zone::now().date()).folded();
    let written = card_saved(shared, &path, &contact, &hours)?;
    Ok((written, offers(&contact, &found)))
}

/// A number or an address a lookup found, added to the card because the
/// person pressed its offer (`what`: "phone" or "address"). Returns {"ok"} or {"error"}.
pub(crate) fn add_to_card(qt: &QtThread, shared: &Arc<Shared>, key: &str, what: &str, value: &str) -> String {
    match added(shared, key, what, value) {
        Ok(changed) => {
            if changed {
                tell(qt, shared, tr().text("contact-saved", None));
                pim::show_pim(qt, shared);
            }
            serde_json::json!({ "ok": true }).to_string()
        }
        Err(e) => serde_json::json!({ "error": e }).to_string(),
    }
}

fn added(shared: &Shared, key: &str, what: &str, value: &str) -> Result<bool, String> {
    let (path, contact) = card(key).ok_or_else(|| tr().text("contact-gone", None))?;
    if contact.read_only {
        return Err(tr().text("hours-read-only", None));
    }
    let text = crate::files::read_to_string(&path).map_err(|_| tr().text("contact-gone", None))?;
    let property = if what == "address" { "ADR" } else { "TEL" };
    let Some(written) = contacts::with_value(&text, property, value) else { return Ok(false) };
    vdir::write_item(&path, &written)?;
    if let Some(account) = pim::account_of(&path) {
        pim::nudge(shared, &account);
    }
    if let Ok(mut cache) = shared.loaded.lock() {
        *cache = None;
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sioul_core::tasks::{self, TaskEdit};

    /// An address book and a task list of the test's own, in the stand-in
    /// folders; a card written there as a sync writes it.
    struct Place {
        book: vdir::Collection,
        list: vdir::Collection,
    }

    impl Place {
        fn new(name: &str) -> Place {
            sioul_core::config::stand_ins_only();
            let book = vdir::create(vdir::Kind::Contacts, vdir::LOCAL, &format!("{name} book"), None, &[]).unwrap();
            let list = vdir::create(vdir::Kind::Calendars, vdir::LOCAL, &format!("{name} list"), None, &["VTODO"]).unwrap();
            Place { book, list }
        }

        fn card(&self, uid: &str, lines: &str) -> String {
            let path = self.book.dir.join(format!("{uid}.vcf"));
            vdir::write_item(&path, &format!("BEGIN:VCARD\r\nVERSION:3.0\r\nUID:{uid}\r\n{lines}END:VCARD\r\n")).unwrap();
            path.display().to_string()
        }

        fn task(&self, uid: &str, kind: &str, office: &str) -> PathBuf {
            let edit = TaskEdit { title: format!("Task {uid}"), kind: kind.into(), contacts: vec![tasks::ContactRef { name: office.into(), uri: sioul_core::links::contact_uri(office) }], ..TaskEdit::default() };
            let text = tasks::new_task(&edit, uid, &jiff::tz::TimeZone::system(), &jiff::Zoned::now()).unwrap();
            let path = self.list.dir.join(format!("{uid}.ics"));
            vdir::write_item(&path, &text).unwrap();
            path
        }

        fn ticked(path: &Path) -> bool {
            tasks::task_of_text(&std::fs::read_to_string(path).unwrap(), &jiff::tz::TimeZone::system()).unwrap().office_hours
        }
    }

    impl Drop for Place {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.book.dir);
            let _ = std::fs::remove_dir_all(&self.list.dir);
        }
    }

    fn typed(open: &str, note: &str, holidays_open: bool) -> String {
        serde_json::json!({ "open": open, "phone": "", "note": note, "holidays_open": holidays_open }).to_string()
    }

    /// The form saved: what Sioul counts written in the map's syntax, what it
    /// cannot count in the free line, the tick for public holidays kept on
    /// the line; saved again unchanged, nothing written, the card's own
    /// uncounted part left where it was (review 20); the card's office's
    /// open calls and outings ticked the first time it gets hours, and only
    /// then, so that an untick holds (review 3).
    #[test]
    fn the_form_saved_on_a_card() {
        let place = Place::new("Offices form");
        let shared = Shared::default();
        let key = place.card("form-hall", "FN:Example town hall\r\nORG:Example town hall\r\nX-OTHER-APP:kept\r\n");
        let call = place.task("form-call", "call", "form-hall");
        let letter = place.task("form-letter", "write", "form-hall");
        let written = saved(&shared, &key, &typed("lu-ve 9-12, 14-17; fermé en août", "By appointment", false)).unwrap();
        assert_eq!(written, Written { changed: true, ticked: 1 });
        let (_, contact) = card(&key).unwrap();
        let open = contact.hours.open.clone().unwrap();
        assert_eq!((open.value.as_str(), open.source.as_str()), ("Mo-Fr 09:00-12:00,14:00-17:00; PH off", opening::TYPED));
        assert_eq!(contact.hours.note, "By appointment; fermé en août");
        assert!(std::fs::read_to_string(&key).unwrap().contains("X-OTHER-APP:kept"));
        assert!(Place::ticked(&call) && !Place::ticked(&letter), "the call follows its office now; a letter needs none");
        // Unticked by the person, then the hours changed: the untick holds.
        let unticked = tasks::apply(&std::fs::read_to_string(&call).unwrap(), &TaskEdit { office_hours: false, ..TaskEdit::of(&tasks::task_of_text(&std::fs::read_to_string(&call).unwrap(), &jiff::tz::TimeZone::system()).unwrap()) }, &jiff::tz::TimeZone::system(), &jiff::Zoned::now()).unwrap();
        vdir::write_item(&call, &unticked).unwrap();
        let again = saved(&shared, &key, &typed("lu-ve 9-12", "By appointment; fermé en août", false)).unwrap();
        assert_eq!(again, Written { changed: true, ticked: 0 });
        assert!(!Place::ticked(&call));
        // Saved unchanged, written the window's way: nothing written.
        let before = std::fs::read_to_string(&key).unwrap();
        assert_eq!(saved(&shared, &key, &typed("lu-ve 9:00-12:00", "By appointment; fermé en août", false)).unwrap(), Written::default());
        assert_eq!(std::fs::read_to_string(&key).unwrap(), before);
        // Open on public holidays: the tick kept on the line, no PH off.
        saved(&shared, &key, &typed("lu-di 8-20", "", true)).unwrap();
        let open = card(&key).unwrap().1.hours.open.unwrap();
        assert!(open.holidays_open && !open.value.contains("PH"), "{open:?}");
        // The field emptied: the hours taken off the card.
        saved(&shared, &key, &typed("", "", false)).unwrap();
        assert!(card(&key).unwrap().1.hours.is_empty());
    }

    /// A card from another program, with a part Sioul cannot count, and a
    /// second line of hours: shown, kept where it is while the person
    /// changes nothing in its hours; the second line's text in the free line
    /// at the person's save (review 20).
    #[test]
    fn what_another_program_wrote_is_never_lost() {
        let place = Place::new("Offices other");
        let shared = Shared::default();
        let key = place.card("other-hall", "FN:Example practice\r\nX-SIOUL-OPENING-HOURS:Mo-Fr 09:00-12:00; Aug off\r\nX-SIOUL-OPENING-HOURS:Sa 10:00-12:00\r\n");
        let shown: serde_json::Value = serde_json::from_str(&contact_hours(&key)).unwrap();
        assert_eq!(shown["note"], "Sa 10:00-12:00", "the second line's text shown in the free line");
        // The short form in the window's words ("mo-fr 9:00-12:00", "lu-ve 9:00-12:00").
        let short = opening::short_form(&opening::parse("Mo-Fr 09:00-12:00").windows, tr());
        assert_eq!(shown["open"]["short"], short.as_str());
        // Saved with the hours as the form shows them: the uncounted part stays in the value.
        saved(&shared, &key, &typed(&short, "Sa 10:00-12:00", false)).unwrap();
        let text = std::fs::read_to_string(&key).unwrap();
        assert!(text.contains("X-SIOUL-OPENING-HOURS:Mo-Fr 09:00-12:00\\; Aug off") && text.matches("X-SIOUL-OPENING-HOURS").count() == 1, "{text}");
        assert!(text.contains("X-SIOUL-HOURS-NOTE:Sa 10:00-12:00"), "{text}");
        // The hours changed: the former uncounted part goes to the free line, never dropped.
        saved(&shared, &key, &typed("mo-fr 9:00-13:00", "Sa 10:00-12:00", false)).unwrap();
        let hours = card(&key).unwrap().1.hours;
        assert_eq!(hours.open.unwrap().value, "Mo-Fr 09:00-13:00; PH off");
        assert_eq!(hours.note, "Sa 10:00-12:00; Aug off");
    }

    /// A card that arrives by sync with hours ticks nothing: the tasks are
    /// read and planned again as an arrival makes them, and no task's file
    /// changes; the same hours saved by the person tick it (review 3).
    #[test]
    fn a_card_arriving_by_sync_ticks_nothing() {
        let place = Place::new("Offices sync");
        let shared = Shared::default();
        let call = place.task("sync-call", "call", "sync-hall");
        let before = std::fs::read_to_string(&call).unwrap();
        // Written into its book as a sync writes it.
        let key = place.card("sync-hall", "FN:Example tax office\r\nORG:Example tax office\r\nX-SIOUL-OPENING-HOURS:Mo-Fr 09:00-12:00\\; PH off\r\n");
        assert!(crate::changes::remade(&[sioul_core::changes::Kind::Contacts].into_iter().collect()).work, "the plan is made again");
        crate::work::publish_now(&shared);
        assert_eq!(std::fs::read_to_string(&call).unwrap(), before, "no task written");
        assert!(!Place::ticked(&call));
        crate::work::forget_plan();
        // The person's own save of hours on a card that had none ticks it.
        let other = place.card("sync-other", "FN:Example office\r\nORG:Example office\r\n");
        let tied = place.task("sync-tied", "out", "sync-other");
        assert_eq!(saved(&shared, &other, &typed("mo-fr 9-17", "", false)).unwrap().ticked, 1);
        assert!(Place::ticked(&tied));
        assert!(!key.is_empty());
    }

    /// "This is an office, a shop or a practice": the card marked through
    /// the contacts' writer, a lookup offered from then on; a person's card
    /// read from its text offers none, and a press for it is refused (review 1).
    #[test]
    fn a_lookup_only_for_an_office_s_card() {
        let place = Place::new("Offices mark");
        let shared = Shared::default();
        let key = place.card("mark-jeanne", "FN:Jeanne Exemple\r\nN:Exemple;Jeanne;;;\r\nTEL:01 23 45 67 89\r\nADR:;;1 rue de l'Exemple;Exempleville;;99999;\r\n");
        let shown: serde_json::Value = serde_json::from_str(&contact_hours(&key)).unwrap();
        assert!(shown["asked"].is_null() && shown["mark"] == true, "{shown}");
        let press = serde_json::json!({ "name": "Jeanne Exemple", "place": "99999 Exempleville", "france": true, "press": 1 }).to_string();
        assert!(asked_for(&card(&key).unwrap().1, &press).is_err(), "nothing can be asked for it");
        assert!(marked(&shared, &key).unwrap().is_some());
        let shown: serde_json::Value = serde_json::from_str(&contact_hours(&key)).unwrap();
        assert!(!shown["asked"].is_null() && shown["mark"] == false, "{shown}");
        assert_eq!(shown["card"], "org");
        let (asked, number) = asked_for(&card(&key).unwrap().1, &press).unwrap();
        assert_eq!((asked.name.as_str(), number), ("Jeanne Exemple", 1));
        assert!(marked(&shared, &key).unwrap().is_none(), "marked once");
        // Words typed in the dialog that say another country: the directory is not asked.
        let abroad = serde_json::json!({ "name": "Jeanne Exemple", "place": "1 Example Street\nExampletown\nUnited Kingdom", "france": true, "press": 2 }).to_string();
        assert!(!asked_for(&card(&key).unwrap().1, &abroad).unwrap().0.france);
    }

    /// The mark pressed by mistake on a person's card: kept with the
    /// contacts' other changes, its Undo (`duplicates::put_back`, the core's
    /// `duplicates::undo`) puts the card back byte for byte, a person's
    /// again, no lookup offered; a card changed since is set aside first,
    /// never lost.
    #[test]
    fn the_mark_is_taken_back() {
        let place = Place::new("Offices unmark");
        let shared = Shared::default();
        let key = place.card("unmark-jeanne", "FN:Jeanne Exemple\r\nN:Exemple;Jeanne;;;\r\nORG:\r\nX-OTHER-APP:kept\r\n");
        let before = std::fs::read_to_string(&key).unwrap();
        let (id, name) = marked(&shared, &key).unwrap().unwrap();
        assert_eq!(name, "Jeanne Exemple");
        assert_ne!(std::fs::read_to_string(&key).unwrap(), before);
        let dir = sioul_core::duplicates::undo_dir();
        let kept = sioul_core::duplicates::done(&dir, jiff::Timestamp::now().as_second());
        assert!(kept.iter().any(|d| d.id == id && d.kind == MARKED && d.names == ["Jeanne Exemple"]), "kept with the contacts' changes: {kept:?}");
        let (touched, aside) = sioul_core::duplicates::undo(&dir, &id, &vdir::Kind::Contacts.root()).unwrap();
        assert_eq!((touched, aside), (vec![PathBuf::from(&key)], 0));
        assert_eq!(std::fs::read_to_string(&key).unwrap(), before, "byte for byte");
        let shown: serde_json::Value = serde_json::from_str(&contact_hours(&key)).unwrap();
        assert!(shown["asked"].is_null() && shown["mark"] == true && shown["card"] != "org", "a person's card again: {shown}");
        assert!(sioul_core::duplicates::undo(&dir, &id, &vdir::Kind::Contacts.root()).is_err(), "undone once");
        // Marked again, then changed elsewhere (a sync): Undo puts it back, the newer version set aside.
        let (id, _) = marked(&shared, &key).unwrap().unwrap();
        let elsewhere = std::fs::read_to_string(&key).unwrap().replace("X-OTHER-APP:kept", "X-OTHER-APP:changed");
        vdir::write_item(Path::new(&key), &elsewhere).unwrap();
        let (_, aside) = sioul_core::duplicates::undo(&dir, &id, &vdir::Kind::Contacts.root()).unwrap();
        assert_eq!(aside, 1);
        assert_eq!(std::fs::read_to_string(&key).unwrap(), before);
    }

    /// "Call back…": on an office's card, a call ticked "Needs an open
    /// office"; on a card that is not an office's and has no hours, a plain
    /// call; a second press within a second makes nothing (review 12).
    #[test]
    fn call_back_makes_the_right_task_once() {
        let contact = |card: &'static str, hours: Option<&str>| Contact { uid: "x".into(), name: "Example".into(), card, hours: CardHours { open: hours.map(|v| Hours { value: v.into(), ..Hours::default() }), ..CardHours::default() }, ..Contact::default() };
        assert!(call_back_edit(&contact("org", None)).office_hours);
        assert!(call_back_edit(&contact("", Some("Mo-Fr 09:00-12:00"))).office_hours);
        let friend = call_back_edit(&contact("", None));
        assert!(!friend.office_hours && friend.kind == "call" && friend.title.contains("Example"));
        assert!(call_back_now("/books/a.vcf") && !call_back_now("/books/a.vcf"), "twice in a second: once");
        assert!(call_back_now("/books/b.vcf"), "another card");
    }

    /// A pick: its hours written with their source and the day, the tick for
    /// public holidays as the source says, "kept on the card" said only when
    /// something changed (review 17), what the card lacks offered, never
    /// written by itself.
    #[test]
    fn a_pick_writes_only_what_changes() {
        let place = Place::new("Offices pick");
        let shared = Shared::default();
        let key = place.card("pick-hall", "FN:Example town hall\r\nORG:Example town hall\r\nTEL:01 23 45 67 89\r\n");
        let found = serde_json::json!({ "source": "openstreetmap.org", "name": "Example town hall", "address": "1 Place de l'Exemple\n99999 Exempleville", "phone": "+33 1 23 45 67 89", "hours": "Mo-Su 08:00-20:00", "note": "Aug off", "holidays_open": true }).to_string();
        let (written, offers) = picked(&shared, &key, &found).unwrap();
        assert!(written.changed);
        assert_eq!((offers["phone"].as_str(), offers["address"].as_str()), (Some(""), Some("1 Place de l'Exemple\n99999 Exempleville")), "the same number written otherwise is not offered");
        let hours = card(&key).unwrap().1.hours;
        let open = hours.open.unwrap();
        assert_eq!((open.source.as_str(), open.holidays_open), (opening::MAP, true));
        assert_eq!(hours.note, "Aug off");
        assert!(!picked(&shared, &key, &found).unwrap().0.changed, "picked again: nothing written");
        // Its address added only on the press, once.
        assert!(added(&shared, &key, "address", "1 Place de l'Exemple\n99999 Exempleville").unwrap());
        assert!(!added(&shared, &key, "address", "1 Place de l'Exemple\n99999 Exempleville").unwrap());
        // A source that is neither: read as the map, never as the directory.
        assert_eq!(found_of(&serde_json::json!({ "source": "elsewhere", "name": "x", "hours": "" }).to_string()).unwrap().source, opening::MAP);
    }

    /// The form's reading: the short form in the window's words, a doubt said.
    #[test]
    fn the_form_reads_what_is_typed() {
        let read: serde_json::Value = serde_json::from_str(&read_hours("Mo-Fr 09:00-12:00; PH off")).unwrap();
        assert_eq!(read["days"][0], "09:00-12:00");
        assert_eq!(read["days"][5], "");
        assert!(read["holidays_said"] == true && read["holidays_off"] == true);
        let doubt: serde_json::Value = serde_json::from_str(&read_hours("Di-Fr 09:00-12:00")).unwrap();
        assert!(doubt["lines"].as_array().unwrap().is_empty() && !doubt["doubt"].as_str().unwrap().is_empty());
        let always: serde_json::Value = serde_json::from_str(&read_hours("24/7")).unwrap();
        assert!(always["holidays_said"] == true && always["holidays_off"] == false, "24/7 ticks Open on public holidays");
    }
}
