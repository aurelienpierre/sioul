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
    let root = load_config().notes_root_path().ok_or_else(|| tr().text("papers-no-store", None))?;
    Wallet::load(&root)
}

#[derive(Serialize)]
struct PaperView {
    id: String,
    kind: &'static str,
    kind_label: String,
    title: String,
    /// Whom it is for, in words: "" yours alone, else "You, Léa" (`persons::label`).
    holder: String,
    /// Whom it is for, by person id (`ME` for you), and what names nobody, as written.
    persons: Vec<String>,
    texts: Vec<String>,
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
    /// A notes folder is set: papers have a home.
    store: bool,
    problem: String,
    kinds: Vec<Choice>,
    families: Vec<Family>,
    /// The persons a paper may be for (the form's "For"): {id, name}, you first.
    choices: Vec<serde_json::Value>,
    /// The persons with a paper (the page's chips, after "Everyone").
    chips: Vec<serde_json::Value>,
}

/// A paper as the page, its card and a person's page show it, as JSON.
pub(crate) fn view_json(wallet: &Wallet, paper: &Paper, store: &sioul_core::persons::Persons, local: &sioul_core::links::LocalLinks, own: &[String], today: jiff::civil::Date) -> serde_json::Value {
    serde_json::to_value(view_of(wallet, paper, &store.holders_of(paper, local, own), store, today)).unwrap_or_default()
}

fn view_of(wallet: &Wallet, paper: &Paper, holders: &sioul_core::persons::Holders, store: &sioul_core::persons::Persons, today: jiff::civil::Date) -> PaperView {
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
        holder: crate::persons::label(store, holders),
        persons: holders.persons.clone(),
        texts: holders.texts.clone(),
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
        Err(problem) => return json(&PapersView { store: load_config().notes_root_path().is_some(), problem, kinds, ..PapersView::default() }),
    };
    let today = sioul_core::zone::now().date();
    let config = load_config();
    let of = crate::persons::ForPapers::read(&config);
    let holders: std::collections::BTreeMap<String, sioul_core::persons::Holders> = wallet.papers.iter().map(|p| (p.id.clone(), of.store.holders_of(p, &of.local, &of.own))).collect();
    let label = |p: &Paper| holders.get(&p.id).map(|h| crate::persons::label(&of.store, h)).unwrap_or_default();
    let mut families: Vec<Family> = Vec::new();
    for family in ["identity", "health", "home", "money", "warranty", "other"] {
        let mut papers: Vec<&Paper> = wallet.papers.iter().filter(|p| p.kind.family() == family).collect();
        papers.sort_by(|a, b| (a.kind.id(), label(a), &a.title).cmp(&(b.kind.id(), label(b), &b.title)));
        if !papers.is_empty() {
            let views = papers.into_iter().map(|p| view_of(&wallet, p, &holders[&p.id], &of.store, today)).collect();
            families.push(Family { id: family, label: tr().text(&format!("paper-family-{family}"), None), papers: views });
        }
    }
    let choices = crate::persons::paper_choices(&of.store);
    let chips = crate::persons::chips(&of.store, &holders);
    json(&PapersView { store: true, problem: String::new(), kinds, families, choices, chips })
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
    /// Whom it is for as a text, from a caller that chooses no persons.
    #[serde(default)]
    holder: String,
    /// Whom it is for, by person id ("me" for you; several allowed), when the
    /// form's chooser said: their ties written in Sioul's links, `holder`
    /// written with their names (docs/persons.md).
    #[serde(default, rename = "for")]
    for_persons: Option<Vec<String>>,
    /// Holder texts that name nobody, kept as written beside them.
    #[serde(default)]
    texts: Vec<String>,
    #[serde(default)]
    notes: String,
}

/// A paper saved: new when `id` is empty; returns what went wrong, else "".
pub(crate) fn save(id: &str, edit: &str) -> String {
    let edit: PaperEdit = match serde_json::from_str(edit) {
        Ok(edit) => edit,
        Err(e) => return e.to_string(),
    };
    let wallet = match wallet() {
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
    let holder = match &edit.for_persons {
        None => edit.holder.trim().to_string(),
        // The names older versions show; the ties themselves once the paper has its id.
        Some(chosen) => {
            let store = crate::persons::store();
            let chosen: Vec<String> = chosen.iter().filter(|id| *id == sioul_core::persons::ME || store.get(id).is_some()).cloned().collect();
            let you = crate::persons::own_names(&load_config()).into_iter().next().unwrap_or_else(crate::persons::you);
            sioul_core::persons::holder_text(&store, &chosen, &edit.texts, &you)
        }
    };
    // Set over the wallet as the file holds it now (the file copied first), under its lock, which the sharing takes too.
    let saved = Wallet::change(&wallet.root, |wallet| {
        let old = wallet.get(id).cloned().unwrap_or_default();
        let paper = Paper {
            id: old.id.clone(),
            kind,
            title,
            file,
            issued,
            until,
            holder,
            notes: edit.notes.trim().to_string(),
            // A new end: a new renewal to plan.
            renewal: if old.until == until { old.renewal.clone() } else { String::new() },
            added: old.added.or_else(|| Some(sioul_core::zone::now().date())),
        };
        wallet.put(paper)
    });
    match (saved, &edit.for_persons) {
        (Err(e), _) => e,
        (Ok(id), Some(chosen)) => crate::persons::tie_paper(&id, chosen, &edit.texts).err().unwrap_or_default(),
        (Ok(_), None) => String::new(),
    }
}

/// A paper taken out; its file stays where it is.
pub(crate) fn remove(id: &str) -> String {
    let Some(root) = load_config().notes_root_path() else { return tr().text("papers-no-store", None) };
    if let Err(e) = Wallet::change(&root, |wallet| wallet.remove(id)) {
        return e;
    }
    // Its ties to persons go with it.
    sioul_core::links::LocalLinks::change(&sioul_core::links::LocalLinks::default_path(), |local| sioul_core::persons::tie_paper(local, id, &[sioul_core::persons::ME.to_string()])).err().unwrap_or_default()
}

/// Its renewal, as a task in your usual list (your phone has it): from when
/// renewing starts to the day it ends, with the steps known for its kind.
pub(crate) fn plan_renewal(qt: &QtThread, shared: &Arc<Shared>, id: &str) -> String {
    let wallet = match wallet() {
        Ok(wallet) => wallet,
        Err(e) => return e,
    };
    let Some(paper) = wallet.get(id).cloned() else { return tr().text("papers-gone", None) };
    let (Some(from), Some(until)) = (paper.renew_from(), paper.until) else { return String::new() };
    let today = sioul_core::zone::now().date();
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
        // The task made: its uid set on the paper as the wallet holds it now, under its lock.
        Ok(uid) => Wallet::change(&wallet.root, |wallet| {
            if let Some(mut paper) = wallet.get(id).cloned() {
                paper.renewal = uid;
                wallet.put(paper);
            }
        })
        .err()
        .unwrap_or_default(),
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
