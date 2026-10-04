// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Cases: the dossiers your admin is organised by.
//!
//! The detailed record of a case lives outside Sioul, in a folder of Markdown
//! files you own (the case store: an Obsidian vault, a git repository, a home
//! folder of notes). Sioul reads one manifest at its root, `sioul-cases.toml`,
//! which names each case, points at its files, and says which mail belongs to
//! it (docs/case-store.md). Sioul links to the record; it never owns it.

use crate::card::Card;
use crate::text::{contains_word, fold};
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// The manifest's file name, at the root of the case store.
pub const MANIFEST: &str = "sioul-cases.toml";

#[derive(Debug, Deserialize)]
struct Manifest {
    #[serde(rename = "case", default)]
    cases: Vec<Case>,
}

/// One case, as the manifest describes it.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Case {
    /// Short and stable: tasks, events and notes refer to it (RFC 9253 REFID).
    pub id: String,
    pub title: String,
    /// Files of the record, relative to the case store's root.
    #[serde(default)]
    pub files: Vec<String>,
    /// "open", "waiting" or "closed"; free text is kept as written.
    #[serde(default)]
    pub status: Option<String>,
    /// Which mail belongs here: a message matches if any route matches.
    #[serde(rename = "route", default)]
    pub routes: Vec<Route>,
    /// "project" for work done for someone, its time billable; a case otherwise.
    #[serde(default)]
    pub kind: Option<String>,
    /// Who a project is for: a name, or a contact's address ("sioul:contact/…").
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
}

impl Case {
    /// Work done for someone: its time can be billed.
    pub fn is_project(&self) -> bool {
        self.kind.as_deref() == Some("project")
    }
}

/// A case or project as its form gives it.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, Deserialize)]
pub struct CaseEdit {
    pub title: String,
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
}

impl CaseEdit {
    /// A case as its form shows it.
    pub fn of(case: &Case) -> CaseEdit {
        CaseEdit {
            title: case.title.clone(),
            kind: case.kind.clone().unwrap_or_default(),
            status: case.status.clone().unwrap_or_default(),
            client: case.client.clone().unwrap_or_default(),
            rate: case.rate.unwrap_or(0.0),
            budget: case.budget.clone().unwrap_or_default(),
            area: case.area.clone().unwrap_or_default(),
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
    let id = if id.is_empty() { "case".to_string() } else { id };
    let mut candidate = id.clone();
    let mut n = 2;
    while taken.iter().any(|t| t == &candidate) {
        candidate = format!("{id}-{n}");
        n += 1;
    }
    candidate
}

/// Writes a case's fields from its form, in place, its comments and routes
/// kept; a new case (`id` not there yet) goes at the end. Returns its id.
pub fn save_case(manifest: &Path, id: &str, edit: &CaseEdit) -> Result<String, String> {
    use toml_edit::{ArrayOfTables, DocumentMut, Item, Table, value};
    let fail = |e: String| format!("{}: {e}", manifest.display());
    // No manifest yet: the first case makes it. One there but unreadable (its
    // rights, an encoding other than UTF-8) is left alone: written over, every case would go.
    let text = match std::fs::read_to_string(manifest) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(fail(e.to_string())),
    };
    let mut doc: DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| fail(e.to_string()))?;
    let cases = doc.entry("case").or_insert(Item::ArrayOfTables(ArrayOfTables::new())).as_array_of_tables_mut().ok_or_else(|| fail("`case` is not a list of tables".into()))?;
    let taken: Vec<String> = cases.iter().filter_map(|t| t.get("id").and_then(Item::as_str).map(str::to_string)).collect();
    let id = if id.is_empty() { new_id(&edit.title, &taken) } else { id.to_string() };
    if !taken.contains(&id) {
        let mut table = Table::new();
        table["id"] = value(id.as_str());
        cases.push(table);
    }
    let case = cases.iter_mut().find(|t| t.get("id").and_then(Item::as_str) == Some(id.as_str())).ok_or_else(|| fail(format!("no case {id}")))?;
    case["title"] = value(edit.title.trim());
    for (key, text) in [("kind", edit.kind.trim()), ("status", edit.status.trim()), ("client", edit.client.trim()), ("budget", edit.budget.trim()), ("area", edit.area.trim())] {
        if text.is_empty() {
            case.remove(key);
        } else {
            case[key] = value(text);
        }
    }
    if edit.rate > 0.0 {
        case["rate"] = value(edit.rate);
    } else {
        case.remove("rate");
    }
    let temporary = manifest.with_extension("toml.new");
    std::fs::write(&temporary, doc.to_string()).map_err(|e| fail(e.to_string()))?;
    std::fs::rename(&temporary, manifest).map_err(|e| fail(e.to_string()))?;
    Ok(id)
}

/// A route matches when every list it fills has at least one match.
#[derive(Debug, Clone, Default, Deserialize)]
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
    /// Not a route: a message of the same conversation belongs to the case.
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
    /// matches here (such mail is tied to its case when it arrives).
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

/// The cases of one case store.
#[derive(Debug, Clone)]
pub struct CaseStore {
    pub root: PathBuf,
    pub cases: Vec<Case>,
    /// Messages tied to a case, by Message-ID (bare): by hand, or when they
    /// arrived. A reply in their conversation goes to the same case.
    pub ties: std::collections::BTreeMap<String, String>,
}

/// A case a message was routed to, and what matched.
#[derive(Debug, Clone)]
pub struct Routing<'a> {
    pub case: &'a Case,
    pub matched: Vec<RouteMatch>,
}

impl CaseStore {
    /// Reads `sioul-cases.toml` at the root of a case store.
    pub fn load(root: &Path) -> Result<CaseStore, String> {
        let path = root.join(MANIFEST);
        let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let manifest: Manifest = toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(CaseStore { root: root.to_path_buf(), cases: manifest.cases, ties: Default::default() })
    }

    /// With the messages tied to cases, from the links Sioul keeps.
    pub fn with_ties(mut self, links: &crate::links::LocalLinks) -> CaseStore {
        self.ties = crate::links::mail_cases(links);
        self
    }

    /// Every case a message belongs to, in manifest order: the first is its
    /// home. Without a route, a message tied to a case, or answering or citing
    /// one that is, belongs to it.
    pub fn route(&self, card: &Card) -> Vec<Routing<'_>> {
        let routed: Vec<Routing<'_>> = self
            .cases
            .iter()
            .filter_map(|case| {
                let matched = case.routes.iter().find_map(|r| r.explain(card))?;
                Some(Routing { case, matched })
            })
            .collect();
        if !routed.is_empty() || self.ties.is_empty() {
            return routed;
        }
        crate::threads::ids_of(card)
            .iter()
            .find_map(|id| self.ties.get(id))
            .and_then(|id| self.get(id))
            .map(|case| Routing { case, matched: vec![RouteMatch { field: RouteField::Thread, value: String::new() }] })
            .into_iter()
            .collect()
    }

    /// Files the manifest names that are not there (moved or renamed).
    pub fn missing_files(&self) -> Vec<(String, String)> {
        self.cases
            .iter()
            .flat_map(|c| c.files.iter().map(move |f| (c.id.clone(), f.clone())))
            .filter(|(_, f)| !self.root.join(f).exists())
            .collect()
    }

    /// A case by id.
    pub fn get(&self, id: &str) -> Option<&Case> {
        self.cases.iter().find(|c| fold(&c.id) == fold(id))
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

/// Takes a case out of the manifest, its comments elsewhere kept. Its tasks,
/// notes, mail and time stay; they are no longer gathered under it.
pub fn remove_case(manifest: &Path, case_id: &str) -> Result<(), String> {
    use toml_edit::{DocumentMut, Item};
    let fail = |e: String| format!("{}: {e}", manifest.display());
    let text = std::fs::read_to_string(manifest).map_err(|e| fail(e.to_string()))?;
    let mut doc: DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| fail(e.to_string()))?;
    let cases = doc.get_mut("case").and_then(Item::as_array_of_tables_mut).ok_or_else(|| fail(format!("no case {case_id}")))?;
    let before = cases.len();
    cases.retain(|t| t.get("id").and_then(Item::as_str) != Some(case_id));
    if cases.len() == before {
        return Err(fail(format!("no case {case_id}")));
    }
    let temporary = manifest.with_extension("toml.new");
    std::fs::write(&temporary, doc.to_string()).map_err(|e| fail(e.to_string()))?;
    std::fs::rename(&temporary, manifest).map_err(|e| fail(e.to_string()))
}

/// Replaces a case's routes in the manifest, in place: its other fields and
/// the file's comments stay. Routes left empty are dropped.
pub fn set_routes(manifest: &Path, case_id: &str, routes: &[RouteValue]) -> Result<(), String> {
    use toml_edit::{Array, ArrayOfTables, DocumentMut, Item, Table, value};
    let fail = |e: String| format!("{}: {e}", manifest.display());
    let text = std::fs::read_to_string(manifest).map_err(|e| fail(e.to_string()))?;
    let mut doc: DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| fail(e.to_string()))?;
    let case = doc
        .get_mut("case")
        .and_then(Item::as_array_of_tables_mut)
        .and_then(|cases| cases.iter_mut().find(|t| t.get("id").and_then(Item::as_str) == Some(case_id)))
        .ok_or_else(|| fail(format!("no case {case_id}")))?;
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
        case.remove("route");
    } else {
        case["route"] = Item::ArrayOfTables(tables);
    }
    let temporary = manifest.with_extension("toml.new");
    std::fs::write(&temporary, doc.to_string()).map_err(|e| fail(e.to_string()))?;
    std::fs::rename(&temporary, manifest).map_err(|e| fail(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projects_are_cases_with_a_client() {
        let dir = std::env::temp_dir().join(format!("sioul-projects-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let manifest = dir.join(MANIFEST);
        std::fs::write(&manifest, "# My cases\n[[case]]\nid = \"housing\"\ntitle = \"Housing\"\n\n[[case.route]]\nfrom_domains = [\"caf.example\"]\n").unwrap();
        let edit = CaseEdit { title: "Studio Lumen — site".into(), kind: "project".into(), client: "Studio Lumen".into(), rate: 60.0, ..CaseEdit::default() };
        let id = save_case(&manifest, "", &edit).unwrap();
        assert_eq!(id, "studio-lumen-site");
        let store = CaseStore::load(&dir).unwrap();
        let project = store.get(&id).unwrap();
        assert!(project.is_project());
        assert_eq!((project.client.as_deref(), project.rate), (Some("Studio Lumen"), Some(60.0)));
        // Edited in place: the routes and the comment stay.
        save_case(&manifest, "housing", &CaseEdit { title: "Housing".into(), status: "waiting".into(), ..CaseEdit::default() }).unwrap();
        let text = std::fs::read_to_string(&manifest).unwrap();
        assert!(text.starts_with("# My cases"));
        assert!(text.contains("from_domains = [\"caf.example\"]"));
        assert_eq!(CaseStore::load(&dir).unwrap().get("housing").unwrap().status.as_deref(), Some("waiting"));
        assert_eq!(new_id("Studio Lumen — site", &[id]), "studio-lumen-site-2");
        // A manifest that cannot be read (here, not UTF-8) is never written over.
        let broken = b"# Mes dossiers\n[[case]]\nid = \"caf\xe9\"\n".to_vec();
        std::fs::write(&manifest, &broken).unwrap();
        assert!(save_case(&manifest, "", &CaseEdit { title: "New".into(), ..CaseEdit::default() }).is_err());
        assert_eq!(std::fs::read(&manifest).unwrap(), broken);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn routes_are_written_in_place() {
        let dir = std::env::temp_dir().join(format!("sioul-routes-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(MANIFEST);
        std::fs::write(&path, "# My cases.\n[[case]]\nid = \"taxes\"\ntitle = \"Taxes\"\n[[case.route]]\nfrom_domains = [\"old.example\"]\n\n[[case]]\nid = \"bill\"\ntitle = \"Bill\"\n").unwrap();
        set_routes(&path, "taxes", &[RouteValue { from_domains: vec!["finances.example".into()], subject_contains: vec!["avis".into()], ..RouteValue::default() }, RouteValue::default()]).unwrap();
        let store = CaseStore::load(&dir).unwrap();
        assert_eq!(store.cases[0].routes.len(), 1);
        assert_eq!((store.cases[0].routes[0].from_domains.clone(), store.cases[0].routes[0].subject_contains.clone()), (vec!["finances.example".to_string()], vec!["avis".to_string()]));
        assert_eq!(store.cases[1].title, "Bill");
        assert!(std::fs::read_to_string(&path).unwrap().starts_with("# My cases."));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn card(from: &str, subject: &str) -> Card {
        let raw = format!("From: {from}\r\nSubject: {subject}\r\n\r\nBonjour.\r\n");
        Card::from_bytes(raw.as_bytes()).unwrap()
    }

    fn store() -> CaseStore {
        let manifest: Manifest = toml::from_str(
            r#"
            [[case]]
            id = "taxes"
            title = "Taxes"
            [[case.route]]
            from_domains = ["finances.example"]

            [[case]]
            id = "bill"
            title = "The canteen bill"
            [[case.route]]
            from_domains = ["finances.example"]
            subject_contains = ["titre", "avis des sommes"]
            "#,
        )
        .unwrap();
        CaseStore { root: PathBuf::from("."), cases: manifest.cases, ties: Default::default() }
    }

    #[test]
    fn routes_by_domain_and_subject() {
        let s = store();
        let notice = card("DGFiP <ne-pas-repondre@dgfip.finances.example>", "Votre avis d'impôt");
        let ids: Vec<&str> = s.route(&notice).iter().map(|r| r.case.id.as_str()).collect();
        assert_eq!(ids, vec!["taxes"]);
        let bill = card("<noreply@dgfip.finances.example>", "Avis des sommes à payer");
        let ids: Vec<&str> = s.route(&bill).iter().map(|r| r.case.id.as_str()).collect();
        assert_eq!(ids, vec!["taxes", "bill"]);
    }

    #[test]
    fn attachments_and_conversations() {
        let manifest: Manifest = toml::from_str("[[case]]\nid = \"lumen\"\ntitle = \"Studio Lumen\"\n[[case.route]]\nattachment_contains = [\"devis\"]\n").unwrap();
        let mut store = CaseStore { root: PathBuf::from("."), cases: manifest.cases, ties: Default::default() };
        let quote = "From: Jane <jane@lumen.example.net>\r\nSubject: Our offer\r\nMessage-ID: <q1@lumen.example.net>\r\nMIME-Version: 1.0\r\n\
            Content-Type: multipart/mixed; boundary=\"b\"\r\n\r\n--b\r\nContent-Type: text/plain\r\n\r\nHere it is.\r\n--b\r\n\
            Content-Type: application/pdf\r\nContent-Disposition: attachment; filename=\"Devis_Lumen_2026.pdf\"\r\n\r\nJVBERi0=\r\n--b--\r\n";
        let quote = Card::from_bytes(quote.as_bytes()).unwrap();
        let routed = store.route(&quote);
        assert_eq!(routed.first().map(|r| (r.case.id.as_str(), r.matched[0].field)), Some(("lumen", RouteField::Attachment)));
        // The answer has no attachment: it follows the quote once the quote is tied to the case.
        let answer = Card::from_bytes(b"From: Jane <jane@lumen.example.net>\r\nSubject: Re: Our offer\r\nIn-Reply-To: <q1@lumen.example.net>\r\n\r\nAny question?\r\n").unwrap();
        assert!(store.route(&answer).is_empty());
        store.ties.insert("q1@lumen.example.net".into(), "lumen".into());
        assert_eq!(store.route(&answer).first().map(|r| (r.case.id.as_str(), r.matched[0].field)), Some(("lumen", RouteField::Thread)));
    }

    #[test]
    fn domain_boundaries() {
        assert!(domain_matches("dgfip.finances.example", "finances.example"));
        assert!(!domain_matches("notfinances.example", "finances.example"));
        assert!(domain_matches("insurer.example", "@insurer.example"));
    }
}
