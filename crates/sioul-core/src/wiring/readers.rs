// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What exists today, read as rules (docs/rules.md, "What exists, read"):
//! each project's routes and the routes that make tasks with their models,
//! the budgets' mail rules, mail's filters, a topic's sender rules, each a
//! rule of the same shape as a stored one, and where it is edited today.
//! Read-only in phase 1, nothing migrated: the existing mechanisms keep
//! running exactly as they do, and the engine never runs a rule read here
//! (`ReadRule` is never a `store::Rule` of the list).

use crate::projects::{ProjectStore, Route};
use crate::rules::{Act as FilterAct, Field, Filter};
use crate::topics::labels;
use crate::wiring::store::{Condition, Join, Param, Rule, Then, When};
use serde::Serialize;
use std::collections::BTreeMap;

/// Where a rule read here comes from, and where it is edited today.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Source {
    /// "project-route", "route-task", "budget-mail-rule", "mail-filter", "topic-sender".
    pub kind: &'static str,
    /// The place it is edited: "project" (the project's page, "Mail that
    /// comes here by itself"), "budgets" (by hand in the budgets' file:
    /// none has an editor), "mail-filters" (Mail ▸ ⚙ ▸ Filters), "topics"
    /// (Mail ▸ ⚙ ▸ Topics, or "Always … for this sender" on a message).
    pub edited: &'static str,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub project: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub route: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub budget: String,
    /// A mail filter: its place in the list, its name; whether the filters
    /// after it are asked no more when it matched (`stop`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<usize>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub stop: bool,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub topic: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub address: String,
    /// A budget's mail rule that adds its line without asking (`automatic`,
    /// which nothing reads today: said as it is written).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub automatic: bool,
}

/// A rule read from what exists.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReadRule {
    pub rule: Rule,
    pub source: Source,
    /// Where it runs today, a message id ("rules-runs-route"…).
    pub runs: &'static str,
}

/// Everything that exists today, as rules: the projects' routes (and the
/// tasks they make), the budgets' mail rules, the mail filters, the topics'
/// sender rules.
pub fn read_all(projects: Option<&ProjectStore>, ledger: Option<&crate::budget::Ledger>, filters: &[Filter], said: &labels::Said) -> Vec<ReadRule> {
    let mut out = Vec::new();
    if let Some(store) = projects {
        out.extend(of_projects(store));
    }
    if let Some(ledger) = ledger {
        out.extend(of_mail_rules(ledger));
    }
    out.extend(of_filters(filters));
    out.extend(of_senders(said));
    out
}

fn condition(field: &str, test: &str, value: &[String]) -> Condition {
    Condition { field: field.into(), test: test.into(), value: value.iter().map(|v| v.trim().to_string()).filter(|v| !v.is_empty()).collect(), until: String::new() }
}

/// A route's conditions: its sender's domains and addresses, words of its
/// subject and text, words in an attachment's name; all of them.
fn when_of(route: &Route) -> When {
    let mut conditions = Vec::new();
    for (field, test, list) in [("sender-domain", "is", &route.from_domains), ("sender", "is", &route.from_addresses), ("subject", "has-words", &route.subject_contains), ("text", "has-words", &route.text_contains), ("attachment", "contains", &route.attachment_contains)] {
        if list.iter().any(|v| !v.trim().is_empty()) {
            conditions.push(condition(field, test, list));
        }
    }
    When { kind: "message".into(), event: "arrives".into(), join: Join::All, conditions }
}

fn with(pairs: &[(&str, Param)]) -> BTreeMap<String, Param> {
    pairs.iter().map(|(k, v)| ((*k).to_string(), v.clone())).collect()
}

fn read(id: String, when: When, then: Then, on: bool) -> Rule {
    Rule { id, on, when, then, ..Rule::default() }
}

/// Each project's routes: "a message arrives … → tie it to this project";
/// a route that makes tasks, also "→ make a task from its model".
pub fn of_projects(store: &ProjectStore) -> Vec<ReadRule> {
    let mut out = Vec::new();
    for project in &store.projects {
        for (at, route) in project.routes.iter().enumerate() {
            let when = when_of(route);
            if when.conditions.is_empty() {
                continue;
            }
            let key = if route.id.is_empty() { at.to_string() } else { route.id.clone() };
            let source = Source { kind: "project-route", edited: "project", project: project.id.clone(), route: route.id.clone(), ..Source::default() };
            let tie = Then { kind: "message".into(), action: "tie-to-project".into(), with: with(&[("project", Param::Text(project.id.clone()))]) };
            out.push(ReadRule { rule: read(format!("read:project/{}/route/{key}", project.id), when.clone(), tie, true), source: source.clone(), runs: "rules-runs-route" });
            if let Some(model) = project.model(&route.makes) {
                let make = Then {
                    kind: "task".into(),
                    action: "make".into(),
                    with: with(&[("model", Param::Text(format!("{}/{}", project.id, model.id))), ("project", Param::Text(project.id.clone())), ("title", Param::From { from: "subject".into() })]),
                };
                out.push(ReadRule { rule: read(format!("read:project/{}/route/{key}/task", project.id), when, make, true), source: Source { kind: "route-task", ..source }, runs: "rules-runs-route-task" });
            }
        }
    }
    out
}

/// The budgets' mail rules: "a message arrives … → plan a movement in this
/// budget", its direction, its amount (the message's when none is fixed).
pub fn of_mail_rules(ledger: &crate::budget::Ledger) -> Vec<ReadRule> {
    ledger
        .mail_rules
        .iter()
        .enumerate()
        .filter_map(|(at, rule)| {
            let when = when_of(&rule.route);
            if when.conditions.is_empty() {
                return None;
            }
            let direction = match rule.direction {
                crate::budget::Direction::Credit => "credit",
                crate::budget::Direction::Debit => "debit",
            };
            let mut pairs = vec![("budget", Param::Text(rule.budget.clone())), ("direction", Param::Text(direction.into()))];
            pairs.push(("amount", rule.amount.map_or_else(|| Param::From { from: "amount".into() }, |m| Param::Text(format!("{:.2}", m.cents() as f64 / 100.0)))));
            if let Some(label) = rule.label.as_ref().filter(|l| !l.trim().is_empty()) {
                pairs.push(("label", Param::Text(label.clone())));
            }
            if let Some(preset) = rule.preset.as_ref().filter(|p| !p.trim().is_empty()) {
                pairs.push(("preset", Param::Text(preset.clone())));
            }
            let then = Then { kind: "movement".into(), action: "plan".into(), with: with(&pairs) };
            Some(ReadRule { rule: read(format!("read:budget/mail-rule/{at}"), when, then, true), source: Source { kind: "budget-mail-rule", edited: "budgets", budget: rule.budget.clone(), automatic: rule.automatic, ..Source::default() }, runs: "rules-runs-mail-rule" })
        })
        .collect()
}

/// A filter's field as a rule's.
fn field_of(field: Field) -> &'static str {
    match field {
        Field::From => "from",
        Field::To => "to",
        Field::Cc => "cc",
        Field::ReplyTo => "reply-to",
        Field::Subject => "subject",
        Field::Body => "text",
        Field::Attachment => "attachment",
        Field::AttachmentType => "attachment-type",
        Field::Sender => "circle",
        Field::List => "list",
        Field::Date => "date",
        Field::Weekday => "weekday",
        Field::Hour => "hour",
        Field::Size => "size",
        Field::Anywhere => "anywhere",
        Field::Account => "account",
        Field::Topic => "topic",
        Field::Folder | Field::Mark | Field::Unknown => "unknown",
    }
}

/// Mail's filters, in their order: one rule per action of a filter (one
/// action per rule), each "a message arrives … → move it, mark it…", its
/// accounts a condition; off when the filter is.
pub fn of_filters(filters: &[Filter]) -> Vec<ReadRule> {
    let mut out = Vec::new();
    for (at, filter) in filters.iter().enumerate() {
        let mut conditions: Vec<Condition> = filter
            .conditions
            .iter()
            .map(|c| {
                let values: Vec<String> = if c.field == Field::Weekday { c.value.split(',').map(|d| d.trim().to_string()).collect() } else { vec![c.value.clone()] };
                Condition { field: field_of(c.field).into(), test: c.test.id().into(), value: values.into_iter().filter(|v| !v.trim().is_empty()).collect(), until: c.until.clone() }
            })
            .collect();
        if !filter.accounts.is_empty() {
            conditions.push(condition("account", "is", &filter.accounts));
        }
        let join = if filter.join == crate::rules::Join::Any { Join::Any } else { Join::All };
        let when = When { kind: "message".into(), event: "arrives".into(), join, conditions };
        for (n, act) in filter.actions.iter().enumerate() {
            let (action, pairs): (&str, Vec<(&str, Param)>) = match act {
                FilterAct::Move { folder } => ("move-to-folder", vec![("folder", Param::Text(folder.clone()))]),
                FilterAct::Archive => ("archive", Vec::new()),
                FilterAct::Junk => ("junk", Vec::new()),
                FilterAct::Flag => ("flag", Vec::new()),
                FilterAct::Read => ("mark-read", Vec::new()),
                FilterAct::Trash => ("trash", Vec::new()),
                FilterAct::Keyword { name } => ("keyword", vec![("name", Param::Text(name.clone()))]),
                FilterAct::Unknown => ("unknown", Vec::new()),
            };
            let then = Then { kind: "message".into(), action: action.into(), with: with(&pairs) };
            out.push(ReadRule {
                rule: read(format!("read:mail-filter/{at}/{n}"), when.clone(), then, filter.enabled),
                source: Source { kind: "mail-filter", edited: "mail-filters", filter: Some(at), name: filter.name.clone(), stop: filter.stop, ..Source::default() },
                runs: "rules-runs-filter",
            });
        }
    }
    out
}

/// The topics' sender rules: "a message arrives from … → give it this
/// topic" (Always), "→ never this topic" (Never); proven senders only, as
/// the topics read them.
pub fn of_senders(said: &labels::Said) -> Vec<ReadRule> {
    let mut addresses: Vec<&String> = said.senders.keys().collect();
    addresses.sort();
    let mut out = Vec::new();
    for address in addresses {
        for (topic, rule) in &said.senders[address] {
            let action = match rule {
                labels::Rule::Always => "give-topic",
                labels::Rule::Never => "never-topic",
                labels::Rule::Off => continue,
            };
            let when = When { kind: "message".into(), event: "arrives".into(), join: Join::All, conditions: vec![condition("sender", "is", std::slice::from_ref(address))] };
            let then = Then { kind: "message".into(), action: action.into(), with: with(&[("topic", Param::Text(topic.clone()))]) };
            out.push(ReadRule { rule: read(format!("read:topic/{topic}/sender/{address}"), when, then, true), source: Source { kind: "topic-sender", edited: "topics", topic: topic.clone(), address: address.clone(), ..Source::default() }, runs: "rules-runs-topic" });
        }
    }
    out
}
