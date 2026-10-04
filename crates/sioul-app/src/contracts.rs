// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Contracts and subscriptions, for the window (docs/accounting.md): the list
//! on the Budgets page, the recurring payments that have no contract yet, the
//! form, the letter that stops one, a mail kept as a contract.

use crate::backend::{json, load_config, say, tr};
use serde::{Deserialize, Serialize};
use sioul_core::budget::Ledger;
use sioul_core::contracts::{Contract, Contracts, Kind};

fn contracts() -> Result<Contracts, String> {
    let root = load_config().case_store_path().ok_or_else(|| tr().text("papers-no-store", None))?;
    Contracts::load(&root)
}

fn ledger() -> Option<Ledger> {
    load_config().case_store_path().and_then(|root| Ledger::load(&root).ok())
}

#[derive(Serialize)]
struct ContractView {
    id: String,
    kind: &'static str,
    kind_label: String,
    title: String,
    party: String,
    reference: String,
    preset: String,
    /// What it costs, from the payment that pays it: "€62.00 a month".
    cost: String,
    started: String,
    renews: String,
    every: String,
    notice_days: u32,
    cancel: String,
    cancel_is_page: bool,
    covers: String,
    notes: String,
    ended: String,
    /// Where it stands, in a sentence.
    line: String,
}

#[derive(Serialize)]
struct Suggestion {
    preset: String,
    title: String,
    cost: String,
    kind: &'static str,
}

#[derive(Serialize)]
struct Choice {
    id: &'static str,
    label: String,
    notice: u32,
}

#[derive(Serialize, Default)]
struct View {
    store: bool,
    problem: String,
    kinds: Vec<Choice>,
    contracts: Vec<ContractView>,
    /// Recurring payments out, with no contract yet.
    suggestions: Vec<Suggestion>,
    /// Every recurring payment out, to tie a contract to: {id, title}.
    presets: Vec<PresetChoice>,
}

#[derive(Serialize)]
struct PresetChoice {
    id: String,
    title: String,
}

fn cost_of(ledger: Option<&Ledger>, preset: &str) -> String {
    let Some(p) = ledger.and_then(|l| l.presets.iter().find(|p| p.id.as_deref() == Some(preset))) else { return String::new() };
    let amount = tr().money(sioul_core::money::Money(-p.amount.cents()));
    say(if matches!(p.every, sioul_core::budget::Period::Year) { "contract-cost-year" } else { "contract-cost-month" }, &[("amount", amount)])
}

fn line_of(contract: &Contract, today: jiff::civil::Date) -> String {
    let day = |d: jiff::civil::Date| tr().day_in(d, today);
    if let Some(ended) = contract.ended {
        return say("contract-ended", &[("date", day(ended))]);
    }
    match (contract.next_renewal(today), contract.cancel_by(today)) {
        (Some(renewal), Some(by)) if contract.notice_days > 0 && by >= today => say("contract-renews-notice", &[("date", day(renewal)), ("by", day(by))]),
        (Some(renewal), Some(_)) if contract.notice_days > 0 => say("contract-renews-late", &[("date", day(renewal))]),
        (Some(renewal), _) => say("contract-renews", &[("date", day(renewal))]),
        _ if contract.notice_days > 0 => say("contract-open-notice", &[("days", contract.notice_days.to_string())]),
        _ => tr().text("contract-open", None),
    }
}

/// The Budgets page's contracts, as JSON.
pub(crate) fn view() -> String {
    let kinds = Kind::ALL.iter().map(|k| Choice { id: k.id(), label: tr().text(&format!("contract-kind-{}", k.id()), None), notice: k.usual_notice() }).collect();
    let contracts = match contracts() {
        Ok(c) => c,
        Err(problem) => return json(&View { store: false, problem, kinds, ..View::default() }),
    };
    let ledger = ledger();
    let today = jiff::Zoned::now().date();
    let mut list: Vec<&Contract> = contracts.list.iter().collect();
    // Open ones first, by their next date; ended ones last.
    list.sort_by_key(|c| (c.ended.is_some(), c.next_renewal(today).unwrap_or(jiff::civil::Date::MAX), c.title.to_lowercase()));
    let views = list
        .into_iter()
        .map(|c| ContractView {
            id: c.id.clone(),
            kind: c.kind.id(),
            kind_label: tr().text(&format!("contract-kind-{}", c.kind.id()), None),
            title: c.title.clone(),
            party: c.party.clone(),
            reference: c.reference.clone(),
            preset: c.preset.clone(),
            cost: cost_of(ledger.as_ref(), &c.preset),
            started: c.started.map(|d| d.to_string()).unwrap_or_default(),
            renews: c.renews.map(|d| d.to_string()).unwrap_or_default(),
            every: c.every.clone(),
            notice_days: c.notice_days,
            cancel: c.cancel.clone(),
            cancel_is_page: c.cancel.starts_with("https://") || c.cancel.starts_with("http://"),
            covers: c.covers.clone(),
            notes: c.notes.clone(),
            ended: c.ended.map(|d| d.to_string()).unwrap_or_default(),
            line: line_of(c, today),
        })
        .collect();
    let tied: Vec<&str> = contracts.list.iter().map(|c| c.preset.as_str()).filter(|p| !p.is_empty()).collect();
    let suggestions = ledger
        .as_ref()
        .map(|l| {
            l.presets
                .iter()
                .filter(|p| p.amount.cents() < 0 && !p.estimate && p.until.is_none_or(|u| u >= today))
                .filter(|p| p.id.as_deref().is_some_and(|id| !tied.contains(&id)))
                .map(|p| Suggestion { preset: p.id.clone().unwrap_or_default(), title: p.label.clone(), cost: cost_of(Some(l), p.id.as_deref().unwrap_or("")), kind: Kind::guess(&p.label).unwrap_or_default().id() })
                .collect()
        })
        .unwrap_or_default();
    let presets = ledger.as_ref().map(|l| l.presets.iter().filter(|p| p.amount.cents() < 0 && !p.estimate).filter_map(|p| Some(PresetChoice { id: p.id.clone()?, title: p.label.clone() })).collect()).unwrap_or_default();
    json(&View { store: true, problem: String::new(), kinds, contracts: views, suggestions, presets })
}

#[derive(Deserialize)]
struct Edit {
    #[serde(default)]
    kind: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    party: String,
    #[serde(default)]
    reference: String,
    #[serde(default)]
    preset: String,
    #[serde(default)]
    started: String,
    #[serde(default)]
    renews: String,
    #[serde(default)]
    every: String,
    #[serde(default)]
    notice_days: u32,
    #[serde(default)]
    cancel: String,
    #[serde(default)]
    covers: String,
    #[serde(default)]
    notes: String,
    #[serde(default)]
    ended: String,
}

/// A contract saved (new when `id` is empty); returns what went wrong, else "".
pub(crate) fn save(id: &str, edit: &str) -> String {
    let edit: Edit = match serde_json::from_str(edit) {
        Ok(e) => e,
        Err(e) => return e.to_string(),
    };
    let mut contracts = match contracts() {
        Ok(c) => c,
        Err(e) => return e,
    };
    if edit.title.trim().is_empty() {
        return tr().text("papers-no-title", None);
    }
    let date = |text: &str| -> Result<Option<jiff::civil::Date>, String> {
        let text = text.trim();
        if text.is_empty() { Ok(None) } else { text.parse().map(Some).map_err(|_| say("papers-bad-date", &[("date", text.to_string())])) }
    };
    let (started, renews, ended) = match (date(&edit.started), date(&edit.renews), date(&edit.ended)) {
        (Ok(a), Ok(b), Ok(c)) => (a, b, c),
        (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => return e,
    };
    let old = contracts.get(id).cloned().unwrap_or_default();
    let contract = Contract {
        id: old.id.clone(),
        kind: Kind::of(&edit.kind),
        title: edit.title.trim().to_string(),
        party: edit.party.trim().to_string(),
        reference: edit.reference.trim().to_string(),
        preset: edit.preset.trim().to_string(),
        started,
        renews,
        every: if matches!(edit.every.as_str(), "year" | "month") { edit.every.clone() } else { String::new() },
        notice_days: edit.notice_days.min(365),
        cancel: edit.cancel.trim().to_string(),
        covers: edit.covers.trim().to_string(),
        notes: edit.notes.trim().to_string(),
        paper: old.paper.clone(),
        ended,
    };
    contracts.put(contract);
    contracts.save().err().unwrap_or_default()
}

pub(crate) fn remove(id: &str) -> String {
    let mut contracts = match contracts() {
        Ok(c) => c,
        Err(e) => return e,
    };
    if contracts.remove(id) { contracts.save().err().unwrap_or_default() } else { String::new() }
}

/// The letter that stops a contract, as a draft to read, sign and send: to its
/// address when it is a mail address. Returns {"draft"} or {"error"}.
pub(crate) fn letter(id: &str) -> String {
    let made = (|| -> Result<String, String> {
        let contracts = contracts()?;
        let contract = contracts.get(id).ok_or_else(|| tr().text("papers-gone", None))?;
        let config = load_config();
        let account = config.accounts.iter().filter(|a| a.syncs()).min_by_key(|a| a.priority).ok_or_else(|| tr().text("mail-no-account", None))?;
        let mut draft = sioul_core::compose::Draft::new(&account.id);
        if contract.cancel.contains('@') && !contract.cancel.contains(' ') {
            draft.to = vec![contract.cancel.clone()];
        }
        let reference = if contract.reference.is_empty() { String::new() } else { say("contract-letter-reference", &[("reference", contract.reference.clone())]) };
        draft.subject = say("contract-letter-subject", &[("title", contract.title.clone()), ("reference", reference.clone())]).trim().to_string();
        draft.body = say("contract-letter-body", &[("title", contract.title.clone()), ("party", if contract.party.is_empty() { contract.title.clone() } else { contract.party.clone() }), ("reference", reference)]);
        draft.links = vec![format!("sioul:contract/{}", contract.id)];
        draft.add_signature(account.signature.as_deref());
        draft.save()?;
        Ok(draft.id)
    })();
    match made {
        Ok(id) => serde_json::json!({ "draft": id }).to_string(),
        Err(e) => serde_json::json!({ "error": e }).to_string(),
    }
}

/// What a mail says of a contract, to start the form: who sent it, its subject, the kind they suggest.
pub(crate) fn from_card(subject: &str, from: &str) -> String {
    let kind = Kind::guess(subject).or_else(|| Kind::guess(from)).unwrap_or_default();
    serde_json::json!({ "title": subject, "party": from, "kind": kind.id(), "notice_days": kind.usual_notice() }).to_string()
}
