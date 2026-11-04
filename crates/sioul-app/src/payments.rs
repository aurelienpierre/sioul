// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Money movements from the window (docs/accounting.md, "Moving a movement",
//! "A payment from any mail"): "A payment…" from a message or a letter, a
//! short form filled from what it says, any budget chosen whatever the hour;
//! a movement moved from its row, its effect said, and the payee's next
//! movements asked about once, your answer the consent.

use crate::backend::{load_config, say, tr};
use serde_json::json;
use sioul_core::movements::{self, Place, Way, acts};

/// Every budget, whatever the hour: what a page shows is no limit on what a
/// payment may be given to (the owner's report of 4 November 2026).
fn budgets(ledger: &sioul_core::budget::Ledger) -> Vec<serde_json::Value> {
    ledger.budgets.iter().map(|b| json!({ "id": b.id, "title": b.title })).collect()
}

/// What "A payment…" starts from: a message by its file (`kind` "mail", `key`
/// its file, `trust` its sender's level as the list says it: "verified" and
/// "own" need no check), or a paper letter by its id (`kind` "letter").
/// Returns {payment, title, budgets}, or {problem}.
pub(crate) fn from(kind: &str, key: &str, trust: &str) -> String {
    let config = load_config();
    let Some(root) = config.notes_root_path() else { return json!({ "problem": tr().text("papers-no-store", None) }).to_string() };
    let words = sioul_core::words::Words::of(&config);
    let ledger = sioul_core::budget::Ledger::load(&root).unwrap_or_default();
    let made = match kind {
        "letter" => sioul_core::letters::Letters::load(&root).ok().and_then(|letters| letters.get(key).cloned()).map(|letter| (acts::Payment::from_letter(&letter), letter.reading.sender.clone())),
        _ => sioul_core::maildir::locate(std::path::Path::new(key)).and_then(|path| {
            let mut card = sioul_core::card::Card::from_bytes(&std::fs::read(&path).ok()?)?;
            card.path = Some(path);
            let address = crate::work::uri_of("mail", key);
            let doubtful = !matches!(trust, "verified" | "own");
            Some((acts::Payment::from_card(&words, &card, &address, doubtful), card.subject.clone()))
        }),
    };
    match made {
        Some((payment, title)) if !payment.source.is_empty() => json!({ "payment": payment, "title": title, "budgets": budgets(&ledger) }).to_string(),
        _ => json!({ "problem": tr().text("payment-unread", None) }).to_string(),
    }
}

/// The form's payment written into the budget file (`acts::Payment::record`):
/// its message or letter in its links, a comment saying where it came from
/// (`title`: the message's subject, the letter's sender). Returns {said} or {problem}.
pub(crate) fn add(edit: &str) -> String {
    #[derive(serde::Deserialize)]
    struct Edit {
        payment: acts::Payment,
        #[serde(default)]
        title: String,
    }
    let config = load_config();
    let Some(root) = config.notes_root_path() else { return json!({ "problem": tr().text("papers-no-store", None) }).to_string() };
    let edit: Edit = match serde_json::from_str(edit) {
        Ok(edit) => edit,
        Err(e) => return json!({ "problem": e.to_string() }).to_string(),
    };
    let today = sioul_core::zone::now().date();
    let origin = if edit.payment.source.starts_with("sioul:letter/") { "payment-origin-letter" } else { "payment-origin-mail" };
    let origin = say(origin, &[("title", edit.title.clone()), ("date", today.to_string())]);
    match edit.payment.record(&root, today, &origin) {
        Ok(_) => {
            let ledger = sioul_core::budget::Ledger::load(&root).unwrap_or_default();
            json!({ "said": say("ui-line-added", &[("budget", ledger.budget_title(&edit.payment.budget).to_string())]) }).to_string()
        }
        Err(problem) => json!({ "problem": problem }).to_string(),
    }
}

/// One of a bank account's movements placed from its row: `budget:<id>`,
/// `none`, or `""` (as the rules say again). Moved to a budget, its effect is
/// said ("Home holds €12 more for October 2026."), and the payee's next movements
/// are asked about once. Returns {said, ask: {payee, budget, title, way}|null} or {problem}.
pub(crate) fn place(account: &str, movement: &str, choice: &str) -> String {
    let config = load_config();
    let Some(root) = config.notes_root_path() else { return json!({ "problem": tr().text("papers-no-store", None) }).to_string() };
    let path = root.join(sioul_core::budget::LEDGER);
    let Some(budget) = choice.strip_prefix("budget:") else {
        return match sioul_core::accounts::set_assignment(&path, account, movement, choice) {
            Ok(()) => json!({ "said": "", "ask": null }).to_string(),
            Err(problem) => json!({ "problem": problem }).to_string(),
        };
    };
    let words = sioul_core::words::Words::of(&config);
    let today = sioul_core::zone::now().date();
    let ledger = sioul_core::budget::Ledger::load(&root).unwrap_or_default();
    // The movement, read around its own day.
    let day = sioul_core::bank::Bank::load(&root).ok().and_then(|bank| bank.movements.iter().find(|m| m.id == movement && sioul_core::accounts::account_of(&ledger, &m.account).is_some_and(|a| a.id == account)).map(|m| m.date)).unwrap_or(today);
    let shift = |days: i64| day.checked_add(jiff::Span::new().days(days)).unwrap_or(day);
    let all = movements::load(&root, &words, shift(-70), shift(70), today);
    let Some(found) = all.find(&movements::bank_key(account, movement)) else {
        return match sioul_core::accounts::set_assignment(&path, account, movement, choice) {
            Ok(()) => json!({ "said": "", "ask": null }).to_string(),
            Err(problem) => json!({ "problem": problem }).to_string(),
        };
    };
    match acts::move_to(&root, &words, found, &Place::Budget(budget.to_string())) {
        Ok(moved) => {
            let said = effect(&ledger, &moved);
            let way = if found.way == Way::In { "in" } else { "out" };
            let ask = moved.ask.map(|payee| json!({ "payee": payee, "budget": budget, "title": ledger.budget_title(budget), "way": way }));
            json!({ "said": said, "ask": ask }).to_string()
        }
        Err(problem) => json!({ "problem": problem }).to_string(),
    }
}

/// "Home holds €12 more for October 2026." ("for 2026" in a yearly budget); none when nothing moved.
fn effect(ledger: &sioul_core::budget::Ledger, moved: &acts::Moved) -> String {
    if moved.budget.is_empty() || moved.more == sioul_core::money::Money::ZERO {
        return String::new();
    }
    let yearly = ledger.budgets.iter().find(|b| b.id == moved.budget).is_some_and(|b| b.period == sioul_core::budget::Period::Year);
    let period = if yearly { moved.day.year().to_string() } else { tr().month_year(moved.day) };
    say("moved-more", &[("budget", ledger.budget_title(&moved.budget).to_string()), ("amount", tr().money(moved.more)), ("period", period)])
}

/// Your answer, once, on whether `payee`'s next movements go to `budget` too
/// (`acts::decide`): a yes is a rule of the budget file. Returns what went wrong, else what it did, in words.
pub(crate) fn decide(payee: &str, budget: &str, way: &str, next: bool) -> String {
    let config = load_config();
    let Some(root) = config.notes_root_path() else { return tr().text("papers-no-store", None) };
    let way = if way == "in" { Way::In } else { Way::Out };
    match acts::decide(&root, payee, budget, way, next) {
        Ok(()) if next => {
            let ledger = sioul_core::budget::Ledger::load(&root).unwrap_or_default();
            say("moved-next-yes", &[("payee", payee.to_string()), ("budget", ledger.budget_title(budget).to_string())])
        }
        Ok(()) => String::new(),
        Err(problem) => problem,
    }
}

/// The movement a key names in the notes folder's books (its steadiest key,
/// any of its sources' keys, or its address), and what it needs to be moved.
fn found(key: &str) -> Result<(std::path::PathBuf, std::sync::Arc<sioul_core::words::Words>, std::sync::Arc<movements::Books>, usize), String> {
    let config = load_config();
    let root = config.notes_root_path().ok_or_else(|| tr().text("papers-no-store", None))?;
    let words = sioul_core::words::Words::of(&config);
    let books = movements::books(&root, &words, sioul_core::zone::now().date());
    let bare = key.strip_prefix("sioul:movement/").map_or_else(|| key.to_string(), sioul_core::notes::decode);
    let at = books.movements.iter().position(|m| m.has(&bare) || m.address() == key).ok_or_else(|| tr().text("budget-line-gone", None))?;
    Ok((root, words, books, at))
}

/// A movement of the budgets page moved by its key (a row of What is
/// coming, a bank card's row): `budget:<id>`; or split, `parts` as
/// [{budget, amount}] in units, the rest in `budget:<id>`. Its effect said,
/// the payee asked about once, as `place`. Returns {said, ask} or {problem}.
pub(crate) fn move_movement(key: &str, choice: &str, parts: &str) -> String {
    let (root, words, books, at) = match found(key) {
        Ok(found) => found,
        Err(problem) => return json!({ "problem": problem }).to_string(),
    };
    let movement = &books.movements[at];
    let rest = choice.strip_prefix("budget:").unwrap_or_default().to_string();
    let parts: Vec<(String, sioul_core::money::Money)> = serde_json::from_str::<Vec<serde_json::Value>>(parts)
        .unwrap_or_default()
        .iter()
        .filter_map(|p| Some((p["budget"].as_str()?.to_string(), sioul_core::money::Money::from(p["amount"].as_f64()?))))
        .filter(|(budget, amount)| !budget.is_empty() && *amount != sioul_core::money::Money::ZERO)
        .collect();
    let place = if parts.is_empty() {
        if rest.is_empty() {
            return json!({ "problem": tr().text("budget-add-bad-amount", None) }).to_string();
        }
        Place::Budget(rest.clone())
    } else {
        // Each part in the movement's sign; the rest where it said.
        let sign = if movement.sum.amount().is_negative() { -1 } else { 1 };
        let mut parts: Vec<(String, sioul_core::money::Money)> = parts.into_iter().map(|(b, a)| (b, sioul_core::money::Money(a.cents().abs() * sign))).collect();
        let left = movement.sum.amount() - parts.iter().map(|(_, a)| *a).sum::<sioul_core::money::Money>();
        if left != sioul_core::money::Money::ZERO && !rest.is_empty() {
            parts.push((rest.clone(), left));
        }
        Place::Parts(parts)
    };
    let ledger = sioul_core::budget::Ledger::load(&root).unwrap_or_default();
    match acts::move_to(&root, &words, movement, &place) {
        Ok(moved) => {
            let way = if movement.way == Way::In { "in" } else { "out" };
            let budget = place.budgets().first().map(|b| b.to_string()).unwrap_or_default();
            let ask = moved.ask.clone().filter(|_| matches!(place, Place::Budget(_))).map(|payee| json!({ "payee": payee, "budget": budget, "title": ledger.budget_title(&budget), "way": way }));
            json!({ "said": effect(&ledger, &moved), "ask": ask }).to_string()
        }
        Err(problem) => json!({ "problem": problem }).to_string(),
    }
}

/// "Not the same payment": the source `source`, probably joined to the
/// movement `key`, kept apart from the rest of it for good. Returns what went wrong, else "".
pub(crate) fn not_same(key: &str, source: &str) -> String {
    let (root, _, books, at) = match found(key) {
        Ok(found) => found,
        Err(problem) => return problem,
    };
    let movement = &books.movements[at];
    // Apart from the movement's steadiest key among its other sources.
    let Some(other) = movements::steadiest_of(movement.sources.iter().filter(|s| s.key != source)) else { return String::new() };
    acts::not_same(&root, source, other).err().unwrap_or_default()
}

/// The movements that may be the same payment as `key`'s: the same way,
/// within a month of its day, not already one with it; nearest first, ten at most.
pub(crate) fn same_candidates(key: &str) -> String {
    let Ok((_, _, books, at)) = found(key) else { return "[]".into() };
    let movement = &books.movements[at];
    let near = |d: jiff::civil::Date| (d - movement.day).get_days().abs();
    // A share set aside may be paid any day of its quarter, before its declaration's day.
    let aside = movement.sources.iter().any(|s| s.kind == movements::Kind::SetAside);
    let within = |d: jiff::civil::Date| if aside { (-92..=31).contains(&(d - movement.day).get_days()) } else { near(d) <= 31 };
    let mut list: Vec<&movements::Movement> = books.movements.iter().enumerate().filter(|(i, m)| *i != at && m.way == movement.way && within(m.day)).map(|(_, m)| m).collect();
    list.sort_by_key(|m| (near(m.day), (m.sum.amount().cents() - movement.sum.amount().cents()).abs()));
    let rows: Vec<serde_json::Value> = list
        .iter()
        .take(10)
        .map(|m| json!({ "key": m.steady_key(), "label": sioul_core::moneyview::who(tr(), m), "date": sioul_core::moneyview::short_day(tr(), m.day), "amount": sioul_core::moneyview::sum_words(tr(), m.sum) }))
        .collect();
    serde_json::Value::Array(rows).to_string()
}

/// "The same as…": the movements `key` and `other` one payment. Returns what went wrong, else "".
pub(crate) fn same(key: &str, other: &str) -> String {
    let (root, words, books, at) = match found(key) {
        Ok(found) => found,
        Err(problem) => return problem,
    };
    let Some(theirs) = books.movements.iter().find(|m| m.has(other)) else { return tr().text("budget-line-gone", None) };
    acts::same(&root, &words, &books.movements[at], theirs).err().unwrap_or_default()
}

/// The budgets' filter as this device keeps it (docs/areas.md, "A page's
/// filter by area"): `$XDG_STATE_HOME/sioul/budgets-view.toml`, never
/// shared: the areas you ticked, while they hold.
#[derive(serde::Serialize, serde::Deserialize, Default, PartialEq, Clone, Debug)]
struct BudgetsView {
    #[serde(default, skip_serializing_if = "is_as_the_hours_say")]
    areas: sioul_core::areafilter::AreaFilter,
}

fn is_as_the_hours_say(filter: &sioul_core::areafilter::AreaFilter) -> bool {
    *filter == sioul_core::areafilter::AreaFilter::default()
}

fn view_path() -> std::path::PathBuf {
    sioul_core::config::state_dir().join("budgets-view.toml")
}

fn kept_view(path: &std::path::Path) -> BudgetsView {
    std::fs::read_to_string(path).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default()
}

fn write_view(path: &std::path::Path, view: &BudgetsView) {
    if kept_view(path) != *view
        && let Ok(text) = toml::to_string(view)
    {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = sioul_core::filelock::replace(path, text.as_bytes());
    }
}

/// What the budgets show at `mode`, `now` in Unix seconds: the areas ticked
/// as the hours say (`areafilter::hours_say`: leisure at meals and at night,
/// so the page is never empty), unless you ticked others, and the filter's
/// button. Your ticks forgotten once they no longer hold.
pub(crate) fn budget_filter(mode: &sioul_core::quiet::Mode, now: i64) -> (sioul_core::areas::Area, sioul_core::areafilter::Shown) {
    budget_filter_in(&view_path(), mode, now)
}

fn budget_filter_in(path: &std::path::Path, mode: &sioul_core::quiet::Mode, now: i64) -> (sioul_core::areas::Area, sioul_core::areafilter::Shown) {
    let mut kept = kept_view(path);
    let said = sioul_core::areafilter::hours_say(mode);
    if kept.areas.settle(said, now) {
        write_view(path, &kept);
    }
    (kept.areas.shown(said, now), sioul_core::areafilter::shown(&kept.areas, said, now, tr()))
}

/// The areas you ticked in the budgets' filter, at `mode`: kept until the hours change, a day at most.
pub(crate) fn tick_areas(areas: &str, mode: &sioul_core::quiet::Mode, now: i64) {
    tick_areas_in(&view_path(), areas, mode, now);
}

fn tick_areas_in(path: &std::path::Path, areas: &str, mode: &sioul_core::quiet::Mode, now: i64) {
    let mut view = kept_view(path);
    let ticked = sioul_core::areas::Area::parse(areas).unwrap_or_default();
    view.areas.tick(ticked, sioul_core::areafilter::hours_say(mode), mode.until.as_ref().map(|z| z.timestamp().as_second()), now);
    write_view(path, &view);
}

/// "As the hours say": your ticks in the budgets' filter forgotten.
pub(crate) fn areas_as_hours() {
    let path = view_path();
    let mut view = kept_view(&path);
    view.areas.as_hours();
    write_view(&path, &view);
}

#[cfg(test)]
mod tests {
    use super::*;
    use sioul_core::areas::{Area, Time, Week};
    use sioul_core::quiet::{Mode, Reason};

    fn mode(time: Time) -> Mode {
        Mode { quiet: time != Time::Work, time, week: Week { work_hours: true, admin_hours: true, ..Week::default() }, reason: Reason::Working, until: None, back: None, label: String::new() }
    }

    /// The budgets' filter (docs/areas.md, "A page's filter by area"): as
    /// the hours say by default, leisure at meals and at night, so the page is
    /// never empty; your ticks hold while the hours say the same, then are
    /// forgotten; "As the hours say" forgets them at once; each device keeps
    /// its own, in its view file.
    #[test]
    fn the_budgets_filter_follows_the_hours_until_you_tick() {
        let dir = std::env::temp_dir().join(format!("sioul-budgets-view-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("sioul").join("budgets-view.toml");
        let now = 1_800_000_000;
        let (shown, filter) = budget_filter_in(&path, &mode(Time::Work), now);
        assert_eq!((shown, filter.own), (Area::WORK, false), "at work, work's budgets");
        assert_eq!(budget_filter_in(&path, &mode(Time::Sleep), now).0, Area::LEISURE, "at night, leisure's: never nothing");
        assert_eq!(budget_filter_in(&path, &mode(Time::Meals), now).0, Area::LEISURE);
        // Leisure ticked too at work: held while the hours say work, written in the view file.
        tick_areas_in(&path, "work+leisure", &mode(Time::Work), now);
        let (shown, filter) = budget_filter_in(&path, &mode(Time::Work), now + 60);
        assert_eq!((shown, filter.own), (Area { leisure: true, ..Area::WORK }, true));
        assert!(std::fs::read_to_string(&path).unwrap().contains("ticked = \"work+leisure\""));
        // The hours change: theirs again, and the ticks forgotten for good.
        assert_eq!(budget_filter_in(&path, &mode(Time::Admin), now + 120).0, Area::ADMIN);
        assert_eq!(budget_filter_in(&path, &mode(Time::Work), now + 180), (Area::WORK, sioul_core::areafilter::shown(&Default::default(), Area::WORK, now, crate::backend::tr())));
        // "As the hours say".
        tick_areas_in(&path, "admin", &mode(Time::Work), now);
        let mut view = kept_view(&path);
        view.areas.as_hours();
        write_view(&path, &view);
        assert_eq!(budget_filter_in(&path, &mode(Time::Work), now).0, Area::WORK);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
