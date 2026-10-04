// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! GitHub, for those who work there; off unless asked in the task settings
//! (docs/github.md). Issues and pull requests that are yours (assigned to you,
//! your review asked; what you opened or where you are mentioned when you ask
//! for it) become tasks in a local list, "GitHub", never sent anywhere.
//! GitHub's notification mail is tied to its task.
//!
//! What GitHub says is written into a task when GitHub changes it: its title,
//! its link, and whether it is open (kept as `X-SIOUL-GITHUB`, so a task you
//! marked done or dropped here stays so until GitHub's state changes). What
//! you add here (a date, a length, steps, a case) stays.

use crate::card::Card;
use crate::lines;
use crate::tasks::{self, Link};
use jiff::Zoned;
use serde::{Deserialize, Serialize};

/// The list GitHub's tasks go to: kept on this computer only.
pub const LIST_NAME: &str = "GitHub";
pub const LIST_ID: &str = "github";
const STATE: &str = "X-SIOUL-GITHUB";
const REASON: &str = "X-SIOUL-GITHUB-REASON";

/// An issue or a pull request, as GitHub lists it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Issue {
    /// "owner/repo".
    pub repo: String,
    pub number: u64,
    pub title: String,
    /// Its page.
    pub url: String,
    pub pull: bool,
    /// "open" or "closed".
    pub state: String,
    /// "completed", "not_planned", "duplicate", "reopened", or "".
    pub state_reason: String,
    /// RFC 3339, when closed.
    pub closed_at: String,
    /// RFC 3339, its last change.
    pub updated_at: String,
    /// Why it is yours: "assign", "review_requested", "author", "mention".
    pub reason: String,
}

impl Issue {
    /// What GitHub says of it, in one word: "open", "completed", "not_planned", "duplicate".
    fn state_key(&self) -> &str {
        match (self.state.as_str(), self.state_reason.as_str()) {
            ("open", _) => "open",
            (_, "not_planned" | "duplicate") => self.state_reason.as_str(),
            _ => "completed",
        }
    }
}

/// How strongly an issue is yours: an assignment first, a mention last.
pub fn rank(reason: &str) -> u8 {
    match reason {
        "assign" => 0,
        "review_requested" => 1,
        "author" => 2,
        _ => 3,
    }
}

/// The task's UID: "github:owner/repo#12".
pub fn uid(repo: &str, number: u64) -> String {
    format!("github:{repo}#{number}")
}

/// "github:owner/repo#12" → ("owner/repo", 12).
pub fn parse_uid(uid: &str) -> Option<(String, u64)> {
    let (repo, number) = uid.strip_prefix("github:")?.rsplit_once('#')?;
    Some((repo.to_string(), number.parse().ok()?))
}

/// Why a task's issue was yours, as last written.
pub fn reason_of(text: &str) -> String {
    lines::unfold(text).iter().find(|l| lines::name(l) == REASON).map(|l| lines::value(l).trim().to_string()).unwrap_or_default()
}

/// The task's file: "github-owner-repo-12.ics".
pub fn file_name(repo: &str, number: u64) -> String {
    let repo: String = repo.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '-' }).collect();
    format!("github-{repo}-{number}.ics")
}

/// The issue or pull request a GitHub notification mail is about, from its
/// thread's root (In-Reply-To, else Message-ID): `<owner/repo/issues/12@github.com>`,
/// `<owner/repo/pull/12/c345@github.com>`, `<owner/repo/issue/12/issue_event/6@github.com>`.
pub fn thread_of(card: &Card) -> Option<(String, u64)> {
    if card.from_address.as_deref() != Some("notifications@github.com") {
        return None;
    }
    let root = card.headers.first("In-Reply-To").or(card.message_id.as_deref())?;
    let id = root.trim().trim_start_matches('<').trim_end_matches('>');
    let (path, host) = id.rsplit_once('@')?;
    if host != "github.com" {
        return None;
    }
    let parts: Vec<&str> = path.split('/').collect();
    match parts.as_slice() {
        [owner, repo, "issues" | "issue" | "pull", number, ..] => Some((format!("{owner}/{repo}"), number.parse().ok()?)),
        _ => None,
    }
}

/// The lines GitHub owns in a task, as it says them now.
fn status_lines(issue: &Issue) -> Vec<String> {
    let mut out = match issue.state_key() {
        "open" => vec!["STATUS:NEEDS-ACTION".to_string()],
        "not_planned" | "duplicate" => vec!["STATUS:CANCELLED".to_string()],
        _ => vec!["STATUS:COMPLETED".to_string()],
    };
    if issue.state_key() == "completed"
        && let Ok(at) = issue.closed_at.parse::<jiff::Timestamp>()
    {
        out.push(format!("COMPLETED:{}", at.strftime("%Y%m%dT%H%M%SZ")));
    }
    out.push(format!("{STATE}:{}", issue.state_key()));
    out
}

fn link(issue: &Issue) -> String {
    tasks::link_line(&Link { uri: issue.url.clone(), label: format!("{} #{}", issue.repo, issue.number), rel: "related".into() })
}

/// A new task for an issue, in `case` when one of your projects takes it.
pub fn new_task(issue: &Issue, case: Option<&str>, now: &Zoned) -> String {
    let stamp = now.timestamp().strftime("%Y%m%dT%H%M%SZ").to_string();
    let mut out = vec!["BEGIN:VCALENDAR".to_string(), "VERSION:2.0".into(), "PRODID:-//Sioul//Sioul//EN".into(), "BEGIN:VTODO".into()];
    out.push(format!("UID:{}", uid(&issue.repo, issue.number)));
    out.push(format!("DTSTAMP:{stamp}"));
    out.push(format!("CREATED:{stamp}"));
    out.push(format!("SUMMARY:{}", lines::escape(&issue.title)));
    out.extend(status_lines(issue));
    out.push(format!("{REASON}:{}", issue.reason));
    out.push(link(issue));
    if let Some(case) = case {
        out.push(format!("REFID:{}", lines::escape(case)));
    }
    out.extend(["END:VTODO".to_string(), "END:VCALENDAR".to_string()]);
    lines::fold(&out)
}

/// A task's text with what GitHub changed written in; None when nothing
/// changed. Its open or closed state only when GitHub's changed since it was
/// last written: done or dropped here stays so.
pub fn updated(text: &str, issue: &Issue, case: Option<&str>, now: &Zoned) -> Option<String> {
    let source = lines::unfold(text);
    let value_of = |name: &str| source.iter().find(|l| lines::name(l) == name).map(|l| lines::unescape(lines::value(l).trim()));
    let mut dropped: Vec<&str> = Vec::new();
    let mut added: Vec<String> = Vec::new();
    if value_of("SUMMARY").as_deref() != Some(issue.title.as_str()) {
        dropped.push("SUMMARY");
        added.push(format!("SUMMARY:{}", lines::escape(&issue.title)));
    }
    if value_of(STATE).as_deref() != Some(issue.state_key()) {
        dropped.extend(["STATUS", "COMPLETED", STATE]);
        added.extend(status_lines(issue));
    }
    if value_of(REASON).as_deref() != Some(issue.reason.as_str()) {
        dropped.push(REASON);
        added.push(format!("{REASON}:{}", issue.reason));
    }
    let ours = |l: &str| lines::name(l) == "LINK" && lines::value(l).contains("://github.com/");
    let link = link(issue);
    let relinked = !source.iter().any(|l| *l == link);
    if relinked {
        added.push(link);
    }
    if let Some(case) = case
        && !source.iter().any(|l| lines::name(l) == "REFID")
    {
        added.push(format!("REFID:{}", lines::escape(case)));
    }
    if added.is_empty() {
        return None;
    }
    tasks::replace_lines(text, |l| dropped.contains(&lines::name(l).as_str()) || (relinked && ours(l)), added, now).ok()
}

/// GitHub no longer lists it as yours (unassigned, or closed before Sioul
/// saw it): dropped here, unless already done or dropped. None when nothing changes.
pub fn gone(text: &str, now: &Zoned) -> Option<String> {
    let source = lines::unfold(text);
    let state = source.iter().find(|l| lines::name(l) == STATE).map(|l| lines::value(l).trim().to_string());
    if state.as_deref() != Some("open") {
        return None;
    }
    let open = source.iter().find(|l| lines::name(l) == "STATUS").is_none_or(|l| matches!(lines::value(l).trim(), "NEEDS-ACTION" | "IN-PROCESS"));
    let mut added = vec![format!("{STATE}:gone")];
    if open {
        added.push("STATUS:CANCELLED".into());
    }
    tasks::replace_lines(text, |l| lines::name(l) == STATE || (open && lines::name(l) == "STATUS"), added, now).ok()
}

/// What GitHub last said of a task: "open", "completed"…, "gone"; None for a task not from GitHub.
pub fn state_of(text: &str) -> Option<String> {
    lines::unfold(text).iter().find(|l| lines::name(l) == STATE).map(|l| lines::value(l).trim().to_string())
}

/// A mail-like card for an issue, for your projects' routes: from
/// notifications@github.com, its subject as GitHub's mail writes it.
pub fn as_card(issue: &Issue) -> Card {
    let mut card = Card::from_bytes(b"From: GitHub <notifications@github.com>\r\nSubject: x\r\n\r\n").unwrap_or_else(|| unreachable!("a fixed message parses"));
    card.subject = format!("[{}] {} ({} #{})", issue.repo, issue.title, if issue.pull { "PR" } else { "Issue" }, issue.number);
    card
}

#[cfg(test)]
mod tests {
    use super::*;

    fn issue(state: &str, reason: &str) -> Issue {
        Issue {
            repo: "example/project".into(),
            number: 12,
            title: "Crash when exporting".into(),
            url: "https://github.com/example/project/issues/12".into(),
            state: state.into(),
            state_reason: reason.into(),
            closed_at: if state == "closed" { "2026-10-03T10:00:00Z".into() } else { String::new() },
            reason: "assign".into(),
            ..Issue::default()
        }
    }

    #[test]
    fn a_mail_finds_its_issue() {
        let mail = |headers: &str| Card::from_bytes(format!("From: Someone <notifications@github.com>\r\n{headers}Subject: Re: x\r\n\r\nbody").as_bytes()).unwrap();
        assert_eq!(thread_of(&mail("Message-ID: <example/project/issues/12@github.com>\r\n")), Some(("example/project".into(), 12)));
        assert_eq!(thread_of(&mail("Message-ID: <o/r/pull/7/c99@github.com>\r\nIn-Reply-To: <o/r/pull/7@github.com>\r\n")), Some(("o/r".into(), 7)));
        assert_eq!(thread_of(&mail("Message-ID: <o/r/issue/5/issue_event/1@github.com>\r\n")), Some(("o/r".into(), 5)));
        assert_eq!(thread_of(&mail("Message-ID: <o/r/commit/abc/1@github.com>\r\n")), None);
        let other = Card::from_bytes(b"From: x@example.org\r\nMessage-ID: <o/r/issues/1@github.com>\r\n\r\n").unwrap();
        assert_eq!(thread_of(&other), None);
    }

    #[test]
    fn github_changes_what_it_owns_only() {
        let now = Zoned::now();
        let zone = jiff::tz::TimeZone::UTC;
        let text = new_task(&issue("open", ""), Some("project"), &now);
        let task = tasks::task_of_text(&text, &zone).unwrap();
        assert_eq!((task.uid.as_str(), task.status, task.cases.clone()), ("github:example/project#12", tasks::Status::NeedsAction, vec!["project".to_string()]));
        assert_eq!(task.links[0].uri, "https://github.com/example/project/issues/12");
        // Nothing changed there: nothing written.
        assert_eq!(updated(&text, &issue("open", ""), Some("project"), &now), None);
        // Done here while still open there: stays done.
        let done = tasks::set_status(&text, tasks::Status::Completed, &zone, &now).unwrap();
        assert_eq!(updated(&done, &issue("open", ""), None, &now), None);
        // Closed as not planned there: dropped here.
        let closed = updated(&done, &issue("closed", "not_planned"), None, &now).unwrap();
        assert_eq!(tasks::task_of_text(&closed, &zone).unwrap().status, tasks::Status::Cancelled);
        // Reopened there: open here again; a new title too.
        let mut again = issue("open", "reopened");
        again.title = "Crash when exporting to TIFF".into();
        let reopened = tasks::task_of_text(&updated(&closed, &again, None, &now).unwrap(), &zone).unwrap();
        assert_eq!((reopened.status, reopened.title.as_str()), (tasks::Status::NeedsAction, "Crash when exporting to TIFF"));
        // No longer yours: dropped, once.
        let left = gone(&text, &now).unwrap();
        assert_eq!(tasks::task_of_text(&left, &zone).unwrap().status, tasks::Status::Cancelled);
        assert_eq!(gone(&left, &now), None);
        assert_eq!(as_card(&issue("open", "")).subject, "[example/project] Crash when exporting (Issue #12)");
        assert_eq!(parse_uid("github:example/project#12"), Some(("example/project".into(), 12)));
        assert_eq!(reason_of(&text), "assign");
    }
}
