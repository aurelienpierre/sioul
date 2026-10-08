// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Projects: every matter you follow, in one list.
//!
//! A project is work for a client, its time billable, or a matter of your
//! own: a tax return, a health-cover request, a bill to contest. Its detailed
//! record lives outside Sioul, in your notes folder: Markdown files you own
//! (an Obsidian vault, a git repository, a folder of notes). Sioul reads one
//! file at its root, `sioul-projects.toml`, which names each project, points
//! at its files, and says which mail belongs to it (docs/notes-folder.md).
//! Sioul links to the record; it never owns it.
//!
//! The file's first name was `sioul-cases.toml`, its tables `[[case]]`. Both
//! names are still read, and writing keeps the names a file already has, so
//! that a device not yet updated reads it as before. Renaming it is the
//! person's choice, made once every device is updated (`rename_file`).

use crate::card::Card;
use crate::text::{contains_word, fold};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use toml_edit::{DocumentMut, Item, Table};

/// The projects' file, at the root of the notes folder.
pub const FILE: &str = "sioul-projects.toml";
/// The file's first name: still read when only it is there (`file_in`).
pub const OLD_FILE: &str = "sioul-cases.toml";
/// The copy of the old file that renaming keeps beside the new one (`rename_file`).
pub const BEFORE_RENAME: &str = "sioul-cases.toml.before-rename";
/// The table each project is written in: `[[project]]`.
const TABLE: &str = "project";
/// The table's first name, `[[case]]`: a file written before keeps it.
const OLD_TABLE: &str = "case";

/// The projects' file of a notes folder: `sioul-projects.toml`, or
/// `sioul-cases.toml` when only that one is there. With neither, the new
/// name: the first project made makes it.
pub fn file_in(root: &Path) -> PathBuf {
    let (new, old) = (root.join(FILE), root.join(OLD_FILE));
    if !new.exists() && old.exists() { old } else { new }
}

/// Both files are in the notes folder: `sioul-projects.toml` is read, and
/// `sioul-cases.toml` is left as it is (Settings says so).
pub fn both_in(root: &Path) -> bool {
    root.join(FILE).exists() && root.join(OLD_FILE).exists()
}

/// Only the file's first name is there: Settings offers to rename it.
pub fn only_old_in(root: &Path) -> bool {
    !root.join(FILE).exists() && root.join(OLD_FILE).exists()
}

#[derive(Debug, Deserialize)]
struct Manifest {
    #[serde(rename = "project", default)]
    projects: Vec<Project>,
    /// The table's first name, `[[case]]`.
    #[serde(rename = "case", default)]
    older: Vec<Project>,
}

/// The projects a file's text names, under either table's name.
fn read_projects(text: &str) -> Result<Vec<Project>, toml::de::Error> {
    let manifest: Manifest = toml::from_str(text)?;
    let mut projects = manifest.projects;
    projects.extend(manifest.older);
    Ok(projects)
}

/// One project, as its file describes it.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Project {
    /// Short and stable: tasks, events and notes refer to it (RFC 9253 REFID).
    pub id: String,
    pub title: String,
    /// Files of the record, relative to the notes folder's root.
    #[serde(default)]
    pub files: Vec<String>,
    /// "open", "waiting" or "closed"; free text is kept as written.
    #[serde(default)]
    pub status: Option<String>,
    /// Which mail belongs here: a message matches if any route matches.
    #[serde(rename = "route", default)]
    pub routes: Vec<Route>,
    /// "project" for work done for a client, its time billable; unsaid for a
    /// matter of your own (housing, health, a trip).
    #[serde(default)]
    pub kind: Option<String>,
    /// Who it is for: a name, or a contact's address ("sioul:contact/…").
    #[serde(default)]
    pub client: Option<String>,
    /// An hour's fee; the invoice settings' when unsaid.
    #[serde(default)]
    pub rate: Option<f64>,
    /// The budget its invoices are expected in.
    #[serde(default)]
    pub budget: Option<String>,
    /// "personal" for a matter of yours outside work (a garden, a trip): it stays in view in quiet time.
    #[serde(default)]
    pub area: Option<String>,
    /// Open to AI agents (`ai = true`): an agent connected with `sioul mcp`
    /// reads its things and writes into it (`consent`, docs/ai.md). Every
    /// project is closed until you open it: a file written before the field
    /// existed opens none.
    #[serde(default)]
    pub ai: bool,
}

impl Project {
    /// Work done for a client: its time can be billed.
    pub fn is_for_client(&self) -> bool {
        self.kind.as_deref() == Some("project")
    }
}

/// A project as its form gives it.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, Deserialize)]
pub struct ProjectEdit {
    pub title: String,
    /// "project" for work for a client, "" for one of yours.
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub client: String,
    /// 0 when unsaid.
    #[serde(default)]
    pub rate: f64,
    #[serde(default)]
    pub budget: String,
    /// "personal", or "" for work and admin.
    #[serde(default)]
    pub area: String,
    /// Open to AI agents; unsaid (a form that does not show it), the file's value stays.
    #[serde(default)]
    pub ai: Option<bool>,
}

impl ProjectEdit {
    /// A project as its form shows it.
    pub fn of(project: &Project) -> ProjectEdit {
        ProjectEdit {
            title: project.title.clone(),
            kind: project.kind.clone().unwrap_or_default(),
            status: project.status.clone().unwrap_or_default(),
            client: project.client.clone().unwrap_or_default(),
            rate: project.rate.unwrap_or(0.0),
            budget: project.budget.clone().unwrap_or_default(),
            area: project.area.clone().unwrap_or_default(),
            ai: Some(project.ai),
        }
    }
}

/// A short, stable id from a title: "Studio Lumen — site" → "studio-lumen-site";
/// another number when it is taken.
pub fn new_id(title: &str, taken: &[String]) -> String {
    let folded: String = crate::text::fold(title).into_iter().collect();
    let mut id = String::new();
    for c in folded.chars() {
        if c.is_ascii_alphanumeric() {
            id.push(c);
        } else if !id.ends_with('-') && !id.is_empty() {
            id.push('-');
        }
    }
    let id = id.trim_end_matches('-').chars().take(40).collect::<String>();
    let id = if id.is_empty() { "project".to_string() } else { id };
    let mut candidate = id.clone();
    let mut n = 2;
    while taken.iter().any(|t| t == &candidate) {
        candidate = format!("{id}-{n}");
        n += 1;
    }
    candidate
}

/// The file's text as toml_edit holds it, comments and all. A file not there
/// yet reads as empty when `missing_empty`; one there but unreadable (its
/// rights, an encoding other than UTF-8) is an error: written over, every
/// project would go.
fn read_doc(file: &Path, missing_empty: bool) -> Result<DocumentMut, String> {
    let fail = |e: String| format!("{}: {e}", file.display());
    let text = match std::fs::read_to_string(file) {
        Ok(text) => text,
        Err(e) if missing_empty && e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(fail(e.to_string())),
    };
    text.parse().map_err(|e: toml_edit::TomlError| fail(e.to_string()))
}

/// Writes the file whole, through a file beside it, so that it is never half written.
fn write_doc(file: &Path, doc: &DocumentMut) -> Result<(), String> {
    let fail = |e: std::io::Error| format!("{}: {e}", file.display());
    let temporary = file.with_extension("toml.new");
    std::fs::write(&temporary, doc.to_string()).map_err(fail)?;
    std::fs::rename(&temporary, file).map_err(fail)
}

/// The table a file's new projects go in: the one it has, `[[project]]` or
/// `[[case]]`. A file with neither gets `[[case]]` under its first name, so
/// that an older Sioul still reads it, and `[[project]]` otherwise.
fn table_of(doc: &DocumentMut, file: &Path) -> &'static str {
    if doc.contains_key(TABLE) {
        TABLE
    } else if doc.contains_key(OLD_TABLE) || file.file_name().is_some_and(|name| name == OLD_FILE) {
        OLD_TABLE
    } else {
        TABLE
    }
}

fn id_of(table: &Table) -> Option<&str> {
    table.get("id").and_then(Item::as_str)
}

/// Every project's id in the file, under either table's name.
fn ids_in(doc: &DocumentMut) -> Vec<String> {
    [TABLE, OLD_TABLE].iter().filter_map(|name| doc.get(name).and_then(Item::as_array_of_tables)).flat_map(|list| list.iter()).filter_map(|t| id_of(t).map(str::to_string)).collect()
}

/// A project's entry in the file, whichever table holds it.
fn entry_mut<'d>(doc: &'d mut DocumentMut, id: &str) -> Option<&'d mut Table> {
    let name = [TABLE, OLD_TABLE].into_iter().find(|name| doc.get(name).and_then(Item::as_array_of_tables).is_some_and(|list| list.iter().any(|t| id_of(t) == Some(id))))?;
    doc.get_mut(name)?.as_array_of_tables_mut()?.iter_mut().find(|t| id_of(t) == Some(id))
}

/// Writes a project's fields from its form, in place, its comments and routes
/// kept; a new project (`id` not there yet) goes at the end, and an empty
/// `id` is made from its title. Returns its id. `file` is the notes folder's
/// projects' file (`file_in`): a file not there yet is made.
pub fn save_project(file: &Path, id: &str, edit: &ProjectEdit) -> Result<String, String> {
    use toml_edit::{ArrayOfTables, value};
    let fail = |e: String| format!("{}: {e}", file.display());
    let mut doc = read_doc(file, true)?;
    let taken = ids_in(&doc);
    let id = if id.is_empty() { new_id(&edit.title, &taken) } else { id.to_string() };
    if !taken.contains(&id) {
        let name = table_of(&doc, file);
        let list = doc.entry(name).or_insert(Item::ArrayOfTables(ArrayOfTables::new())).as_array_of_tables_mut().ok_or_else(|| fail(format!("`{name}` is not a list of tables")))?;
        let mut table = Table::new();
        table["id"] = value(id.as_str());
        list.push(table);
    }
    let project = entry_mut(&mut doc, &id).ok_or_else(|| fail(format!("no project {id}")))?;
    project["title"] = value(edit.title.trim());
    for (key, text) in [("kind", edit.kind.trim()), ("status", edit.status.trim()), ("client", edit.client.trim()), ("budget", edit.budget.trim()), ("area", edit.area.trim())] {
        if text.is_empty() {
            project.remove(key);
        } else {
            project[key] = value(text);
        }
    }
    if edit.rate > 0.0 {
        project["rate"] = value(edit.rate);
    } else {
        project.remove("rate");
    }
    if let Some(open) = edit.ai {
        write_ai(project, open);
    }
    write_doc(file, &doc)?;
    Ok(id)
}

/// A project open to AI agents says so (`ai = true`); a closed one says
/// nothing, as every project is closed until opened.
fn write_ai(project: &mut Table, open: bool) {
    if open {
        project["ai"] = toml_edit::value(true);
    } else {
        project.remove("ai");
    }
}

/// Opens a project to AI agents, or closes it, in its file, in place: its
/// other fields and the file's comments stay. The sharing carries the
/// project's entry, so the choice holds on every device.
pub fn set_ai(file: &Path, project_id: &str, open: bool) -> Result<(), String> {
    let mut doc = read_doc(file, false)?;
    let project = entry_mut(&mut doc, project_id).ok_or_else(|| format!("{}: no project {project_id}", file.display()))?;
    write_ai(project, open);
    write_doc(file, &doc)
}

/// Renames the file's first name, `sioul-cases.toml`, to
/// `sioul-projects.toml`, and its `[[case]]` tables to `[[project]]`, in one
/// write: every field, route and comment kept, and checked to be so before
/// anything is written. The old file stays beside the new one, as
/// `sioul-cases.toml.before-rename`. Never done by itself: an older Sioul
/// reads only the first name, so the person chooses when (Settings ▸ Your
/// folder and sharing). Refused when `sioul-projects.toml` is already there.
/// Returns where the old file was kept.
pub fn rename_file(root: &Path) -> Result<PathBuf, String> {
    let (old, new) = (root.join(OLD_FILE), root.join(FILE));
    if new.exists() {
        return Err(format!("{}: already there; {OLD_FILE} is left as it is.", new.display()));
    }
    let text = std::fs::read_to_string(&old).map_err(|e| format!("{}: {e}", old.display()))?;
    let fail = |e: String| format!("{}: {e}", old.display());
    let mut doc: DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| fail(e.to_string()))?;
    if let Some(older) = doc.remove(OLD_TABLE) {
        let Item::ArrayOfTables(older) = older else { return Err(fail(format!("`{OLD_TABLE}` is not a list of tables"))) };
        match doc.get_mut(TABLE) {
            None => {
                doc.insert(TABLE, Item::ArrayOfTables(older));
            }
            Some(Item::ArrayOfTables(list)) => {
                for table in older.iter() {
                    list.push(table.clone());
                }
            }
            Some(_) => return Err(fail(format!("`{TABLE}` is not a list of tables"))),
        }
    }
    let renamed = doc.to_string();
    // Nothing lost: the same projects in the same order, every field the
    // same, and the rest of the file as it was.
    let before: toml::Table = text.parse().map_err(|e: toml::de::Error| fail(e.to_string()))?;
    let after: toml::Table = renamed.parse().map_err(|e: toml::de::Error| fail(e.to_string()))?;
    let split = |mut table: toml::Table| {
        let mut list = match table.remove(TABLE) {
            Some(toml::Value::Array(list)) => list,
            _ => Vec::new(),
        };
        if let Some(toml::Value::Array(older)) = table.remove(OLD_TABLE) {
            list.extend(older);
        }
        (list, table)
    };
    if split(before) != split(after) {
        return Err(fail("renaming it would change what it says: it is left as it is".into()));
    }
    std::fs::write(new.with_extension("toml.new"), &renamed).map_err(|e| fail(e.to_string()))?;
    std::fs::rename(new.with_extension("toml.new"), &new).map_err(|e| format!("{}: {e}", new.display()))?;
    let kept = (1..)
        .map(|n| if n == 1 { root.join(BEFORE_RENAME) } else { root.join(format!("{BEFORE_RENAME}-{n}")) })
        .find(|path| !path.exists())
        .unwrap_or_else(|| root.join(BEFORE_RENAME));
    std::fs::rename(&old, &kept).map_err(|e| fail(e.to_string()))?;
    Ok(kept)
}

/// A route matches when every list it fills has at least one match.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Route {
    /// The sender's domain or any of its subdomains: "finances.example" covers "dgfip.finances.example".
    #[serde(default)]
    pub from_domains: Vec<String>,
    #[serde(default)]
    pub from_addresses: Vec<String>,
    /// Whole words or phrases, case and accents ignored.
    #[serde(default)]
    pub subject_contains: Vec<String>,
    #[serde(default)]
    pub text_contains: Vec<String>,
    /// Words in an attachment's file name: "devis", "facture", ".ics".
    #[serde(default)]
    pub attachment_contains: Vec<String>,
}

/// Which field of a message matched a route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteField {
    SenderDomain,
    Sender,
    Subject,
    Text,
    Attachment,
    /// Not a route: a message of the same conversation belongs to the project.
    Thread,
}

/// Whether a file name holds a word, case and accents aside: names are not prose.
fn name_contains(name: &str, word: &str) -> bool {
    let folded = |s: &str| fold(s.trim()).into_iter().flat_map(char::to_lowercase).collect::<String>();
    let word = folded(word);
    !word.is_empty() && folded(name).contains(&word)
}

/// One condition of a route that a message met: the field and the value it matched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteMatch {
    pub field: RouteField,
    pub value: String,
}

impl Route {
    /// What in the card this route matched; none if it does not take the card.
    pub fn explain(&self, card: &Card) -> Option<Vec<RouteMatch>> {
        if self.is_empty() {
            return None;
        }
        let domain = card.sender_domain().unwrap_or("");
        let address = card.from_address.as_deref().unwrap_or("");
        let checks = [
            any_match(&self.from_domains, |d| domain_matches(domain, d), RouteField::SenderDomain),
            any_match(&self.from_addresses, |a| address.eq_ignore_ascii_case(a), RouteField::Sender),
            any_match(&self.subject_contains, |w| contains_word(&card.subject, w), RouteField::Subject),
            any_match(&self.text_contains, |w| contains_word(&card.excerpt, w), RouteField::Text),
            any_match(&self.attachment_contains, |w| card.attachments.iter().any(|name| name_contains(name, w)), RouteField::Attachment),
        ];
        let mut matched = Vec::new();
        for check in checks {
            match check {
                Check::Unused => {}
                Check::Matched(m) => matched.push(m),
                Check::Failed => return None,
            }
        }
        Some(matched)
    }

    /// Whether a message with this sender and subject is taken, as far as
    /// they tell: a route that needs words in the text or an attachment never
    /// matches here (such mail is tied to its project when it arrives).
    pub fn takes_header(&self, address: &str, subject: &str) -> bool {
        if self.is_empty() || !self.text_contains.is_empty() || !self.attachment_contains.is_empty() {
            return false;
        }
        let domain = address.rsplit_once('@').map_or("", |(_, d)| d);
        (self.from_domains.is_empty() || self.from_domains.iter().any(|d| domain_matches(domain, d)))
            && (self.from_addresses.is_empty() || self.from_addresses.iter().any(|a| address.eq_ignore_ascii_case(a)))
            && (self.subject_contains.is_empty() || self.subject_contains.iter().any(|w| contains_word(subject, w)))
    }

    fn is_empty(&self) -> bool {
        self.from_domains.is_empty()
            && self.from_addresses.is_empty()
            && self.subject_contains.is_empty()
            && self.text_contains.is_empty()
            && self.attachment_contains.is_empty()
    }

    /// Whether only the whole message tells: words in its text or an attachment.
    pub fn needs_body(&self) -> bool {
        !self.text_contains.is_empty() || !self.attachment_contains.is_empty()
    }
}

enum Check {
    Unused,
    Matched(RouteMatch),
    Failed,
}

fn any_match(list: &[String], test: impl Fn(&str) -> bool, field: RouteField) -> Check {
    if list.is_empty() {
        return Check::Unused;
    }
    list.iter()
        .find(|item| test(item))
        .map_or(Check::Failed, |item| Check::Matched(RouteMatch { field, value: item.clone() }))
}

/// "dgfip.finances.example" matches "finances.example"; "notfinances.example" does not.
fn domain_matches(domain: &str, pattern: &str) -> bool {
    let domain = domain.to_ascii_lowercase();
    let pattern = pattern.trim_start_matches('@').to_ascii_lowercase();
    domain == pattern || domain.ends_with(&format!(".{pattern}"))
}


/// The projects of one notes folder.
#[derive(Debug, Clone)]
pub struct ProjectStore {
    pub root: PathBuf,
    pub projects: Vec<Project>,
    /// Messages tied to a project, by Message-ID (bare): by hand, or when they
    /// arrived. A reply in their conversation goes to the same project.
    pub ties: std::collections::BTreeMap<String, String>,
}

/// A project a message was routed to, and what matched.
#[derive(Debug, Clone)]
pub struct Routing<'a> {
    pub project: &'a Project,
    pub matched: Vec<RouteMatch>,
}

impl ProjectStore {
    /// Reads the projects' file at the root of a notes folder
    /// (`file_in`: `sioul-projects.toml`, or `sioul-cases.toml`).
    pub fn load(root: &Path) -> Result<ProjectStore, String> {
        let path = file_in(root);
        let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let projects = read_projects(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(ProjectStore { root: root.to_path_buf(), projects, ties: Default::default() })
    }

    /// The file its projects are written to (`file_in`).
    pub fn file(&self) -> PathBuf {
        file_in(&self.root)
    }

    /// With the messages tied to projects, from the links Sioul keeps.
    pub fn with_ties(mut self, links: &crate::links::LocalLinks) -> ProjectStore {
        self.ties = crate::links::mail_projects(links);
        self
    }

    /// Every project a message belongs to, in the file's order: the first is
    /// its home. Without a route, a message tied to a project, or answering
    /// or citing one that is, belongs to it.
    pub fn route(&self, card: &Card) -> Vec<Routing<'_>> {
        let routed: Vec<Routing<'_>> = self
            .projects
            .iter()
            .filter_map(|project| {
                let matched = project.routes.iter().find_map(|r| r.explain(card))?;
                Some(Routing { project, matched })
            })
            .collect();
        if !routed.is_empty() || self.ties.is_empty() {
            return routed;
        }
        crate::threads::ids_of(card)
            .iter()
            .find_map(|id| self.ties.get(id))
            .and_then(|id| self.get(id))
            .map(|project| Routing { project, matched: vec![RouteMatch { field: RouteField::Thread, value: String::new() }] })
            .into_iter()
            .collect()
    }

    /// Files the projects' file names that are not there (moved or renamed).
    pub fn missing_files(&self) -> Vec<(String, String)> {
        self.projects
            .iter()
            .flat_map(|p| p.files.iter().map(move |f| (p.id.clone(), f.clone())))
            .filter(|(_, f)| !self.root.join(f).exists())
            .collect()
    }

    /// A project by id.
    pub fn get(&self, id: &str) -> Option<&Project> {
        self.projects.iter().find(|p| fold(&p.id) == fold(id))
    }
}

/// A route as the settings edit it: each list may be empty.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, Deserialize)]
pub struct RouteValue {
    #[serde(default)]
    pub from_domains: Vec<String>,
    #[serde(default)]
    pub from_addresses: Vec<String>,
    #[serde(default)]
    pub subject_contains: Vec<String>,
    #[serde(default)]
    pub text_contains: Vec<String>,
    #[serde(default)]
    pub attachment_contains: Vec<String>,
}

impl From<&Route> for RouteValue {
    fn from(route: &Route) -> RouteValue {
        RouteValue {
            from_domains: route.from_domains.clone(),
            from_addresses: route.from_addresses.clone(),
            subject_contains: route.subject_contains.clone(),
            text_contains: route.text_contains.clone(),
            attachment_contains: route.attachment_contains.clone(),
        }
    }
}


/// Takes a project out of its file, the file's comments elsewhere kept. Its
/// tasks, notes, mail and time stay; they are no longer gathered under it.
pub fn remove_project(file: &Path, project_id: &str) -> Result<(), String> {
    let mut doc = read_doc(file, false)?;
    let mut found = false;
    for name in [TABLE, OLD_TABLE] {
        if let Some(list) = doc.get_mut(name).and_then(Item::as_array_of_tables_mut) {
            let before = list.len();
            list.retain(|t| id_of(t) != Some(project_id));
            found |= list.len() != before;
        }
    }
    if !found {
        return Err(format!("{}: no project {project_id}", file.display()));
    }
    write_doc(file, &doc)
}

/// Replaces a project's routes in its file, in place: its other fields and
/// the file's comments stay. Routes left empty are dropped.
pub fn set_routes(file: &Path, project_id: &str, routes: &[RouteValue]) -> Result<(), String> {
    use toml_edit::{Array, ArrayOfTables, value};
    let mut doc = read_doc(file, false)?;
    let project = entry_mut(&mut doc, project_id).ok_or_else(|| format!("{}: no project {project_id}", file.display()))?;
    let mut tables = ArrayOfTables::new();
    for route in routes {
        let mut table = Table::new();
        for (key, list) in [
            ("from_domains", &route.from_domains),
            ("from_addresses", &route.from_addresses),
            ("subject_contains", &route.subject_contains),
            ("text_contains", &route.text_contains),
            ("attachment_contains", &route.attachment_contains),
        ] {
            let cleaned: Vec<&str> = list.iter().map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
            if !cleaned.is_empty() {
                table[key] = value(cleaned.into_iter().collect::<Array>());
            }
        }
        if !table.is_empty() {
            tables.push(table);
        }
    }
    if tables.is_empty() {
        project.remove("route");
    } else {
        project["route"] = Item::ArrayOfTables(tables);
    }
    write_doc(file, &doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An empty folder of its own for a test, under the system's temporary folder.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sioul-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn projects_are_written_in_place() {
        let dir = scratch("projects");
        let file = dir.join(FILE);
        std::fs::write(&file, "# My projects\n[[project]]\nid = \"housing\"\ntitle = \"Housing\"\n\n[[project.route]]\nfrom_domains = [\"caf.example\"]\n").unwrap();
        let edit = ProjectEdit { title: "Studio Lumen — site".into(), kind: "project".into(), client: "Studio Lumen".into(), rate: 60.0, ..ProjectEdit::default() };
        let id = save_project(&file, "", &edit).unwrap();
        assert_eq!(id, "studio-lumen-site");
        let store = ProjectStore::load(&dir).unwrap();
        let lumen = store.get(&id).unwrap();
        assert!(lumen.is_for_client());
        assert_eq!((lumen.client.as_deref(), lumen.rate), (Some("Studio Lumen"), Some(60.0)));
        // Edited in place: the routes and the comment stay.
        save_project(&file, "housing", &ProjectEdit { title: "Housing".into(), status: "waiting".into(), ..ProjectEdit::default() }).unwrap();
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(text.starts_with("# My projects"));
        assert!(text.contains("from_domains = [\"caf.example\"]"));
        assert_eq!(ProjectStore::load(&dir).unwrap().get("housing").unwrap().status.as_deref(), Some("waiting"));
        assert_eq!(new_id("Studio Lumen — site", &[id]), "studio-lumen-site-2");
        // A file that cannot be read (here, not UTF-8) is never written over.
        let broken = b"# Mes projets\n[[project]]\nid = \"caf\xe9\"\n".to_vec();
        std::fs::write(&file, &broken).unwrap();
        assert!(save_project(&file, "", &ProjectEdit { title: "New".into(), ..ProjectEdit::default() }).is_err());
        assert_eq!(std::fs::read(&file).unwrap(), broken);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_old_file_reads_and_is_written_as_before() {
        let dir = scratch("old-file");
        let old = dir.join(OLD_FILE);
        std::fs::write(&old, "# My cases\n[[case]]\nid = \"taxes\"\ntitle = \"Taxes\"\n[[case.route]]\nfrom_domains = [\"finances.example\"]\n").unwrap();
        assert_eq!(file_in(&dir), old);
        assert!(only_old_in(&dir) && !both_in(&dir));
        let store = ProjectStore::load(&dir).unwrap();
        assert_eq!((store.projects.len(), store.projects[0].routes[0].from_domains[0].as_str()), (1, "finances.example"));
        // Everything written keeps its names: an older Sioul still reads it.
        save_project(&store.file(), "", &ProjectEdit { title: "Lease".into(), ..ProjectEdit::default() }).unwrap();
        set_ai(&store.file(), "taxes", true).unwrap();
        set_routes(&store.file(), "lease", &[RouteValue { from_domains: vec!["landlord.example".into()], ..RouteValue::default() }]).unwrap();
        let text = std::fs::read_to_string(&old).unwrap();
        assert!(!dir.join(FILE).exists());
        assert!(text.starts_with("# My cases") && !text.contains("[[project") && text.matches("[[case]]").count() == 2 && text.contains("[[case.route]]"), "{text}");
        let store = ProjectStore::load(&dir).unwrap();
        assert_eq!(store.projects.iter().map(|p| (p.id.as_str(), p.ai, p.routes.len())).collect::<Vec<_>>(), [("taxes", true, 1), ("lease", false, 1)]);
        remove_project(&old, "lease").unwrap();
        assert!(ProjectStore::load(&dir).unwrap().get("lease").is_none());
        assert!(remove_project(&old, "lease").is_err());
        // An old file with no project yet: its first one is written as an older Sioul reads it.
        std::fs::write(&old, "# Nothing yet.\n").unwrap();
        save_project(&old, "", &ProjectEdit { title: "Garden".into(), ..ProjectEdit::default() }).unwrap();
        assert!(std::fs::read_to_string(&old).unwrap().contains("[[case]]"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_new_folder_gets_the_new_names() {
        let dir = scratch("new-file");
        assert_eq!(file_in(&dir), dir.join(FILE));
        assert!(ProjectStore::load(&dir).is_err());
        save_project(&file_in(&dir), "", &ProjectEdit { title: "Garden".into(), ..ProjectEdit::default() }).unwrap();
        let text = std::fs::read_to_string(dir.join(FILE)).unwrap();
        assert!(text.contains("[[project]]") && !text.contains("case"), "{text}");
        assert_eq!(ProjectStore::load(&dir).unwrap().projects[0].id, "garden");
        // Both files there: the new one is read, and the old one left as it is.
        std::fs::write(dir.join(OLD_FILE), "[[case]]\nid = \"old\"\ntitle = \"Old\"\n").unwrap();
        assert!(both_in(&dir) && !only_old_in(&dir));
        assert_eq!(ProjectStore::load(&dir).unwrap().projects.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), ["garden"]);
        // A file holding both tables, edited by hand: every project is read and found.
        std::fs::write(dir.join(FILE), "[[project]]\nid = \"a\"\ntitle = \"A\"\n\n[[case]]\nid = \"b\"\ntitle = \"B\"\n").unwrap();
        assert_eq!(ProjectStore::load(&dir).unwrap().projects.len(), 2);
        set_ai(&dir.join(FILE), "b", true).unwrap();
        assert!(ProjectStore::load(&dir).unwrap().get("b").unwrap().ai);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn renaming_keeps_everything_and_a_copy() {
        let dir = scratch("rename");
        let old = dir.join(OLD_FILE);
        let text = "# My cases: kept as I wrote them.\n\n[[case]] # the taxes\nid = \"taxes\"\ntitle = \"Taxes\"\nfiles = [\"admin/taxes.md\"]\nstatus = \"open\"\n\n[[case.route]]\nfrom_domains = [\"finances.example\"]\nsubject_contains = [\"avis\"]\n\n[[case.route]]\nattachment_contains = [\"devis\"]\n\n# A client.\n[[case]]\nid = \"lumen\"\ntitle = \"Studio Lumen\"\nkind = \"project\"\nclient = \"sioul:contact/lumen\"\nrate = 60.5\nbudget = \"work\"\narea = \"personal\"\nai = true\n";
        std::fs::write(&old, text).unwrap();
        let before = ProjectStore::load(&dir).unwrap().projects;
        assert_eq!(before.len(), 2);
        let kept = rename_file(&dir).unwrap();
        // The copy, beside it, byte for byte; the old name gone; the new one read.
        assert_eq!((kept.clone(), std::fs::read_to_string(&kept).unwrap()), (dir.join(BEFORE_RENAME), text.to_string()));
        assert!(!old.exists() && file_in(&dir) == dir.join(FILE));
        let renamed = std::fs::read_to_string(dir.join(FILE)).unwrap();
        assert!(renamed.starts_with("# My cases: kept as I wrote them.") && renamed.contains("[[project]] # the taxes") && renamed.contains("# A client.\n[[project]]"), "{renamed}");
        assert!(renamed.matches("[[project.route]]").count() == 2 && !renamed.contains("[[case"), "{renamed}");
        // Every field, the AI field included, in the same order.
        assert_eq!(ProjectStore::load(&dir).unwrap().projects, before);
        assert!(ProjectStore::load(&dir).unwrap().get("lumen").unwrap().ai);
        // Never twice, and never over a file of the new name.
        assert!(rename_file(&dir).is_err());
        std::fs::write(&old, text).unwrap();
        assert!(rename_file(&dir).is_err());
        assert_eq!(std::fs::read_to_string(&old).unwrap(), text);
        // A second renaming, later, keeps the first copy.
        std::fs::remove_file(dir.join(FILE)).unwrap();
        assert_eq!(rename_file(&dir).unwrap(), dir.join(format!("{BEFORE_RENAME}-2")));
        assert_eq!(std::fs::read_to_string(dir.join(BEFORE_RENAME)).unwrap(), text);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn routes_are_written_in_place() {
        let dir = scratch("routes");
        let path = dir.join(FILE);
        std::fs::write(&path, "# My projects.\n[[project]]\nid = \"taxes\"\ntitle = \"Taxes\"\n[[project.route]]\nfrom_domains = [\"old.example\"]\n\n[[project]]\nid = \"bill\"\ntitle = \"Bill\"\n").unwrap();
        set_routes(&path, "taxes", &[RouteValue { from_domains: vec!["finances.example".into()], subject_contains: vec!["avis".into()], ..RouteValue::default() }, RouteValue::default()]).unwrap();
        let store = ProjectStore::load(&dir).unwrap();
        assert_eq!(store.projects[0].routes.len(), 1);
        assert_eq!((store.projects[0].routes[0].from_domains.clone(), store.projects[0].routes[0].subject_contains.clone()), (vec!["finances.example".to_string()], vec!["avis".to_string()]));
        assert_eq!(store.projects[1].title, "Bill");
        assert!(std::fs::read_to_string(&path).unwrap().starts_with("# My projects."));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn projects_are_closed_to_agents_until_opened() {
        let dir = scratch("ai");
        let path = dir.join(FILE);
        // A file written before the field: every project closed.
        std::fs::write(&path, "# Mine.\n[[project]]\nid = \"taxes\"\ntitle = \"Taxes\"\n\n[[project]]\nid = \"lumen\"\ntitle = \"Studio Lumen\"\nkind = \"project\"\n").unwrap();
        assert!(ProjectStore::load(&dir).unwrap().projects.iter().all(|p| !p.ai));
        set_ai(&path, "lumen", true).unwrap();
        let store = ProjectStore::load(&dir).unwrap();
        assert_eq!(store.projects.iter().map(|p| (p.id.as_str(), p.ai)).collect::<Vec<_>>(), [("taxes", false), ("lumen", true)]);
        assert!(std::fs::read_to_string(&path).unwrap().starts_with("# Mine."));
        // A form that does not say it keeps it; one that does changes it.
        save_project(&path, "lumen", &ProjectEdit { title: "Studio Lumen".into(), kind: "project".into(), ..ProjectEdit::default() }).unwrap();
        assert!(ProjectStore::load(&dir).unwrap().get("lumen").unwrap().ai);
        assert_eq!(ProjectEdit::of(ProjectStore::load(&dir).unwrap().get("lumen").unwrap()).ai, Some(true));
        save_project(&path, "lumen", &ProjectEdit { title: "Studio Lumen".into(), ai: Some(false), ..ProjectEdit::default() }).unwrap();
        assert!(!ProjectStore::load(&dir).unwrap().get("lumen").unwrap().ai);
        // Closed writes nothing: absent is closed.
        assert!(!std::fs::read_to_string(&path).unwrap().contains("ai ="));
        assert!(set_ai(&path, "nowhere", true).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn card(from: &str, subject: &str) -> Card {
        let raw = format!("From: {from}\r\nSubject: {subject}\r\n\r\nBonjour.\r\n");
        Card::from_bytes(raw.as_bytes()).unwrap()
    }

    fn store() -> ProjectStore {
        let projects = read_projects(
            r#"
            [[project]]
            id = "taxes"
            title = "Taxes"
            [[project.route]]
            from_domains = ["finances.example"]

            [[project]]
            id = "bill"
            title = "The canteen bill"
            [[project.route]]
            from_domains = ["finances.example"]
            subject_contains = ["titre", "avis des sommes"]
            "#,
        )
        .unwrap();
        ProjectStore { root: PathBuf::from("."), projects, ties: Default::default() }
    }

    #[test]
    fn routes_by_domain_and_subject() {
        let s = store();
        let notice = card("DGFiP <ne-pas-repondre@dgfip.finances.example>", "Votre avis d'impôt");
        let ids: Vec<&str> = s.route(&notice).iter().map(|r| r.project.id.as_str()).collect();
        assert_eq!(ids, vec!["taxes"]);
        let bill = card("<noreply@dgfip.finances.example>", "Avis des sommes à payer");
        let ids: Vec<&str> = s.route(&bill).iter().map(|r| r.project.id.as_str()).collect();
        assert_eq!(ids, vec!["taxes", "bill"]);
    }

    #[test]
    fn attachments_and_conversations() {
        let projects = read_projects("[[case]]\nid = \"lumen\"\ntitle = \"Studio Lumen\"\n[[case.route]]\nattachment_contains = [\"devis\"]\n").unwrap();
        let mut store = ProjectStore { root: PathBuf::from("."), projects, ties: Default::default() };
        let quote = "From: Jane <jane@lumen.example.net>\r\nSubject: Our offer\r\nMessage-ID: <q1@lumen.example.net>\r\nMIME-Version: 1.0\r\n\
            Content-Type: multipart/mixed; boundary=\"b\"\r\n\r\n--b\r\nContent-Type: text/plain\r\n\r\nHere it is.\r\n--b\r\n\
            Content-Type: application/pdf\r\nContent-Disposition: attachment; filename=\"Devis_Lumen_2026.pdf\"\r\n\r\nJVBERi0=\r\n--b--\r\n";
        let quote = Card::from_bytes(quote.as_bytes()).unwrap();
        let routed = store.route(&quote);
        assert_eq!(routed.first().map(|r| (r.project.id.as_str(), r.matched[0].field)), Some(("lumen", RouteField::Attachment)));
        // The answer has no attachment: it follows the quote once the quote is tied to the project.
        let answer = Card::from_bytes(b"From: Jane <jane@lumen.example.net>\r\nSubject: Re: Our offer\r\nIn-Reply-To: <q1@lumen.example.net>\r\n\r\nAny question?\r\n").unwrap();
        assert!(store.route(&answer).is_empty());
        store.ties.insert("q1@lumen.example.net".into(), "lumen".into());
        assert_eq!(store.route(&answer).first().map(|r| (r.project.id.as_str(), r.matched[0].field)), Some(("lumen", RouteField::Thread)));
    }

    #[test]
    fn domain_boundaries() {
        assert!(domain_matches("dgfip.finances.example", "finances.example"));
        assert!(!domain_matches("notfinances.example", "finances.example"));
        assert!(domain_matches("insurer.example", "@insurer.example"));
    }
}
