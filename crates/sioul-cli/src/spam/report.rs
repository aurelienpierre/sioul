// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What `sioul spam` says: in lines for a person, as data for a program
//! (`--json`), the same for the MCP's spam tools (`mcp/spam.rs`). The corpus
//! and the table (`status`), a download (`fetch`), a training (`train`), the
//! table's test with its worst errors (`eval`), what the matrix would do now
//! (`dry_run`), the review queue (`review`), a label (`label`), a job (`job`).
//!
//! A message is listed by its date, its account and folder, its sender's
//! address and its subject, masked as the MCP masks them (`mcp::mask`: a
//! code, a sign-in link, an account number hidden); hostile mail to a
//! shielded address by "someone at" its domain, without its subject; mail
//! from a sender you blocked not at all, only counted. Never its text.

use super::jobs;
use crate::mcp::mask;
use crate::{Session, one_line};
use serde_json::{Map, Value, json};
use sioul_core::card::Card;
use sioul_core::folders::{self, Role};
use sioul_core::porch::{self, KnownSenders, Lane, Triaged};
use sioul_core::spam::labels::{self as label_log, Entry, Label, Source};
use sioul_core::spam::table::Table;
use sioul_core::spam::{Action, Actions, Class, Filter};
use sioul_core::state::PorchState;
use sioul_learn::detail::{Confusion, Detail, Errors, Wrong};
use sioul_learn::{Cancel, Dirs, LearnError, Progress, corpus, eval, external, train};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// What a command says: lines for a person, the same as data for a program.
pub(crate) struct Report {
    pub lines: Vec<String>,
    pub data: Value,
}

// Numbers, dates and words.

/// A number with this language's decimal separator.
pub(crate) fn decimal(s: &Session, value: f64, places: usize) -> String {
    format!("{value:.places$}").replace('.', &s.tr.text("decimal-separator", None))
}

/// A fraction as a percentage: "0.4 %" in this language's way.
fn percent(s: &Session, fraction: f64) -> String {
    s.say("spam-percent", &[("n", decimal(s, fraction * 100.0, 1))])
}

/// A day, in this language's way, with its year when not this year's; a dash for an unknown date.
pub(crate) fn day(s: &Session, seconds: i64) -> String {
    let today = jiff::Zoned::now().date();
    jiff::Timestamp::from_second(seconds).ok().filter(|_| seconds > 0).map(|t| s.tr.day_in(t.to_zoned(jiff::tz::TimeZone::system()).date(), today)).unwrap_or_else(|| "—".to_string())
}

/// Unix seconds as a moment in the person's time zone, for data: "2026-10-05T09:00:00+02:00"; null when unknown.
fn instant(seconds: i64) -> Value {
    jiff::Timestamp::from_second(seconds)
        .ok()
        .filter(|_| seconds > 0)
        .map_or(Value::Null, |t| Value::String(t.to_zoned(jiff::tz::TimeZone::system()).strftime("%Y-%m-%dT%H:%M:%S%:z").to_string()))
}

/// A probability for data: four decimals.
fn rounded(p: f64) -> f64 {
    (p * 10_000.0).round() / 10_000.0
}

fn label_name(label: Label) -> &'static str {
    match label {
        Label::Spam => "spam",
        Label::Ham => "ham",
    }
}

/// A class said as the review queue says it: "probably spam".
fn class_words(s: &Session, class: Class) -> String {
    s.tr.text(&format!("spam-chip-{}", class.as_str()), None)
}

/// An action said as the settings say it: "Move to spam".
fn action_words(s: &Session, action: Action) -> String {
    s.tr.text(&format!("set-spam-action-{}", action.as_str()), None)
}

/// A folder's name as the person reads it.
fn folder_name(name: &str) -> String {
    one_line(&folders::decode_utf7(name))
}

/// A long job's failure, in words.
pub(crate) fn failure(s: &Session, e: &LearnError, options: &train::Options) -> String {
    match e {
        LearnError::Cancelled => s.tr.text("spam-stopped", None),
        LearnError::TooFew { ham, spam } => s.say("spam-too-few", &[("ham", ham.to_string()), ("spam", spam.to_string()), ("least", options.least_of_each.to_string())]),
        LearnError::Disk(detail) => s.say("spam-disk", &[("detail", detail.clone())]),
        LearnError::Fold(detail) => s.say("spam-fold", &[("detail", detail.clone())]),
        LearnError::NoTable(_) => s.tr.text("spam-no-table", None),
        other => s.say("spam-error", &[("detail", other.to_string())]),
    }
}

fn to_data<T: serde::Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap_or(Value::Null)
}

// Messages, as they are listed.

/// What is hidden of a message listed, as the MCP hides it: hostile mail
/// to a shielded address by its words, mail from a blocked sender whole.
pub(crate) struct Hiding {
    shielded: Vec<String>,
    read_by_ai: BTreeMap<String, sioul_core::shield::Assessment>,
    senders: porch::Senders,
}

/// A message's sender and subject as they are listed.
pub(crate) struct Envelope {
    pub from: String,
    pub subject: String,
    /// Hostile mail to a shielded address: neither its sender's name nor its subject.
    pub hidden: bool,
}

impl Hiding {
    pub(crate) fn of(config: &sioul_core::config::Config) -> Hiding {
        let shielded = config.accounts.iter().filter(|a| a.shield).map(|a| a.id.clone()).collect::<Vec<_>>();
        let read_by_ai = if shielded.is_empty() { BTreeMap::new() } else { sioul_core::shield::AiCache::load_all() };
        Hiding { shielded, read_by_ai, senders: porch::Senders::load(config) }
    }

    /// The message's sender (its address) and its subject, masked; none
    /// from a sender you blocked: never listed, only counted.
    pub(crate) fn envelope(&self, s: &Session, account: &str, card: &Card) -> Option<Envelope> {
        if self.senders.who_of(card) == sioul_core::reach::Who::Blocked {
            return None;
        }
        let address = card.from_address.as_deref().unwrap_or_default().trim().to_lowercase();
        if self.shielded.iter().any(|a| a == account) && sioul_core::shield::reading(card, Some(&self.read_by_ai)).tone == sioul_core::shield::Tone::Hostile {
            let domain = address.rsplit_once('@').map(|(_, d)| d.to_string()).unwrap_or_default();
            return Some(Envelope { from: s.say("hostile-someone-at", &[("domain", domain)]), subject: s.tr.text("hostile-subject", None), hidden: true });
        }
        let (subject, _) = mask::message(&card.subject, &card.excerpt);
        Some(Envelope { from: one_line(&address), subject: one_line(&subject), hidden: false })
    }
}

/// A message's address (`mid:…`), for read_message, spam_label and links; "" without a Message-ID.
fn uri_of(card: &Card) -> String {
    card.message_id.as_deref().map(sioul_core::links::mail_uri).filter(|u| u.len() > 4).unwrap_or_default()
}

/// One message listed: its line and its data.
struct Listed {
    line: String,
    data: Value,
}

/// A message of a list, from what is known of it: its line (`p · day ·
/// account · folder · sender · subject`, then `more` words), its data.
fn listed(s: &Session, p: f64, date: i64, account: &str, folder: &str, envelope: &Envelope, more: &str, data: Map<String, Value>) -> Listed {
    // Outside material has no folder; a message without a sender or a subject, none said.
    let (probability, day, account) = (decimal(s, p, 3), day(s, date), one_line(account));
    let parts = [probability.as_str(), day.as_str(), account.as_str(), folder, envelope.from.as_str(), envelope.subject.as_str(), more];
    let line = format!("  {}", parts.iter().filter(|part| !part.is_empty()).copied().collect::<Vec<_>>().join(" · "));
    let mut item = Map::new();
    item.insert("p".into(), json!(rounded(p)));
    item.insert("date".into(), instant(date));
    item.insert("account".into(), json!(account));
    item.insert("folder".into(), json!(folder));
    item.insert("from".into(), json!(envelope.from));
    item.insert("subject".into(), json!(envelope.subject));
    item.insert("hidden".into(), json!(envelope.hidden));
    item.extend(data);
    Listed { line, data: Value::Object(item) }
}

/// The note above every list of messages: their words are their senders'.
fn data_note(s: &Session) -> String {
    s.tr.text("spam-list-data", None)
}

// `sioul spam status`.

/// What the corpus holds, the outside material, the providers' verdicts, the table in use, the last training, the jobs.
pub(crate) fn status(s: &Session, dirs: &Dirs) -> Result<Report, String> {
    let mut lines = Vec::new();
    let corpus = corpus::status(dirs);
    if corpus.accounts.is_empty() {
        lines.push(s.tr.text("spam-corpus-empty", None));
    }
    let mut accounts = Vec::new();
    for account in &corpus.accounts {
        let records: u64 = account.folders.iter().map(|f| f.records).sum();
        let size = decimal(s, account.bytes as f64 / f64::from(1u32 << 20), 1);
        lines.push(s.say("spam-corpus-account", &[("account", account.id.clone()), ("records", records.to_string()), ("size", size), ("folders", account.folders.len().to_string())]));
        let mut kept = Vec::new();
        for folder in &account.folders {
            let name = folder_name(&folder.name);
            lines.push(format!("  {}", s.say("spam-corpus-folder", &[("folder", name.clone()), ("server", folder.on_server.to_string()), ("records", folder.records.to_string())])));
            kept.push(json!({ "folder": name, "role": format!("{:?}", folder.role).to_lowercase(), "records": folder.records, "on_server": folder.on_server, "recorded": folder.recorded }));
        }
        accounts.push(json!({ "account": account.id, "records": records, "bytes": account.bytes, "folders": kept }));
    }
    if let Some(at) = corpus.last_run {
        lines.push(s.say("spam-corpus-last", &[("date", day(s, at))]));
    }
    if let Some(held) = corpus.held {
        lines.push(s.say("spam-held", &[("free", (held.free >> 20).to_string())]));
    }
    for (account, detail) in &corpus.errors {
        lines.push(s.say("spam-account-failed", &[("account", account.clone()), ("detail", one_line(detail))]));
    }
    // Outside material: what each source holds, kept apart, never shared.
    let mut outside = Vec::new();
    for source in external::sources(dirs) {
        let size = decimal(s, source.bytes as f64 / f64::from(1u32 << 20), 1);
        let pairs = [
            ("source", one_line(&source.name)),
            ("ham", source.counts.ham.to_string()),
            ("spam", source.counts.spam.to_string()),
            ("first", day(s, source.counts.first)),
            ("last", day(s, source.counts.last)),
            ("size", size),
        ];
        lines.push(s.say("spam-outside", &pairs));
        outside.push(json!({ "source": source.name, "ham": source.counts.ham, "spam": source.counts.spam, "first": instant(source.counts.first), "last": instant(source.counts.last), "bytes": source.bytes }));
    }
    // Whether your providers' own spam flags still count, as Sioul reads them now: totals only.
    let mut verdicts = Map::new();
    for (account, v) in corpus::verdicts(dirs).map_err(|e| e.to_string())? {
        lines.push(s.say("spam-verdicts", &[("account", account.clone()), ("records", v.records.to_string()), ("with", v.with_header.to_string()), ("read", v.read.to_string()), ("flagged", v.flagged.to_string())]));
        verdicts.insert(account, json!({ "records": v.records, "with_header": v.with_header, "read": v.read, "flagged": v.flagged }));
    }
    // The table in use here.
    let table = match Table::read(&dirs.table()) {
        Ok(table) => {
            lines.push(s.say("spam-table", &[("date", day(s, table.meta.trained_at)), ("ham", table.meta.ham.to_string()), ("spam", table.meta.spam.to_string())]));
            to_data(&table.meta)
        }
        Err(why) if dirs.table().exists() => {
            lines.push(s.say("spam-table-refused", &[("detail", one_line(&why))]));
            json!({ "refused": why })
        }
        Err(_) => {
            lines.push(s.tr.text("spam-no-table", None));
            Value::Null
        }
    };
    let last = train::last(dirs);
    match &last {
        Some(summary) => {
            lines.push(String::new());
            lines.push(s.say("spam-last-training", &[("date", day(s, summary.trained_at))]));
            lines.extend(summary_lines(s, summary));
        }
        None => lines.push(s.tr.text("spam-never-trained", None)),
    }
    let running: Vec<jobs::Job> = jobs::list().into_iter().filter(|j| matches!(j.state, jobs::State::Starting | jobs::State::Running)).collect();
    for job in &running {
        lines.push(job_line(s, job));
    }
    let corpus_data = json!({
        "accounts": accounts, "last_run": corpus.last_run.map_or(Value::Null, instant),
        "held": corpus.held.map(|h| json!({ "at": instant(h.at), "free_mb": h.free >> 20 })), "errors": corpus.errors,
    });
    let jobs: Vec<Value> = running.iter().map(job_data).collect();
    let data = json!({ "corpus": corpus_data, "outside": outside, "verdicts": verdicts, "table": table, "last_training": last.as_ref().map(to_data), "jobs": jobs });
    Ok(Report { lines, data })
}

// A training's summary, in lines.

/// What a training did with the table and why, what it learned from, its numbers.
pub(crate) fn summary_lines(s: &Session, summary: &train::Summary) -> Vec<String> {
    let mut lines = Vec::new();
    let c = summary.compared.unwrap_or_default();
    let pairs = [("n", (c.ham + c.spam).to_string()), ("new", c.new_ham_lost.to_string()), ("old", c.current_ham_lost.to_string())];
    let outcome = match (summary.trial, summary.reason.as_str()) {
        (false, "no-table") => s.tr.text("spam-replaced-no-table", None),
        (false, "unreadable") => s.tr.text("spam-replaced-unreadable", None),
        (false, "nothing-new") => s.tr.text("spam-replaced-nothing-new", None),
        (false, "worse") => s.say("spam-kept-worse", &pairs),
        (false, _) => s.say("spam-replaced-no-worse", &pairs),
        (true, "no-table") => s.tr.text("spam-trial-no-table", None),
        (true, "unreadable") => s.tr.text("spam-trial-unreadable", None),
        (true, "nothing-new") => s.tr.text("spam-trial-nothing-new", None),
        (true, "worse") => s.say("spam-trial-worse", &pairs),
        (true, _) => s.say("spam-trial-no-worse", &pairs),
    };
    if summary.trial {
        lines.push(s.tr.text("spam-trial", None));
    }
    lines.push(outcome);
    if let Some(refit) = &summary.refit {
        lines.push(s.say("spam-refit", &[("n", (refit.ham + refit.spam).to_string())]));
    }
    lines.extend(label_lines(s, &summary.labels));
    for (account, c) in &summary.accounts {
        let pairs = [("account", account.clone()), ("trainham", c.train_ham.to_string()), ("trainspam", c.train_spam.to_string()), ("testham", c.test_ham.to_string()), ("testspam", c.test_spam.to_string())];
        lines.push(s.say("spam-account-counts", &pairs));
    }
    lines.extend(numbers_lines(s, &summary.test, summary.split.test_from));
    if let Some(settled) = summary.test_settled.as_ref().filter(|n| n.ham + n.spam < summary.test.ham + summary.test.spam) {
        let left = (summary.test.ham + summary.test.spam) - (settled.ham + settled.spam);
        lines.push(s.say("spam-settled", &[("days", sioul_learn::detail::UNSETTLED_DAYS.to_string()), ("n", left.to_string())]));
        lines.extend(measured_lines(s, settled));
    }
    for (source, outside) in &summary.outside {
        let pairs = [
            ("source", one_line(source)),
            ("trainham", outside.train_ham.to_string()),
            ("trainspam", outside.train_spam.to_string()),
            ("heldham", outside.held_ham.to_string()),
            ("heldspam", outside.held_spam.to_string()),
        ];
        lines.push(s.say("spam-outside-learned", &pairs));
        if let Some(baseline) = &outside.baseline {
            lines.extend(measured_lines(s, baseline));
        }
    }
    if let Some(current) = summary.current.as_ref().filter(|c| c.ham + c.spam > 0) {
        lines.push(s.say("spam-current", &[("n", (current.ham + current.spam).to_string())]));
        lines.extend(numbers_lines(s, current, summary.compared.map_or(summary.split.test_from, |c| c.since)));
    }
    let m = &summary.model;
    let pairs = [("vocabulary", m.vocabulary.to_string()), ("dim", m.dim.to_string()), ("c", decimal(s, m.c, 2)), ("seconds", decimal(s, summary.seconds, 0)), ("bytes", (m.table_bytes >> 10).to_string())];
    lines.push(s.say("spam-model", &pairs));
    if m.threads > 0 {
        let costs = if m.costs.is_empty() { decimal(s, m.c, 2) } else { m.costs.iter().map(|c| format!("{c}").replace('.', &s.tr.text("decimal-separator", None))).collect::<Vec<_>>().join(", ") };
        let pairs = [
            ("epochs", m.epochs.to_string()),
            ("minn", m.minn.to_string()),
            ("maxn", m.maxn.to_string()),
            ("bucket", m.bucket.to_string()),
            ("mincount", m.min_count.to_string()),
            ("threads", m.threads.to_string()),
            ("hamweight", decimal(s, m.ham_weight, 1)),
            ("costs", costs),
        ];
        lines.push(s.say("spam-model-asked", &pairs));
    }
    lines
}

fn label_lines(s: &Session, l: &sioul_learn::labels::Summary) -> Vec<String> {
    let pairs = [
        ("ham", l.ham.to_string()),
        ("spam", l.spam.to_string()),
        ("folder", l.by_folder.to_string()),
        ("junk", l.by_junk_folder.to_string()),
        ("keyword", l.by_keyword.to_string()),
        ("log", l.by_log.to_string()),
        ("ambiguous", l.ambiguous.to_string()),
    ];
    let mut lines = vec![s.say("spam-labels", &pairs)];
    if l.moved > 0 {
        lines.push(s.say("spam-labels-moved", &[("n", l.moved.to_string())]));
    }
    lines
}

fn numbers_lines(s: &Session, n: &eval::Numbers, since: i64) -> Vec<String> {
    let mut lines = vec![s.say("spam-tested", &[("ham", n.ham.to_string()), ("spam", n.spam.to_string()), ("since", day(s, since))])];
    lines.extend(measured_lines(s, n));
    lines
}

/// The numbers at both thresholds, the share unsure, the AUC.
fn measured_lines(s: &Session, n: &eval::Numbers) -> Vec<String> {
    let mut lines = Vec::new();
    for (at, what) in [(&n.at_spam, "spam-what-set-aside"), (&n.at_unsure, "spam-what-unsure")] {
        let pairs = [
            ("threshold", decimal(s, at.threshold, 2)),
            ("what", s.tr.text(what, None)),
            ("ham", percent(s, at.ham_called_spam.fraction())),
            ("hamlow", percent(s, at.ham_called_spam.low)),
            ("hamhigh", percent(s, at.ham_called_spam.high)),
            ("spam", percent(s, at.spam_caught.fraction())),
            ("spamlow", percent(s, at.spam_caught.low)),
            ("spamhigh", percent(s, at.spam_caught.high)),
        ];
        lines.push(s.say("spam-at-threshold", &pairs));
    }
    if let Some(strict) = &n.strict {
        let pairs = [
            ("threshold", decimal(s, strict.threshold.min(1.0), 3)),
            ("spam", percent(s, strict.spam_caught.fraction())),
            ("spamlow", percent(s, strict.spam_caught.low)),
            ("spamhigh", percent(s, strict.spam_caught.high)),
        ];
        lines.push(s.say("spam-strict", &pairs));
    }
    lines.push(s.tr.text("spam-intervals", None));
    lines.push(s.say("spam-unsure-share", &[("share", percent(s, n.unsure.fraction()))]));
    if let Some(auc) = n.auc {
        lines.push(s.say("spam-auc", &[("auc", decimal(s, auc, 4))]));
    }
    lines
}

// What a test said message by message.

fn confusion_line(s: &Session, name: &str, c: &Confusion) -> String {
    let pairs = [
        ("name", name.to_string()),
        ("ham", c.ham.total().to_string()),
        ("hamspam", c.ham.spam.to_string()),
        ("hamunsure", c.ham.unsure.to_string()),
        ("hamham", c.ham.ham.to_string()),
        ("spam", c.spam.total().to_string()),
        ("spamspam", c.spam.spam.to_string()),
        ("spamunsure", c.spam.unsure.to_string()),
        ("spamham", c.spam.ham.to_string()),
    ];
    s.say("spam-confusion", &pairs)
}

/// One error listed; none from a sender you blocked (counted by the caller).
/// `learned_before`: when the table judging it was trained (`eval`): a
/// message from before, it learned from.
fn wrong_listed(s: &Session, hiding: &Hiding, w: &Wrong, learned_before: Option<i64>) -> Option<Listed> {
    let envelope = match &w.card {
        Some(card) if w.outside => {
            // Outside material: no account of yours, no shield; a blocked sender still left out.
            if hiding.senders.who_of(card) == sioul_core::reach::Who::Blocked {
                return None;
            }
            let (subject, _) = mask::message(&card.subject, &card.excerpt);
            Envelope { from: one_line(card.from_address.as_deref().unwrap_or_default()), subject: one_line(&subject), hidden: false }
        }
        Some(card) => hiding.envelope(s, &w.account, card)?,
        None => Envelope { from: String::new(), subject: String::new(), hidden: false },
    };
    let folder = folder_name(&w.folder);
    let seen = learned_before.map(|since| w.date <= since);
    let mut more = s.tr.text(&format!("spam-evidence-{}", w.evidence), None);
    if seen == Some(true) {
        more.push_str(", ");
        more.push_str(&s.tr.text("spam-learned-from", None));
    }
    let mut data = Map::new();
    data.insert("class".into(), json!(w.class.as_str()));
    data.insert("label".into(), json!(label_name(w.label)));
    data.insert("evidence".into(), json!(w.evidence));
    data.insert("outside".into(), json!(w.outside));
    data.insert("uri".into(), json!(w.card.as_ref().filter(|_| !w.outside).map(uri_of).unwrap_or_default()));
    if let Some(seen) = seen {
        data.insert("learned_from".into(), json!(seen));
    }
    Some(listed(s, w.p, w.date, &w.account, &folder, &envelope, &more, data))
}

/// The errors of one set, in lines and data.
fn errors_listed(s: &Session, hiding: &Hiding, errors: &Errors, learned_before: Option<i64>, lines: &mut Vec<String>) -> Value {
    let mut blocked = 0u64;
    let mut kinds = Map::new();
    for (key, title, wrongs) in [("ham_called_spam", "spam-errors-ham", &errors.ham_called_spam), ("spam_missed", "spam-errors-spam", &errors.spam_missed)] {
        let listed: Vec<Listed> = wrongs
            .iter()
            .filter_map(|w| {
                let shown = wrong_listed(s, hiding, w, learned_before);
                blocked += u64::from(shown.is_none());
                shown
            })
            .collect();
        lines.push(s.say(title, &[("n", listed.len().to_string())]));
        if listed.is_empty() {
            lines.push(format!("  {}", s.tr.text("spam-errors-none", None)));
        }
        lines.extend(listed.iter().map(|l| l.line.clone()));
        kinds.insert(key.into(), Value::Array(listed.into_iter().map(|l| l.data).collect()));
    }
    if blocked > 0 {
        lines.push(s.say("spam-errors-blocked", &[("n", blocked.to_string())]));
    }
    kinds.insert("blocked".into(), json!(blocked));
    Value::Object(kinds)
}

/// A test's detail: by account and folder, the grid, the errors asked, your
/// own mail's then each outside source's. Lines only when `said`; the data
/// always. `learned_before`: when the table tested was trained (`eval`).
fn detail_report(s: &Session, detail: &Detail, learned_before: Option<i64>, said: bool, lines: &mut Vec<String>) -> Map<String, Value> {
    let mut out: Vec<String> = Vec::new();
    let hiding = Hiding::of(&s.config);
    out.push(s.tr.text("spam-by-account", None));
    out.extend(detail.by_account.iter().map(|(account, c)| format!("  {}", confusion_line(s, &one_line(account), c))));
    out.push(s.tr.text("spam-by-folder", None));
    let mut by_folder = Vec::new();
    for (account, folders) in &detail.by_folder {
        for (folder, c) in folders {
            let name = format!("{} · {}", one_line(account), folder_name(folder));
            out.push(format!("  {}", confusion_line(s, &name, c)));
            by_folder.push(json!({ "account": account, "folder": folder_name(folder), "confusion": c }));
        }
    }
    out.push(s.tr.text("spam-grid", None));
    for at in &detail.grid {
        let pairs = [
            ("threshold", decimal(s, at.threshold, 2)),
            ("ham", percent(s, at.ham_called_spam.fraction())),
            ("hamcount", at.ham_called_spam.count.to_string()),
            ("hamof", at.ham_called_spam.of.to_string()),
            ("spam", percent(s, at.spam_caught.fraction())),
            ("spamcount", at.spam_caught.count.to_string()),
            ("spamof", at.spam_caught.of.to_string()),
        ];
        out.push(format!("  {}", s.say("spam-grid-row", &pairs)));
    }
    let listing = !detail.errors.ham_called_spam.is_empty() || !detail.errors.spam_missed.is_empty() || detail.outside.values().any(|o| !o.errors.ham_called_spam.is_empty() || !o.errors.spam_missed.is_empty());
    if listing {
        out.push(data_note(s));
    }
    let errors = errors_listed(s, &hiding, &detail.errors, learned_before, &mut out);
    let mut outside = Map::new();
    for (source, o) in &detail.outside {
        out.push(s.say("spam-errors-outside", &[("source", one_line(source))]));
        out.push(format!("  {}", confusion_line(s, &one_line(source), &o.confusion)));
        let errors = errors_listed(s, &hiding, &o.errors, None, &mut out);
        outside.insert(source.clone(), json!({ "confusion": o.confusion, "grid": o.grid, "errors": errors }));
    }
    if said {
        lines.extend(out);
    }
    let mut data = Map::new();
    data.insert("by_account".into(), json!(detail.by_account));
    data.insert("by_folder".into(), Value::Array(by_folder));
    data.insert("grid".into(), json!(detail.grid));
    data.insert("grid_settled".into(), json!(detail.grid_settled));
    data.insert("errors".into(), errors);
    data.insert("outside_detail".into(), Value::Object(outside));
    data
}

// `sioul spam fetch`.

/// The corpus brought up to date: every account that syncs, or one.
pub(crate) fn fetch(s: &Session, dirs: &Dirs, only: Option<&str>, progress: &mut dyn FnMut(&Progress), cancel: &Cancel) -> Result<Report, String> {
    let accounts: Vec<_> = s.config.accounts.iter().filter(|a| a.syncs() && only.is_none_or(|id| a.id == id)).cloned().collect();
    if accounts.is_empty() {
        return Err(match only {
            Some(id) => s.say("account-unknown", &[("id", id.to_string())]),
            None => s.tr.text("sync-nothing", None),
        });
    }
    let update = corpus::update(&accounts, &|a| sioul_sync::secret::password(a), dirs, &corpus::free_space, progress, cancel);
    if update.cancelled {
        return Err(s.tr.text("spam-stopped", None));
    }
    let mut lines = Vec::new();
    for (account, detail) in &update.failed {
        lines.push(s.say("spam-account-failed", &[("account", account.clone()), ("detail", one_line(detail))]));
    }
    lines.push(s.say("spam-fetch-added", &[("n", update.added.to_string())]));
    if let Some(held) = update.held {
        lines.push(s.say("spam-held", &[("free", (held.free >> 20).to_string())]));
    }
    let failed: Vec<Value> = update.failed.iter().map(|(account, detail)| json!({ "account": account, "detail": one_line(detail) })).collect();
    let data = json!({ "added": update.added, "failed": failed, "held": update.held.map(|h| json!({ "free_mb": h.free >> 20 })) });
    Ok(Report { lines, data })
}

// `sioul spam train`.

/// What a training is asked.
#[derive(Debug, Clone, Default)]
pub(crate) struct TrainAsk {
    /// The corpus brought up to date first.
    pub fetch: bool,
    /// The table in place may be replaced, when no worse; else a trial.
    pub replace: bool,
    /// The worst errors of each kind to list; none: the detail as data only.
    pub errors: Option<usize>,
    /// fastText's and the SVM's settings instead of the defaults.
    pub settings: Settings,
}

/// fastText's and the SVM's settings, each instead of its default when given (recorded in `trained.toml`).
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Settings {
    pub threads: Option<u32>,
    pub dim: Option<u32>,
    pub epochs: Option<u32>,
    pub bucket: Option<u32>,
    pub minn: Option<u32>,
    pub maxn: Option<u32>,
    pub c: Option<f64>,
    pub ham_weight: Option<f64>,
}

impl Settings {
    /// The options with these settings in them; a setting out of its range said.
    pub(crate) fn apply(&self, options: &mut train::Options) -> Result<(), String> {
        let within = |name: &str, value: Option<u32>, least: u32, most: u32| match value {
            Some(v) if !(least..=most).contains(&v) => Err(format!("--{name} is from {least} to {most}.")),
            _ => Ok(value),
        };
        let positive = |name: &str, value: Option<f64>| match value {
            Some(v) if !(v.is_finite() && v > 0.0 && v <= 1e6) => Err(format!("--{name} is a number above 0.")),
            _ => Ok(value),
        };
        if let Some(v) = within("threads", self.threads, 1, 256)? {
            options.threads = v;
        }
        if let Some(v) = within("dim", self.dim, 2, 1000)? {
            options.dim = v;
        }
        if let Some(v) = within("epochs", self.epochs, 1, 100)? {
            options.epochs = v;
        }
        if let Some(v) = within("bucket", self.bucket, 0, 4_000_000)? {
            options.bucket = v;
        }
        if let Some(v) = within("minn", self.minn, 0, 20)? {
            options.minn = v;
        }
        if let Some(v) = within("maxn", self.maxn, 0, 20)? {
            options.maxn = v;
        }
        // No n-grams at all: none to hash either (fastText's own rule); else the shortest first, somewhere to hash them.
        if options.maxn == 0 {
            (options.minn, options.bucket) = (0, 0);
        } else if options.minn > options.maxn {
            return Err(format!("--minn ({}) is at most --maxn ({}).", options.minn, options.maxn));
        } else if options.bucket == 0 {
            return Err("--bucket 0 goes with --maxn 0 (no n-grams).".into());
        }
        if let Some(c) = positive("c", self.c)? {
            options.costs = vec![c];
        }
        if let Some(w) = positive("ham-weight", self.ham_weight)? {
            options.ham_weight = w;
        }
        Ok(())
    }

    /// The same, as the command line takes them (a job's arguments).
    pub(crate) fn args(&self) -> Vec<String> {
        let mut args = Vec::new();
        for (name, value) in [("threads", self.threads), ("dim", self.dim), ("epochs", self.epochs), ("bucket", self.bucket), ("minn", self.minn), ("maxn", self.maxn)] {
            if let Some(v) = value {
                args.extend([format!("--{name}"), v.to_string()]);
            }
        }
        for (name, value) in [("c", self.c), ("ham-weight", self.ham_weight)] {
            if let Some(v) = value {
                args.extend([format!("--{name}"), v.to_string()]);
            }
        }
        args
    }
}

/// A training, the corpus brought up to date first when asked: a trial
/// unless `replace`; its summary, and its test's detail (lines when errors are asked).
pub(crate) fn train(s: &Session, dirs: &Dirs, ask: &TrainAsk, progress: &mut dyn FnMut(&Progress), cancel: &Cancel) -> Result<Report, String> {
    let mut options = train::Options::of(&s.config);
    ask.settings.apply(&mut options)?;
    options.replace = ask.replace;
    lower_priority();
    let mut lines = Vec::new();
    let mut data = Map::new();
    if ask.fetch {
        let fetched = fetch(s, dirs, None, progress, cancel)?;
        lines.extend(fetched.lines);
        data.insert("fetch".into(), fetched.data);
    }
    let (summary, detail) = train::train_with(dirs, &train::trusted_ids(&s.config), &options, ask.errors.unwrap_or(0), progress, cancel).map_err(|e| failure(s, &e, &options))?;
    lines.extend(summary_lines(s, &summary));
    data.insert("summary".into(), json!(summary));
    data.insert("trial".into(), json!(summary.trial));
    data.extend(detail_report(s, &detail, None, ask.errors.is_some(), &mut lines));
    Ok(Report { lines, data: Value::Object(data) })
}

/// A training, below the other programs' priority: the computer stays
/// usable while every core learns (the window's training does the same).
/// Linux gives each thread its own priority, which the threads it starts take.
fn lower_priority() {
    #[cfg(target_os = "linux")]
    // SAFETY: setpriority only reads its arguments; 0 is this thread on Linux.
    unsafe {
        libc::setpriority(libc::PRIO_PROCESS, 0, 10);
    }
}

// `sioul spam eval`.

/// The table in place, tested on the newest fifth: the numbers, those of
/// the messages it never learned from, the detail, and up to `errors`
/// errors of each kind.
pub(crate) fn eval(s: &Session, dirs: &Dirs, errors: usize, progress: &mut dyn FnMut(&Progress), cancel: &Cancel) -> Result<Report, String> {
    let options = train::Options::of(&s.config);
    let (evaluation, detail) = train::evaluate_with(dirs, &train::trusted_ids(&s.config), &options, errors, progress, cancel).map_err(|e| failure(s, &e, &options))?;
    let table = &evaluation.table;
    let mut lines = vec![s.say("spam-table", &[("date", day(s, table.trained_at)), ("ham", table.ham.to_string()), ("spam", table.spam.to_string())])];
    lines.extend(label_lines(s, &evaluation.labels));
    lines.extend(numbers_lines(s, &evaluation.numbers, evaluation.split.test_from));
    match &evaluation.unseen {
        Some(unseen) => {
            lines.push(s.say("spam-unseen", &[("n", (unseen.numbers.ham + unseen.numbers.spam).to_string()), ("since", day(s, unseen.since))]));
            lines.extend(measured_lines(s, &unseen.numbers));
        }
        None => lines.push(s.say("spam-unseen-none", &[("since", day(s, table.trained_at))])),
    }
    let mut outside = Map::new();
    for (source, numbers) in &evaluation.outside {
        lines.push(s.say("spam-outside-baseline", &[("source", one_line(source)), ("ham", numbers.ham.to_string()), ("spam", numbers.spam.to_string())]));
        lines.extend(measured_lines(s, numbers));
        outside.insert(source.clone(), json!(numbers));
    }
    let mut data = Map::new();
    data.insert("table".into(), json!(table));
    data.insert("labels".into(), json!(evaluation.labels));
    data.insert("split".into(), json!(evaluation.split));
    data.insert("numbers".into(), json!(evaluation.numbers));
    data.insert("unseen".into(), evaluation.unseen.as_ref().map_or(Value::Null, |u| json!({ "since": instant(u.since), "numbers": u.numbers })));
    data.insert("outside".into(), Value::Object(outside));
    data.extend(detail_report(s, &detail, Some(table.trained_at), true, &mut lines));
    Ok(Report { lines, data: Value::Object(data) })
}

// `sioul spam dry-run`.

/// What a dry run is asked: which inboxes, how many messages listed, and
/// the matrix and thresholds to try instead of the settings'.
#[derive(Debug, Clone, Default)]
pub(crate) struct DryAsk {
    pub account: Option<String>,
    pub limit: usize,
    pub spam: Option<Action>,
    pub unsure: Option<Action>,
    pub ham: Option<Action>,
    pub threshold_spam: Option<f32>,
    pub threshold_unsure: Option<f32>,
}

/// The files of a Maildir folder's `new/` and `cur/`: its inbox, for an account's root.
fn inbox_files(folder: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = ["new", "cur"]
        .iter()
        .flat_map(|sub| std::fs::read_dir(folder.join(sub)).into_iter().flatten().filter_map(Result::ok).map(|e| e.path()))
        .filter(|p| p.is_file())
        .collect();
    files.sort();
    files
}

/// What your own filter would do now with the mail in each inbox, as the
/// Porch judges it after a fetch (protections first): counts per class and
/// action, the messages it would move into the Junk folder and those it
/// would flag, with the matrix and thresholds of the settings or those asked.
/// Nothing is moved, nothing is written.
pub(crate) fn dry_run(s: &Session, ask: &DryAsk) -> Result<Report, String> {
    let (spam_from, unsure_from) = s.config.spam.thresholds();
    let threshold_spam = ask.threshold_spam.unwrap_or(spam_from);
    let threshold_unsure = ask.threshold_unsure.unwrap_or(unsure_from.min(threshold_spam));
    if !(0.0..=1.0).contains(&threshold_spam) || !(0.0..=1.0).contains(&threshold_unsure) || threshold_unsure > threshold_spam {
        return Err(s.tr.text("spam-dry-thresholds", None));
    }
    let mut matrix = s.config.spam.actions();
    for (class, asked) in [(Class::Spam, ask.spam), (Class::Unsure, ask.unsure), (Class::Ham, ask.ham)] {
        if let Some(action) = asked {
            matrix = matrix.with(class, action);
        }
    }
    let table_path = sioul_core::spam::table_path();
    // Every class flagged while judging, so that each verdict shows; the matrix applied after.
    let judging = Filter { actions: Actions { spam: Action::Flag, unsure: Action::Flag, ham: Action::Flag }, threshold_spam, threshold_unsure, table: table_path.clone() };
    let sources: Vec<sioul_core::config::Source> = s.config.mail_sources().into_iter().map(|src| sioul_core::config::Source { spam: Some(judging.clone()), ..src }).collect();
    if let Some(id) = &ask.account
        && !sources.iter().any(|src| src.account.as_deref() == Some(id.as_str()))
    {
        return Err(s.say("account-unknown", &[("id", id.clone())]));
    }
    let chosen: Vec<&sioul_core::config::Source> = sources.iter().filter(|src| ask.account.as_ref().is_none_or(|id| src.account.as_deref() == Some(id.as_str()))).collect();
    let matrix_data = json!({ "spam": matrix.spam.as_str(), "unsure": matrix.unsure.as_str(), "ham": matrix.ham.as_str() });
    let thresholds = json!({ "spam": rounded(f64::from(threshold_spam)), "unsure": rounded(f64::from(threshold_unsure)) });
    let mut lines = Vec::new();
    let table = match Table::read(&table_path) {
        Ok(table) => table,
        Err(_) => {
            lines.push(s.tr.text("spam-dry-no-table", None));
            lines.push(s.tr.text("spam-dry-nothing-moved", None));
            let data = json!({ "table": null, "matrix": matrix_data, "thresholds": thresholds, "accounts": [], "would_move": { "total": 0, "messages": [] }, "would_flag": { "total": 0, "messages": [] }, "moved": 0 });
            return Ok(Report { lines, data });
        }
    };
    let store = crate::load_store(&s.config);
    let known = KnownSenders::load(&s.config.known_senders_path());
    let senders = porch::Senders::load(&s.config);
    let hiding = Hiding::of(&s.config);
    let now = jiff::Timestamp::now().as_second();
    let mut accounts = Vec::new();
    let (mut would_move, mut would_flag): (Vec<(f64, Listed)>, Vec<(f64, Listed)>) = (Vec::new(), Vec::new());
    let mut all = 0usize;
    for src in chosen {
        let account = src.account.clone().unwrap_or_default();
        let files = inbox_files(&src.folder);
        let judged = porch::judge(&files, &sources, store.as_ref(), &known, &senders, now);
        let mut counts: BTreeMap<&'static str, (u64, Action)> = Class::ALL.iter().map(|c| (c.as_str(), (0, matrix.of(*c)))).collect();
        let (mut protected, mut aside, mut hostile) = (0u64, 0u64, 0u64);
        // Mail from a blocked sender is judged by nothing, never shown: counted, with files that cannot be read.
        let blocked = files.len().saturating_sub(judged.len()) as u64;
        let mut listed_blocked = 0u64;
        for t in &judged {
            let Some((class, p)) = t.reasons.iter().find_map(porch::Reason::learned) else {
                match t.lane {
                    Lane::SetAside => aside += 1,
                    Lane::Hostile => hostile += 1,
                    _ => protected += 1,
                }
                continue;
            };
            let action = matrix.of(class);
            if let Some(count) = counts.get_mut(class.as_str()) {
                count.0 += 1;
            }
            if action == Action::Nothing {
                continue;
            }
            let Some(item) = judged_listed(s, &hiding, &account, t, class, f64::from(p)) else {
                listed_blocked += 1;
                continue;
            };
            match action {
                Action::Move => would_move.push((f64::from(p), item)),
                _ => would_flag.push((f64::from(p), item)),
            }
        }
        all += files.len();
        let judged_count: u64 = counts.values().map(|(n, _)| n).sum();
        let pairs = [
            ("account", one_line(&account)),
            ("messages", files.len().to_string()),
            ("judged", judged_count.to_string()),
            ("protected", protected.to_string()),
            ("aside", aside.to_string()),
            ("hostile", hostile.to_string()),
            ("blocked", (blocked + listed_blocked).to_string()),
        ];
        lines.push(s.say("spam-dry-account", &pairs));
        for class in Class::ALL {
            let (n, action) = counts[class.as_str()];
            lines.push(format!("  {}", s.say("spam-dry-class", &[("class", class_words(s, class)), ("n", n.to_string()), ("action", action_words(s, action))])));
        }
        let classes: Map<String, Value> = counts.iter().map(|(class, (n, action))| ((*class).to_string(), json!({ "count": n, "action": action.as_str() }))).collect();
        accounts.push(json!({
            "account": account, "messages": files.len(), "judged": judged_count, "protected": protected, "set_aside": aside,
            "hostile": hostile, "blocked": blocked + listed_blocked, "classes": classes,
        }));
    }
    let mut data = Map::new();
    for (key, title, mut items) in [("would_move", "spam-dry-would-move", would_move), ("would_flag", "spam-dry-would-flag", would_flag)] {
        items.sort_by(|a, b| b.0.total_cmp(&a.0));
        let total = items.len();
        lines.push(s.say(title, &[("n", total.to_string())]));
        if total > 0 {
            lines.push(format!("  {}", data_note(s)));
        }
        let more = total.saturating_sub(ask.limit);
        let shown: Vec<Listed> = items.into_iter().take(ask.limit).map(|(_, l)| l).collect();
        lines.extend(shown.iter().map(|l| l.line.clone()));
        if more > 0 {
            lines.push(s.say("spam-more", &[("n", more.to_string())]));
        }
        data.insert(key.into(), json!({ "total": total, "messages": shown.into_iter().map(|l| l.data).collect::<Vec<_>>() }));
    }
    let matrix_line = [
        ("spam", percent(s, f64::from(threshold_spam))),
        ("spamaction", action_words(s, matrix.spam)),
        ("unsure", percent(s, f64::from(threshold_unsure))),
        ("unsureaction", action_words(s, matrix.unsure)),
        ("hamaction", action_words(s, matrix.ham)),
    ];
    let mut head = vec![s.say("spam-dry-title", &[("n", all.to_string())]), s.say("spam-dry-matrix", &matrix_line)];
    head.append(&mut lines);
    head.push(s.tr.text("spam-dry-nothing-moved", None));
    data.insert("table".into(), json!(table.meta));
    data.insert("matrix".into(), matrix_data);
    data.insert("thresholds".into(), thresholds);
    data.insert("accounts".into(), Value::Array(accounts));
    data.insert("moved".into(), json!(0));
    Ok(Report { lines: head, data: Value::Object(data) })
}

/// A message the Porch judged, as a list shows it; none from a blocked sender.
fn judged_listed(s: &Session, hiding: &Hiding, account: &str, t: &Triaged, class: Class, p: f64) -> Option<Listed> {
    let envelope = hiding.envelope(s, account, &t.card)?;
    let key = t.card.path.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
    let date = t.card.date.or_else(|| t.card.path.as_deref().and_then(|p| std::fs::metadata(p).ok()?.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()).and_then(|d| i64::try_from(d.as_secs()).ok())).unwrap_or(0);
    let folder = folder_of(s, account, t.card.path.as_deref());
    let mut data = Map::new();
    data.insert("class".into(), json!(class.as_str()));
    data.insert("key".into(), json!(key));
    data.insert("uri".into(), json!(uri_of(&t.card)));
    let more = class_words(s, class);
    Some(listed(s, p, date, account, &folder, &envelope, &more, data))
}

/// The folder a stored message is in, by its name as the person reads it.
fn folder_of(s: &Session, account: &str, file: Option<&Path>) -> String {
    let folder = file.and_then(|f| s.config.account(account).and_then(|a| sioul_sync::mailbox::folder_of(a, f)));
    folder.map_or_else(|| "INBOX".to_string(), |f| if f.role == Role::Inbox { "INBOX".to_string() } else { one_line(&f.display) })
}

// `sioul spam features`.

/// Each header feature's mean by account and label (`sioul_learn::diagnose`):
/// a table, features down, groups across; numbers only.
pub(crate) fn features(s: &Session, dirs: &Dirs, domain: Option<&str>) -> Result<Report, String> {
    let groups = sioul_learn::diagnose::feature_means(dirs, &train::trusted_ids(&s.config), domain).map_err(|e| e.to_string())?;
    let names = sioul_learn::diagnose::names();
    let mut lines = vec![s.tr.text("spam-features", None)];
    let heads: Vec<String> = groups.iter().map(|g| format!("{}·{} ({})", one_line(&g.account), label_name(g.label), g.messages)).collect();
    lines.push(format!("{:<22} {}", "", heads.join("  ")));
    for (h, name) in names.iter().enumerate() {
        let cells: Vec<String> = groups.iter().zip(&heads).map(|(g, head)| format!("{:>width$}", g.means[h].map_or("—".to_string(), |m| format!("{m:.2}")), width = head.chars().count())).collect();
        lines.push(format!("{name:<22} {}", cells.join("  ")));
    }
    let data = json!({ "names": names, "domain": domain, "groups": groups });
    Ok(Report { lines, data })
}

// `sioul spam review`.

/// The review queue, as the Porch has it: what your filter flagged or
/// moved, waiting for your word; the surest spam first.
pub(crate) fn review(s: &Session, limit: usize) -> Result<Report, String> {
    let sources = s.config.mail_sources();
    if sources.is_empty() {
        return Err(s.tr.text("error-no-mail", None));
    }
    let store = crate::load_store(&s.config);
    let known = KnownSenders::load(&s.config.known_senders_path());
    let senders = porch::Senders::load(&s.config);
    let hiding = Hiding::of(&s.config);
    let state = PorchState::load(&PorchState::default_path());
    let items = porch::gather(&sources, store.as_ref(), &known, &senders, &state, jiff::Timestamp::now().as_second());
    let mut queue: Vec<(f64, Listed)> = items
        .iter()
        .filter(|t| t.lane == Lane::Review)
        .filter_map(|t| {
            let (class, p) = t.reasons.iter().find_map(porch::Reason::learned)?;
            let moved = t.reasons.contains(&porch::Reason::MovedToJunk);
            let account = t.card.account.clone().unwrap_or_default();
            let mut item = judged_listed(s, &hiding, &account, t, class, f64::from(p))?;
            let how = s.tr.text(if moved { "spam-review-moved" } else { "spam-review-flagged" }, None);
            item.line.push_str(&format!(", {how}"));
            if let Value::Object(map) = &mut item.data {
                map.insert("moved".into(), json!(moved));
            }
            Some((f64::from(p), item))
        })
        .collect();
    queue.sort_by(|a, b| b.0.total_cmp(&a.0));
    let total = queue.len();
    let mut lines = vec![if total == 0 { s.tr.text("spam-review-none", None) } else { s.say("spam-review-title", &[("n", total.to_string())]) }];
    if total > 0 {
        lines.push(format!("  {}", data_note(s)));
    }
    let shown: Vec<Listed> = queue.into_iter().take(limit).map(|(_, l)| l).collect();
    lines.extend(shown.iter().map(|l| l.line.clone()));
    if total > limit {
        lines.push(s.say("spam-more", &[("n", (total - limit).to_string())]));
    }
    Ok(Report { lines, data: json!({ "total": total, "messages": shown.into_iter().map(|l| l.data).collect::<Vec<_>>() }) })
}

// `sioul spam label`.

/// Whether a text names a message by its Message-ID rather than by its file.
fn by_id(text: &str) -> bool {
    let text = text.trim();
    text.starts_with("mid:") || text.starts_with('<') || (text.contains('@') && !text.contains('/') && !text.contains('\\'))
}

/// Says a message is spam or not: a line in this device's label log, as
/// the window's Spam and Not spam write it, which every device and the next
/// training read. With `moving`, the window's act itself, on the server:
/// the keyword, and the move ("Spam" into the Junk folder; "Not spam" out
/// of it, back to the inbox). A message no longer stored here, named by its
/// Message-ID, is labelled by its place in the corpus (never moved).
pub(crate) fn label(s: &Session, dirs: &Dirs, message: &str, label: Label, moving: bool) -> Result<Report, String> {
    let (entry, place, moved) = match crate::mcp::read::find_message(s, message) {
        Ok(file) => {
            let account = s.config.account_of(&file).cloned().ok_or_else(|| s.tr.text("mail-message-gone", None))?;
            let folder = sioul_sync::mailbox::folder_of(&account, &file).ok_or_else(|| s.tr.text("mail-message-gone", None))?;
            let in_junk = folder.role == Role::Junk;
            let source = match (label, moving, in_junk) {
                (Label::Spam, _, _) => Source::Junk,
                (Label::Ham, true, true) => Source::NotJunk,
                (Label::Ham, _, _) => Source::NotSpam,
            };
            let entry = Entry::of_file(&account.id, &folder.name, &file, source).ok_or_else(|| s.tr.text("spam-label-not-fetched", None))?;
            let moved = if moving {
                let action = match source {
                    Source::Junk => sioul_sync::mailbox::Action::Junk,
                    Source::NotJunk => sioul_sync::mailbox::Action::NotJunk,
                    _ => sioul_sync::mailbox::Action::NotSpam,
                };
                let password = sioul_sync::secret::password(&account).map_err(|e| e.sentence(&s.tr, &account.id))?;
                sioul_sync::mailbox::act(&account, &password, &file, &action).map_err(|e| e.sentence(&s.tr, &account.id))?;
                Some(match (source, in_junk) {
                    (Source::Junk, true) => "kept-in-junk",
                    (Source::Junk, false) => "into-junk",
                    (Source::NotJunk, _) => "back-to-inbox",
                    _ => "marked-where-it-is",
                })
            } else {
                sioul_sync::mailbox::label(&account, &file, source).map_err(|e| e.sentence(&s.tr, &account.id))?;
                None
            };
            (entry, "here", moved)
        }
        Err(not_here) if by_id(message) => {
            let copies = corpus::copies_of(dirs, message).map_err(|e| e.to_string())?;
            let Some((place, _)) = copies.iter().find(|(_, role)| *role != Role::Junk).or(copies.first()).cloned() else { return Err(not_here) };
            if moving {
                return Err(s.tr.text("spam-label-move-needs-file", None));
            }
            let source = if label == Label::Spam { Source::Junk } else { Source::NotSpam };
            let entry = Entry {
                at: jiff::Timestamp::now().as_second(),
                account: place.account,
                folder: place.folder,
                uidvalidity: place.uidvalidity,
                uid: place.uid,
                message_id: Some(sioul_core::mailindex::bare_id(message)).filter(|id| !id.is_empty()),
                label,
                source,
            };
            let device = sioul_sync::share::Here::load(&sioul_core::config::state_dir()).id;
            label_log::append_to(&label_log::own_log(&label_log::state(), &device), &entry)?;
            (entry, "corpus", None)
        }
        Err(e) => return Err(e),
    };
    let said = s.tr.text(if label == Label::Spam { "spam-label-spam" } else { "spam-label-ham" }, None);
    let folder = folder_name(&entry.folder);
    let mut lines = vec![s.say("spam-label-done", &[("label", said), ("account", one_line(&entry.account)), ("folder", folder.clone())])];
    if place == "corpus" {
        lines.push(s.tr.text("spam-label-corpus", None));
    }
    match moved {
        Some(what) => lines.push(s.tr.text(&format!("spam-label-acted-{what}"), None)),
        None => lines.push(s.tr.text("spam-label-nothing-moved", None)),
    }
    let uri = entry.message_id.as_deref().map(sioul_core::links::mail_uri).unwrap_or_default();
    let data = json!({
        "label": label_name(label), "account": entry.account, "folder": folder, "uidvalidity": entry.uidvalidity, "uid": entry.uid,
        "uri": uri, "source": to_data(&entry.source), "found": place, "moved": moved.is_some(), "act": moved,
    });
    Ok(Report { lines, data })
}

// `sioul spam job`.

/// A job's line: its id, its kind, where it stands, since when.
fn job_line(s: &Session, job: &jobs::Job) -> String {
    let state = s.tr.text(&format!("spam-job-state-{}", job.state.as_str()), None);
    let mut line = s.say("spam-job-line", &[("id", job.id.clone()), ("kind", job.kind.clone()), ("state", state), ("when", day(s, job.created))]);
    if let Some(step) = job.progress.as_ref().filter(|_| matches!(job.state, jobs::State::Running | jobs::State::Starting)) {
        line.push_str(" · ");
        line.push_str(&step.line);
    }
    line
}

fn job_data(job: &jobs::Job) -> Value {
    let mut value = to_data(job);
    if let Value::Object(map) = &mut value {
        for key in ["created", "started", "ended", "updated"] {
            if let Some(seconds) = map.get(key).and_then(Value::as_i64) {
                map.insert(key.into(), instant(seconds));
            }
        }
    }
    value
}

/// One job as it stands (asked to stop first, with `stop`), with its result
/// once done; or, without an id, every job kept, the newest first.
pub(crate) fn job(s: &Session, id: Option<&str>, stop: bool) -> Result<Report, String> {
    let Some(id) = id else {
        let jobs = jobs::list();
        let mut lines: Vec<String> = jobs.iter().take(20).map(|j| job_line(s, j)).collect();
        if lines.is_empty() {
            lines.push(s.tr.text("spam-job-none", None));
        }
        return Ok(Report { lines, data: json!({ "jobs": jobs.iter().take(20).map(job_data).collect::<Vec<_>>() }) });
    };
    let job = if stop { jobs::stop(id)? } else { jobs::look(id)? };
    let mut lines = vec![job_line(s, &job)];
    if job.stop_asked && matches!(job.state, jobs::State::Running | jobs::State::Starting) {
        lines.push(s.tr.text("spam-job-stopping", None));
    }
    if let Some(error) = &job.error {
        lines.push(crate::plain_lines(error));
    }
    lines.extend(job.lines.iter().cloned());
    Ok(Report { lines, data: job_data(&job) })
}

/// Starts a job apart, for an agent: its first line, and its data.
pub(crate) fn started(s: &Session, kind: &str, args: Vec<String>) -> Result<Report, String> {
    let job = jobs::start(s, kind, args)?;
    let lines = vec![s.say("spam-job-started", &[("id", job.id.clone()), ("kind", job.kind.clone())])];
    Ok(Report { lines, data: job_data(&job) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_named_by_id_or_by_file() {
        for id in ["mid:a@example.org", "<a@example.org>", "a@example.org"] {
            assert!(by_id(id), "{id}");
        }
        for file in ["/mail/home/cur/1.x", "cur/a@b", r"C:\mail\a@b"] {
            assert!(!by_id(file), "{file}");
        }
    }

    #[test]
    fn settings_in_their_ranges() {
        let mut options = train::Options::default();
        let asked = Settings { threads: Some(2), dim: Some(50), epochs: Some(3), bucket: Some(1000), minn: Some(2), maxn: Some(4), c: Some(0.5), ham_weight: Some(3.0) };
        asked.apply(&mut options).unwrap();
        assert_eq!((options.threads, options.dim, options.epochs, options.bucket, options.minn, options.maxn), (2, 50, 3, 1000, 2, 4));
        assert_eq!((options.costs.clone(), options.ham_weight), (vec![0.5], 3.0));
        assert_eq!(asked.args(), ["--threads", "2", "--dim", "50", "--epochs", "3", "--bucket", "1000", "--minn", "2", "--maxn", "4", "--c", "0.5", "--ham-weight", "3"]);
        for wrong in [
            Settings { dim: Some(1), ..Settings::default() },
            Settings { minn: Some(5), maxn: Some(3), ..Settings::default() },
            Settings { bucket: Some(0), ..Settings::default() },
            Settings { c: Some(0.0), ..Settings::default() },
            Settings { ham_weight: Some(f64::NAN), ..Settings::default() },
        ] {
            assert!(wrong.apply(&mut train::Options::default()).is_err(), "{wrong:?}");
        }
        // No n-grams: none to hash, and the table still reads (minn never above maxn).
        let mut none = train::Options::default();
        Settings { maxn: Some(0), ..Settings::default() }.apply(&mut none).unwrap();
        assert_eq!((none.minn, none.maxn, none.bucket), (0, 0, 0));
    }
}
