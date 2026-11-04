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
/// that are not mail, `view`); "dismiss", "read", "done" {kind, ids,
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
/// gathered, whatever the hour shows, for the words kept (`forget`).
pub(crate) fn apply_mail(porch: &mut sioul_core::view::PorchView, current: &[String]) {
    let states = states(&|_, _| None);
    for item in porch.lanes.iter_mut().flat_map(|l| l.items.iter_mut()) {
        item.notice.state = states.of(&item.notice.id, item.notice.state.clone());
    }
    for code in &mut porch.right_now {
        code.notice.state = states.of(&code.notice.id, code.notice.state.clone());
    }
    forget_once_a_day(&[Kind::Mail], current);
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
    let (sites, money, letters) = (sites(), money(), letters());
    // Once a day: the words of these kinds forgotten once a week old and their notices gone.
    let current: Vec<String> = sites.iter().chain(&money).chain(&letters).flat_map(|(_, ids)| ids.iter().cloned()).collect();
    forget_once_a_day(&[Kind::Site, Kind::Money, Kind::Letter], &current);
    let keep = |rows: Vec<(Value, Vec<String>)>| -> Vec<Value> { rows.into_iter().filter(|(_, ids)| !settled(&states, ids)).map(|(row, _)| row).collect() };
    json!({ "sites": keep(sites), "money": keep(money), "letters": keep(letters) })
}

/// The sites' notifications kept for the Porch (`sites::notified`), one row
/// per site, its lines under it, newest last; in quiet time, only your own
/// sites' (`sites::waiting`'s rule).
fn sites() -> Vec<(Value, Vec<String>)> {
    use sioul_core::sites::Notices;
    let notices = Notices::load(&Notices::default_path());
    let mode = crate::backend::mode_now();
    sioul_core::sites::sites(&load_config())
        .into_iter()
        .filter(|site| sioul_core::areas::in_view(site.area, mode.time, mode.week))
        .filter_map(|site| {
            let mine = notices.of(&site.id);
            let newest = mine.iter().max_by_key(|n| n.at)?;
            let ids: Vec<String> = mine.iter().map(|n| rules::site_id(&site.id, n.at, &format!("{}\u{1f}{}", n.title, n.text))).collect();
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
        .collect()
}

/// The week's money, as Budgets reads it from your bank's file
/// (`bank::view`): what the watch found, then each payment coming, its day,
/// what it is and its amount; each opens Budgets. Part 2 of the Porch's
/// notices reads `due::coming` instead.
fn money() -> Vec<(Value, Vec<String>)> {
    let bank: Value = serde_json::from_str(&crate::bank::view()).unwrap_or_default();
    let mut rows = Vec::new();
    for finding in bank["findings"].as_array().into_iter().flatten().filter_map(Value::as_str) {
        let id = rules::money_id(&format!("finding-{}", rules::digest(finding)));
        let notice = Notice { who: tr().text("ui-budgets", None), what: finding.to_string(), ..Notice::new(Kind::Money, id.clone(), "money") };
        rows.push((row(notice, vec![id.clone()], Vec::new(), json!({ "amount": "" })), vec![id]));
    }
    for coming in bank["coming"].as_array().into_iter().flatten().filter_map(Value::as_str) {
        let (day, label, amount) = coming_parts(coming);
        let id = rules::money_id(&format!("coming-{}", rules::digest(coming)));
        let notice = Notice { who: label, when: day, ..Notice::new(Kind::Money, id.clone(), "money") };
        rows.push((row(notice, vec![id.clone()], Vec::new(), json!({ "amount": amount })), vec![id]));
    }
    rows
}

/// A payment coming, as Budgets writes it ("Thursday 8 October · EDF ·
/// €62.00"): its day, what it is, its amount; anything else whole as what it is.
fn coming_parts(coming: &str) -> (String, String, String) {
    match coming.split(" · ").collect::<Vec<_>>().as_slice() {
        [day, label, amount] => (day.to_string(), label.to_string(), amount.to_string()),
        _ => (String::new(), coming.to_string(), String::new()),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Each verb says its word; an unknown verb says nothing.
    #[test]
    fn verbs_name_their_words() {
        assert_eq!(mark_of("dismiss"), Some(Mark::Dismissed));
        assert_eq!(mark_of("read"), Some(Mark::Read));
        assert_eq!(mark_of("done"), Some(Mark::Done));
        assert_eq!(mark_of("seen"), None);
    }

    /// A payment coming, its day, what it is and its amount apart (the amount right-aligned in its row).
    #[test]
    fn a_payment_coming_is_split_for_its_row() {
        assert_eq!(coming_parts("Thursday 8 October · EDF · €62.00"), ("Thursday 8 October".into(), "EDF".into(), "€62.00".into()));
        assert_eq!(coming_parts("jeudi 8 octobre · EDF · 62,00 €"), ("jeudi 8 octobre".into(), "EDF".into(), "62,00 €".into()));
        assert_eq!(coming_parts("Something else"), (String::new(), "Something else".into(), String::new()), "whole, never lost");
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
