// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! A rule in words (docs/rules.md, "One sentence"): the one place that makes
//! every sentence a rule shows (the list, the editor's summary, the status
//! line, the history, the readers of existing routes and filters).
//!
//! A first version: plain sentences from the declarations' names, which
//! the view's builder replaces with the sentences of `rule-*` messages.

use crate::config::Config;
use crate::i18n::Translator;
use crate::projects::ProjectStore;
use crate::wiring::acts::Act;
use crate::wiring::declare;
use crate::wiring::store::{Param, Rule, Rules};
use std::collections::BTreeMap;

/// The names a sentence gives things by their ids.
#[derive(Debug, Clone, Default)]
pub struct Names {
    /// Projects' titles, by id.
    pub projects: BTreeMap<String, String>,
    /// Topics' names, by id.
    pub topics: BTreeMap<String, String>,
    /// Models' names, by the id a rule gives them ("<project>/<model>" for a project's).
    pub models: BTreeMap<String, String>,
    /// Mail accounts' addresses, by id.
    pub accounts: BTreeMap<String, String>,
    /// Task lists' and calendars' names, by "account/id".
    pub lists: BTreeMap<String, String>,
    /// Contacts' names, by UID.
    pub contacts: BTreeMap<String, String>,
    /// Messages' subjects, by address (`mid:…`): filled by whoever has them
    /// (the engine for what it just did, the history from the mail index).
    pub subjects: BTreeMap<String, String>,
}

impl Names {
    /// The names of the things a rule may name, as this device has them.
    pub fn load(config: &Config, store: Option<&ProjectStore>, rules: &Rules) -> Names {
        let mut names = Names::default();
        for project in store.map(|s| s.projects.as_slice()).unwrap_or_default() {
            names.projects.insert(project.id.clone(), project.title.clone());
        }
        for topic in &config.topics {
            names.topics.insert(topic.id.clone(), topic.name.clone());
        }
        for model in crate::wiring::store::models(rules, store) {
            names.models.insert(model.id.clone(), model.model.name.clone());
        }
        for account in &config.accounts {
            names.accounts.insert(account.id.clone(), account.address.clone().unwrap_or_else(|| account.id.clone()));
        }
        for list in crate::tasks::lists() {
            names.lists.insert(format!("{}/{}", list.account, list.id), list.name.clone());
        }
        names
    }
}

/// The rule in one sentence.
pub fn sentence(rule: &Rule, names: &Names, tr: &Translator) -> String {
    let conditions: Vec<String> = rule.when.conditions.iter().map(|c| format!("{} {} {}", declare::field_name(tr, &rule.when.kind, &c.field), declare::test_name(tr, &c.test), c.value.join(", "))).collect();
    let with: Vec<String> = rule
        .then
        .with
        .iter()
        .map(|(key, value)| {
            let said = match value {
                Param::From { from } => declare::field_name(tr, &rule.when.kind, from),
                other => {
                    let text = other.text().unwrap_or_default();
                    names.projects.get(&text).or_else(|| names.topics.get(&text)).or_else(|| names.models.get(&text)).cloned().unwrap_or(text)
                }
            };
            format!("{key}: {said}")
        })
        .collect();
    let action = tr.text(&format!("rules-action-{}-{}", rule.then.kind, rule.then.action), None);
    format!("{}, {}: {} ({}).", when_heading(rule, tr), conditions.join(", "), action, with.join(", "))
}

/// What the rules sharing it are shown under: "When a message arrives".
pub fn when_heading(rule: &Rule, tr: &Translator) -> String {
    format!("{} · {}", tr.text(&format!("rules-type-{}", rule.when.kind), None), tr.text(&format!("rules-event-{}-{}", rule.when.kind, rule.when.event), None))
}

/// One act of a rule, as the status line and its history say it.
pub fn act_line(act: &Act, names: &Names, _tr: &Translator) -> String {
    let subject = names.subjects.get(&act.object).cloned().unwrap_or_else(|| act.object.clone());
    let result = names.projects.get(&crate::links::id_of(&act.result)).or_else(|| names.topics.get(&act.result)).cloned().unwrap_or_else(|| act.result.clone());
    format!("{} · {} · {}", act.act, subject, result)
}
