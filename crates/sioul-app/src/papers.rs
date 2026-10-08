// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The papers wallet, for the window (docs/papers.md): the page, its form,
//! a paper kept from a mail's attachment or a file, its renewal planned as a task.

use crate::backend::{QtThread, Shared, json, load_config, say, tr};
use serde::{Deserialize, Serialize};
use sioul_core::papers::{Kind, Paper, Standing, Wallet};
use sioul_core::tasks::{Link, TaskEdit};
use std::path::Path;
use std::sync::Arc;

fn wallet() -> Result<Wallet, String> {
    let root = load_config().case_store_path().ok_or_else(|| tr().text("papers-no-store", None))?;
    Wallet::load(&root)
}

#[derive(Serialize)]
struct PaperView {
    id: String,
    kind: &'static str,
    kind_label: String,
    title: String,
    holder: String,
    notes: String,
    /// Its file where it is on this computer; "" without one.
    file: String,
    file_name: String,
    /// The file is there (its sync may not be done yet).
    file_there: bool,
    issued: String,
    until: String,
    /// "valid", "renew", "ended", "fresh", "old", "undated".
    standing: &'static str,
    line: String,
    renewal: String,
    /// Renewing can be planned: it is renewed, it has an end, no task yet.
    can_renew: bool,
}

#[derive(Serialize)]
struct Family {
    id: &'static str,
    label: String,
    papers: Vec<PaperView>,
}

#[derive(Serialize)]
struct Choice {
    id: &'static str,
    label: String,
}

#[derive(Serialize, Default)]
struct PapersView {
    /// A case store is set: papers have a home.
    store: bool,
    problem: String,
    kinds: Vec<Choice>,
    families: Vec<Family>,
}

fn view_of(wallet: &Wallet, paper: &Paper, today: jiff::civil::Date) -> PaperView {
    let file = wallet.file_path(paper);
    let date = |d: jiff::civil::Date| tr().day_in(d, today);
    let (standing, line) = match paper.standing(today) {
        Standing::Valid(d) => ("valid", say("paper-valid", &[("date", date(d))])),
        Standing::Renew(d) => ("renew", say("paper-renew", &[("date", date(d))])),
        Standing::Ended(d) => ("ended", say("paper-ended", &[("date", date(d))])),
        Standing::Fresh(d) => ("fresh", say("paper-fresh", &[("date", date(d))])),
        Standing::Old(d) => ("old", say("paper-old", &[("date", date(d))])),
        Standing::Undated => ("undated", String::new()),
    };
    PaperView {
        id: paper.id.clone(),
        kind: paper.kind.id(),
        kind_label: tr().text(&format!("paper-kind-{}", paper.kind.id()), None),
        title: paper.title.clone(),
        holder: paper.holder.clone(),
        notes: paper.notes.clone(),
        file_name: file.as_ref().and_then(|f| f.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
        file_there: file.as_ref().is_some_and(|f| f.is_file()),
        file: file.map(|f| f.display().to_string()).unwrap_or_default(),
        issued: paper.issued.map(|d| d.to_string()).unwrap_or_default(),
        until: paper.until.map(|d| d.to_string()).unwrap_or_default(),
        standing,
        line,
        renewal: paper.renewal.clone(),
        can_renew: paper.renewal.is_empty() && paper.renew_from().is_some(),
    }
}

/// The page, as JSON: the papers by family, the kinds to choose from.
pub(crate) fn view() -> String {
    let kinds = Kind::ALL.iter().map(|k| Choice { id: k.id(), label: tr().text(&format!("paper-kind-{}", k.id()), None) }).collect();
    let wallet = match wallet() {
        Ok(wallet) => wallet,
        Err(problem) => return json(&PapersView { store: load_config().case_store_path().is_some(), problem, kinds, families: Vec::new() }),
    };
    let today = jiff::Zoned::now().date();
    let mut families: Vec<Family> = Vec::new();
    for family in ["identity", "health", "home", "money", "warranty", "other"] {
        let mut papers: Vec<&Paper> = wallet.papers.iter().filter(|p| p.kind.family() == family).collect();
        papers.sort_by(|a, b| (a.kind.id(), &a.holder, &a.title).cmp(&(b.kind.id(), &b.holder, &b.title)));
        if !papers.is_empty() {
            families.push(Family { id: family, label: tr().text(&format!("paper-family-{family}"), None), papers: papers.into_iter().map(|p| view_of(&wallet, p, today)).collect() });
        }
    }
    json(&PapersView { store: true, problem: String::new(), kinds, families })
}

/// What the form gives back.
#[derive(Deserialize)]
struct PaperEdit {
    #[serde(default)]
    kind: String,
    #[serde(default)]
    title: String,
    /// A file: kept where it is when it is in the wallet's folder, else copied there.
    #[serde(default)]
    file: String,
    #[serde(default)]
    issued: String,
    #[serde(default)]
    until: String,
    #[serde(default)]
    holder: String,
    #[serde(default)]
    notes: String,
}

/// A paper saved: new when `id` is empty; returns what went wrong, else "".
pub(crate) fn save(id: &str, edit: &str) -> String {
    let edit: PaperEdit = match serde_json::from_str(edit) {
        Ok(edit) => edit,
        Err(e) => return e.to_string(),
    };
    let mut wallet = match wallet() {
        Ok(wallet) => wallet,
        Err(e) => return e,
    };
    let title = edit.title.trim().to_string();
    if title.is_empty() {
        return tr().text("papers-no-title", None);
    }
    let date = |text: &str| -> Result<Option<jiff::civil::Date>, String> {
        let text = text.trim();
        if text.is_empty() { Ok(None) } else { text.parse().map(Some).map_err(|_| say("papers-bad-date", &[("date", text.to_string())])) }
    };
    let (issued, until) = match (date(&edit.issued), date(&edit.until)) {
        (Ok(issued), Ok(until)) => (issued, until),
        (Err(e), _) | (_, Err(e)) => return e,
    };
    let kind = Kind::of(&edit.kind);
    let old = wallet.get(id).cloned().unwrap_or_default();
    // A file from elsewhere is copied into the wallet's folder: it travels with it.
    let file = if edit.file.trim().is_empty() {
        String::new()
    } else {
        let path = crate::backend::local_path(edit.file.trim());
        let inside = wallet.file_path(&old).is_some_and(|f| f == path) || path.starts_with(wallet.root.join(sioul_core::papers::FOLDER));
        if inside {
            path.strip_prefix(&wallet.root).map_or_else(|_| path.display().to_string(), |p| p.to_string_lossy().replace('\\', "/"))
        } else if path.is_file() {
            let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            match wallet.keep_file(&path, &name) {
                Ok(kept) => kept,
                Err(e) => return e,
            }
        } else {
            return say("papers-no-file", &[("path", path.display().to_string())]);
        }
    };
    // An end proposed from the issue when the kind's usual length is known and none was given.
    let until = until.or_else(|| issued.and_then(|d| sioul_core::papers::usual_end(kind, d)));
    let paper = Paper {
        id: old.id.clone(),
        kind,
        title,
        file,
        issued,
        until,
        holder: edit.holder.trim().to_string(),
        notes: edit.notes.trim().to_string(),
        // A new end: a new renewal to plan.
        renewal: if old.until == until { old.renewal.clone() } else { String::new() },
        added: old.added.or_else(|| Some(jiff::Zoned::now().date())),
    };
    wallet.put(paper);
    wallet.save().err().unwrap_or_default()
}

/// A paper taken out; its file stays where it is.
pub(crate) fn remove(id: &str) -> String {
    let mut wallet = match wallet() {
        Ok(wallet) => wallet,
        Err(e) => return e,
    };
    if !wallet.remove(id) {
        return String::new();
    }
    wallet.save().err().unwrap_or_default()
}

/// Its renewal, as a task in your usual list (your phone has it): from when
/// renewing starts to the day it ends, with the steps known for its kind.
pub(crate) fn plan_renewal(qt: &QtThread, shared: &Arc<Shared>, id: &str) -> String {
    let mut wallet = match wallet() {
        Ok(wallet) => wallet,
        Err(e) => return e,
    };
    let Some(mut paper) = wallet.get(id).cloned() else { return tr().text("papers-gone", None) };
    let (Some(from), Some(until)) = (paper.renew_from(), paper.until) else { return String::new() };
    let today = jiff::Zoned::now().date();
    let steps = format!("paper-steps-{}", paper.kind.id());
    let notes = tr().text(&steps, None);
    let edit = TaskEdit {
        title: say("paper-renew-task", &[("title", paper.title.clone())]),
        notes: if notes == steps { String::new() } else { notes },
        start: from.max(today).to_string(),
        due: until.to_string(),
        estimate: 30,
        links: vec![Link { uri: sioul_core::links::paper_uri(&paper.id), label: paper.title.clone(), rel: "related".into() }],
        ..TaskEdit::default()
    };
    let list = sioul_core::tasks::default_list().map(|l| format!("{}/{}", l.account, l.id));
    let made = match list {
        Some(list) => crate::work::create_task(qt, shared, &edit, &list),
        None => crate::work::local_task(qt, shared, &tr().text("papers-list", None), &edit),
    };
    match made {
        Ok(uid) => {
            paper.renewal = uid;
            wallet.put(paper);
            wallet.save().err().unwrap_or_default()
        }
        Err(e) => e,
    }
}

/// A file checked by the antivirus (an attachment), kept in the wallet's
/// folder; returns its path there, with a title and a kind guessed from its name.
pub(crate) fn keep_checked(file: &Path, name: &str, subject: &str) -> Result<(String, String, &'static str), String> {
    let wallet = wallet()?;
    let kept = wallet.keep_file(file, name)?;
    let title = Path::new(name).file_stem().map(|s| s.to_string_lossy().replace(['_', '-'], " ")).unwrap_or_default();
    let looked = sioul_core::words::Words::of(&load_config());
    let kind = Kind::guess(&looked, name).or_else(|| Kind::guess(&looked, subject)).unwrap_or_default();
    Ok((wallet.root.join(&kept).display().to_string(), title, kind.id()))
}
