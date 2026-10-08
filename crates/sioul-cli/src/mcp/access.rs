// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Consent per project, as every tool keeps to it (docs/mcp.md, "Per-project
//! access"; the rule itself is the core's: `sioul_core::consent`).
//!
//! Every project is closed to agents until the person opens it (`ai = true`
//! in `sioul-projects.toml`); things in no project follow `[mcp]
//! outside_projects`. A tool that lists leaves out what is closed and says
//! how many things it left out, never which: the agent knows something is
//! there and does not invent it. A tool that reads one thing, or writes into
//! one, refuses a closed one, saying so plainly.

use crate::Session;
use sioul_core::consent::Consent;
use sioul_core::links::{self, Kind, LocalLinks, Loaded, World};
use sioul_core::mailindex::MailIndex;
use sioul_core::tasks::Task;

/// What a thing kept from agents is called where its words would be.
pub const KEPT: &str = "(kept from agents)";

/// What the person opened to agents, and what each thing belongs to, from everything read.
pub fn of(s: &Session, loaded: &Loaded) -> Consent {
    Consent::of(loaded, s.config.mcp.outside_projects)
}

/// The same for mail alone, lighter: the projects, Sioul's own ties, and
/// `index` (the mail's headers), which comes back with it.
pub fn for_mail(s: &Session, index: MailIndex) -> (Consent, MailIndex) {
    let projects = s.config.notes_root_path().and_then(|root| sioul_core::projects::ProjectStore::load(&root).ok()).map(|store| store.projects).unwrap_or_default();
    let loaded = Loaded { projects, mail: index, local: LocalLinks::load(&LocalLinks::default_path()), ..Loaded::default() };
    let consent = of(s, &loaded);
    (consent, loaded.mail)
}

/// What the agent is told when things were left out: how many, never which.
pub fn left_out(count: usize) -> Option<String> {
    (count > 0).then(|| {
        let things = if count == 1 { "1 more thing belongs".to_string() } else { format!("{count} more things belong") };
        format!("({things} to projects the person keeps from agents, or to no project while those are kept from agents: left out on purpose. Do not guess what they are; ask the person if it matters.)")
    })
}

/// A thing the agent asked for by its address, kept from agents.
pub fn kept(what: &str) -> String {
    format!(
        "{what} is kept from agents: it belongs to a project the person has not opened to agents, or to no project while things outside projects are closed to them. Sioul neither reads nor writes it for an agent. The person opens a project on its page in Sioul, or in Settings ▸ AI agents."
    )
}

/// A project named by the agent, closed to agents.
pub fn closed_project(id: &str) -> String {
    format!("The project “{}” is closed to agents: Sioul neither reads it nor writes into it for an agent. The person opens it on its page in Sioul, or in Settings ▸ AI agents.", crate::one_line(id))
}

/// A new thing in no project, while those are closed to agents.
pub const OUTSIDE_CLOSED: &str = "Things in no project are closed to agents (Settings ▸ AI agents, “Things in no project”): Sioul writes nothing outside a project for an agent. Give a project open to agents (list_projects lists them), or ask the person.";

/// Whether a message reaches the agent, its whole text read when its file is known.
pub fn allows_mail(consent: &Consent, world: &World, uri: &str) -> bool {
    let id = links::id_of(&world.canonical(uri));
    let card = world.mail.get(uri).and_then(|m| sioul_core::maildir::read_one(&m.path));
    consent.allows_mail(Some(&id), card.as_ref())
}

/// Whether a thing reaches the agent, by its address; a message read whole.
pub fn allows(consent: &Consent, world: &World, uri: &str) -> bool {
    let uri = world.canonical(uri);
    match links::kind_of(&uri) {
        Kind::Mail => allows_mail(consent, world, &uri),
        _ => consent.allows(&uri),
    }
}

/// Tasks and projects as an agent may see them in a plan: the closed ones
/// stay, so that the plan is the person's, but with nothing of theirs (a
/// title, notes, a place, ties), to be left out wherever they are listed.
pub fn redact(loaded: &mut Loaded, consent: &Consent) {
    for task in loaded.tasks.iter_mut().filter(|t| !consent.allows(&links::task_uri(&t.uid))) {
        *task = Task { title: KEPT.into(), notes: String::new(), location: String::new(), categories: Vec::new(), links: Vec::new(), contacts: Vec::new(), ..task.clone() };
    }
    for project in loaded.projects.iter_mut().filter(|c| !consent.is_open(&c.id)) {
        project.title = KEPT.into();
        project.client = None;
        project.files.clear();
        project.routes.clear();
    }
}
