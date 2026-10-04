// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What the window shows, as plain data: every sentence already translated,
//! nothing left to decide.
//!
//! The Qt interface reads these as JSON and only lays them out; an agent or a
//! script can read the very same (docs/architecture.md: the interface holds no
//! logic).

use crate::budget::{Budget, Ledger, MailLine, MailState, Period, Reserve, Verdict};
use crate::money::Money;
use crate::payments::PaymentKind;
use crate::card::Card;
use crate::cases::CaseStore;
use crate::config::{Account, AccountKind, Config, Priority};
use crate::folders::{Folder, Role};
use crate::maildir;
use crate::trust;
use crate::i18n::{self, Translator};
use crate::porch::{self, Lane, Reason, Triaged};
use crate::reading::{self, Html, Part, Person};
use crate::trust::{AuthResults, Outcome, Trust};
use crate::window;
use jiff::Zoned;
use jiff::Timestamp;
use jiff::tz::TimeZone;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

/// Characters of the text shown under a subject.
const PREVIEW: usize = 160;

#[derive(Debug, Serialize)]
pub struct PorchView {
    /// Inside an admin window, without windows, or opened anyway.
    pub open: bool,
    /// Outside a window: when it opens, and nothing else.
    pub closed: Option<String>,
    /// The codes, passwords and links you asked sites for, shown whatever the
    /// time; with a warning when their sender is not verified.
    pub right_now: Vec<CodeView>,
    /// A few sentences about what came: at the top of the Porch, and from `sioul porch`.
    pub summary: String,
    /// One short line: until when the window is open, or that it was opened anyway.
    pub status: String,
    /// The lanes, in the order of the Porch; empty when closed.
    pub lanes: Vec<LaneView>,
}

#[derive(Debug, Serialize)]
pub struct CodeView {
    pub key: String,
    /// "Code from La Banque Exemple (verified)".
    pub title: String,
    /// The code itself, when the message prints one.
    pub code: Option<String>,
    /// "valid about 10 minutes".
    pub validity: Option<String>,
    /// For a sender that is not verified: use it only if you just asked for it.
    pub warning: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LaneView {
    pub key: String,
    pub title: String,
    /// Filed and set-aside mail start folded (docs/porch.md).
    pub folded: bool,
    /// What the lane holds, in one short line, shown under its title.
    pub about: String,
    /// How mail lands here, sentence by sentence, behind the lane's "?".
    pub rules: Vec<String>,
    /// The settings that change it, by page name (`settings::for_view`).
    pub settings: String,
    pub items: Vec<ItemView>,
}

#[derive(Debug, Serialize)]
pub struct ItemView {
    /// The file, to open the message.
    pub key: String,
    pub account: Option<String>,
    pub sender: String,
    pub address: String,
    /// "verified", "not verified", "forged", in your language.
    pub trust: String,
    /// The same, as a fixed word for the interface's colours.
    pub trust_level: &'static str,
    pub subject: String,
    pub date: String,
    pub preview: String,
    /// Every reason, in order; the last one chose the lane.
    pub reasons: Vec<String>,
    pub attachments: Vec<String>,
    /// From someone new: the sender can be let in.
    pub screener: bool,
    /// From an account ranked above the others.
    pub important: bool,
    /// What each check said, one per line: the shield's tooltip.
    pub checks: String,
    /// For a shielded address: what it is about ("work"), its tone ("calm", "rude", "hostile").
    pub topic: String,
    pub tone: String,
    /// Its words are not shown (hostile): no sender's name, subject or text until you ask.
    pub hidden: bool,
    /// What it asks, in one neutral line, when an AI read it.
    pub summary: String,
}

/// The Porch as the window shows it. `opened_anyway` is your "open it anyway"
/// outside a window; codes show either way.
pub fn porch(items: &[Triaged], config: &Config, store: Option<&CaseStore>, tr: &Translator, now: &Zoned, opened_anyway: bool) -> PorchView {
    let right_now = codes(items, tr);
    let open = opened_anyway || config.windows.is_empty() || window::current(&config.windows, now).is_some();
    if !open {
        let mut args = i18n::args();
        let next = window::next_opening(&config.windows, now);
        args.set("when", next.map_or_else(String::new, |z| tr.when(&z)));
        let closed = Some(tr.text("porch-closed", Some(&args)));
        return PorchView { open, closed, right_now, summary: String::new(), status: String::new(), lanes: Vec::new() };
    }
    let status = match window::current(&config.windows, now) {
        Some((_, closing)) => {
            let mut args = i18n::args();
            args.set("time", closing.strftime("%H:%M").to_string());
            tr.text("porch-open-until", Some(&args))
        }
        None if !config.windows.is_empty() => tr.text("porch-opened-anyway", None),
        None => String::new(),
    };
    let order = tr.text("rule-order", None);
    let hidden = |key: &str| key.strip_prefix("case:").is_some_and(|id| config.porch.hidden_projects.iter().any(|h| h == id));
    let lanes = lanes(config, store, tr)
        .into_iter()
        .filter(|l| !hidden(&l.key))
        .map(|LaneInfo { key, title, lane, folded, about, mut rules }| {
            // Mail of the accounts ranked above first, then work for a public address, then the newest.
            let mut ranked: Vec<&Triaged> = items.iter().filter(|t| t.lane == lane).collect();
            ranked.sort_by_key(|t| (t.priority, t.assessment.as_ref().map(|a| a.topic), std::cmp::Reverse(t.card.date.unwrap_or(0))));
            // A folded lane opens by itself when it holds mail of an account ranked above; hostile mail never does.
            let folded = folded && (lane == Lane::Hostile || !ranked.iter().any(|t| t.priority == Priority::Above));
            rules.push(order.clone());
            let settings = format!("lane:{key}");
            LaneView { key, title, folded, about, rules, settings, items: ranked.into_iter().map(|t| item_view(t, store, tr)).collect() }
        })
        .filter(|l| !l.items.is_empty())
        .collect();
    PorchView { open, closed: None, right_now, summary: tr.summary(&porch::summarise(items), store), status, lanes }
}

/// A lane of the Porch, empty or not: what it holds and how mail lands there.
pub struct LaneInfo {
    /// "people", "case:<id>", "public:<account>".
    pub key: String,
    pub title: String,
    pub lane: Lane,
    /// Filed, less important and set-aside mail start folded (docs/porch.md).
    pub folded: bool,
    /// What it holds, in one short line.
    pub about: String,
    /// How mail lands there, sentence by sentence.
    pub rules: Vec<String>,
}

/// Every lane mail can land in, in the order the Porch shows them: cases,
/// public addresses, people, the screener, filed and less important mail,
/// what is set aside, hostile mail when an address is shielded.
pub fn lanes(config: &Config, store: Option<&CaseStore>, tr: &Translator) -> Vec<LaneInfo> {
    let say = |id: &str, pairs: &[(&str, String)]| {
        let mut args = i18n::args();
        for (name, value) in pairs {
            args.set(name.to_string(), value.clone());
        }
        tr.text(id, Some(&args))
    };
    let lane = |key: &str, title: String, lane: Lane, folded: bool, about: String, rules: Vec<String>| LaneInfo { key: key.to_string(), title, lane, folded, about, rules };
    let address_of = |id: &str| config.account(id).and_then(|a| a.address.clone()).unwrap_or_else(|| id.to_string());
    let mut lanes: Vec<LaneInfo> = store
        .map_or(&[][..], |s| &s.cases[..])
        .iter()
        .map(|c| {
            let mut rules = case_rules(c, tr);
            rules.push(tr.text("rule-where-routes", None));
            lane(&format!("case:{}", c.id), c.title.clone(), Lane::Case(c.id.clone()), false, tr.text("lane-about-case", None), rules)
        })
        .collect();
    for account in config.accounts.iter().filter(|a| a.shield) {
        let address = address_of(&account.id);
        let minutes = account.fetch_minutes.or(config.fetch_minutes).unwrap_or(crate::config::DEFAULT_FETCH_MINUTES);
        let mut rules = vec![say("rule-public", &[("minutes", minutes.to_string())])];
        if account.shield_ai {
            rules.push(tr.text("rule-public-ai", None));
        }
        rules.push(tr.text("rule-where-shield", None));
        lanes.push(lane(&format!("public:{}", account.id), say("lane-public", &[("address", address.clone())]), Lane::Public(account.id.clone()), false, say("lane-about-public", &[("address", address)]), rules));
    }
    let known = crate::porch::SenderList::load(&config.known_senders_path()).entries().len();
    lanes.push(lane("people", tr.text("lane-people", None), Lane::People, false, tr.text("lane-about-people", None), vec![say("rule-people", &[("n", known.to_string())])]));
    lanes.push(lane("screener", tr.text("lane-screener", None), Lane::Screener, false, tr.text("lane-about-screener", None), vec![tr.text("rule-screener", None)]));
    let words = config.filed_words.clone().unwrap_or_else(|| porch::AUTOMATIC.iter().map(|w| w.to_string()).collect());
    lanes.push(lane("filed", tr.text("lane-filed", None), Lane::Filed, true, tr.text("lane-about-filed", None), vec![say("rule-filed", &[("words", words.join(", "))])]));
    let below: Vec<String> = config.accounts.iter().filter(|a| a.priority == Priority::Below).map(|a| address_of(&a.id)).collect();
    lanes.push(lane("low", tr.text("lane-low", None), Lane::Low, true, tr.text("lane-about-low", None), if below.is_empty() { vec![tr.text("rule-low-none", None)] } else { vec![say("rule-low", &[("addresses", below.join(", "))]), tr.text("rule-where-rank", None)] }));
    lanes.push(lane(
        "set-aside",
        tr.text("lane-set-aside", None),
        Lane::SetAside,
        true,
        tr.text("lane-about-set-aside", None),
        ["rule-forged", "rule-borrowed", "rule-spam", "rule-blocked", "rule-where-blocked"].iter().map(|id| tr.text(id, None)).collect(),
    ));
    if config.accounts.iter().any(|a| a.shield) {
        lanes.push(lane("hostile", tr.text("lane-hostile", None), Lane::Hostile, true, tr.text("lane-about-hostile", None), vec![tr.text("rule-hostile", None), tr.text("rule-where-shield", None)]));
    }
    lanes
}

/// A case's routes, each in a sentence.
fn case_rules(case: &crate::cases::Case, tr: &Translator) -> Vec<String> {
    if case.routes.is_empty() {
        return vec![tr.text("rule-case-none", None)];
    }
    let part = |id: &str, list: &[String]| -> Option<String> {
        (!list.is_empty()).then(|| {
            let mut args = i18n::args();
            args.set("list", list.join(", "));
            tr.text(id, Some(&args))
        })
    };
    case.routes
        .iter()
        .map(|route| {
            let parts: Vec<String> = [
                part("rule-part-domains", &route.from_domains),
                part("rule-part-addresses", &route.from_addresses),
                part("rule-part-subject", &route.subject_contains),
                part("rule-part-text", &route.text_contains),
            ]
            .into_iter()
            .flatten()
            .collect();
            let mut args = i18n::args();
            args.set("parts", parts.join(&tr.text("rule-and", None)));
            tr.text("rule-route", Some(&args))
        })
        .collect()
}

/// The verified codes among `items`: what a notification says.
pub fn codes(items: &[Triaged], tr: &Translator) -> Vec<CodeView> {
    items.iter().filter(|t| t.lane == Lane::RightNow).map(|t| code_view(t, tr)).collect()
}

impl CodeView {
    /// The code and how long it lasts, on one line: "482913, valid about 10
    /// minutes"; under it, the warning when its sender is not verified.
    pub fn body(&self) -> String {
        let line = [self.code.as_deref(), self.validity.as_deref()].into_iter().flatten().collect::<Vec<_>>().join(", ");
        match self.warning.as_deref() {
            Some(warning) if line.is_empty() => warning.to_string(),
            Some(warning) => format!("{line}\n{warning}"),
            None => line,
        }
    }
}

fn code_view(t: &Triaged, tr: &Translator) -> CodeView {
    let code = t.code.as_ref();
    let kind = code.map_or(crate::codes::CodeKind::Code, |c| c.kind);
    let validity = code.and_then(|c| c.expires_minutes).map(|minutes| tr.validity(minutes));
    CodeView {
        key: key(&t.card),
        title: tr.right_now(kind, t.card.sender(), t.trust, None, None),
        code: code.and_then(|c| c.code.clone()),
        validity,
        warning: (t.trust != Trust::Verified).then(|| tr.text("right-now-unverified", None)),
    }
}

fn item_view(t: &Triaged, store: Option<&CaseStore>, tr: &Translator) -> ItemView {
    let preview: String = t.card.excerpt.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(PREVIEW).collect();
    let mut view = ItemView {
        key: key(&t.card),
        account: t.card.account.clone(),
        sender: t.card.sender().to_string(),
        address: t.card.from_address.clone().unwrap_or_default(),
        trust: tr.trust(t.trust),
        trust_level: match t.trust {
            Trust::Verified => "verified",
            Trust::Unverified => "unverified",
            Trust::Forged => "forged",
        },
        subject: t.card.subject.clone(),
        date: date(tr, t.card.date),
        preview,
        reasons: t.reasons.iter().map(|r| tr.reason(r, store)).collect(),
        attachments: t.card.attachments.clone(),
        screener: t.reasons.contains(&Reason::FirstMessage),
        important: t.priority == Priority::Above,
        checks: checks(t.checks.as_ref(), tr),
        topic: t.assessment.as_ref().map(|a| tr.topic(a.topic)).unwrap_or_default(),
        tone: t.assessment.as_ref().map_or(String::new(), |a| format!("{:?}", a.tone).to_lowercase()),
        hidden: false,
        summary: t.assessment.as_ref().map(|a| a.summary.clone()).unwrap_or_default(),
    };
    // Hostile mail keeps its words to itself: a sender's name can insult too.
    if t.lane == Lane::Hostile {
        view.hidden = true;
        view.sender = t.card.sender_domain().map_or_else(|| tr.text("hostile-someone", None), |d| {
            let mut args = i18n::args();
            args.set("domain", d.to_string());
            tr.text("hostile-someone-at", Some(&args))
        });
        view.subject = tr.text("hostile-subject", None);
        view.preview = String::new();
        view.summary = String::new();
    } else if t.assessment.as_ref().is_some_and(|a| a.tone == crate::shield::Tone::Rude) {
        view.preview = String::new();
    }
    view
}

/// What each check said, one per line: who checked, then DKIM, SPF, DMARC with
/// the domain's policy, ARC and the reverse DNS, each with the domain it was for.
pub fn checks(results: Option<&AuthResults>, tr: &Translator) -> String {
    let Some(r) = results else { return tr.text("checks-none", None) };
    let by = if r.authserv_id.starts_with("sioul-") {
        tr.text("checks-by-sioul", None)
    } else {
        let mut args = i18n::args();
        args.set("id", r.authserv_id.clone());
        tr.text("checks-by-provider", Some(&args))
    };
    let dmarc_detail = match (&r.dmarc_domain, &r.dmarc_policy) {
        (Some(domain), Some(policy)) => {
            let mut args = i18n::args();
            args.set("policy", policy.clone());
            Some(format!("{domain}, {}", tr.text("check-policy", Some(&args))))
        }
        (domain, _) => domain.clone(),
    };
    let rows: [(String, Option<Outcome>, Option<String>); 5] = [
        ("DKIM".into(), r.dkim, r.dkim_domain.clone()),
        ("SPF".into(), r.spf, r.spf_domain.clone()),
        ("DMARC".into(), r.dmarc, dmarc_detail),
        ("ARC".into(), r.arc, None),
        (tr.text("protocol-iprev", None), r.iprev, None),
    ];
    let lines: Vec<String> = rows
        .into_iter()
        .filter_map(|(protocol, outcome, domain)| {
            let outcome = outcome?;
            let mut args = i18n::args();
            args.set("protocol", protocol);
            args.set("outcome", tr.text(outcome_id(outcome), None));
            let id = match domain {
                Some(d) => {
                    args.set("domain", d);
                    "check-line-domain"
                }
                None => "check-line",
            };
            Some(tr.text(id, Some(&args)))
        })
        .collect();
    if lines.is_empty() {
        return tr.text("checks-none", None);
    }
    std::iter::once(by).chain(lines).collect::<Vec<_>>().join("\n")
}

fn outcome_id(outcome: Outcome) -> &'static str {
    match outcome {
        Outcome::Pass => "check-pass",
        Outcome::Fail => "check-fail",
        Outcome::SoftFail => "check-softfail",
        Outcome::Neutral => "check-neutral",
        Outcome::None => "check-none",
        Outcome::TempError => "check-temperror",
        Outcome::PermError => "check-permerror",
        Outcome::Other => "check-other",
    }
}

fn key(card: &Card) -> String {
    card.path.as_ref().map(|p| p.display().to_string()).unwrap_or_default()
}

/// A message's date, short, in your language and time zone.
pub fn date(tr: &Translator, seconds: Option<i64>) -> String {
    seconds
        .and_then(|s| Timestamp::from_second(s).ok())
        .map_or_else(|| tr.text("no-date", None), |t| tr.date(&t.to_zoned(TimeZone::system()), true))
}

#[derive(Debug, Serialize)]
pub struct BudgetsView {
    pub title: String,
    pub budgets: Vec<BudgetCard>,
    pub reserves_title: String,
    pub reserves: Vec<ReserveCard>,
    pub mail_title: String,
    /// Messages about money, newest first; those you said are not payments are left out.
    pub mail: Vec<MailRow>,
    /// The budgets a line can go to.
    pub choices: Vec<Choice>,
}

/// One figure of a card: a label and a value, on their own line.
#[derive(Debug, Serialize)]
pub struct Figure {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Serialize)]
pub struct BudgetCard {
    pub id: String,
    pub title: String,
    /// "monthly", "mensuel".
    pub period: String,
    /// "better", "as-planned", "short", for the interface's colours and icons.
    pub mood: &'static str,
    /// The verdict, at the period's pace: "On track", "Ahead of its pace by €40".
    pub verdict: String,
    /// "10 % of October gone; 12 % of what it still needs has come in."
    pub pace: String,
    pub figures: Vec<Figure>,
    /// What is kept for later, and what a reserve does at the end of the period.
    pub notes: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ReserveCard {
    pub id: String,
    pub title: String,
    pub figures: Vec<Figure>,
}

#[derive(Debug, Serialize)]
pub struct MailRow {
    pub key: String,
    pub date: String,
    /// "received", "paid", "order", "bill", "refund".
    pub kind: PaymentKind,
    pub kind_label: String,
    /// Who paid or was paid.
    pub party: String,
    pub subject: String,
    pub sender: String,
    /// "+€12.00", "−€387.00"; empty when the message gives no amount.
    pub amount: String,
    pub credit: bool,
    /// The budget its rule gives it, else none: you choose.
    pub budget: Option<String>,
    /// "recorded", "duplicate", "proposed".
    pub state: &'static str,
    /// What becomes of it, in a few words.
    pub note: String,
    /// From someone new and not verified: to check before counting.
    pub doubtful: bool,
    /// It can be added: proposed, with an amount.
    pub can_add: bool,
}

#[derive(Debug, Serialize)]
pub struct Choice {
    pub id: String,
    pub title: String,
}

/// Budgets, reserves and the mail about money, itemized: each figure on its own
/// line, the verdict apart.
pub fn budgets(ledger: &Ledger, mail: &[MailLine], tr: &Translator, today: jiff::civil::Date) -> BudgetsView {
    let mut args = i18n::args();
    args.set("month", tr.month_year(today));
    BudgetsView {
        title: tr.text("budgets-title", Some(&args)),
        budgets: ledger.budgets.iter().map(|b| budget_card(ledger, b, tr, today)).collect(),
        reserves_title: tr.text("reserves-title", None),
        reserves: ledger.reserves.iter().map(|r| reserve_card(ledger, r, tr, today)).collect(),
        mail_title: tr.text("mail-section", None),
        mail: mail.iter().filter(|l| l.state != MailState::Ignored).map(|l| mail_row(ledger, mail, l, tr)).collect(),
        choices: ledger.budgets.iter().map(|b| Choice { id: b.id.clone(), title: b.title.clone() }).collect(),
    }
}

/// "10 % of the month gone; 12 % of what it still needs has come in.", or what it may spend.
fn pace_sentence(pace: &crate::budget::Pace, tr: &Translator, monthly: bool) -> String {
    let percent = |x: f64| ((x * 100.0).round().clamp(0.0, 999.0) as i64).to_string();
    let mut args = i18n::args();
    args.set("gone", percent(pace.gone));
    let period = if monthly { "month" } else { "year" };
    let id = if pace.needed.cents() > 0 {
        args.set("part", percent(pace.unscheduled.cents() as f64 / pace.needed.cents() as f64));
        "needs"
    } else if pace.needed.cents() < 0 {
        args.set("part", percent((-pace.unscheduled.cents()).max(0) as f64 / (-pace.needed.cents()) as f64));
        "may-spend"
    } else {
        "nothing-needed"
    };
    tr.text(&format!("pace-{id}-{period}"), Some(&args))
}

fn budget_card(ledger: &Ledger, budget: &Budget, tr: &Translator, today: jiff::civil::Date) -> BudgetCard {
    let status = ledger.status(budget, today);
    let pace = ledger.pace(budget, today);
    let figure = |id: &str, pairs: &[(&str, String)], value: Money| {
        let mut args = i18n::args();
        for (key, v) in pairs {
            args.set(key.to_string(), v.clone());
        }
        Figure { label: tr.text(id, Some(&args)), value: tr.money(value) }
    };
    let mut figures = vec![
        figure("fig-so-far", &[], status.so_far),
        figure("fig-expected", &[("end", tr.day_month(status.end))], pace.realistic),
        figure("fig-target", &[], status.target),
    ];
    if let Some((low, high)) = pace.range {
        let mut args = i18n::args();
        args.set("low", tr.money(low));
        args.set("high", tr.money(high));
        figures.insert(2, Figure { label: tr.text("fig-range", None), value: tr.text("fig-range-value", Some(&args)) });
    }
    if status.opening != Money::ZERO {
        figures.insert(0, figure("fig-opening", &[], status.opening));
    }
    if status.earmarked != Money::ZERO {
        figures.push(figure("fig-earmarked", &[], status.earmarked));
    }
    // Judged at its pace: what simply happens is weighed against the part of the period gone.
    let (mood, verdict) = match pace.verdict {
        Verdict::Better => ("better", "badge-pace-better"),
        Verdict::AsPlanned => ("as-planned", "badge-pace-on-track"),
        Verdict::Short => ("short", "badge-pace-short"),
    };
    let mut args = i18n::args();
    args.set("gap", tr.money(pace.ahead.abs()));
    let mut notes = Vec::new();
    if let Some((reserve, amount)) = &status.reserve_transfer {
        let mut args = i18n::args();
        args.set("reserve", ledger.reserve_title(reserve).to_string());
        args.set("amount", tr.money(amount.abs()));
        args.set("end", tr.day_month(status.end));
        notes.push(tr.text(if amount.is_negative() { "budget-swept" } else { "budget-drawn" }, Some(&args)));
    }
    BudgetCard {
        id: budget.id.clone(),
        title: budget.title.clone(),
        period: tr.text(if budget.period == Period::Month { "period-month" } else { "period-year" }, None),
        mood,
        verdict: tr.text(verdict, Some(&args)),
        pace: pace_sentence(&pace, tr, budget.period == Period::Month),
        figures,
        notes,
    }
}

/// One movement of a budget, as its ledger lists it.
#[derive(Debug, Serialize)]
pub struct MovementRow {
    /// Its address for links, when it is a line of the file.
    pub uri: String,
    pub date: String,
    /// The same, for its form: "2026-10-03", and the amount in units.
    pub day: String,
    pub value: f64,
    pub label: String,
    pub amount: String,
    pub incoming: bool,
    pub planned: bool,
    pub recurring: bool,
}

/// The balance at one point, for the graph.
#[derive(Debug, Serialize)]
pub struct BalancePoint {
    pub label: String,
    pub date: String,
    pub cents: i64,
    pub future: bool,
}

/// A budget opened: its ledger for a period, and its balance as a graph.
#[derive(Debug, Serialize)]
pub struct BudgetDetail {
    pub card: BudgetCard,
    /// "October 2026", "2026".
    pub period_title: String,
    /// The first day of the period shown, to move from.
    pub from: String,
    pub movements: Vec<MovementRow>,
    pub money_in: String,
    pub money_out: String,
    /// "day", "week", "month", "year".
    pub step: String,
    pub points: Vec<BalancePoint>,
    pub low_cents: i64,
    pub high_cents: i64,
    pub target_cents: i64,
    /// Leisure money: it stays in view in quiet time.
    pub personal: bool,
    /// What it is for (docs/areas.md): "work", "admin", "leisure", joined by "+"; "" for your admin, unsaid.
    pub area: String,
    /// "month" or "year", for its form.
    pub period_kind: String,
}

/// A budget's ledger for the period around `anchor`, and its balance by `step`.
pub fn budget_detail(ledger: &Ledger, budget: &Budget, anchor: jiff::civil::Date, step: &str, tr: &Translator, today: jiff::civil::Date) -> BudgetDetail {
    let (start, end) = budget.period.bounds(anchor);
    let moves = ledger.movements(budget, start, end, today);
    let money_in: Money = moves.iter().filter(|m| !m.amount.is_negative()).map(|m| m.amount).sum();
    let money_out: Money = moves.iter().filter(|m| m.amount.is_negative()).map(|m| m.amount).sum();
    let span = |years: i64, months: i64, weeks: i64, days: i64| jiff::Span::new().years(years).months(months).weeks(weeks).days(days);
    let (from, to) = match step {
        "week" => (anchor.checked_sub(span(0, 0, 8, 0)).unwrap_or(anchor), anchor.checked_add(span(0, 0, 4, 0)).unwrap_or(anchor)),
        "month" => (anchor.first_of_year(), anchor.last_of_year()),
        "year" => (budget.since.unwrap_or(anchor.checked_sub(span(4, 0, 0, 0)).unwrap_or(anchor)).first_of_year(), anchor.checked_add(span(1, 0, 0, 0)).unwrap_or(anchor).last_of_year()),
        _ => (anchor.first_of_month(), anchor.last_of_month()),
    };
    let points: Vec<BalancePoint> = ledger
        .balance_series(budget, from, to, step, today)
        .into_iter()
        .map(|p| BalancePoint {
            label: match step {
                "year" => p.date.year().to_string(),
                // Its short name: three letters would make "jui" of both juin and juillet.
                "month" => tr.month_short(p.date),
                "week" => tr.day_month(p.date),
                _ => p.date.day().to_string(),
            },
            date: p.date.to_string(),
            cents: p.balance.cents(),
            future: p.future,
        })
        .collect();
    let mut card = budget_card(ledger, budget, tr, today);
    if !(start <= today && today <= end) {
        // Another period: its movements only, no pace.
        card.verdict = String::new();
        card.pace = String::new();
    }
    BudgetDetail {
        card,
        period_title: if budget.period == Period::Month { tr.month_year(start) } else { start.year().to_string() },
        from: start.to_string(),
        movements: moves
            .iter()
            .rev()
            .map(|m| MovementRow {
                uri: m.line.map(|i| crate::links::budget_uri(&budget.id, i)).unwrap_or_default(),
                date: tr.day(m.date),
                day: m.date.to_string(),
                value: m.amount.cents() as f64 / 100.0,
                label: m.label.clone(),
                amount: tr.money(m.amount),
                incoming: !m.amount.is_negative(),
                planned: m.planned,
                recurring: m.recurring,
            })
            .collect(),
        money_in: tr.money(money_in),
        money_out: tr.money(money_out.abs()),
        step: step.to_string(),
        low_cents: points.iter().map(|p| p.cents).min().unwrap_or(0).min(budget.target.cents()),
        high_cents: points.iter().map(|p| p.cents).max().unwrap_or(0).max(budget.target.cents()),
        target_cents: budget.target.cents(),
        personal: budget.area.as_deref() == Some("personal"),
        // "personal" was the word for leisure money before areas had three.
        area: match budget.area.as_deref() {
            Some("personal") => "leisure".into(),
            Some(other) => crate::areas::Area::parse(other).map(|a| a.id()).unwrap_or_default(),
            None => String::new(),
        },
        period_kind: if budget.period == Period::Year { "year".into() } else { "month".into() },
        points,
    }
}

fn reserve_card(ledger: &Ledger, reserve: &Reserve, tr: &Translator, today: jiff::civil::Date) -> ReserveCard {
    let status = ledger.reserve_status(reserve, today);
    let mut month = i18n::args();
    month.set("done", tr.money(status.month_done));
    month.set("planned", tr.money(status.month_planned));
    let mut year_end = i18n::args();
    year_end.set("date", tr.day_month(today.last_of_year()));
    let pace = match status.months_left {
        Some(months) => {
            // A number, for its plural: "about a month", "about 16 months".
            let mut args = i18n::args();
            args.set("months", months);
            tr.text("fig-pace-months", Some(&args))
        }
        None => tr.text("fig-pace-quiet", None),
    };
    let mut figures = vec![Figure { label: tr.text("fig-today", None), value: tr.money(status.balance_now) }];
    // How long money asked from it takes to arrive, when it does not come at once.
    if reserve.delay_days > 0 {
        let mut args = i18n::args();
        args.set("days", i64::from(reserve.delay_days));
        figures.push(Figure { label: tr.text("fig-delay", None), value: tr.text("reserve-delay-short", Some(&args)) });
    }
    figures.extend([
        Figure { label: tr.text("fig-this-month", None), value: tr.text("fig-month-flows", Some(&month)) },
        Figure { label: tr.text("fig-by-date", Some(&year_end)), value: tr.money(status.year_end) },
        Figure { label: tr.text("fig-pace", None), value: pace },
    ]);
    ReserveCard { id: reserve.id.clone(), title: reserve.title.clone(), figures }
}

fn mail_row(ledger: &Ledger, all: &[MailLine], line: &MailLine, tr: &Translator) -> MailRow {
    let signed = line.signed();
    let amount = signed.map_or_else(String::new, |m| if m.is_negative() { tr.money(m) } else { format!("+{}", tr.money(m)) });
    let say = |id: &str, key: &str, value: String| {
        let mut args = i18n::args();
        args.set(key.to_string(), value);
        tr.text(id, Some(&args))
    };
    let preset_label = |id: &str| ledger.presets.iter().find(|p| p.id.as_deref() == Some(id)).map_or(id.to_string(), |p| p.label.clone());
    let (state, note) = match &line.state {
        MailState::Recorded => {
            let budget = ledger.lines.iter().find(|l| l.links.contains(&line.key)).map_or("", |l| ledger.budget_title(&l.budget));
            ("recorded", say("note-recorded", "budget", budget.to_string()))
        }
        MailState::Duplicate(other) => {
            let other = all.iter().find(|l| &l.key == other).map_or_else(String::new, |l| format!("{} ({})", l.payment.party, date_of(tr, l.date)));
            ("duplicate", say("note-duplicate", "other", other))
        }
        _ => match (&line.preset, signed) {
            (Some(preset), None) => ("proposed", say("note-preset", "preset", preset_label(preset))),
            (Some(preset), Some(_)) => ("proposed", say("note-preset-amount", "preset", preset_label(preset))),
            (None, None) => ("proposed", tr.text("note-no-amount", None)),
            (None, Some(_)) => ("proposed", String::new()),
        },
    };
    MailRow {
        key: line.key.clone(),
        date: date_of(tr, line.date),
        kind: line.payment.kind,
        kind_label: tr.text(line.payment.kind.message_id(), None),
        party: line.payment.party.clone(),
        subject: line.subject.clone(),
        sender: line.sender.clone(),
        amount,
        credit: line.payment.kind.is_credit(),
        budget: line.budget.clone(),
        state,
        note,
        doubtful: line.doubtful,
        can_add: line.state == MailState::Proposed && signed.is_some(),
    }
}

fn date_of(tr: &Translator, date: jiff::civil::Date) -> String {
    tr.day_month(date)
}

#[derive(Debug, Serialize)]
pub struct AccountView {
    pub id: String,
    /// "imap", "portal".
    pub kind: &'static str,
    pub address: Option<String>,
    pub url: Option<String>,
    /// Server, sender checks, where the mail is kept: one per line.
    pub rows: Vec<Figure>,
    /// The last thing sync said, given by the caller.
    pub status: String,
    /// That last thing was a problem.
    pub status_error: bool,
    /// "above", "average", "below".
    pub priority: &'static str,
    /// Signed in with Google (its tokens in the keyring): it can be signed in again.
    pub google: bool,
    /// Its server, for its password's login in a vault (Bitwarden).
    pub host: Option<String>,
    /// No password kept on this device, or its server refused it: one can be
    /// given (an account come from another device arrives without one).
    pub password_wanted: bool,
    /// Your name as recipients see it, and the signature, in Markdown.
    pub name: String,
    pub signature: String,
    /// Switched on: synced and shown.
    pub enabled: bool,
    /// Who it is: its address (lowercase), else its id. One card per identity,
    /// each of its services (mail, calendars and contacts, Google) on it.
    pub identity: String,
    /// "mail", "dav", "google".
    pub service: &'static str,
}

/// The accounts of the configuration, those switched off too (sites are not
/// accounts: docs/sites.md), with what sync last said about each (`status`:
/// the sentence, and whether it is a problem) and those whose password is
/// wanted (`wanted`, by id).
pub fn accounts(config: &Config, tr: &Translator, status: &BTreeMap<String, (String, bool)>, wanted: &BTreeSet<String>) -> Vec<AccountView> {
    config
        .every_account()
        .filter(|a| a.kind != AccountKind::Portal)
        .map(|a| {
            let mut rows = Vec::new();
            if a.auth.as_deref() == Some("google") {
                rows.push(Figure { label: tr.text("account-row-server", None), value: "Google".into() });
            } else if let (Some(host), true) = (&a.host, a.kind == AccountKind::Dav) {
                rows.push(Figure { label: tr.text("account-row-server", None), value: host.clone() });
            } else if let Some(host) = &a.host {
                let mut args = i18n::args();
                args.set("host", host.clone());
                args.set("port", a.port_or_default().to_string());
                args.set("security", tr.text(&format!("security-{}", a.security.as_str()), None));
                rows.push(Figure { label: tr.text("account-row-server", None), value: tr.text("account-server", Some(&args)) });
            }
            if a.kind == AccountKind::Imap {
                let checks = if a.trusted_authserv_ids.is_empty() {
                    tr.text("account-checks-learning", None)
                } else {
                    a.trusted_authserv_ids.join(", ")
                };
                rows.push(Figure { label: tr.text("account-row-checks", None), value: checks });
                rows.push(Figure { label: tr.text("account-row-mail", None), value: a.maildir_path().display().to_string() });
            }
            if let (Some(url), true) = (&a.url, a.kind != AccountKind::Dav) {
                rows.push(Figure { label: tr.text("account-row-web", None), value: url.clone() });
            }
            if a.kind == AccountKind::Dav {
                rows.push(Figure { label: tr.text("account-row-dav", None), value: crate::vdir::Kind::Contacts.root().join(&a.id).display().to_string() });
                rows.push(Figure { label: String::new(), value: crate::vdir::Kind::Calendars.root().join(&a.id).display().to_string() });
            }
            let (status, status_error) = status.get(&a.id).cloned().unwrap_or_default();
            AccountView {
                id: a.id.clone(),
                kind: match a.kind {
                    AccountKind::Imap => "imap",
                    AccountKind::Jmap => "jmap",
                    AccountKind::Portal => "portal",
                    AccountKind::Dav => "dav",
                },
                address: a.address.clone(),
                url: a.url.clone(),
                rows,
                status,
                status_error,
                priority: a.priority.as_str(),
                google: a.auth.as_deref() == Some("google"),
                host: a.host.clone(),
                password_wanted: wanted.contains(&a.id) && a.auth.as_deref() != Some("google"),
                name: a.name.clone().unwrap_or_default(),
                signature: a.signature.clone().unwrap_or_default(),
                enabled: a.enabled,
                identity: a.address.as_deref().map(str::to_lowercase).unwrap_or_else(|| a.id.clone()),
                service: if a.auth.as_deref() == Some("google") {
                    "google"
                } else if a.kind == AccountKind::Dav {
                    "dav"
                } else {
                    "mail"
                },
            }
        })
        .collect()
}

#[derive(Debug, Serialize)]
pub struct MessageView {
    /// "mid:<Message-ID>", to follow and make links (`links`); empty without one.
    pub uri: String,
    pub from_name: String,
    pub from_address: String,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    /// The long date: "Thursday 1 October at 12:03".
    pub date: String,
    pub subject: String,
    pub attachments: Vec<AttachmentView>,
    /// Mail set aside shows its attachments' names, and does not open them.
    pub can_open: bool,
    /// The text in parts, each also as rich text with clickable links.
    pub parts: Vec<PartView>,
    /// The HTML version made safe, shown instead of the parts when there is one.
    pub html: Option<Html>,
    /// What each check said, one per line: the shield's tooltip.
    pub checks: String,
    /// What "block" offers: the address, and `@domain`.
    pub block_address: Option<String>,
    pub block_domain: Option<String>,
    /// The sender may be judged from this message: it is not forged and borrows
    /// no name. A forged message never changes who is safe or blocked.
    pub sender_judgeable: bool,
    /// Someone besides the sender got it: "Reply to all" is offered.
    pub others: bool,
    pub unread: bool,
    pub flagged: bool,
    /// The folder it is in, for the actions that fit there ("Not junk" in the junk).
    pub role: Option<Role>,
    /// Encrypted or signed with OpenPGP, and what came of it.
    pub protection: Option<ProtectionView>,
}

/// A part of a message's text (see `reading::Part`), with its rich text.
#[derive(Debug, Serialize)]
pub struct PartView {
    /// "text", "attribution", "quote", "headers", "signature".
    pub kind: &'static str,
    pub depth: u8,
    pub text: String,
    /// The text as HTML, its links clickable.
    pub rich: String,
    /// For "headers": name, value, name, value…
    pub fields: Vec<String>,
}

impl From<Part> for PartView {
    fn from(part: Part) -> PartView {
        let (kind, depth, text, fields) = match part {
            Part::Text { text } => ("text", 0, text, Vec::new()),
            Part::Attribution { text } => ("attribution", 0, text, Vec::new()),
            Part::Quote { depth, text } => ("quote", depth, text, Vec::new()),
            Part::Signature { text } => ("signature", 0, text, Vec::new()),
            Part::Headers { fields } => ("headers", 0, String::new(), fields.into_iter().flat_map(|(n, v)| [n, v]).collect()),
        };
        PartView { kind, depth, rich: reading::linkify(&text), text, fields }
    }
}

#[derive(Debug, Serialize)]
pub struct AttachmentView {
    pub index: u32,
    pub name: String,
    /// "1.2 MB", "202 Ko".
    pub size: String,
    /// "pdf", "image", "text", "document", "archive", "calendar", "other", for the icon.
    pub kind: &'static str,
}

/// A message for the reading pane: who wrote to whom, the attachments apart, the
/// text in parts (see `reading`).
/// A message laid out for reading. `own` holds your addresses, to tell whether
/// anyone else would get a reply to all.
pub fn message(path: &std::path::Path, triaged: &Triaged, own: &[String], tr: &Translator) -> Option<MessageView> {
    message_from(&std::fs::read(path).ok()?, path, triaged, own, tr, None)
}

/// The same from the message's bytes (decrypted, they live only in memory),
/// with what OpenPGP found when it was protected.
pub fn message_from(raw: &[u8], path: &std::path::Path, triaged: &Triaged, own: &[String], tr: &Translator, pgp: Option<crate::pgp::PgpView>) -> Option<MessageView> {
    let mut reading = reading::read_bytes(raw)?;
    // A signature, or the encrypted payload, is not something to open.
    reading.attachments.retain(|a| !matches!(a.mime.to_ascii_lowercase().as_str(), "application/pgp-signature" | "application/pgp-encrypted" | "application/pgp-keys"));
    let mut people: Vec<String> = reading
        .to
        .iter()
        .chain(&reading.cc)
        .filter_map(|p| p.address.as_ref().map(|a| a.to_ascii_lowercase()))
        .chain(triaged.card.from_address.clone())
        .filter(|a| !own.iter().any(|o| o.eq_ignore_ascii_case(a)))
        .collect();
    people.sort();
    people.dedup();
    let flags = maildir::flags_of(path);
    let unread = !flags.contains('S');
    let flagged = flags.contains('F');
    let person = |p: &Person| match (&p.name, &p.address) {
        (Some(name), Some(address)) => format!("{name} <{address}>"),
        (Some(name), None) => name.clone(),
        (None, Some(address)) => address.clone(),
        (None, None) => String::new(),
    };
    let address = triaged.card.from_address.clone();
    Some(MessageView {
        uri: triaged.card.message_id.as_deref().map(crate::links::mail_uri).unwrap_or_default(),
        from_name: triaged.card.sender().to_string(),
        from_address: address.clone().unwrap_or_default(),
        to: reading.to.iter().map(person).collect(),
        cc: reading.cc.iter().map(person).collect(),
        date: triaged
            .card
            .date
            .and_then(|s| Timestamp::from_second(s).ok())
            .map_or_else(|| tr.text("no-date", None), |t| tr.date(&t.to_zoned(TimeZone::system()), false)),
        subject: reading.subject.clone(),
        attachments: reading
            .attachments
            .iter()
            .map(|a| AttachmentView { index: a.index, name: a.name.clone(), size: size(tr, a.size), kind: attachment_kind(&a.mime) })
            .collect(),
        can_open: triaged.lane != Lane::SetAside,
        parts: reading.parts.into_iter().map(PartView::from).collect(),
        html: reading.html,
        checks: checks(triaged.checks.as_ref(), tr),
        block_domain: triaged.card.sender_domain().map(|d| format!("@{}", d.to_ascii_lowercase())),
        block_address: address,
        sender_judgeable: triaged.trust != Trust::Forged && !triaged.reasons.iter().any(|r| matches!(r, Reason::Impersonation { .. })),
        others: people.len() > 1,
        unread,
        flagged,
        role: None,
        protection: pgp.map(|p| protection_line(&p, tr)),
    })
}

/// What OpenPGP found, said in a line next to the sender.
#[derive(Debug, Serialize)]
pub struct ProtectionView {
    pub encrypted: bool,
    /// "Encrypted · Signed by Jane <jane@example.org>"; "No key of yours opens it."
    pub line: String,
    /// Everything checked out; else the line is said in warm colours, never red.
    pub fine: bool,
}

fn protection_line(p: &crate::pgp::PgpView, tr: &Translator) -> ProtectionView {
    let mut parts = Vec::new();
    if p.encrypted {
        parts.push(tr.text(if p.opened { "pgp-encrypted" } else { "pgp-not-opened" }, None));
    }
    let mut fine = !p.encrypted || p.opened;
    for signature in &p.signatures {
        let mut args = i18n::args();
        if signature.good {
            args.set("signer", signature.signer.clone());
            parts.push(tr.text("pgp-signed-by", Some(&args)));
        } else if signature.problem == "unknown key" {
            parts.push(tr.text("pgp-signed-unknown", None));
        } else {
            fine = false;
            parts.push(tr.text("pgp-signature-bad", None));
        }
    }
    ProtectionView { encrypted: p.encrypted, line: parts.join(" · "), fine }
}

/// An account in the mail page's left column.
#[derive(Debug, Serialize)]
pub struct MailAccountView {
    pub id: String,
    pub title: String,
    /// Something unread in its inbox: a small dot, never a number (docs/client.md, rule 1).
    pub unread: bool,
    /// A work address in quiet time: folded, without dots, there if you look for it.
    pub resting: bool,
    /// Inbox, drafts, sent, archive, junk, trash: the ones the server has.
    pub folders: Vec<FolderEntry>,
    /// The other folders, folded under "More folders".
    pub more: Vec<FolderEntry>,
}

#[derive(Debug, Serialize)]
pub struct FolderEntry {
    /// The server's name, to open it.
    pub name: String,
    pub title: String,
    pub role: Role,
    pub icon: &'static str,
    pub unread: bool,
    /// A copy is kept here; else it stays on the server only.
    pub kept: bool,
}

/// The accounts and their folders, ranked as their mail is: above, average, below.
/// Views of other folders are not listed, but Gmail's "All Mail", its archive.
/// The mail page's accounts; `hours` what the hours now are for (None: no hours
/// set): an address whose area does not fit them rests (docs/areas.md).
pub fn mail_accounts(accounts: &[(Account, Vec<Folder>)], tr: &Translator, hours: Option<(crate::areas::Time, crate::areas::Week)>) -> Vec<MailAccountView> {
    let mut ranked: Vec<&(Account, Vec<Folder>)> = accounts.iter().collect();
    ranked.sort_by_key(|(account, _)| account.priority);
    ranked
        .into_iter()
        .map(|(account, folders)| {
            let root = account.maildir_path();
            // An address whose area is unsaid is work's.
            let area = account.area.as_deref().and_then(crate::areas::Area::parse).unwrap_or(crate::areas::Area::WORK);
            let resting = hours.is_some_and(|(time, week)| !crate::areas::in_view(area, time, week));
            // "All Mail" stands for the archive where there is none (Gmail).
            let has_archive = folders.iter().any(|f| f.role == Role::Archive);
            // Two folders for one purpose ("Archive" and "Archives") each keep their own name.
            let named = |f: &Folder| f.role != Role::Other && folders.iter().filter(|o| o.role == f.role).count() == 1;
            let kept = |f: &Folder| f.role == Role::Inbox || !account.skip_folders.contains(&f.name);
            let entry = |f: &Folder| FolderEntry {
                name: f.name.clone(),
                title: if f.role == Role::All && !has_archive {
                    tr.text(Role::Archive.message_id(), None)
                } else if named(f) {
                    tr.text(f.role.message_id(), None)
                } else {
                    folder_title(&f.display)
                },
                role: f.role,
                icon: f.role.icon(),
                // The junk and the trash never call for attention; work mail neither, in quiet time.
                unread: !resting && kept(f) && matches!(f.role, Role::Inbox | Role::Other) && maildir::has_unread(&root.join(&f.local)),
                kept: kept(f),
            };
            let listed: Vec<&Folder> = folders.iter().filter(|f| f.role != Role::All || !has_archive).collect();
            let folders: Vec<FolderEntry> = listed.iter().filter(|f| f.role != Role::Other).map(|f| entry(f)).collect();
            let more: Vec<FolderEntry> = listed.iter().filter(|f| f.role == Role::Other).map(|f| entry(f)).collect();
            MailAccountView {
                id: account.id.clone(),
                title: account.address.clone().unwrap_or_else(|| account.id.clone()),
                unread: folders.iter().any(|f| f.role == Role::Inbox && f.unread),
                resting,
                folders,
                more,
            }
        })
        .collect()
}

/// "[Gmail]/Important" → "Important": the provider's prefix says nothing.
fn folder_title(display: &str) -> String {
    ["[Gmail]/", "[Google Mail]/"].iter().fold(display.to_string(), |name, prefix| name.strip_prefix(prefix).map_or(name.clone(), str::to_string))
}

/// Days a folder shows before "Earlier" when the configuration says nothing (docs/client.md, rule 7).
pub const RECENT_DAYS: i64 = 14;

/// A folder's messages, newest first: the last two weeks, or everything kept on request.
#[derive(Debug, Serialize)]
pub struct FolderView {
    pub account: String,
    pub folder: String,
    pub title: String,
    pub role: Role,
    /// What is there, in words, said when you open it.
    pub sentence: String,
    pub items: Vec<MailItem>,
    /// Older messages are kept: "Earlier" shows them.
    pub earlier: bool,
}

#[derive(Debug, Serialize)]
pub struct MailItem {
    /// The file, to open the message.
    pub key: String,
    /// Who wrote; in Sent and Drafts, to whom.
    pub who: String,
    pub address: String,
    pub subject: String,
    pub date: String,
    pub unread: bool,
    pub flagged: bool,
    pub answered: bool,
    pub attachments: bool,
    /// "verified", "unverified", "forged"; "own" for what you sent.
    pub trust_level: &'static str,
    /// The same in your language; empty for what you sent.
    pub trust: String,
    pub checks: String,
    /// Shown by conversation: the conversation, the same for all its rows; empty otherwise.
    pub thread: String,
    /// On a conversation's first row: how many messages it holds.
    pub size: usize,
    /// A row inside a conversation, under its first; folded until it is opened.
    pub member: bool,
}

/// Messages by conversation: each one's newest message of the folder first, the
/// conversations by their last message (yours included), then the rest of each,
/// oldest first, your answers from Sent among them. A conversation shows when
/// its first row would.
fn conversations(cards: &[Card], sent: &[Card], shown: &dyn Fn(&Card) -> bool, item: &dyn Fn(&Card, bool) -> MailItem) -> Vec<MailItem> {
    let all: Vec<&Card> = cards.iter().chain(sent.iter()).collect();
    let groups = crate::threads::group(&all);
    let own = |i: usize| i >= cards.len();
    let mut by_group: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (i, g) in groups.iter().enumerate() {
        by_group.entry(*g).or_default().push(i);
    }
    // Conversations with a message of this folder, the newest activity first.
    let mut ordered: Vec<(i64, usize, Vec<usize>)> = by_group
        .into_iter()
        .filter_map(|(_, members)| {
            // Cards are newest first: the folder's first one is the head.
            let head = *members.iter().find(|&&i| !own(i))?;
            shown(all[head]).then(|| (members.iter().map(|&i| all[i].date.unwrap_or(0)).max().unwrap_or(0), head, members))
        })
        .collect();
    ordered.sort_by_key(|(last, head, _)| (std::cmp::Reverse(*last), *head));
    let mut out = Vec::new();
    for (_, head, mut members) in ordered {
        let thread = all[head].path.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
        let mut first = item(all[head], false);
        first.thread = thread.clone();
        first.size = members.len();
        first.unread = members.iter().filter(|&&i| !own(i)).any(|&i| item(all[i], false).unread);
        out.push(first);
        members.retain(|&i| i != head);
        members.sort_by_key(|&i| all[i].date.unwrap_or(0));
        for i in members {
            let mut row = item(all[i], own(i));
            row.thread = thread.clone();
            row.member = true;
            out.push(row);
        }
    }
    out
}

/// What a folder shows: `all` past the last two weeks; `query` searches every
/// message kept, by sender, recipient and subject. With `sent` (the account's
/// Sent folder), messages are shown by conversation, your answers among them.
#[allow(clippy::too_many_arguments)]
pub fn folder(account: &Account, folder: &Folder, cards: Vec<Card>, trusted_ids: &[String], all: bool, query: &str, tr: &Translator, now: i64, sent: Option<Vec<Card>>) -> FolderView {
    // The account's history, two weeks unless set; everything kept when it says so.
    let all = all || account.history_days().is_none();
    let since = now - account.history_days().unwrap_or(RECENT_DAYS) * 86_400;
    let outgoing = matches!(folder.role, Role::Sent | Role::Drafts);
    let query = query.trim().to_lowercase();
    let found = |c: &Card| {
        query.is_empty()
            || [c.sender(), c.subject.as_str(), c.from_address.as_deref().unwrap_or("")]
                .into_iter()
                .chain(c.to.iter().map(String::as_str))
                .any(|t| t.to_lowercase().contains(&query))
    };
    let mut cards: Vec<Card> = cards.into_iter().filter(found).collect();
    cards.sort_by_key(|c| std::cmp::Reverse(c.date.unwrap_or(0)));
    let recent = |c: &Card| c.date.unwrap_or(0) >= since;
    let older = cards.iter().filter(|c| !recent(c)).count();
    let shown = |c: &Card| all || !query.is_empty() || recent(c);
    let item = |card: &Card, own: bool| {
        let path = card.path.clone().unwrap_or_default();
        let flags = maildir::flags_of(&path);
        let (trust_level, trust, checks) = if outgoing || own {
            ("own", String::new(), String::new())
        } else {
            let results = trust::read_auth_results(&card.headers, trusted_ids);
            // As the Porch judges it: a signature counts for the sender's own domain only.
            let judged = trust::judge_sender(results.as_ref(), card.is_list, card.sender_domain()).0;
            let level = match judged {
                Trust::Verified => "verified",
                Trust::Unverified => "unverified",
                Trust::Forged => "forged",
            };
            (level, tr.trust(judged), self::checks(results.as_ref(), tr))
        };
        let who = if outgoing || own {
            let mut args = i18n::args();
            args.set("names", if card.to.is_empty() { tr.text("mail-nobody", None) } else { card.to.join(", ") });
            tr.text(if own { "mail-you-to" } else { "mail-to-whom" }, Some(&args))
        } else {
            card.sender().to_string()
        };
        MailItem {
            key: path.display().to_string(),
            who,
            address: card.from_address.clone().unwrap_or_default(),
            subject: if card.subject.trim().is_empty() { tr.text("mail-no-subject", None) } else { card.subject.clone() },
            date: date(tr, card.date),
            // In Sent, Drafts, Junk and Trash, nothing waits to be read.
            unread: !own && matches!(folder.role, Role::Inbox | Role::Other | Role::Archive) && !flags.contains('S'),
            flagged: flags.contains('F'),
            answered: flags.contains('R'),
            attachments: !card.attachments.is_empty(),
            trust_level,
            trust,
            checks,
            thread: String::new(),
            size: 1,
            member: false,
        }
    };
    let items: Vec<MailItem> = match sent {
        None => cards.iter().filter(|c| shown(c)).map(|c| item(c, false)).collect(),
        Some(sent) => conversations(&cards, &sent, &shown, &item),
    };
    let unread = items.iter().filter(|i| i.unread).count();
    let sentence = if items.is_empty() && !query.is_empty() {
        let mut args = i18n::args();
        args.set("query", query.clone());
        tr.text("mail-not-found", Some(&args))
    } else if items.is_empty() && older > 0 {
        tr.text("mail-nothing-recent", None)
    } else if items.is_empty() {
        tr.text("mail-empty", None)
    } else if unread > 0 {
        tr.text("mail-unread", Some(&tr.counted(unread)))
    } else {
        String::new()
    };
    FolderView {
        account: account.id.clone(),
        folder: folder.name.clone(),
        title: match folder.role {
            Role::Other => folder_title(&folder.display),
            Role::All => tr.text(Role::Archive.message_id(), None),
            role => tr.text(role.message_id(), None),
        },
        role: folder.role,
        sentence,
        items,
        earlier: !all && query.is_empty() && older > 0,
    }
}

fn attachment_kind(mime: &str) -> &'static str {
    let mime = mime.to_ascii_lowercase();
    match mime.as_str() {
        "application/pdf" => "pdf",
        "text/calendar" | "application/ics" => "calendar",
        m if m.starts_with("image/") => "image",
        m if m.starts_with("text/") => "text",
        m if m.contains("zip") || m.contains("compressed") || m.contains("x-tar") || m.contains("x-7z") => "archive",
        m if m.contains("word") || m.contains("officedocument") || m.contains("opendocument") || m.contains("excel") || m.contains("powerpoint") || m.contains("rtf") => "document",
        _ => "other",
    }
}

/// "812 B", "202 KB", "1.2 MB", in your language's units and decimals.
pub fn size(tr: &Translator, bytes: usize) -> String {
    let mut args = i18n::args();
    let (id, value) = match bytes {
        b if b < 1024 => ("size-b", b.to_string()),
        b if b < 1024 * 1024 => ("size-kb", (b / 1024).to_string()),
        b => ("size-mb", tr.decimal(b as f32 / (1024.0 * 1024.0))),
    };
    args.set("n", value);
    tr.text(id, Some(&args))
}

/// Days the agenda shows by default: today, then the next two weeks (docs/client.md, rule 9).
pub const AGENDA_DAYS: i64 = 15;

/// What comes, day by day.
#[derive(Debug, Serialize)]
pub struct AgendaView {
    pub days: Vec<AgendaDay>,
    /// "Nothing planned in the next two weeks.", when so.
    pub sentence: String,
    /// The first day shown, "2026-10-03", to move week by week or month by month.
    pub from: String,
    /// Whether an event can be added: a calendar that takes it exists.
    pub can_add: bool,
}

#[derive(Debug, Serialize)]
pub struct AgendaDay {
    /// "Today", "Tomorrow", "Monday 5 October".
    pub title: String,
    /// "2026-10-05".
    pub date: String,
    pub today: bool,
    /// The day's midnight, Unix seconds, to place what happens in it.
    pub start: i64,
    pub events: Vec<AgendaEvent>,
    /// Its hours given to work, admin or free time, in minutes from midnight; -1 without (`set_hours`).
    pub hours_from: i32,
    pub hours_to: i32,
}

#[derive(Debug, Serialize)]
pub struct AgendaEvent {
    pub key: String,
    pub uid: String,
    /// When this occurrence starts, Unix seconds: to leave it out of a repeating event.
    pub start: i64,
    /// "09:00 – 10:30", "All day", "Until 12:00".
    pub when: String,
    pub summary: String,
    pub location: String,
    pub notes: String,
    pub calendar: String,
    pub color: Option<String>,
    pub recurring: bool,
    pub cancelled: bool,
    pub tentative: bool,
    pub read_only: bool,
    pub organizer: String,
    pub attendees: Vec<crate::agenda::Attendee>,
    pub all_day: bool,
    /// Where it sits in this day's hours, in minutes from midnight: for the week's planning.
    pub from_minute: i32,
    pub to_minute: i32,
    /// Side by side with what overlaps it: its column among `columns`.
    pub column: u32,
    pub columns: u32,
}

/// Places a day's timed events side by side where they overlap
/// (`dayview::side_by_side`); one shorter than a quarter of an hour takes a
/// quarter of an hour, as it is drawn.
fn lay_out(events: &mut [AgendaEvent]) {
    let timed: Vec<usize> = (0..events.len()).filter(|&i| !events[i].all_day).collect();
    let spans: Vec<(i64, i64)> = timed.iter().map(|&i| (i64::from(events[i].from_minute), i64::from(events[i].to_minute.max(events[i].from_minute + 15)))).collect();
    for (&i, (column, columns)) in timed.iter().zip(crate::dayview::side_by_side(&spans)) {
        events[i].column = column;
        events[i].columns = columns;
    }
}

/// Each day's hours given to something, work, your admin or free time, in
/// minutes from its midnight (docs/areas.md): the planning shows those, and
/// the events outside them, rather than the night.
pub fn set_hours(view: &mut AgendaView, windows: &[crate::window::AdminWindow], zone: &TimeZone) {
    for day in &mut view.days {
        let Ok(date) = day.date.parse::<jiff::civil::Date>() else { continue };
        if let Some((opening, closing)) = crate::window::hours_on(windows, date, zone) {
            let minute = |z: &jiff::Zoned| i32::try_from((z.timestamp().as_second() - day.start) / 60).unwrap_or(0).clamp(0, 1440);
            day.hours_from = minute(&opening);
            day.hours_to = minute(&closing);
        }
    }
}

/// The agenda from `from` for `days` days: each day with what it holds; an
/// event over several days shows on each of them.
pub fn agenda(occurrences: &[crate::agenda::Occurrence], from: jiff::civil::Date, days: i64, tr: &Translator, zone: &TimeZone) -> AgendaView {
    let today = Timestamp::now().to_zoned(zone.clone()).date();
    let mut out = Vec::new();
    for offset in 0..days {
        let Ok(date) = from.checked_add(jiff::Span::new().days(offset)) else { break };
        let (Ok(start), Ok(end)) = (date.to_zoned(zone.clone()), date.tomorrow().and_then(|d| d.to_zoned(zone.clone()))) else { continue };
        let (start, end) = (start.timestamp().as_second(), end.timestamp().as_second());
        let mut events: Vec<AgendaEvent> = occurrences
            .iter()
            .filter(|o| o.start < end && o.end > start && !(o.end == start && o.start < start))
            .map(|o| AgendaEvent {
                key: o.key.clone(),
                uid: o.uid.clone(),
                start: o.start,
                when: when(o, start, end, tr, zone),
                summary: if o.summary.is_empty() { tr.text("agenda-untitled", None) } else { o.summary.clone() },
                location: o.location.clone(),
                notes: o.notes.clone(),
                calendar: o.calendar.clone(),
                color: o.color.clone(),
                recurring: o.recurring,
                cancelled: o.cancelled,
                tentative: o.tentative,
                read_only: o.read_only,
                organizer: o.organizer.clone(),
                attendees: o.attendees.clone(),
                all_day: o.all_day || (o.start <= start && o.end >= end),
                from_minute: i32::try_from((o.start.max(start) - start) / 60).unwrap_or(0),
                to_minute: i32::try_from((o.end.min(end) - start) / 60).unwrap_or(1440),
                column: 0,
                columns: 1,
            })
            .collect();
        lay_out(&mut events);
        let title = if date == today {
            tr.text("agenda-today", None)
        } else if Some(date) == today.tomorrow().ok() {
            tr.text("agenda-tomorrow", None)
        } else {
            tr.day(date)
        };
        out.push(AgendaDay { title, date: date.to_string(), today: date == today, start, events, hours_from: -1, hours_to: -1 });
    }
    let empty = out.iter().all(|d| d.events.is_empty());
    let can_add = crate::agenda::default_calendar().is_some();
    let sentence = match (empty, can_add || !crate::vdir::collections(crate::vdir::Kind::Calendars).is_empty()) {
        (true, false) => tr.text("agenda-no-calendar", None),
        (true, true) => tr.text("agenda-nothing", None),
        _ => String::new(),
    };
    AgendaView { sentence, days: out, from: from.to_string(), can_add }
}

/// An occurrence's time on one day: its hours, "All day", or how it continues.
fn when(o: &crate::agenda::Occurrence, day_start: i64, day_end: i64, tr: &Translator, zone: &TimeZone) -> String {
    let hour = |t: i64| Timestamp::from_second(t).map(|t| t.to_zoned(zone.clone()).strftime("%H:%M").to_string()).unwrap_or_default();
    let starts_today = o.start >= day_start;
    let ends_today = o.end <= day_end;
    if o.all_day || (!starts_today && !ends_today) {
        return tr.text("agenda-all-day", None);
    }
    let mut args = i18n::args();
    match (starts_today, ends_today) {
        (true, true) if o.end == o.start => hour(o.start),
        (true, true) => format!("{} – {}", hour(o.start), hour(o.end)),
        (true, false) => {
            args.set("time", hour(o.start));
            tr.text("agenda-from", Some(&args))
        }
        (false, _) => {
            args.set("time", hour(o.end));
            tr.text("agenda-until", Some(&args))
        }
    }
}

/// The address book: names first, details when one is opened (docs/client.md, rule 10).
#[derive(Debug, Serialize)]
pub struct ContactsView {
    pub contacts: Vec<ContactRow>,
    /// "No contact yet.", "Nothing matches “x”.", when so.
    pub sentence: String,
    /// Whether a contact can be added: an address book that takes it exists.
    pub can_add: bool,
}

#[derive(Debug, Serialize)]
pub struct ContactRow {
    pub key: String,
    /// Its address for links: "sioul:contact/<UID>".
    pub uri: String,
    pub name: String,
    /// The first e-mail address, to write to them.
    pub email: String,
    /// The first address, else number, else organisation: one line, never more.
    pub detail: String,
    /// Its picture, as the card gives it (`Contact::photo`).
    pub photo: String,
}

/// The contacts matching `query`, by name.
pub fn contacts(all: &[crate::contacts::Contact], query: &str, tr: &Translator) -> ContactsView {
    let found = crate::contacts::search(all, query);
    let sentence = if all.is_empty() && crate::vdir::collections(crate::vdir::Kind::Contacts).is_empty() {
        tr.text("contacts-no-book", None)
    } else if all.is_empty() {
        tr.text("contacts-none", None)
    } else if found.is_empty() {
        let mut args = i18n::args();
        args.set("query", query.trim().to_string());
        tr.text("mail-not-found", Some(&args))
    } else {
        String::new()
    };
    ContactsView {
        contacts: found
            .into_iter()
            .map(|c| ContactRow {
                key: c.key.clone(),
                uri: crate::links::contact_uri(&c.uid),
                name: c.name.clone(),
                email: c.emails.first().map(|e| e.value.clone()).unwrap_or_default(),
                detail: c.emails.first().map(|e| e.value.clone()).or_else(|| c.phones.first().map(|p| p.value.clone())).unwrap_or_else(|| c.org.clone()),
                photo: c.photo.clone(),
            })
            .collect(),
        sentence,
        can_add: crate::contacts::default_book().is_some(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlapping_events_sit_side_by_side() {
        let event = |from: i32, to: i32| AgendaEvent {
            key: String::new(),
            uid: String::new(),
            start: 0,
            when: String::new(),
            summary: String::new(),
            location: String::new(),
            notes: String::new(),
            calendar: String::new(),
            color: None,
            recurring: false,
            cancelled: false,
            tentative: false,
            read_only: false,
            organizer: String::new(),
            attendees: Vec::new(),
            all_day: false,
            from_minute: from,
            to_minute: to,
            column: 9,
            columns: 9,
        };
        // 9:00–10:00 and 9:30–11:00 overlap; 10:00–10:30 takes the first column again; 14:00 is alone.
        let mut day = vec![event(540, 600), event(570, 660), event(600, 630), event(840, 900)];
        lay_out(&mut day);
        let placed: Vec<(u32, u32)> = day.iter().map(|e| (e.column, e.columns)).collect();
        assert_eq!(placed, [(0, 2), (1, 2), (0, 2), (0, 1)]);
    }

    #[test]
    fn a_folder_shows_two_weeks_and_says_what_is_unread() {
        let now = 1_790_000_000;
        let card = |subject: &str, days_ago: i64, file: &str| {
            let raw = format!("From: Jane <jane@example.org>\r\nTo: Me <me@example.net>\r\nSubject: {subject}\r\n\r\nHi.\r\n");
            let mut card = Card::from_bytes(raw.as_bytes()).unwrap();
            card.date = Some(now - days_ago * 86_400);
            card.path = Some(std::path::PathBuf::from(file));
            card
        };
        let cards = vec![card("Lease", 1, "new/1.U1-1.sioul"), card("Old", 40, "cur/2.U1-2.sioul:2,S"), card("Read", 2, "cur/3.U1-3.sioul:2,FS")];
        let account = Account::imap("me", "me@example.net", "imap.example.net", 993, crate::config::Security::Tls, None);
        let inbox = crate::folders::folder("INBOX", Some("."), None);
        let tr = Translator::new("en");
        let view = folder(&account, &inbox, cards.clone(), &[], false, "", &tr, now, None);
        assert_eq!(view.items.iter().map(|i| i.subject.as_str()).collect::<Vec<_>>(), ["Lease", "Read"]);
        assert!(view.earlier && view.items[0].unread && view.items[1].flagged && !view.items[1].unread);
        assert_eq!(view.sentence, "One message you have not read.");
        let everything = folder(&account, &inbox, cards.clone(), &[], true, "", &tr, now, None);
        assert_eq!(everything.items.len(), 3);
        assert!(!everything.earlier);
        let searched = folder(&account, &inbox, cards.clone(), &[], false, "old", &tr, now, None);
        assert_eq!(searched.items.len(), 1);
        let sent = crate::folders::folder("Sent", Some("."), Some(Role::Sent));
        let view = folder(&account, &sent, cards, &[], false, "", &tr, now, None);
        assert_eq!(view.items[0].who, "To Me");
        assert!(view.items.iter().all(|i| !i.unread && i.trust_level == "own"));
    }

    #[test]
    fn conversations_bring_your_answers() {
        let now = 1_790_000_000;
        let card = |raw: &str, hours_ago: i64, file: &str| {
            let mut card = Card::from_bytes(raw.replace('\n', "\r\n").as_bytes()).unwrap();
            card.date = Some(now - hours_ago * 3600);
            card.path = Some(std::path::PathBuf::from(file));
            card
        };
        let question = card("From: Jane <jane@example.org>\nTo: me@example.net\nSubject: Lease\nMessage-ID: <q@example.org>\n\nWhen?\n", 30, "new/1.U1-1.sioul");
        let thanks = card("From: Jane <jane@example.org>\nTo: me@example.net\nSubject: Re: Lease\nMessage-ID: <t@example.org>\nReferences: <q@example.org> <a@example.net>\n\nThanks.\n", 2, "new/2.U1-2.sioul");
        let other = card("From: Paul <paul@example.com>\nTo: me@example.net\nSubject: Lunch\nMessage-ID: <l@example.com>\n\nNoon?\n", 5, "cur/3.U1-3.sioul:2,S");
        let answer = card("From: Me <me@example.net>\nTo: Jane <jane@example.org>\nSubject: Re: Lease\nMessage-ID: <a@example.net>\nIn-Reply-To: <q@example.org>\n\nMonday.\n", 20, "Sent/cur/4.U1-4.sioul:2,S");
        let unrelated = card("From: Me <me@example.net>\nTo: Bob <bob@example.com>\nSubject: Hi\nMessage-ID: <h@example.net>\n\nHi.\n", 1, "Sent/cur/5.U1-5.sioul:2,S");
        let account = Account::imap("me", "me@example.net", "imap.example.net", 993, crate::config::Security::Tls, None);
        let inbox = crate::folders::folder("INBOX", Some("."), None);
        let tr = Translator::new("en");
        let view = folder(&account, &inbox, vec![question, thanks, other], &[], false, "", &tr, now, Some(vec![answer, unrelated]));
        let rows: Vec<(&str, usize, bool)> = view.items.iter().map(|i| (i.subject.as_str(), i.size, i.member)).collect();
        // The lease first (its last message is the newest), with its question and your answer under it, oldest first.
        assert_eq!(rows, [("Re: Lease", 3, false), ("Lease", 1, true), ("Re: Lease", 1, true), ("Lunch", 1, false)]);
        assert_eq!(view.items[2].who, "You, to Jane");
        assert_eq!(view.items[2].trust_level, "own");
        assert!(view.items[0].unread, "a conversation is unread while one of its messages is");
        assert!(view.items.iter().all(|i| !i.subject.starts_with("Hi")), "your other mail stays in Sent");
    }

    #[test]
    fn the_shield_says_each_check() {
        let results = AuthResults {
            authserv_id: "sioul-0123.invalid".into(),
            dkim: Some(Outcome::Pass),
            dkim_domain: Some("shop.example".into()),
            spf: Some(Outcome::SoftFail),
            spf_domain: Some("mail.shop.example".into()),
            dmarc: Some(Outcome::Pass),
            dmarc_domain: Some("shop.example".into()),
            dmarc_policy: Some("reject".into()),
            iprev: Some(Outcome::Pass),
            ..AuthResults::default()
        };
        assert_eq!(
            checks(Some(&results), &Translator::new("en")),
            "Checked by Sioul when the message arrived:\nDKIM: valid (shop.example)\nSPF: doubtful (mail.shop.example)\n\
             DMARC: valid (shop.example, policy reject)\nReverse DNS: valid"
        );
        let provider = AuthResults { authserv_id: "mx.example.net".into(), dkim: Some(Outcome::Fail), ..AuthResults::default() };
        assert_eq!(checks(Some(&provider), &Translator::new("fr")), "Vérifié par votre fournisseur (mx.example.net)\u{202f}:\nDKIM\u{202f}: échoue");
        assert_eq!(checks(None, &Translator::new("en")), "No check recorded: nothing proves who sent it.");
    }
}
