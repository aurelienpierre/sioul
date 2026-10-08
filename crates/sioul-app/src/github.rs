// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! GitHub, once asked in the task settings (docs/github.md): the issues and
//! pull requests that are yours, in the local "GitHub" list every thirty
//! minutes, each in the project whose routes take it; GitHub's notification
//! mail tied to its task and that task's project.

use crate::backend::{QtThread, Shared, load_config, set_status, tr};
use jiff::Zoned;
use jiff::tz::TimeZone;
use sioul_core::projects::ProjectStore;
use sioul_core::config::Config;
use sioul_core::github::{self, Issue};
use sioul_core::links::{self, LocalLinks};
use sioul_core::trust::Trust;
use sioul_core::vdir::{self, Collection, Kind};
use sioul_core::{porch, tasks};
use sioul_sync::github::Wanted;
use sioul_sync::{Control, SyncError};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

const EVERY: Duration = Duration::from_secs(30 * 60);
/// GitHub's search can take a while to see a change: an issue it stopped
/// finding is let go only when nothing changed on it for this long.
const SETTLED: i64 = 3600;

/// Starts bringing GitHub's issues when asked; brings them now when running.
pub(crate) fn start(qt: &QtThread, shared: &Arc<Shared>) {
    if !load_config().github.enabled || crate::backend::offline() {
        return;
    }
    let control = Arc::new(Control::default());
    {
        let Ok(mut running) = shared.github.lock() else { return };
        if let Some(running) = running.as_ref() {
            running.retry();
            return;
        }
        *running = Some(Arc::clone(&control));
    }
    let (qt, shared) = (qt.clone(), Arc::clone(shared));
    std::thread::spawn(move || {
        while !control.stopped() {
            let config = load_config();
            if !config.github.enabled {
                break;
            }
            match bring(&config) {
                Ok(0) => {}
                Ok(_) => crate::work::show_work(&qt, &shared),
                Err(e) => {
                    set_status(&qt, e.sentence(tr(), "GitHub"));
                    // A lasting error (a token refused): waits for a nudge, never ends.
                    if e.is_lasting() && !control.park() {
                        break;
                    }
                }
            }
            control.pause(EVERY);
        }
        if let Ok(mut running) = shared.github.lock()
            && running.as_ref().is_some_and(|c| Arc::ptr_eq(c, &control))
        {
            *running = None;
        }
    });
}

/// Stops it: turned off.
pub(crate) fn stop(shared: &Shared) {
    if let Some(control) = shared.github.lock().ok().and_then(|mut running| running.take()) {
        control.stop();
    }
}

/// The "GitHub" list, kept on this computer only.
fn list() -> Option<Collection> {
    vdir::collections(Kind::Calendars).into_iter().find(|c| c.account == vdir::LOCAL && c.id == github::LIST_ID)
}

/// Brings GitHub's open issues that are yours into the list, and lets go of
/// those it no longer lists: closed, or no longer yours. Returns how many
/// tasks changed.
fn bring(config: &Config) -> Result<usize, SyncError> {
    let list = match list() {
        Some(list) => list,
        None => vdir::create(Kind::Calendars, vdir::LOCAL, github::LIST_NAME, None, &["VTODO"]).map_err(SyncError::Disk)?,
    };
    let wanted = Wanted { assigned: config.github.assigned, reviews: config.github.reviews, created: config.github.created, mentioned: config.github.mentioned };
    let open = sioul_sync::github::open_issues(wanted)?;
    // Your projects' routes say which project an issue belongs to, as they do for mail.
    let store = config.notes_root_path().and_then(|root| ProjectStore::load(&root).ok()).map(|s| s.with_ties(&LocalLinks::load(&LocalLinks::default_path())));
    let project_of = |issue: &Issue| store.as_ref().and_then(|s| s.route(&github::as_card(issue)).first().map(|r| r.project.id.clone()));
    let now = Zoned::now();
    let mut changed = 0;
    let mut seen: BTreeSet<PathBuf> = BTreeSet::new();
    for issue in &open {
        let path = list.dir.join(github::file_name(&issue.repo, issue.number));
        seen.insert(path.clone());
        let text = match std::fs::read_to_string(&path) {
            Ok(before) => github::updated(&before, issue, project_of(issue).as_deref(), &now),
            Err(_) => Some(github::new_task(issue, project_of(issue).as_deref(), &now)),
        };
        if let Some(text) = text {
            vdir::write_item(&path, &text).map_err(SyncError::Disk)?;
            changed += 1;
        }
    }
    for path in list.items().into_iter().filter(|p| !seen.contains(p)) {
        let Ok(before) = std::fs::read_to_string(&path) else { continue };
        if github::state_of(&before).as_deref() != Some("open") {
            continue;
        }
        let Some((repo, number)) = tasks::task_of_text(&before, &TimeZone::system()).and_then(|t| github::parse_uid(&t.uid)) else { continue };
        let text = match sioul_sync::github::issue(&repo, number, &github::reason_of(&before))? {
            Some(issue) if issue.state == "closed" => github::updated(&before, &issue, None, &now),
            // Still open, and changed a moment ago: the search may not see it yet.
            Some(issue) if issue.updated_at.parse::<jiff::Timestamp>().is_ok_and(|at| now.timestamp().as_second() - at.as_second() < SETTLED) => None,
            _ => github::gone(&before, &now),
        };
        if let Some(text) = text {
            vdir::write_item(&path, &text).map_err(SyncError::Disk)?;
            changed += 1;
        }
    }
    Ok(changed)
}

/// GitHub's notification mail, tied to its issue's task, and to that task's
/// project. Mail about an issue that is not yours makes nothing; nor does a
/// message that only claims to come from GitHub: its signature must say so,
/// as the Porch judges it, or anyone could slip a message into a project.
pub(crate) fn tie_mail(config: &Config, files: &[PathBuf], ties: &mut LocalLinks) {
    if !config.github.enabled {
        return;
    }
    let Some(list) = list() else { return };
    let senders = porch::Senders::load(config);
    let judged = porch::judge(files, &config.mail_sources(), None, &porch::KnownSenders::default(), &senders, Zoned::now().timestamp().as_second());
    for card in judged.into_iter().filter(|t| t.trust == Trust::Verified).map(|t| t.card) {
        let (Some((repo, number)), Some(mid)) = (github::thread_of(&card), card.message_id.as_deref()) else { continue };
        let Ok(text) = std::fs::read_to_string(list.dir.join(github::file_name(&repo, number))) else { continue };
        let Some(task) = tasks::task_of_text(&text, &TimeZone::system()) else { continue };
        ties.add(&links::mail_uri(mid), &links::task_uri(&task.uid), "link");
        if let Some(project) = task.projects.first() {
            ties.add(&links::mail_uri(mid), &links::project_uri(project), "project");
        }
    }
}
