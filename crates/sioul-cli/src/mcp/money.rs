// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Money movements for agents (design §3.3, docs/mcp.md): the full ledger
//! with the window's filters (`movements`), one movement in full
//! (`get_movement`), and the window's own acts (`movements::acts`): one made
//! (`add_movement`), changed (`update_movement`), removed by its kind
//! (`delete_movement`), two said the same payment or not (`same_movement`).
//! These write records, never payments: nothing moves money.
//!
//! Their gate is the `budgets` tool's (`read::open_ledger`): money is seen
//! and written only in budgets opened to agents, a movement in no budget
//! only while things outside projects are; account numbers and card numbers
//! masked. Each write is said in the window, with Undo, for as long as its
//! files stay as the agent left them (`agentwrites`).

use super::items::say_in_window;
use super::tools::{Answer, Args};
use super::{access, mask, read};
use crate::Session;
use serde_json::{Value, json};
use sioul_core::agentwrites::FileChange;
use sioul_core::links::Loaded;
use sioul_core::money::Money;
use sioul_core::moneylist::{self, Query, ToldBy};
use sioul_core::movements::{self, Books, Fee, FeeFrom, FeeKind, Movement, Way, acts};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The money an agent may see: the books, the budgets opened to agents,
/// the labels of lines kept from them, whether things outside projects are open.
struct Opened {
    root: PathBuf,
    books: Arc<Books>,
    ledger: sioul_core::budget::Ledger,
    open: BTreeSet<String>,
    kept_labels: BTreeSet<String>,
    outside: bool,
    consent: sioul_core::consent::Consent,
}

fn opened(s: &Session) -> Result<Opened, String> {
    let root = s.config.notes_root_path().filter(|r| r.join(sioul_core::budget::LEDGER).is_file()).ok_or_else(|| s.tr.text("error-no-ledger", None))?;
    sioul_core::budget::Ledger::load(&root)?;
    let books = movements::books(&root, &sioul_core::words::Words::of(&s.config), sioul_core::zone::now().date());
    let consent = access::of(s, &Loaded::read(&s.config));
    let (ledger, _, kept_labels) = read::open_ledger(books.counted.clone(), &consent);
    let open = ledger.budgets.iter().map(|b| b.id.clone()).collect();
    Ok(Opened { root, books, ledger, open, kept_labels, outside: consent.outside, consent })
}

impl Opened {
    /// Whether an agent may see it: each of its budgets open, none of its lines kept; in no budget, while things outside projects are open.
    fn shows(&self, m: &Movement) -> bool {
        let budgets = m.place.budgets();
        !self.kept_labels.contains(&m.label) && if budgets.is_empty() { self.outside } else { budgets.iter().all(|b| self.open.contains(*b)) }
    }

    /// The movement an address names, when an agent may see it.
    fn find(&self, address: &str) -> Result<&Movement, String> {
        match moneylist::get(&self.books, address) {
            Some((m, _)) if self.shows(m) => Ok(m),
            Some(_) => Err(access::kept("This movement")),
            None => Err(format!("No movement “{}” here: movements gives their addresses (sioul:movement/…).", crate::one_line(address))),
        }
    }
}

/// A movement's words and data, masked (account and card numbers).
fn shown(lines: Vec<String>) -> Vec<String> {
    lines.iter().map(|l| mask::text(l)).collect()
}

fn way_of(args: &Args) -> Result<Option<Way>, String> {
    Ok(match args.text("way")?.as_deref() {
        None => None,
        Some("in") => Some(Way::In),
        Some("out") => Some(Way::Out),
        Some("between") => Some(Way::Between),
        Some(other) => return Err(format!("“way” is in, out or between, not “{}”.", crate::one_line(other))),
    })
}

fn amount_of(args: &Args, name: &str) -> Result<Option<Money>, String> {
    Ok(args.decimal(name, -1e9, 1e9)?.map(Money::from_units))
}

/// The fees an agent gives: [{kind, amount}], each a cost.
fn fees_of(args: &Args) -> Result<Option<Vec<Fee>>, String> {
    let Some(raw) = args.raw("fees") else { return Ok(None) };
    let wrong = || "“fees” is a list of {kind, amount}: kind processor, commission, conversion, contributions or bank; amount a cost, without sign.".to_string();
    let items = raw.as_array().ok_or_else(wrong)?;
    items
        .iter()
        .map(|item| {
            let kind = item["kind"].as_str().unwrap_or("processor");
            let amount = item["amount"].as_f64().filter(|a| a.is_finite()).ok_or_else(wrong)?;
            Ok(Fee { kind: FeeKind::parse(kind), amount: Money::from_units(amount).abs(), from: FeeFrom::Hand, about: false })
        })
        .collect::<Result<Vec<_>, String>>()
        .map(Some)
}

/// The files an act may write, as they are before it.
fn before(root: &Path) -> Vec<(PathBuf, Option<String>)> {
    [root.join(sioul_core::budget::LEDGER), movements::Store::path(root)].into_iter().map(|p| {
        let text = std::fs::read_to_string(&p).ok();
        (p, text)
    }).collect()
}

/// Those it changed, for the window's Undo.
fn changed(before: Vec<(PathBuf, Option<String>)>) -> Vec<FileChange> {
    before.into_iter().map(|(path, was)| FileChange::of(&path, was)).filter(|f| f.before != f.after).collect()
}

/// The window's line: what the agent did, then its effect.
fn window_line(s: &Session, id: &str, pairs: &[(&str, String)], effect: &str) -> String {
    let line = s.say(id, pairs);
    if effect.is_empty() { line } else { format!("{line} {effect}") }
}

pub fn movements(s: &Session, args: &Args) -> Result<Answer, String> {
    let o = opened(s)?;
    let today = sioul_core::zone::now().date();
    let told_by: Vec<ToldBy> = args.list("told_by")?.iter().map(|w| ToldBy::parse(w).ok_or_else(|| format!("“told_by” holds bank, mail, plan or hand, not “{}”.", crate::one_line(w)))).collect::<Result<_, _>>()?;
    let query = Query {
        from: args.date("from")?,
        to: args.date("to")?,
        account: args.text("account")?,
        budget: args.text("budget")?,
        way: way_of(args)?,
        areas: args.text("area")?.as_deref().and_then(sioul_core::areas::Area::parse),
        at_least: amount_of(args, "at_least")?,
        at_most: amount_of(args, "at_most")?,
        text: args.text("search")?.unwrap_or_default(),
        told_by,
        page: args.number("page", 1, 1, 10_000)? as usize - 1,
    };
    let page = moneylist::query_in(&o.ledger, o.books.movements.iter().filter(|m| o.shows(m)), &query);
    // What the filters keep that is kept from agents: counted, never named.
    let kept = o.books.movements.iter().filter(|m| !o.shows(m) && moneylist::matches(&o.books.ledger, &query, m)).count();
    let mut lines: Vec<String> = Vec::new();
    if let Some(total) = &page.total {
        lines.push(moneylist::total_words(&s.tr, total));
    }
    for m in &page.movements {
        let mut row = shown(moneylist::row_words(&s.tr, &o.books.ledger, m, today));
        row[0] = format!("{} ({})", row[0], m.address());
        lines.extend(row.into_iter().enumerate().map(|(i, l)| if i == 0 { l } else { format!("  {l}") }));
    }
    if page.movements.is_empty() {
        lines.push(s.tr.text("movements-none", None));
    }
    if page.more {
        lines.push(s.say("movements-more", &[("page", (page.page + 2).to_string())]));
    }
    lines.extend(access::left_out(kept));
    let rows: Vec<Value> = page.movements.iter().map(|m| mask::json(moneylist::data(&o.books.ledger, m))).collect();
    let total = page.total.map(|t| json!({ "gross": t.gross.cents() as f64 / 100.0, "net": t.net.cents() as f64 / 100.0 }));
    Ok(Answer { text: lines.join("\n"), data: json!({ "movements": rows, "page": page.page + 1, "more": page.more, "total": total, "kept_from_agents": kept }) })
}

pub fn get_movement(s: &Session, args: &Args) -> Result<Answer, String> {
    let o = opened(s)?;
    let m = o.find(&args.needed("movement")?)?;
    let today = sioul_core::zone::now().date();
    let lines = shown(moneylist::detail_words(&s.tr, &o.books.ledger, m, today));
    Ok(Answer { text: lines.join("\n"), data: mask::json(moneylist::data(&o.books.ledger, m)) })
}

pub fn add_movement(s: &Session, args: &Args) -> Result<Answer, String> {
    let o = opened(s)?;
    let budget = args.needed("budget")?;
    if !o.open.contains(&budget) {
        return Err(if o.books.ledger.budgets.iter().any(|b| b.id == budget) { access::kept("This budget") } else { s.tr.text("movement-no-budget", None) });
    }
    let ties = args.list("ties")?;
    if ties.iter().any(|t| !o.consent.allows(t)) {
        return Err(access::kept("What it would be tied to"));
    }
    let today = sioul_core::zone::now().date();
    let new = acts::New {
        label: args.needed_line("who")?,
        gross: amount_of(args, "gross")?,
        net: amount_of(args, "net")?,
        fees: fees_of(args)?.unwrap_or_default(),
        account: args.text("account")?,
        budget,
        day: args.date("on")?.ok_or("“on” is needed: its day, as 2026-11-05.")?,
        paid_on: args.date("paid_on")?,
        planned: args.flag("planned")?,
        note: args.body("note")?,
        ties,
    };
    let files = before(&o.root);
    let origin = s.say("movement-origin-agent", &[("date", today.to_string())]);
    let act = acts::make(&o.root, &s.tr, &new, &origin, today)?;
    let amount = moneylist::signed(&s.tr, new.net.or(new.gross).unwrap_or_default());
    say_in_window(window_line(s, "agent-movement-made", &[("who", new.label.clone()), ("amount", amount)], &act.said), changed(files));
    Ok(Answer { text: [act.said.as_str(), act.address.as_str()].into_iter().filter(|t| !t.is_empty()).collect::<Vec<_>>().join("\n"), data: json!({ "movement": act.address, "said": act.said }) })
}

pub fn update_movement(s: &Session, args: &Args) -> Result<Answer, String> {
    let o = opened(s)?;
    let address = args.needed("movement")?;
    let m = o.find(&address)?.clone();
    if let Some(budget) = args.text("budget")?
        && !o.open.contains(&budget)
    {
        return Err(if o.books.ledger.budgets.iter().any(|b| b.id == budget) { access::kept("This budget") } else { s.tr.text("movement-no-budget", None) });
    }
    let gross = match args.raw("gross") {
        Some(Value::String(t)) if t.trim() == "none" => Some(None),
        Some(_) => Some(amount_of(args, "gross")?),
        None => None,
    };
    let paid_on = match args.raw("paid_on") {
        Some(Value::String(t)) if t.trim() == "none" => Some(None),
        Some(_) => Some(args.date("paid_on")?),
        None => None,
    };
    let change = acts::Change {
        label: args.line("who")?,
        gross,
        fees: fees_of(args)?,
        net: amount_of(args, "net")?,
        day: args.date("on")?,
        paid_on,
        note: if args.has("note") { Some(args.body("note")?) } else { None },
        budget: args.text("budget")?,
        planned: if args.has("planned") { Some(args.flag("planned")?) } else { None },
        account: args.text("account")?,
    };
    let today = sioul_core::zone::now().date();
    let files = before(&o.root);
    let origin = s.say("movement-origin-agent", &[("date", today.to_string())]);
    let act = acts::change(&o.root, &sioul_core::words::Words::of(&s.config), &s.tr, &address, &change, &origin, today)?;
    say_in_window(window_line(s, "agent-movement-changed", &[("who", mask::text(&m.label))], &act.said), changed(files));
    Ok(Answer { text: act.said.clone(), data: json!({ "movement": act.address, "said": act.said }) })
}

pub fn delete_movement(s: &Session, args: &Args) -> Result<Answer, String> {
    let o = opened(s)?;
    let address = args.needed("movement")?;
    let m = o.find(&address)?.clone();
    let today = sioul_core::zone::now().date();
    let files = before(&o.root);
    let act = acts::remove(&o.root, &sioul_core::words::Words::of(&s.config), &s.tr, &address, today)?;
    say_in_window(window_line(s, "agent-movement-removed", &[("who", mask::text(&m.label))], &act.said), changed(files));
    Ok(Answer { text: act.said.clone(), data: json!({ "movement": act.address, "said": act.said }) })
}

pub fn same_movement(s: &Session, args: &Args) -> Result<Answer, String> {
    let o = opened(s)?;
    let (a, b) = (o.find(&args.needed("movement")?)?.clone(), o.find(&args.needed("other")?)?.clone());
    if a.steady_key() == b.steady_key() {
        return Err("A movement is the same as itself.".into());
    }
    let same = if args.has("same") { args.flag("same")? } else { true };
    let files = before(&o.root);
    let words = sioul_core::words::Words::of(&s.config);
    if same {
        acts::same(&o.root, &words, &a, &b)?;
    } else {
        acts::not_same(&o.root, a.steady_key(), b.steady_key())?;
    }
    sioul_core::changes::said(sioul_core::changes::Kind::Movements);
    let pairs = [("who", mask::text(&a.label)), ("other", mask::text(&b.label))];
    let line = s.say(if same { "agent-movement-same" } else { "agent-movement-apart" }, &pairs);
    say_in_window(line.clone(), changed(files));
    Ok(Answer { text: line, data: json!({ "movement": a.address(), "other": b.address(), "same": same }) })
}
