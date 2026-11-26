// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! A rule in words (docs/rules.md, "One sentence"): the one place that makes
//! every sentence a rule shows. The list of Settings ▸ Rules, the editor's
//! summary, the status line, a rule's history and the readers of what exists
//! (projects' routes, budgets' mail rules, mail filters, topics' senders)
//! all say a rule through `sentence`, or through its two halves,
//! `when_part` and `then_part`, which it joins: the same words everywhere.
//!
//! A sentence is made of whole phrases, never of words glued by code, so
//! that each language keeps its own grammar (`rule-*` messages in
//! `locales/*/sioul.ftl`):
//! - what happens, one phrase per type and event: "When a message arrives"
//!   (`rule-heading-<type>-<event>`);
//! - each condition, one phrase per type and field, chosen by its test,
//!   its values as inserts: "from impots.example", "with “avis” in its
//!   subject" (`rule-if-<type>-<field>`), joined by "and" or "or";
//! - what a message is recognised as, when it is the condition, as the
//!   sentence's verb: "When a message from energy.example says a payment
//!   was refused" (`rule-says-<kind>`);
//! - what is done, one phrase per type and action, its parameters as
//!   inserts, an action on the object itself said with "it" ("tie it to the
//!   project Taxes 2025"), on another one by its name (`rule-do-<type>-<action>`);
//! - the two joined: "When …: tie it to the project Taxes 2025."
//!
//! What has no phrase of its own (a type, a field or an action a newer
//! Sioul declared) is said with the declarations' names, plainly: a
//! sentence is always made, never an id.

use crate::config::Config;
use crate::i18n::Translator;
use crate::projects::ProjectStore;
use crate::wiring::acts::Act;
use crate::wiring::declare::{self, Value};
use crate::wiring::store::{Condition, Join, Param, Rule, Rules};
use fluent_bundle::FluentArgs;
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
    /// Budgets' titles, by id.
    pub budgets: BTreeMap<String, String>,
    /// The persons your admin is done for, by id ("me" is said as "you").
    pub persons: BTreeMap<String, String>,
    /// Papers' titles, by id: what a rule kept, named in its history.
    pub papers: BTreeMap<String, String>,
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
        if let Some(ledger) = config.records_root().and_then(|root| crate::budget::Ledger::load(&root).ok()) {
            for budget in ledger.budgets {
                names.budgets.insert(budget.id, budget.title);
            }
        }
        for person in crate::persons::Persons::load(&crate::persons::Persons::default_path()).persons {
            names.persons.insert(person.id, person.name);
        }
        if let Some(wallet) = config.places().and_then(|places| crate::papers::Wallet::load_at(&places).ok()) {
            for paper in wallet.papers {
                names.papers.insert(paper.id, paper.title);
            }
        }
        names
    }
}

/// A message's words, or none when this language and English both lack it.
fn said(tr: &Translator, id: &str, args: Option<&FluentArgs>) -> Option<String> {
    let text = tr.text(id, args);
    (text != id).then_some(text)
}

/// Arguments from pairs.
fn args<'a>(pairs: &[(&'a str, String)]) -> FluentArgs<'a> {
    let mut out = FluentArgs::new();
    for (key, value) in pairs {
        out.set(*key, value.clone());
    }
    out
}

/// Typed words in quotes: “avis”, « avis ».
fn quoted(tr: &Translator, text: &str) -> String {
    tr.text("rule-quoted", Some(&args(&[("text", text.trim().to_string())])))
}

/// A folder by its purpose, as a person says it ("your junk folder"), else by its name, quoted.
fn folder(tr: &Translator, id: &str) -> String {
    said(tr, &format!("rule-folder-{}", id.trim().to_lowercase()), None).unwrap_or_else(|| tr.text("rule-folder-named", Some(&args(&[("name", quoted(tr, id))]))))
}

/// A pattern's inserts ({sender}, {month}…) in words, between ‹ ›: what the message puts there.
fn inserts(tr: &Translator, pattern: &str) -> String {
    let mut out = pattern.to_string();
    for insert in ["sender", "month", "year", "subject", "file"] {
        out = out.replace(&format!("{{{insert}}}"), &tr.text("rule-insert", Some(&args(&[("name", tr.text(&format!("rule-insert-{insert}"), None))]))));
    }
    out
}

/// Several phrases or values, the last joined by `last` ("and", "or"): "a, b or c".
fn joined(tr: &Translator, items: &[String], last: &str) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., end] => format!("{}{}{}", rest.join(&tr.text("rule-join-comma", None)), tr.text(last, None), end),
    }
}

/// The rule in one sentence: "When a message arrives from impots.example
/// with “avis” in its subject: tie it to the project Taxes 2025."
pub fn sentence(rule: &Rule, names: &Names, tr: &Translator) -> String {
    tr.text("rule-sentence", Some(&args(&[("when", when_part(rule, names, tr)), ("then", then_part(rule, names, tr))])))
}

/// What the rules sharing it are shown under: "When a message arrives".
pub fn when_heading(rule: &Rule, tr: &Translator) -> String {
    heading(&rule.when.kind, &rule.when.event, tr)
}

/// "When a message arrives", by a type and an event.
pub fn heading(kind: &str, event: &str, tr: &Translator) -> String {
    said(tr, &format!("rule-heading-{kind}-{event}"), None).unwrap_or_else(|| {
        let a = |id: String| said(tr, &id, None).unwrap_or_default();
        tr.text("rule-heading-any", Some(&args(&[("type", a(format!("rules-type-{kind}")).to_lowercase()), ("event", a(format!("rules-event-{kind}-{event}")))])))
    })
}

/// The fields that say who sent a message: before the verb when the kind is the verb.
const SENDER_FIELDS: [&str; 4] = ["sender", "sender-domain", "circle", "card"];

/// What happens and its conditions: "When a message arrives from
/// impots.example, with “avis” in its subject", or, with a recognised kind
/// as its verb, "When a message from energy.example says a payment was refused".
pub fn when_part(rule: &Rule, names: &Names, tr: &Translator) -> String {
    let when = &rule.when;
    let any = when.join == Join::Any;
    let phrases: Vec<String> = when.conditions.iter().map(|c| condition(rule, c, names, tr)).collect();
    // The kind Sioul recognises, said as the verb, when every value has a verb.
    let verb = (when.kind == "message" && when.event == "arrives" && !any)
        .then(|| when.conditions.iter().position(|c| c.field == "kind" && c.test == "is" && !c.value.is_empty()))
        .flatten()
        .and_then(|at| {
            let says: Option<Vec<String>> = when.conditions[at].value.iter().map(|v| said(tr, &format!("rule-says-{}", v.trim()), None)).collect();
            says.map(|says| (at, says))
        });
    if let Some((at, says)) = verb {
        let mut from = Vec::new();
        let mut rest = Vec::new();
        for (i, (c, phrase)) in when.conditions.iter().zip(&phrases).enumerate() {
            if i == at {
                continue;
            }
            if SENDER_FIELDS.contains(&c.field.as_str()) { from.push(phrase.clone()) } else { rest.push(phrase.clone()) }
        }
        let from = if from.is_empty() { String::new() } else { format!(" {}", joined(tr, &from, "rule-join-and")) };
        let rest = if rest.is_empty() { String::new() } else { format!("{}{}", tr.text("rule-join-comma", None), joined(tr, &rest, "rule-join-and")) };
        return tr.text("rule-when-message-says", Some(&args(&[("from", from), ("says", joined(tr, &says, "rule-join-or")), ("rest", rest)])));
    }
    let head = when_heading(rule, tr);
    if phrases.is_empty() {
        return head;
    }
    let list = if any { joined(tr, &phrases, "rule-join-or") } else { joined(tr, &phrases, "rule-join-and") };
    format!("{head} {list}")
}

/// What its value is, as the declarations say for this field and test.
fn value_of(rule: &Rule, c: &Condition) -> Value {
    declare::of(&rule.when.kind).and_then(|t| t.field(&c.field)).and_then(|f| f.test(&c.test)).map_or(Value::Text, |t| t.value)
}

/// One condition as a phrase: "from impots.example", "with “avis” in its subject".
fn condition(rule: &Rule, c: &Condition, names: &Names, tr: &Translator) -> String {
    let value = values(&c.field, value_of(rule, c), &c.value, names, tr, "rule-join-or");
    let until = values(&c.field, value_of(rule, c), std::slice::from_ref(&c.until), names, tr, "rule-join-or");
    let pairs = [("test", c.test.clone()), ("value", value.clone()), ("until", until.clone())];
    said(tr, &format!("rule-if-{}-{}", rule.when.kind, c.field), Some(&args(&pairs))).unwrap_or_else(|| {
        let field = declare::field_name(tr, &rule.when.kind, &c.field);
        let test = declare::test_name(tr, &c.test);
        tr.text("rule-if-any", Some(&args(&[("field", field), ("test", test), ("value", value)])))
    })
}

/// Values in words, any of them: quoted words, names for ids, days, dates.
fn values(field: &str, kind: Value, list: &[String], names: &Names, tr: &Translator, last: &str) -> String {
    let named = |map: &BTreeMap<String, String>, v: &str| map.get(v).cloned().unwrap_or_else(|| v.to_string());
    let words: Vec<String> = list
        .iter()
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
        .map(|v| match kind {
            Value::Text if field == "folder" => folder(tr, v),
            Value::Text | Value::Words => quoted(tr, v),
            Value::Choice if field == "circle" => said(tr, &format!("rule-circle-{v}"), None).unwrap_or_else(|| declare::choice_name(tr, field, v)),
            Value::Choice => declare::choice_name(tr, field, v),
            Value::Topic => named(&names.topics, v),
            Value::Project => named(&names.projects, v),
            Value::Contact => named(&names.contacts, v),
            Value::Account => named(&names.accounts, v),
            Value::List => named(&names.lists, v),
            Value::Model => quoted(tr, &named(&names.models, v)),
            Value::Days => crate::rules::DAYS.iter().position(|d| *d == v).map_or_else(|| v.to_string(), |i| tr.text(&format!("weekday-{}", i + 1), None)),
            Value::Date => v.parse::<jiff::civil::Date>().map_or_else(|_| v.to_string(), |d| tr.day_month(d)),
            Value::Level => said(tr, &format!("rules-level-{v}"), None).unwrap_or_else(|| v.to_string()),
            Value::PaperKind => quoted(tr, &said(tr, &format!("paper-kind-{v}"), None).unwrap_or_else(|| v.to_string())),
            Value::Person if v == crate::persons::ME => tr.text("rule-for-me", None),
            Value::Person => named(&names.persons, v),
            Value::Pattern => quoted(tr, &inserts(tr, v)),
            Value::FileType => said(tr, &format!("rule-attachments-{v}"), None).unwrap_or_else(|| declare::choice_name(tr, "attachment-type", v)),
            _ => v.to_string(),
        })
        .collect();
    joined(tr, &words, last)
}

/// What is done: "tie it to the project Taxes 2025", "make a task from the
/// model “Pay another way”, asked three business days after it came".
pub fn then_part(rule: &Rule, names: &Names, tr: &Translator) -> String {
    let then = &rule.then;
    if then.kind.is_empty() || then.action.is_empty() {
        return tr.text("rule-do-unchosen", None);
    }
    // Done to the object that triggered it: "it"; else to another, by its name.
    let same = if then.kind == rule.when.kind { "yes" } else { "no" };
    let param = |id: &str| then.with.get(id);
    let kind_of = |id: &str| declare::of(&then.kind).and_then(|t| t.action(&then.action)).and_then(|a| a.param(id)).map_or(Value::Text, |p| p.value);
    // A parameter in words: fixed, by its name; taken from the object, by the field's own words ("its subject").
    let word = |id: &str| -> String {
        match param(id) {
            None => String::new(),
            Some(Param::From { from }) => said(tr, &format!("rule-from-{}-{from}", rule.when.kind), None).unwrap_or_else(|| declare::field_name(tr, &rule.when.kind, from)),
            Some(fixed) => values(id, kind_of(id), &[fixed.text().unwrap_or_default()], names, tr, "rule-join-or"),
        }
    };
    let from = |id: &str| if param(id).and_then(Param::from).is_some() { "yes" } else { "no" };
    // Optional parts after the main phrase, each its own message: ", in the project Taxes".
    let part = |id: &str, message: &str| -> String {
        match param(id) {
            None => String::new(),
            Some(Param::Text(t)) if t.trim().is_empty() => String::new(),
            Some(p) => {
                let number = match p {
                    Param::Number(n) => *n,
                    Param::Text(t) => t.trim().parse().unwrap_or(0),
                    _ => 0,
                };
                let count = tr.count(usize::try_from(number).unwrap_or(0), false, false);
                tr.text(message, Some(&args(&[("value", word(id)), ("from", from(id).to_string()), ("n", number.to_string()), ("count", count)])))
            }
        }
    };
    // An action whose required details are not given (a preset Sioul runs its own way): its plain phrase.
    let missing = declare::of(&then.kind).and_then(|t| t.action(&then.action)).is_some_and(|a| a.params.iter().any(|p| p.required && param(p.id).is_none()));
    let id = format!("rule-do-{}-{}{}", then.kind, then.action, if missing { "-plain" } else { "" });
    let main = match (then.kind.as_str(), then.action.as_str()) {
        ("task", "make") => said(tr, &id, Some(&args(&[("model", word("model")), ("same", same.into())]))).map(|main| {
            format!("{main}{}{}{}", part("project", "rule-do-task-make-project"), part("title", "rule-do-task-make-title"), part("days", "rule-do-task-make-days"))
        }),
        ("none", "tell") => {
            let level = param("level").and_then(Param::text).unwrap_or_else(|| "now".into());
            let words = param("words").and_then(Param::text).filter(|w| !w.is_empty());
            // Quietly, nothing is told: its words are a notice's, never said.
            let id = if words.is_some() && level != "quiet" { "rule-do-none-tell-words" } else { "rule-do-none-tell" };
            said(tr, id, Some(&args(&[("level", level), ("words", words.map(|w| quoted(tr, &w)).unwrap_or_default())])))
        }
        // A message's attachments kept as papers: which ones as the phrase's insert, then its details.
        ("message", "keep-attachments") => {
            // Unsaid, PDFs: what the engine keeps (`engine::keep_attachments`).
            let which = param("type").and_then(Param::text).filter(|t| !t.is_empty()).unwrap_or_else(|| "pdf".into());
            let which = said(tr, &format!("rule-attachments-{which}"), None).unwrap_or_else(|| tr.text("rule-attachments-any", None));
            said(tr, &id, Some(&args(&[("which", which), ("same", same.into())]))).map(|main| {
                format!(
                    "{main}{}{}{}{}",
                    part("name", "rule-do-message-keep-attachments-name"),
                    part("kind", "rule-do-message-keep-attachments-kind"),
                    part("title", "rule-do-message-keep-attachments-title"),
                    part("for", "rule-do-message-keep-attachments-for")
                )
            })
        }
        ("movement", "plan") => {
            let direction = param("direction").and_then(Param::text).unwrap_or_else(|| "debit".into());
            let budget = param("budget").and_then(Param::text).map(|b| names.budgets.get(&b).cloned().unwrap_or(b)).unwrap_or_default();
            said(tr, &id, Some(&args(&[("direction", direction), ("budget", budget), ("same", same.into())]))).map(|main| format!("{main}{}{}", part("amount", "rule-do-movement-plan-amount"), part("label", "rule-do-movement-plan-label")))
        }
        _ => {
            // Each parameter in words by its id, and whether it is taken from the object ("<id>-from").
            let ids: Vec<String> = then.with.keys().cloned().collect();
            let words: Vec<(String, String)> = ids.iter().map(|k| (k.clone(), word(k))).collect();
            let froms: Vec<(String, String)> = ids.iter().map(|k| (format!("{k}-from"), from(k).to_string())).collect();
            let mut pairs: Vec<(&str, String)> = vec![("same", same.into())];
            pairs.extend(words.iter().chain(&froms).map(|(k, w)| (k.as_str(), w.clone())));
            said(tr, &id, Some(&args(&pairs)))
        }
    };
    main.unwrap_or_else(|| {
        // An action this Sioul has no phrase for: its name, and its parameters by their names.
        let name = said(tr, &format!("rules-action-{}-{}", then.kind, then.action), None).unwrap_or_else(|| then.action.clone());
        let given: Vec<String> = then.with.keys().map(|k| format!("{} {}", said(tr, &format!("rules-param-{k}"), None).unwrap_or_else(|| k.clone()), word(k))).collect();
        if given.is_empty() { name } else { tr.text("rule-do-any", Some(&args(&[("action", name), ("params", given.join(&tr.text("rule-join-comma", None)))]))) }
    })
}

/// The "when" that rules share, said once above them: "When a message arrives from Free:".
pub fn when_shared(rule: &Rule, names: &Names, tr: &Translator) -> String {
    tr.text("rule-when-shared", Some(&args(&[("when", when_part(rule, names, tr))])))
}

/// One act of a rule, as the status line and its history say it:
/// "“Avis d'impôt” tied to the project Taxes 2025."
pub fn act_line(act: &Act, names: &Names, tr: &Translator) -> String {
    // Its message by its subject, quoted; else "A message", the line starting with it.
    let subject = names.subjects.get(&act.object).filter(|s| !s.trim().is_empty()).map_or_else(|| tr.text("rule-a-message-first", None), |s| quoted(tr, s));
    let project = crate::links::id_of(&act.result);
    let project = names.projects.get(&project).cloned().unwrap_or(project);
    let topic = names.topics.get(&act.result).cloned().unwrap_or_else(|| act.result.clone());
    let wrote = if act.wrote { "yes" } else { "no" };
    // A paper by its title, so that two kept from one message read apart.
    let paper = names.papers.get(&crate::links::id_of(&act.result)).filter(|t| !t.trim().is_empty()).map_or_else(|| tr.text("rule-a-paper", None), |t| quoted(tr, t));
    let pairs = [("Subject", subject), ("project", project), ("topic", topic), ("paper", paper), ("wrote", wrote.to_string()), ("act", act.act.clone())];
    said(tr, &format!("rule-act-{}", act.act), Some(&args(&pairs))).unwrap_or_else(|| tr.text("rule-act-other", Some(&args(&pairs))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wiring::store::{Then, When};

    fn rule(join: Join, conditions: &[(&str, &str, &[&str])], then: (&str, &str), with: &[(&str, Param)]) -> Rule {
        Rule {
            id: "r".into(),
            when: When {
                kind: "message".into(),
                event: "arrives".into(),
                join,
                conditions: conditions.iter().map(|(field, test, value)| Condition { field: (*field).into(), test: (*test).into(), value: value.iter().map(|v| (*v).to_string()).collect(), until: String::new() }).collect(),
            },
            then: Then { kind: then.0.into(), action: then.1.into(), with: with.iter().map(|(k, v)| ((*k).to_string(), v.clone())).collect() },
            ..Rule::default()
        }
    }

    fn names() -> Names {
        let mut names = Names::default();
        names.projects.insert("taxes".into(), "Taxes 2025".into());
        names.topics.insert("bills".into(), "Bills".into());
        names.models.insert("pay".into(), "Pay another way".into());
        names.budgets.insert("home".into(), "Household".into());
        names.subjects.insert("mid:a@impots.example".into(), "Your notice".into());
        names
    }

    fn fixed(t: &str) -> Param {
        Param::Text(t.into())
    }

    /// Each kind of rule the list shows, in both languages: whole phrases,
    /// inserts in their place, agreements and quotes as each language has them.
    #[test]
    fn every_kind_of_rule_reads_as_a_sentence() {
        let n = names();
        let tie = rule(Join::All, &[("sender-domain", "is", &["impots.example"]), ("subject", "has-words", &["avis", "notice"])], ("message", "tie-to-project"), &[("project", fixed("taxes"))]);
        let refused = rule(
            Join::All,
            &[("sender-domain", "is", &["shop.example"]), ("kind", "is", &["payment-refused"])],
            ("task", "make"),
            &[("model", fixed("pay")), ("title", Param::From { from: "subject".into() }), ("days", Param::Number(3))],
        );
        let topic = rule(Join::All, &[("circle", "is", &["safe"])], ("message", "give-topic"), &[("topic", fixed("bills"))]);
        let code = rule(Join::All, &[("kind", "is", &["code"])], ("none", "tell"), &[("level", fixed("now"))]);
        let quiet = rule(Join::All, &[("subject", "contains", &["rent"])], ("none", "tell"), &[("level", fixed("quiet")), ("words", fixed("Rent is due"))]);
        let filter = rule(Join::Any, &[("from", "contains", &["@bank.example"]), ("weekday", "is", &["sat", "sun"])], ("message", "mark-read"), &[]);
        let money = rule(Join::All, &[("sender-domain", "is", &["energy.example"])], ("movement", "plan"), &[("budget", fixed("home")), ("direction", fixed("debit")), ("amount", Param::From { from: "amount".into() })]);
        let papers = rule(
            Join::All,
            &[("sender-domain", "is", &["phone.example"]), ("kind", "is", &["bill"])],
            ("message", "keep-attachments"),
            &[("type", fixed("pdf")), ("kind", fixed("rent-receipt")), ("title", fixed("{sender} {month}")), ("for", fixed(crate::persons::ME))],
        );
        let unchosen = rule(Join::All, &[("attachment", "exists", &[])], ("", ""), &[]);
        let en = Translator::new("en");
        let said = |r: &Rule| sentence(r, &n, &en);
        assert_eq!(said(&tie), "When a message arrives from anyone at impots.example and with “avis” or “notice” in its subject: tie it to the project Taxes 2025.");
        assert_eq!(said(&refused), "When a message from anyone at shop.example says a payment was refused: make a task from the model “Pay another way”, titled with its subject, asked three business days after it came.");
        assert_eq!(said(&topic), "When a message arrives from someone in your Safe circle: give it the topic Bills.");
        assert_eq!(said(&code), "When a message holds a code: tell you.");
        assert_eq!(said(&quiet), "When a message arrives whose subject contains “rent”: keep it quietly in this rule's history.");
        assert_eq!(said(&filter), "When a message arrives whose sender contains “@bank.example” or on Saturday or Sunday: mark it read.");
        assert_eq!(said(&money), "When a message arrives from anyone at energy.example: propose a payment for the budget Household, of the amount it states.");
        assert_eq!(said(&papers), "When a message from anyone at phone.example brings a bill: keep its PDFs in Papers, as “Rent receipt”, named “‹sender› ‹month›”, for you.");
        assert_eq!(said(&unchosen), "When a message arrives with an attachment: (what it does, still to choose).");
        assert_eq!(when_heading(&tie, &en), "When a message arrives");
        assert_eq!(when_shared(&tie, &n, &en), "When a message arrives from anyone at impots.example and with “avis” or “notice” in its subject:");
        let fr = Translator::new("fr");
        let dit = |r: &Rule| sentence(r, &n, &fr);
        assert_eq!(dit(&tie), "Quand un courriel arrive du domaine impots.example et avec «\u{202f}avis\u{202f}» ou «\u{202f}notice\u{202f}» dans son objet\u{202f}: le rattacher au projet Taxes 2025.");
        assert_eq!(dit(&refused), "Quand un courriel du domaine shop.example dit qu’un paiement a été refusé\u{202f}: créer une tâche d’après le modèle «\u{202f}Pay another way\u{202f}», intitulée d’après son objet, demandée trois jours travaillés après son arrivée.");
        assert_eq!(dit(&topic), "Quand un courriel arrive de quelqu’un de votre cercle Sûrs\u{202f}: lui donner le thème Bills.");
        assert_eq!(dit(&code), "Quand un courriel contient un code\u{202f}: vous le dire.");
        assert_eq!(dit(&filter), "Quand un courriel arrive dont l’expéditeur contient «\u{202f}@bank.example\u{202f}» ou le samedi ou dimanche\u{202f}: le marquer comme lu.");
        assert_eq!(dit(&money), "Quand un courriel arrive du domaine energy.example\u{202f}: proposer un paiement dans le budget Household, du montant qu’il annonce.");
        assert_eq!(dit(&papers), "Quand un courriel du domaine phone.example apporte une facture\u{202f}: garder ses PDF dans Papiers, comme «\u{202f}Quittance de loyer\u{202f}», sous le nom «\u{202f}‹expéditeur› ‹mois›\u{202f}», pour vous.");
    }

    /// What is left unset reads as the engine does it: PDFs kept, told now,
    /// the title, the project and the days the engine takes said as nothing
    /// more than "from the model".
    #[test]
    fn unset_details_read_as_the_engine_does_them() {
        let n = names();
        let en = Translator::new("en");
        let papers = rule(Join::All, &[("kind", "is", &["bill"])], ("message", "keep-attachments"), &[]);
        assert_eq!(then_part(&papers, &n, &en), "keep its PDFs in Papers");
        let tell = rule(Join::All, &[("kind", "is", &["bill"])], ("none", "tell"), &[]);
        assert_eq!(then_part(&tell, &n, &en), "tell you");
        let quiet_words = rule(Join::All, &[("kind", "is", &["bill"])], ("none", "tell"), &[("level", fixed("quiet")), ("words", fixed("Pay"))]);
        assert_eq!(then_part(&quiet_words, &n, &en), "keep it quietly in this rule's history", "quiet words are never said");
        let task = rule(Join::All, &[("kind", "is", &["bill"])], ("task", "make"), &[("model", fixed("pay"))]);
        assert_eq!(then_part(&task, &n, &en), "make a task from the model “Pay another way”");
        let moved = rule(Join::All, &[("folder", "is", &["junk"])], ("message", "move-to-folder"), &[("folder", fixed("inbox"))]);
        assert_eq!(then_part(&moved, &n, &en), "move it into your inbox");
    }

    /// An action on another object than the one that triggered it says
    /// whose; one this Sioul has no phrase for is said by its declared name,
    /// never an id; an act of a rule names its message and what it did.
    #[test]
    fn other_objects_unknown_actions_and_acts() {
        let n = names();
        let en = Translator::new("en");
        let mut other = rule(Join::All, &[("kind", "is", &["bill"])], ("message", "flag"), &[]);
        other.when.kind = "task".into();
        other.when.event = "is-done".into();
        other.when.conditions.clear();
        assert_eq!(sentence(&other, &n, &en), "When a task is done: flag its message.");
        let unknown = rule(Join::All, &[("kind", "is", &["bill"])], ("message", "sing"), &[]);
        assert!(!sentence(&unknown, &n, &en).contains("rule-"), "{}", sentence(&unknown, &n, &en));
        let act = Act { at: 1, rule: "r".into(), object: "mid:a@impots.example".into(), act: "tie".into(), result: "sioul:project/taxes".into(), wrote: true, ..Act::default() };
        assert_eq!(act_line(&act, &n, &en), "“Your notice” tied to the project Taxes 2025.");
        let found = Act { wrote: false, object: "mid:gone@example.org".into(), ..act };
        assert_eq!(act_line(&found, &n, &en), "A message was in the project Taxes 2025 already.");
    }

    /// Every preset, and every action and event the declarations have,
    /// reads as words in both languages: no message id left, no empty part.
    #[test]
    fn every_preset_and_declaration_has_words() {
        let n = names();
        for language in ["en", "fr"] {
            let tr = Translator::new(language);
            for preset in declare::presets() {
                let said = sentence(&preset.rule, &n, &tr);
                assert!(!said.contains("rule-") && !said.contains("rules-") && !said.contains("  ") && !said.contains(" ,"), "{language} {}: {said}", preset.id);
            }
            for kind in declare::TYPES {
                for event in kind.events {
                    let heading = heading(kind.id, event.id, &tr);
                    assert!(!heading.contains("rule"), "{language} {}.{}: {heading}", kind.id, event.id);
                }
                for action in kind.actions {
                    let r = rule(Join::All, &[], (kind.id, action.id), &[]);
                    let said = then_part(&r, &n, &tr);
                    assert!(!said.contains("rule-") && !said.trim().is_empty(), "{language} {}.{}: {said}", kind.id, action.id);
                }
            }
        }
    }

    /// Every preset has its own written sentence in both languages, and
    /// each setting it names (its switch, its details) is a setting today,
    /// one a page shows: a preset never claims a switch that does not exist.
    #[test]
    fn every_preset_is_written_and_its_settings_exist() {
        let views = ["parameters", "general", "porch", "mail", "senders", "accounts", "agenda", "tasks", "notes", "sites", "texts", "money", "contacts", "supplies", "letters", "papers"];
        let tr = Translator::new("en");
        let config = crate::config::Config::default();
        let keys: Vec<String> = views.iter().flat_map(|v| crate::settings::for_view(v, &config, &tr, &[], None)).map(|s| s.key).collect();
        for preset in declare::presets() {
            for language in ["en", "fr"] {
                let written = Translator::new(language).own(&format!("rule-preset-{}", preset.id));
                assert!(written.is_some_and(|w| !w.is_empty()), "{language}: no sentence for the preset {}", preset.id);
            }
            for key in preset.setting.into_iter().chain(preset.params.iter().map(|p| p.key)) {
                assert!(keys.iter().any(|k| k == key), "the preset {} names {key}, which no page shows", preset.id);
            }
        }
    }
}
