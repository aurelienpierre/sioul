// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The notices on this device (docs/porch.md, "Notices"; the rules are
//! `sioul_core::notices`).
//!
//! - **The words** said here (Dismiss, Mark read, Done), each in this
//!   device's own log, which the sharing carries, with ten seconds to take
//!   it back (`call`'s "say" and "undo"). Read reads the item too, each kind
//!   through its own path (`Paths`): a message gets `\Seen` here and on its
//!   server at once; a text's conversation is read in Texts once its ten
//!   seconds are over (`PENDING`), since that mark cannot be taken back.
//! - **The Porch's own sections** that are not mail (`view`): the sites'
//!   notifications, the money of the week, the paper letters, each a row of
//!   notices; "From your phone" comes from the calls' and the phone
//!   messages' own views, which read the same words (`calls::view`,
//!   `phonemsgs::view`).
//! - **System notifications follow notices**: new mail's notification is
//!   taken away where it shows once each message it told is dismissed, read
//!   or done on any device (`posted`, `settled`, `withdraw_if_settled`).

use crate::backend::{QtThread, Shared, load_config, say, tr};
use serde_json::{Value, json};
use sioul_core::config::state_dir;
use sioul_core::notices::{self as rules, Asked, Kind, Logs, Mark, Notice, Posted, ReadPath, State, States, Told};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// How long a word waits to be taken back (as Seen's did).
const UNDO: Duration = Duration::from_secs(10);

/// This device's name in the sharing (share/here.toml), made once.
pub(crate) fn here_id() -> String {
    sioul_sync::share::Here::load(&state_dir()).id
}

/// This device's logs; "this-device" before it shares (its words stay here).
pub(crate) fn logs() -> Logs {
    Logs::here(&here_id())
}

/// Every word said on any device, the calls' and the phone messages'
/// older Seen logs named as notices by `named` (a source's id → its notice).
pub(crate) fn states(named: &dyn Fn(Kind, &str) -> Option<String>) -> States {
    logs().states(named)
}

/// The calls' or the phone messages' Seen as the notices' words make it:
/// the source ids (`pairs`: each and its notice) settled on any device, an
/// older Sioul's Seen among them; their old words forgotten once a day.
pub(crate) fn seen_of(kind: Kind, pairs: &std::collections::BTreeMap<String, String>) -> sioul_core::calls::Seen {
    let states = states(&|k, raw| if k == kind { pairs.get(raw).cloned() } else { None });
    forget_once_a_day(&[kind], &pairs.values().cloned().collect::<Vec<_>>());
    sioul_core::calls::Seen { seen: pairs.iter().filter(|(_, id)| states.of(id, State::default()).mark.settled()).map(|(raw, _)| raw.clone()).collect() }
}

/// A word said of notices without a row's asking (a conversation read in
/// Texts, an app's Mark as read handed on the phone): in both logs, nothing
/// else read. Whether it was written.
pub(crate) fn say_quietly(kind: Kind, ids: Vec<String>, mark: Mark) -> bool {
    struct Nothing;
    impl ReadPath for Nothing {
        fn read(&mut self, _: Kind, _: &[String]) -> Result<(), String> {
            Ok(())
        }
        fn unread(&mut self, _: Kind, _: &[String]) {}
    }
    if ids.is_empty() {
        return false;
    }
    let asked = Asked { kind: kind.as_str().into(), ids, targets: Vec::new() };
    match logs().say(&asked, mark, &mut Nothing, jiff::Timestamp::now().as_millisecond()) {
        Ok(at) => at > 0,
        Err(e) => {
            eprintln!("sioul: notices: {e}");
            false
        }
    }
}

/// A text's conversation to read once its word's ten seconds are over: when
/// the word was said, and the conversations.
static PENDING: Mutex<Vec<(i64, Vec<String>)>> = Mutex::new(Vec::new());

/// The texts' Read, done now: its ten seconds over, or Sioul closing.
fn read_texts(at: i64) {
    let due: Vec<(i64, Vec<String>)> = {
        let Ok(mut pending) = PENDING.lock() else { return };
        let (due, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut *pending).into_iter().partition(|(when, _)| at == 0 || *when == at);
        *pending = kept;
        due
    };
    for conversation in due.into_iter().flat_map(|(_, c)| c) {
        crate::texts::read(&conversation);
    }
}

/// Sioul closing: what waits for its ten seconds is done now.
pub(crate) fn flush() {
    read_texts(0);
}

/// Each kind's own read path, on this device.
struct Paths<'a> {
    qt: &'a QtThread,
    shared: &'a Arc<Shared>,
    /// When the word is said (a text's waits for its ten seconds).
    at: i64,
}

impl ReadPath for Paths<'_> {
    fn read(&mut self, kind: Kind, targets: &[String]) -> Result<(), String> {
        match kind {
            // The client's own path: \Seen here at once, on the server at its next look.
            Kind::Mail => {
                for key in targets {
                    crate::mail::act(self.qt, self.shared, key, "read", "");
                }
            }
            // Read in Texts once the ten seconds are over: that mark is never taken back.
            Kind::Text | Kind::Phone => {
                let conversations: Vec<String> = targets.iter().filter(|t| !t.is_empty()).cloned().collect();
                if !conversations.is_empty()
                    && let Ok(mut pending) = PENDING.lock()
                {
                    pending.push((self.at, conversations));
                    let at = self.at;
                    std::thread::spawn(move || {
                        std::thread::sleep(UNDO);
                        read_texts(at);
                    });
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn unread(&mut self, kind: Kind, targets: &[String]) {
        match kind {
            Kind::Mail => {
                for key in targets {
                    crate::mail::act(self.qt, self.shared, key, "unread", "");
                }
            }
            // Not read in Texts yet: it no longer will be.
            Kind::Text | Kind::Phone => {
                if let Ok(mut pending) = PENDING.lock() {
                    pending.retain(|(when, _)| *when != self.at);
                }
            }
            _ => {}
        }
    }
}

/// The word a verb says.
fn mark_of(verb: &str) -> Option<Mark> {
    match verb {
        "dismiss" => Some(Mark::Dismissed),
        "read" => Some(Mark::Read),
        "done" => Some(Mark::Done),
        _ => None,
    }
}

/// What the window asks (`Sioul::notices`): "view" (the Porch's sections
/// that are not mail, `view`); "agenda" (the agenda's conflicts as rows: the
/// `overlaps` property given, `agenda_rows`); "dismiss", "read", "done" {kind, ids,
/// targets}: the word said, {at, said} to take it back; "undo" {kind, ids,
/// targets, mark, at}: taken back within its ten seconds. Answers its JSON,
/// and whether what travels changed (to share now).
pub(crate) fn call(qt: &QtThread, shared: &Arc<Shared>, verb: &str, json: &str) -> (String, bool) {
    let asked: Value = serde_json::from_str(json).unwrap_or_default();
    let row: Asked = serde_json::from_value(asked.clone()).unwrap_or_default();
    let now = jiff::Timestamp::now().as_millisecond();
    if verb == "view" {
        return (view().to_string(), false);
    }
    if verb == "agenda" {
        return (agenda_rows(json, &sioul_core::zone::now()).to_string(), false);
    }
    if verb == "undo" {
        let mark = asked["mark"].as_str().and_then(Mark::parse).unwrap_or(Mark::Dismissed);
        let at = asked["at"].as_i64().unwrap_or(0);
        let mut paths = Paths { qt, shared, at };
        return match logs().take_back(&row, mark, at, &mut paths) {
            Ok(gone) => (json!({ "said": "", "at": 0 }).to_string(), gone),
            Err(e) => (json!({ "said": e, "at": 0 }).to_string(), false),
        };
    }
    let Some(mark) = mark_of(verb) else { return (json!({}).to_string(), false) };
    let mut paths = Paths { qt, shared, at: now };
    match logs().say(&row, mark, &mut paths, now) {
        Ok(at) => (json!({ "said": tr().text(&format!("notice-said-{}", mark.as_str()), None), "at": at, "mark": mark.as_str() }).to_string(), at > 0),
        Err(e) => (json!({ "said": e, "at": 0 }).to_string(), false),
    }
}

/// The Porch's mail (`backend::compute`): each message's notice as the words
/// said on any device leave it, what its file says counting too (a message
/// read elsewhere is Read); `current`, every message's notice the Porch
/// gathered, whatever the hour shows, for the words kept (`forget`); `seen`,
/// those of them read here, to which a day-old Read word gives way
/// (`forget_read_mail`): a Read never fights the server.
pub(crate) fn apply_mail(porch: &mut sioul_core::view::PorchView, current: &[String], seen: &[String]) {
    give_read_back_hourly(seen);
    let states = states(&|_, _| None);
    for item in porch.lanes.iter_mut().flat_map(|l| l.items.iter_mut()) {
        item.notice.state = states.of(&item.notice.id, item.notice.state.clone());
    }
    for code in &mut porch.right_now {
        code.notice.state = states.of(&code.notice.id, code.notice.state.clone());
    }
    forget_once_a_day(&[Kind::Mail], current);
}

/// When mail's Read words were last given back to the messages' flags (ms).
static GAVE_BACK: Mutex<i64> = Mutex::new(0);

/// Mail's day-old Read words given back to the messages' own `\Seen`, once
/// an hour (`notices::forget_read_mail`); `seen`, the messages read here.
fn give_read_back_hourly(seen: &[String]) {
    let now = jiff::Timestamp::now().as_millisecond();
    let due = GAVE_BACK.lock().is_ok_and(|mut last| {
        let due = now - *last >= 3_600_000;
        if due {
            *last = now;
        }
        due
    });
    if !due {
        return;
    }
    let logs = logs();
    let seen: std::collections::HashSet<&str> = seen.iter().map(String::as_str).collect();
    if let Err(e) = rules::forget_read_mail(&logs.folder, &logs.device, &|id| seen.contains(id), now) {
        eprintln!("sioul: notices: {e}");
    }
}

// ---------------------------------------------------------------- the Porch's sections

/// A row of notices: the first's fields, every id, the items they name.
fn row(notice: Notice, ids: Vec<String>, targets: Vec<String>, extra: Value) -> Value {
    let mut row = json!({ "notice": notice, "ids": ids, "targets": targets });
    if let (Some(row), Value::Object(extra)) = (row.as_object_mut(), extra) {
        row.extend(extra);
    }
    row
}

/// Whether every one of a row's notices was settled on any device.
fn settled(states: &States, ids: &[String]) -> bool {
    !ids.is_empty() && ids.iter().all(|id| states.of(id, State::default()).mark.settled())
}

/// The Porch's sections that are not mail (PorchPage.qml): {sites, money,
/// letters}, each a list of rows ({notice, ids, targets} and what its kind
/// shows), without those settled on any device. The phone's come from
/// their own views (`calls::view`, `phonemsgs::view`).
pub(crate) fn view() -> Value {
    let states = states(&|_, _| None);
    let ((sites, every_site), (money, every_movement), letters) = (sites(), money(&sioul_core::zone::now()), letters());
    // Once a day: the words of these kinds forgotten once a week old and their notices
    // gone, whatever the hour shows (a work movement dismissed stays dismissed in the evening).
    let current: Vec<String> = every_site.into_iter().chain(every_movement).chain(letters.iter().flat_map(|(_, ids)| ids.iter().cloned())).collect();
    forget_once_a_day(&[Kind::Site, Kind::Money, Kind::Letter], &current);
    let keep = |rows: Vec<(Value, Vec<String>)>| -> Vec<Value> { rows.into_iter().filter(|(_, ids)| !settled(&states, ids)).map(|(row, _)| row).collect() };
    json!({ "sites": keep(sites), "money": keep(money), "letters": keep(letters) })
}

/// The sites' notifications kept for the Porch (`sites::notified`), one row
/// per site, its lines under it, newest last; in quiet time, only your own
/// sites' (`sites::waiting`'s rule). With every site's notices' ids, whatever
/// the hour shows: what their words are kept for (`forget`).
fn sites() -> (Vec<(Value, Vec<String>)>, Vec<String>) {
    use sioul_core::sites::Notices;
    let notices = Notices::load(&Notices::default_path());
    let mode = crate::backend::mode_now();
    let all = sioul_core::sites::sites(&load_config());
    let ids_of = |site: &sioul_core::sites::Site| -> Vec<String> { notices.of(&site.id).iter().map(|n| rules::site_id(&site.id, n.at, &format!("{}\u{1f}{}", n.title, n.text))).collect() };
    let every: Vec<String> = all.iter().flat_map(ids_of).collect();
    let rows = all
        .into_iter()
        .filter(|site| sioul_core::areas::in_view(site.area, mode.time, mode.week))
        .filter_map(|site| {
            let mine = notices.of(&site.id);
            let newest = mine.iter().max_by_key(|n| n.at)?;
            let ids = ids_of(&site);
            let lines: Vec<String> = mine.iter().map(|n| if n.text.is_empty() { n.title.clone() } else { say("site-news-line", &[("title", n.title.clone()), ("text", n.text.clone())]) }).collect();
            let at = newest.at * 1000;
            let notice = Notice {
                address: format!("site:{}", site.id),
                at,
                when: sioul_core::view::date(tr(), Some(newest.at)),
                who: site.name.clone(),
                what: lines.last().cloned().unwrap_or_default(),
                ..Notice::new(Kind::Site, ids[0].clone(), "sites").with_area(Some(site.area))
            };
            Some((row(notice, ids.clone(), vec![site.id.clone()], json!({ "site": site.id, "lines": lines })), ids))
        })
        .collect();
    (rows, every)
}

/// How far ahead the Money section looks (docs/porch.md, "Money").
const MONEY_DAYS: u32 = 14;

/// The Porch's Money (`due::coming`, the money movements'): what is due or
/// expected in the next two weeks, what the bank still holds on a card, and
/// what was expected and not seen on its account, said once (Dismiss keeps
/// it said); each its day, who, its state in words and its budget, its
/// signed amount; opening it opens Budgets. Work money is said only in
/// work's hours, as each row's area says (docs/areas.md); nothing while you
/// sleep (the page). An account's shortfall comes first, once the budgets
/// say one (`shortfall`). With every movement's notice, whatever the hour
/// shows: what their words are kept for (`forget`).
fn money(now: &jiff::Zoned) -> (Vec<(Value, Vec<String>)>, Vec<String>) {
    let mode = crate::backend::mode_now();
    let tr = tr();
    let mut rows = shortfall();
    let coming = sioul_core::due::coming(now.date(), MONEY_DAYS);
    let every: Vec<String> = coming.iter().map(|c| rules::money_id(&c.address)).chain(rows.iter().flat_map(|(_, ids)| ids.iter().cloned())).collect();
    for coming in coming.into_iter().filter(|c| sioul_core::areas::in_view(c.area, mode.time, mode.week)) {
        let (who, what, amount) = coming_words(&coming, tr);
        let id = rules::money_id(&coming.address);
        let at = coming.day.to_zoned(now.time_zone().clone()).map(|z| z.timestamp().as_millisecond()).unwrap_or(0);
        let notice = Notice { address: coming.address.clone(), at, when: tr.day_short(coming.day), who, what, ..Notice::new(Kind::Money, id.clone(), "money").with_area(Some(coming.area)) };
        rows.push((row(notice, vec![id.clone()], Vec::new(), json!({ "amount": amount, "state": coming.state.id() })), vec![id]));
    }
    (rows, every)
}

/// The agenda's conflicts waiting for an answer, as the Porch's rows
/// (docs/porch.md, "Agenda"): `overlaps`, the property the agenda's job
/// makes (`pim::overlaps_of`), whose rules are the agenda's own (only those
/// someone else's change made, each once, at its time); each a notice of
/// its own, `agenda:<digest of its key>`, work's when one of its events is
/// on a work agenda, else admin's, as the agenda says it, shown while that
/// area is in view now (the property is made when the agenda changes, not
/// at each minute) and while no word settled it on any device: Dismiss,
/// or Done once Keep both or Move answered it.
fn agenda_rows(overlaps: &str, now: &jiff::Zoned) -> Value {
    let rows: Vec<Value> = serde_json::from_str(overlaps).unwrap_or_default();
    if rows.is_empty() {
        return json!([]);
    }
    let mode = crate::backend::mode_now();
    let calendars = sioul_core::vdir::collections(sioul_core::vdir::Kind::Calendars);
    let work = |key: &str| std::path::Path::new(key).parent().and_then(|dir| calendars.iter().find(|c| c.dir == dir)).is_some_and(|c| sioul_core::agendas::marks_of(c).work);
    let (shown, current) = agenda_rows_in(rows, now, mode.time, mode.week, &states(&|_, _| None), &work, tr());
    forget_once_a_day(&[Kind::Agenda], &current);
    shown
}

/// `agenda_rows` at a time of the day and week (`time`, `week`), with the
/// words said (`states`) and whether an event's file is on a work agenda
/// (`work`): the rows shown, and every conflict's notice (what its words are kept for).
fn agenda_rows_in(rows: Vec<Value>, now: &jiff::Zoned, time: sioul_core::areas::Time, week: sioul_core::areas::Week, states: &States, work: &dyn Fn(&str) -> bool, tr: &sioul_core::i18n::Translator) -> (Value, Vec<String>) {
    let mut current = Vec::new();
    let mut out = Vec::new();
    for conflict in rows {
        let key = conflict["key"].as_str().unwrap_or_default().to_string();
        if key.is_empty() {
            continue;
        }
        let id = rules::agenda_id(&key);
        current.push(id.clone());
        let during = conflict["kind"] == "during";
        let events: Vec<&Value> = if during { vec![&conflict["event"]] } else { vec![&conflict["first"], &conflict["second"]] };
        let area = if events.iter().any(|e| work(e["key"].as_str().unwrap_or_default())) { sioul_core::areas::Area::WORK } else { sioul_core::areas::Area::ADMIN };
        if !sioul_core::areas::in_view(area, time, week) || settled(states, std::slice::from_ref(&id)) {
            continue;
        }
        let first = events[0];
        let at = first["start"].as_i64().unwrap_or(0) * 1000;
        let day = conflict["day"].as_str().and_then(|d| d.parse::<jiff::civil::Date>().ok()).unwrap_or_else(|| now.date());
        let title = |e: &Value| e["title"].as_str().map(str::trim).filter(|t| !t.is_empty()).map_or_else(|| tr.text("agenda-untitled", None), str::to_string);
        let when = |e: &Value| e["when"].as_str().unwrap_or_default().to_string();
        let what = if during {
            let mut args = sioul_core::i18n::args();
            args.set("span", when(first));
            tr.text("porch-conflict-during", Some(&args))
        } else {
            let second = events[1];
            let mut args = sioul_core::i18n::args();
            args.set("title", title(second));
            args.set("whose", "none");
            args.set("why", if conflict["tentative"] == true { "tentative" } else { "none" });
            let said = if conflict["at_once"] == true {
                tr.text("conflict-same-time", Some(&args))
            } else {
                args.set("gap", conflict["gap"].as_i64().unwrap_or(0));
                args.set("needed", conflict["needed"].as_i64().unwrap_or(0));
                args.set("source", conflict["source"].as_str().unwrap_or_default().to_string());
                tr.text("conflict-tight", Some(&args))
            };
            format!("{} · {}", when(first), said.trim_end_matches('.'))
        };
        let notice = Notice { address: first["key"].as_str().unwrap_or_default().to_string(), at, when: tr.day_short(day), who: title(first), what, ..Notice::new(Kind::Agenda, id.clone(), "agenda").with_area(Some(area)) };
        let mut shown = row(notice, vec![id], Vec::new(), json!({}));
        shown["conflict"] = row_copy(&conflict);
        out.push(shown);
    }
    (Value::Array(out), current)
}

/// A conflict's own fields, for the Porch's answers (its key, its events, what may be done).
fn row_copy(row: &Value) -> Value {
    json!({ "key": row["key"], "kind": row["kind"], "keys": row["keys"], "answers": row["answers"], "first": row["first"], "second": row["second"], "event": row["event"] })
}

/// An account's shortfalls, first in Money: the budgets' own notices
/// (`ahead::shortfall_notices`), one per account and day of the shortfall,
/// of every area, so said whatever the hours show; each opening Budgets.
/// Its sentence (the payment, what is missing, what covers it) under the
/// row whole, which one line would cut; the row says the account and its day.
fn shortfall() -> Vec<(Value, Vec<String>)> {
    let config = load_config();
    let Some(root) = config.notes_root_path() else { return Vec::new() };
    let words = sioul_core::words::Words::of(&config);
    let tr = tr();
    let zone = sioul_core::zone::current();
    sioul_core::ahead::shortfall_notices(&root, &words, tr, sioul_core::zone::now().date())
        .into_iter()
        .map(|notice| {
            let id = notice.id.clone();
            let sentence = notice.what.clone();
            let day = jiff::Timestamp::from_millisecond(notice.at).map(|t| t.to_zoned(zone.clone()).date()).ok();
            let notice = Notice { what: tr.text("porch-shortfall-what", None), when: day.map(|d| tr.day_short(d)).unwrap_or(notice.when.clone()), ..notice };
            (row(notice, vec![id.clone()], Vec::new(), json!({ "note": sentence })), vec![id])
        })
        .collect()
}

/// A movement's row in words: who, its state ("Planned", "Not seen on its
/// account") and its budget, and its signed amount in the locale's format
/// ("−€62.00", "+€1,200"; "about" an estimate; another currency by its code).
fn coming_words(c: &sioul_core::due::Coming, tr: &sioul_core::i18n::Translator) -> (String, String, String) {
    let state = tr.text(&format!("due-state-{}", c.state.id()), None);
    let what = std::iter::once(state).chain(c.budget.clone().filter(|b| !b.is_empty())).collect::<Vec<_>>().join(" · ");
    let money = sioul_core::money::Money(c.amount_cents);
    // The locale's money is in euros (`Translator::money`); another currency by its code.
    let figure = if c.currency.is_empty() || c.currency.eq_ignore_ascii_case("EUR") { tr.money(money) } else { format!("{} {}", tr.number(money), c.currency) };
    let signed = if c.amount_cents > 0 { format!("+{figure}") } else { figure };
    let amount = if c.about {
        let mut args = sioul_core::i18n::args();
        args.set("amount", signed);
        tr.text("due-about", Some(&args))
    } else {
        signed
    };
    (c.label.clone(), what, amount)
}

/// The paper letters waiting (`letters::view`), one row each: who sent it,
/// what it is, when it came; its scan opens as a letter opens now, until
/// the Letters page opens it. Its own actions follow it (`letter`).
fn letters() -> Vec<(Value, Vec<String>)> {
    let shown: Value = serde_json::from_str(&crate::letters::view()).unwrap_or_default();
    let projects = shown["projects"].clone();
    shown["letters"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|letter| {
            let id = rules::letter_id(letter["id"].as_str()?);
            let notice = Notice {
                address: letter["file"].as_str().unwrap_or_default().to_string(),
                when: letter["received"].as_str().unwrap_or_default().to_string(),
                who: letter["sender"].as_str().unwrap_or_default().to_string(),
                what: letter["kind_label"].as_str().unwrap_or_default().to_string(),
                project: letter["project"].as_str().unwrap_or_default().to_string(),
                ..Notice::new(Kind::Letter, id.clone(), "letters")
            };
            Some((row(notice, vec![id.clone()], Vec::new(), json!({ "letter": letter, "projects": projects })), vec![id]))
        })
        .collect()
}

/// When each group of kinds last forgot its old words (ms): once a day.
static FORGOT: Mutex<Vec<(Vec<Kind>, i64)>> = Mutex::new(Vec::new());

/// The words of `kinds` forgotten once a day (`notices::forget`), `current`
/// the notices their sources give here now, whatever the hour shows.
pub(crate) fn forget_once_a_day(kinds: &[Kind], current: &[String]) {
    let now = jiff::Timestamp::now().as_millisecond();
    let due = FORGOT.lock().is_ok_and(|mut last| match last.iter_mut().find(|(k, _)| k == kinds) {
        Some((_, at)) if now - *at < 86_400_000 => false,
        Some((_, at)) => {
            *at = now;
            true
        }
        None => {
            last.push((kinds.to_vec(), now));
            true
        }
    });
    if !due {
        return;
    }
    let logs = logs();
    let current: std::collections::HashSet<&str> = current.iter().map(String::as_str).collect();
    if let Err(e) = rules::forget(&logs.folder, &logs.device, kinds, &|id| current.contains(id), now) {
        eprintln!("sioul: notices: {e}");
    }
}

// ---------------------------------------------------------------- system notifications

/// New mail's notification told here (`mailnote::show`): its messages'
/// notices and files, kept beside the log (never shared) until it is
/// withdrawn or replaced.
pub(crate) fn posted_mail(ids: Vec<String>, files: Vec<String>) -> Told {
    let told = Told { at: jiff::Timestamp::now().as_millisecond(), ids, items: files };
    let path = Posted::default_path();
    let written = sioul_core::filelock::with_lock(&path, || {
        let mut posted = Posted::load(&path);
        posted.told.insert(Kind::Mail.as_str().into(), told.clone());
        posted.save(&path)
    });
    if let Err(e) = written {
        eprintln!("sioul: notices: {e}");
    }
    told
}

/// Whether every message new mail's last notification told (`told`) is
/// settled: a word said of it on any device, or read or gone here.
pub(crate) fn settled_mail(told: &Told) -> bool {
    let states = states(&|_, _| None);
    let file_of = |id: &str| told.ids.iter().position(|i| i == id).and_then(|i| told.items.get(i)).filter(|f| !f.is_empty()).map(std::path::PathBuf::from);
    rules::all_settled(&told.ids, &states, &|id| file_of(id).is_none_or(|file| !rules::message_unread(&file)))
}

/// New mail's notification as last told here, if it still waits to be withdrawn.
pub(crate) fn mail_told() -> Option<Told> {
    Posted::load(&Posted::default_path()).told.get(Kind::Mail.as_str()).cloned()
}

/// At the window's minute and a phone's background step: new mail's
/// notification taken away where it shows, once each message it told is
/// settled (a phone's through Java; a computer's own thread looks itself,
/// `mailnote::show`), and forgotten.
pub(crate) fn withdraw_if_settled() {
    withdraw_codes();
    let Some(told) = mail_told() else { return };
    if !settled_mail(&told) {
        return;
    }
    if cfg!(target_os = "android") {
        crate::steps::java("mail-withdraw", "{}");
    }
    let path = Posted::default_path();
    let _ = sioul_core::filelock::with_lock(&path, || {
        let mut posted = Posted::load(&path);
        if posted.told.get(Kind::Mail.as_str()).is_some_and(|t| t.at == told.at) {
            posted.told.remove(Kind::Mail.as_str());
        }
        posted.save(&path)
    });
}

/// A phone's code told (`backend::code_notices`): its message's notice and
/// file kept beside the log (never shared), under "code|" and its address
/// (`code_key`), until its message is settled on any device or a day went by.
pub(crate) fn posted_code(about: &str, mut told: Told) {
    told.at = jiff::Timestamp::now().as_millisecond();
    let path = Posted::default_path();
    let written = sioul_core::filelock::with_lock(&path, || {
        let mut posted = Posted::load(&path);
        posted.told.insert(code_key(about), told);
        posted.save(&path)
    });
    if let Err(e) = written {
        eprintln!("sioul: notices: {e}");
    }
}

/// Where a code's notice is kept in `Posted`: "code|" and its message's address.
fn code_key(about: &str) -> String {
    format!("code|{about}")
}

/// The codes kept (`posted_code`) whose message is settled now (read,
/// dismissed or done on any device), or kept a day already (expired long
/// since: Android took them away), as `settled` says of each: their
/// addresses, to take away, and the codes left kept. Pure.
fn codes_done(posted: &Posted, now_ms: i64, settled: impl Fn(&Told) -> bool) -> Vec<String> {
    posted.told.iter().filter_map(|(key, told)| key.strip_prefix("code|").filter(|_| now_ms - told.at > 86_400_000 || settled(told)).map(str::to_string)).collect()
}

/// At the window's minute and a phone's background step: each code's
/// notice on this phone taken away once its message is settled on any
/// device (`eventalarms::withdraw`), and forgotten; forgotten too after a
/// day. Copied or expired, Android took it away already.
fn withdraw_codes() {
    let path = Posted::default_path();
    let posted = Posted::load(&path);
    if !posted.told.keys().any(|key| key.starts_with("code|")) {
        return;
    }
    let done = codes_done(&posted, jiff::Timestamp::now().as_millisecond(), settled_mail);
    if done.is_empty() {
        return;
    }
    for about in &done {
        crate::eventalarms::withdraw(about);
    }
    let _ = sioul_core::filelock::with_lock(&path, || {
        let mut posted = Posted::load(&path);
        for about in &done {
            posted.told.remove(&code_key(about));
        }
        posted.save(&path)
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A phone's code is taken away once its message is settled on any
    /// device, and forgotten after a day whatever became of it; new mail's
    /// notification is no code, and stays its own.
    #[test]
    fn a_phones_code_goes_once_its_message_is_settled() {
        let told = |at: i64, id: &str| Told { at, ids: vec![id.into()], items: vec![String::new()] };
        let now = 1_794_000_000_000;
        let mut posted = Posted::default();
        posted.told.insert(code_key("mid:read@example.org"), told(now - 60_000, "read"));
        posted.told.insert(code_key("mid:waiting@example.org"), told(now - 60_000, "waiting"));
        posted.told.insert(code_key("mid:old@example.org"), told(now - 86_400_001, "old"));
        posted.told.insert(Kind::Mail.as_str().into(), told(now - 60_000, "read"));
        let mut done = codes_done(&posted, now, |t| t.ids == ["read"]);
        done.sort();
        assert_eq!(done, ["mid:old@example.org", "mid:read@example.org"]);
    }

    /// Each verb says its word; an unknown verb says nothing.
    #[test]
    fn verbs_name_their_words() {
        assert_eq!(mark_of("dismiss"), Some(Mark::Dismissed));
        assert_eq!(mark_of("read"), Some(Mark::Read));
        assert_eq!(mark_of("done"), Some(Mark::Done));
        assert_eq!(mark_of("seen"), None);
    }

    /// A movement's row: its state in words, its budget, its signed amount,
    /// "about" an estimate, another currency by its code.
    #[test]
    fn a_movement_is_said_in_words() {
        let tr = sioul_core::i18n::Translator::new("en");
        let coming = |cents: i64, currency: &str, about: bool, state: sioul_core::movements::State, budget: Option<&str>| sioul_core::due::Coming {
            day: "2026-11-09".parse().unwrap(),
            label: "Linden Housing".into(),
            amount_cents: cents,
            currency: currency.into(),
            about,
            budget: budget.map(str::to_string),
            area: sioul_core::areas::Area::ADMIN,
            state,
            address: "sioul:movement/rent".into(),
        };
        use sioul_core::movements::State;
        assert_eq!(coming_words(&coming(-62_000, "EUR", false, State::Planned, Some("Home")), &tr), ("Linden Housing".into(), "Planned · Home".into(), "−€620".into()));
        assert_eq!(coming_words(&coming(120_000, "EUR", true, State::Announced, None), &tr).2, "about +€1,200");
        assert_eq!(coming_words(&coming(-1_250, "CHF", false, State::NotSeen, None), &tr), ("Linden Housing".into(), "Not seen on its account".into(), "−12.50 CHF".into()));
        assert_eq!(coming_words(&coming(-990, "EUR", false, State::NotFinal, None), &tr).1, "Not final yet");
    }

    /// The agenda's conflicts as the Porch's rows: each its own notice, by
    /// its key's digest, what it says and what may be done kept; at work's
    /// time a conflict of a work agenda, else admin's; gone once a word
    /// settled it on any device.
    #[test]
    fn a_conflict_is_a_row_until_a_word_settles_it() {
        use sioul_core::areas::{Time, Week};
        let tr = sioul_core::i18n::Translator::new("en");
        let now: jiff::Zoned = "2026-11-04T10:00:00+01:00[Europe/Paris]".parse().unwrap();
        let pair = json!({ "key": "/cal/home/a.ics@1794000000|/cal/home/b.ics@1794001800", "day": "2026-11-12", "today": false,
            "first": { "key": "/cal/home/a.ics", "start": 1_794_000_000, "title": "Dentist", "when": "14:00 – 15:00" },
            "second": { "key": "/cal/home/b.ics", "start": 1_794_001_800, "title": "Swimming", "when": "14:30 – 15:30" },
            "gap": 0, "needed": 0, "at_once": true, "tentative": false, "answers": ["move", "keep"], "source": "" });
        let week = Week { work_hours: true, admin_hours: true, meals: false, sleep: false };
        let mine = |_: &str| false;
        let (shown, current) = agenda_rows_in(vec![pair.clone()], &now, Time::Admin, week, &States::default(), &mine, &tr);
        let id = rules::agenda_id(pair["key"].as_str().unwrap());
        assert_eq!(current, [id.clone()]);
        let row = &shown[0];
        assert_eq!(row["notice"]["id"], id.as_str());
        assert_eq!(row["notice"]["kind"], "agenda");
        assert_eq!((row["notice"]["who"].as_str(), row["notice"]["what"].as_str(), row["notice"]["when"].as_str()), (Some("Dentist"), Some("14:00 – 15:00 · At the same time as “Swimming”"), Some("Thu 12 Nov")));
        assert_eq!(row["conflict"]["answers"], json!(["move", "keep"]));
        assert_eq!(row["conflict"]["first"]["key"], "/cal/home/a.ics");
        // A work agenda's: at work's time only.
        let work = |key: &str| key.ends_with("b.ics");
        assert_eq!(agenda_rows_in(vec![pair.clone()], &now, Time::Admin, week, &States::default(), &work, &tr).0.as_array().map(Vec::len), Some(0));
        assert_eq!(agenda_rows_in(vec![pair.clone()], &now, Time::Work, week, &States::default(), &work, &tr).0.as_array().map(Vec::len), Some(1));
        // Dismissed on another device: gone.
        let dir = std::env::temp_dir().join(format!("sioul-app-agenda-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        rules::say(&dir, "desk", std::slice::from_ref(&id), Mark::Dismissed, 1_000).unwrap();
        assert_eq!(agenda_rows_in(vec![pair], &now, Time::Admin, week, &States::read(&dir), &mine, &tr).0.as_array().map(Vec::len), Some(0));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A row with every notice settled is left out; one with a notice still
    /// new stays (a site's news that came after a dismiss).
    #[test]
    fn a_row_goes_once_each_of_its_notices_is_settled() {
        let dir = std::env::temp_dir().join(format!("sioul-app-notices-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (a, b) = ("site:bank:1-aa".to_string(), "site:bank:2-bb".to_string());
        rules::say(&dir, "desk", std::slice::from_ref(&a), Mark::Dismissed, 1_000).unwrap();
        let states = States::read(&dir);
        assert!(settled(&states, std::slice::from_ref(&a)));
        assert!(!settled(&states, &[a.clone(), b.clone()]), "news since the dismiss: the row stays");
        assert!(!settled(&states, &[]));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
