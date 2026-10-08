// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Consent per project: what of yours an AI agent connected with `sioul mcp`
//! may read, and write into (docs/ai.md, "Consent per project"; docs/mcp.md).
//!
//! **Every project is closed until you open it** (`ai = true` on its entry in
//! `sioul-cases.toml`, `cases::set_ai`): the switch on its page, in its form,
//! or in Settings ▸ AI agents. The sharing carries the entry, so the choice
//! holds on every device. **Things in no project** (the Porch's other mail,
//! tasks of no project, general notes, the agenda, contacts, the phone's
//! messages and calls, papers) follow one setting, `[mcp] outside_projects`,
//! open unless you close it.
//!
//! A thing belongs to a project as the project's page gathers it
//! (`project::view`): a task or an event by its project (REFID) or a tie; a
//! step by its bigger task's too; mail by the project's routes (the sender
//! and the subject; the text and the attachments once Sioul read it, which it
//! ties then), by a tie, and the rest of their conversations; a note named in
//! the project's files, inside a folder they name, or tied; a draft by its
//! ties and the message it answers; a contact tied, or the project's client;
//! a budget's line tied, or in the budget the project's invoices go to; time
//! and invoices by their project; a paper letter by its project.
//!
//! **A thing in two projects reaches an agent only when both are open**: an
//! agent never sees a closed project's things, not even half of them. A
//! project taken out of the manifest leaves its things in no project, as its
//! page does.

use crate::card::Card;
use crate::cases::Case;
use crate::links::{self, Kind, Loaded};
use crate::notes;
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Which projects are open to agents, and which project each thing is in.
#[derive(Debug, Clone, Default)]
pub struct Consent {
    /// Things in no project: open to agents (`[mcp] outside_projects`).
    pub outside: bool,
    /// Every project, by id: open or not.
    projects: BTreeMap<String, bool>,
    /// The projects' routes, for a message read whole.
    cases: Vec<Case>,
    /// What each thing is tied to, by its address: its projects' ids.
    tied: HashMap<String, BTreeSet<String>>,
}

impl Consent {
    /// Every project closed, things outside projects as `outside` says; nothing tied.
    pub fn new(cases: &[Case], outside: bool) -> Consent {
        Consent { outside, projects: cases.iter().map(|c| (c.id.clone(), c.ai)).collect(), cases: cases.to_vec(), tied: HashMap::new() }
    }

    /// What everything Sioul reads belongs to, from the files read once
    /// (`Loaded::read`) and the configuration's `[mcp] outside_projects`.
    pub fn of(loaded: &Loaded, outside: bool) -> Consent {
        let mut consent = Consent::new(&loaded.cases, outside);
        let world = loaded.world();
        let case_of = |uri: &str| (links::kind_of(uri) == Kind::Case).then(|| links::id_of(uri));
        // Every tie with a project at one end: tasks' and events' REFID, notes'
        // front matter, budget lines' sources, Sioul's own links file.
        for edge in world.edges() {
            if let Some(id) = case_of(&edge.to) {
                consent.tie(&edge.from, &id);
            } else if let Some(id) = case_of(&edge.from) {
                consent.tie(&edge.to, &id);
            }
        }
        // A step is in its bigger task's projects too, however far up.
        let parent: HashMap<&str, &str> = loaded
            .tasks
            .iter()
            .filter_map(|t| t.relations.iter().find(|r| r.kind.eq_ignore_ascii_case("PARENT")).map(|r| (t.uid.as_str(), r.uid.as_str())))
            .collect();
        for task in &loaded.tasks {
            let mut seen = BTreeSet::new();
            let mut at = task.uid.as_str();
            let mut inherited = BTreeSet::new();
            while let Some(up) = parent.get(at).copied().filter(|up| seen.insert(*up)) {
                inherited.extend(consent.tied.get(&links::task_uri(up)).cloned().unwrap_or_default());
                at = up;
            }
            for id in inherited {
                consent.tie(&links::task_uri(&task.uid), &id);
            }
        }
        // Mail: by the routes that read the sender and the subject, and the
        // ties above; then everything in the same conversations.
        consent.gather_mail(loaded);
        // A draft is in the projects of the message it answers.
        for draft in &loaded.drafts {
            if let Some(answered) = draft.in_reply_to.as_deref().filter(|id| !id.is_empty()) {
                for id in consent.tied.get(&links::mail_uri(answered)).cloned().unwrap_or_default() {
                    consent.tie(&links::draft_uri(&draft.id), &id);
                }
            }
        }
        // A contact the project is for: its address, or its name or organisation.
        for case in &loaded.cases {
            let Some(client) = case.client.as_deref().map(str::trim).filter(|c| !c.is_empty()) else { continue };
            for contact in &loaded.contacts {
                let named = contact.name.eq_ignore_ascii_case(client) || (!contact.org.is_empty() && contact.org.eq_ignore_ascii_case(client));
                if named || world.canonical(client) == links::contact_uri(&contact.uid) {
                    consent.tie(&links::contact_uri(&contact.uid), &case.id);
                }
            }
        }
        consent
    }

    fn tie(&mut self, uri: &str, project: &str) {
        self.tied.entry(uri.to_string()).or_default().insert(project.to_string());
    }

    /// Mail's projects, as a project's page finds them: a route on the sender
    /// and the subject, or a tie, then the rest of each conversation.
    fn gather_mail(&mut self, loaded: &Loaded) {
        // One union of the conversations, over every message and those cited.
        let mut parent: HashMap<String, String> = HashMap::new();
        fn root(parent: &mut HashMap<String, String>, id: &str) -> String {
            let mut at = id.to_string();
            while let Some(up) = parent.get(&at).filter(|up| **up != at).cloned() {
                at = up;
            }
            parent.insert(id.to_string(), at.clone());
            at
        }
        for (id, mail) in loaded.mail.iter() {
            for cited in &mail.refs {
                let (a, b) = (root(&mut parent, id), root(&mut parent, cited));
                if a != b {
                    parent.insert(a, b);
                }
            }
        }
        let mut by_root: HashMap<String, BTreeSet<String>> = HashMap::new();
        for (id, mail) in loaded.mail.iter() {
            let mut own: BTreeSet<String> = self.tied.get(&links::mail_uri(id)).cloned().unwrap_or_default();
            own.extend(self.cases.iter().filter(|c| c.routes.iter().any(|r| r.takes_header(&mail.address, &mail.subject))).map(|c| c.id.clone()));
            if !own.is_empty() {
                by_root.entry(root(&mut parent, id)).or_default().extend(own);
            }
        }
        if by_root.is_empty() {
            return;
        }
        let ids: Vec<String> = loaded.mail.iter().map(|(id, _)| id.clone()).collect();
        for id in ids {
            if let Some(projects) = by_root.get(&root(&mut parent, &id)).cloned() {
                for project in projects {
                    self.tie(&links::mail_uri(&id), &project);
                }
            }
        }
    }

    /// Whether this project is open to agents; an unknown one is not.
    pub fn is_open(&self, project: &str) -> bool {
        self.projects.get(project).copied().unwrap_or(false)
    }

    /// Whether this project is one of the manifest's.
    pub fn knows(&self, project: &str) -> bool {
        self.projects.contains_key(project)
    }

    /// The projects open to agents, by id.
    pub fn open_projects(&self) -> Vec<String> {
        self.projects.iter().filter(|(_, open)| **open).map(|(id, _)| id.clone()).collect()
    }

    /// How many projects are closed.
    pub fn closed_projects(&self) -> usize {
        self.projects.values().filter(|open| !**open).count()
    }

    /// Whether a thing in these projects reaches an agent: every project it is
    /// in open; in none (or only in projects the manifest no longer names), as
    /// things outside projects are.
    pub fn allows_projects<'a>(&self, projects: impl IntoIterator<Item = &'a str>) -> bool {
        let mut any = false;
        for project in projects.into_iter().filter(|p| !p.is_empty() && self.knows(p)) {
            if !self.is_open(project) {
                return false;
            }
            any = true;
        }
        any || self.outside
    }

    /// The projects a thing is in, by its address (`mid:…`, `sioul:task/…`,
    /// `sioul:note/…`, `sioul:case/…`…): a project is in itself.
    pub fn projects_of(&self, uri: &str) -> BTreeSet<String> {
        let uri = uri.trim();
        let uri = if uri.starts_with("mid:") { links::mail_uri(uri) } else { notes::path_of(uri).map_or_else(|| uri.to_string(), |path| notes::uri_of(&path)) };
        let mut out = self.tied.get(&uri).cloned().unwrap_or_default();
        match links::kind_of(&uri) {
            Kind::Case => {
                let id = links::id_of(&uri);
                if self.knows(&id) {
                    out.insert(id);
                }
            }
            Kind::Note => {
                if let Some(path) = notes::path_of(&uri) {
                    out.extend(self.note_projects(&path));
                }
            }
            // A budget a project's invoices go to, `sioul:budget/<budget>`, or one of its lines, `…/<place>`.
            Kind::Budget => {
                let id = links::id_of(&uri);
                out.extend(self.budget_projects(id.rsplit_once('/').map_or(id.as_str(), |(budget, _)| budget)));
            }
            _ => {}
        }
        out
    }

    /// Whether a thing reaches an agent, by its address.
    pub fn allows(&self, uri: &str) -> bool {
        self.allows_projects(self.projects_of(uri).iter().map(String::as_str))
    }

    /// The projects whose files name this note: the file itself, or a folder holding it.
    pub fn note_projects(&self, path: &str) -> BTreeSet<String> {
        let path = path.trim_start_matches('/');
        self.cases
            .iter()
            .filter(|c| {
                c.files.iter().any(|f| {
                    let f = f.trim().trim_start_matches("./").trim_matches('/');
                    !f.is_empty() && (path == f || path.strip_prefix(f).is_some_and(|rest| rest.starts_with('/')))
                })
            })
            .map(|c| c.id.clone())
            .collect()
    }

    /// Whether a note reaches an agent, by its path in the notes.
    pub fn allows_note(&self, path: &str) -> bool {
        self.allows(&notes::uri_of(path))
    }

    /// The projects whose invoices go to this budget, by its id.
    pub fn budget_projects(&self, budget: &str) -> BTreeSet<String> {
        self.cases.iter().filter(|c| c.budget.as_deref().is_some_and(|b| !b.is_empty() && b == budget)).map(|c| c.id.clone()).collect()
    }

    /// The projects a message is in: by its address as above, and, read
    /// whole, by every route of a project (its text, its attachments).
    pub fn mail_projects(&self, message_id: Option<&str>, card: Option<&Card>) -> BTreeSet<String> {
        let mut out = message_id.filter(|id| !id.is_empty()).map(|id| self.projects_of(&links::mail_uri(id))).unwrap_or_default();
        if let Some(card) = card {
            out.extend(self.cases.iter().filter(|c| c.routes.iter().any(|r| r.explain(card).is_some())).map(|c| c.id.clone()));
            if let Some(id) = card.message_id.as_deref() {
                out.extend(self.projects_of(&links::mail_uri(id)));
            }
        }
        out
    }

    /// Whether a message reaches an agent.
    pub fn allows_mail(&self, message_id: Option<&str>, card: Option<&Card>) -> bool {
        self.allows_projects(self.mail_projects(message_id, card).iter().map(String::as_str))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cases::Route;
    use crate::mailindex::{MailIndex, MailRef};
    use crate::tasks::{Relation, Task};

    fn case(id: &str, open: bool) -> Case {
        Case { id: id.into(), title: id.to_uppercase(), ai: open, ..Case::default() }
    }

    #[test]
    fn closed_until_opened_and_outside_by_its_setting() {
        let mut lumen = case("lumen", true);
        lumen.routes = vec![Route { from_domains: vec!["lumen.example.net".into()], ..Route::default() }];
        lumen.files = vec!["work/lumen/".into()];
        lumen.client = Some("Studio Lumen".into());
        let mut taxes = case("taxes", false);
        taxes.routes = vec![Route { from_domains: vec!["finances.example".into()], ..Route::default() }];
        taxes.files = vec!["admin/taxes.md".into()];
        taxes.budget = Some("income".into());
        let tasks = vec![
            Task { uid: "t1".into(), title: "Declare".into(), cases: vec!["taxes".into()], ..Task::default() },
            // A step of a closed project's task, without a project of its own.
            Task { uid: "t2".into(), title: "Find the form".into(), relations: vec![Relation { kind: "PARENT".into(), uid: "t1".into(), gap: 0 }], ..Task::default() },
            Task { uid: "t3".into(), title: "Mock-ups".into(), cases: vec!["lumen".into()], ..Task::default() },
            // In both: closed, as one of them is.
            Task { uid: "t4".into(), title: "Both".into(), cases: vec!["lumen".into(), "taxes".into()], ..Task::default() },
            Task { uid: "t5".into(), title: "Groceries".into(), ..Task::default() },
            // A project the manifest no longer names: in no project.
            Task { uid: "t6".into(), title: "Old".into(), cases: vec!["gone".into()], ..Task::default() },
        ];
        let mut mail = MailIndex::default();
        mail.insert("n@finances.example", MailRef { subject: "Avis".into(), address: "x@dgfip.finances.example".into(), ..Default::default() });
        // Your answer, from your own address: in its conversation, so closed too.
        mail.insert("r@you.example.org", MailRef { subject: "Re: Avis".into(), address: "you@example.org".into(), refs: vec!["n@finances.example".into()], ..Default::default() });
        mail.insert("q@lumen.example.net", MailRef { subject: "Quote".into(), address: "jane@lumen.example.net".into(), ..Default::default() });
        mail.insert("o@example.org", MailRef { subject: "Hello".into(), address: "friend@example.org".into(), ..Default::default() });
        let contacts = vec![crate::contacts::Contact { uid: "c1".into(), name: "Jane".into(), org: "Studio Lumen".into(), ..Default::default() }];
        let loaded = Loaded { tasks, mail, contacts, cases: vec![lumen, taxes], ..Loaded::default() };
        let consent = Consent::of(&loaded, true);
        let allows = |uri: &str| consent.allows(uri);
        assert!(allows("sioul:case/lumen") && !allows("sioul:case/taxes"));
        assert!(!allows("sioul:task/t1") && !allows("sioul:task/t2"), "a closed project's task and its step");
        assert!(allows("sioul:task/t3") && !allows("sioul:task/t4"));
        assert!(allows("sioul:task/t5") && allows("sioul:task/t6"));
        assert!(!allows("mid:n@finances.example") && !allows("mid:<r@you.example.org>"), "routed, and its conversation");
        assert!(allows("mid:q@lumen.example.net") && allows("mid:o@example.org"));
        assert!(!allows("sioul:note/admin/taxes.md") && allows("sioul:note/work/lumen/brief.md") && allows("sioul:note/journal.md"));
        assert!(consent.note_projects("admin/taxes.md.bak").is_empty() && allows("sioul:note/admin/taxes.md.bak"), "a name that only starts like it");
        assert!(!allows("sioul:budget/income/3") && !allows("sioul:budget/income") && allows("sioul:budget/food/1"));
        assert_eq!(consent.projects_of("sioul:contact/c1"), BTreeSet::from(["lumen".to_string()]));
        // Things outside projects closed: only the open project's.
        let only = Consent::of(&loaded, false);
        assert!(only.allows("sioul:task/t3") && !only.allows("sioul:task/t5") && !only.allows("mid:o@example.org") && !only.allows("sioul:note/journal.md"));
        assert_eq!((only.open_projects(), only.closed_projects()), (vec!["lumen".to_string()], 1));
        // A message read whole: a route on its text counts too.
        let mut words = case("words", false);
        words.routes = vec![Route { text_contains: vec!["dossier 4471".into()], ..Route::default() }];
        let consent = Consent::new(&[words], true);
        let card = Card::from_bytes(b"From: a@example.org\r\nSubject: Hi\r\nMessage-ID: <w@example.org>\r\n\r\nAbout dossier 4471.\r\n").unwrap();
        assert!(consent.allows("mid:w@example.org"), "by its address alone, nothing says");
        assert!(!consent.allows_mail(Some("w@example.org"), Some(&card)));
    }
}
