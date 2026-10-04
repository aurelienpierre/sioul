// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! GitHub's REST API, read only, with your fine-grained token from the
//! keyring (docs/github.md): the open issues and pull requests that are yours,
//! found by search across GitHub (assigned to you, your review asked, opened
//! by you, mentioning you), and one issue by its number. Answers are asked
//! with their ETag: an unchanged one costs nothing and comes from the cache.

use crate::SyncError;
use serde_json::Value;
use sioul_core::config::state_dir;
use sioul_core::github::Issue;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

const API: &str = "https://api.github.com";
const KEYRING: &str = "GitHub token";
/// Supported until March 2028.
const VERSION: &str = "2022-11-28";
/// What a refused token says (`SyncError::Login`).
pub const TOKEN_REFUSED: &str = "github-token";

/// The token kept in the keyring.
pub fn token() -> Option<String> {
    // A local stand-in for GitHub, in tests only (never in Sioul's builds).
    #[cfg(feature = "insecure-test-tls")]
    if let Some(token) = std::env::var_os("SIOUL_TEST_GITHUB_TOKEN") {
        return Some(token.to_string_lossy().to_string());
    }
    crate::secret::named(KEYRING).filter(|t| !t.trim().is_empty())
}

fn api() -> String {
    #[cfg(feature = "insecure-test-tls")]
    if let Some(base) = std::env::var_os("SIOUL_TEST_GITHUB") {
        return base.to_string_lossy().trim_end_matches('/').to_string();
    }
    API.to_string()
}

/// Keeps the token in the keyring; an empty one is forgotten.
pub fn save_token(token: &str) -> Result<(), SyncError> {
    if token.trim().is_empty() { crate::secret::forget_named(KEYRING) } else { crate::secret::save_named(KEYRING, token.trim()) }
}

/// What to bring, from the settings.
#[derive(Debug, Clone, Copy)]
pub struct Wanted {
    pub assigned: bool,
    pub reviews: bool,
    pub created: bool,
    pub mentioned: bool,
}

impl Wanted {
    /// The searches, with why what they find is yours.
    fn searches(self) -> Vec<(&'static str, &'static str)> {
        [
            (self.assigned, "is:open assignee:@me archived:false", "assign"),
            (self.reviews, "is:open is:pr user-review-requested:@me archived:false", "review_requested"),
            (self.created, "is:open author:@me archived:false", "author"),
            (self.mentioned, "is:open mentions:@me archived:false", "mention"),
        ]
        .into_iter()
        .filter(|(on, _, _)| *on)
        .map(|(_, query, reason)| (query, reason))
        .collect()
    }
}

/// Answers kept with their ETag, by address.
#[derive(Default, serde::Serialize, serde::Deserialize)]
struct Cache {
    answers: BTreeMap<String, (String, Value)>,
}

fn cache_path() -> PathBuf {
    state_dir().join("github-cache.json")
}

impl Cache {
    fn load() -> Cache {
        std::fs::read_to_string(cache_path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }

    fn save(&self) {
        if let Ok(text) = serde_json::to_string(self) {
            let _ = std::fs::create_dir_all(state_dir());
            let _ = std::fs::write(cache_path(), text);
        }
    }
}

struct Client {
    agent: ureq::Agent,
    token: String,
    cache: Cache,
}

impl Client {
    fn new() -> Result<Client, SyncError> {
        let token = token().ok_or(SyncError::NoPassword)?;
        let agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(30))).http_status_as_error(false).build().into();
        Ok(Client { agent, token, cache: Cache::load() })
    }

    /// One answer, from the cache when GitHub says it did not change.
    fn get(&mut self, path: &str) -> Result<Option<Value>, SyncError> {
        let url = format!("{}{path}", api());
        let mut request = self
            .agent
            .get(&url)
            .header("Authorization", &format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", VERSION)
            .header("User-Agent", "Sioul");
        if let Some((etag, _)) = self.cache.answers.get(&url) {
            request = request.header("If-None-Match", etag);
        }
        let mut response = request.call().map_err(|e| SyncError::Network(e.to_string()))?;
        let status = response.status().as_u16();
        let header = |name: &str| response.headers().get(name).and_then(|v| v.to_str().ok()).map(str::to_string);
        let (etag, remaining, reset) = (header("ETag"), header("x-ratelimit-remaining"), header("x-ratelimit-reset"));
        match status {
            304 => Ok(self.cache.answers.get(&url).map(|(_, body)| body.clone())),
            200 => {
                let body: Value = serde_json::from_str(&response.body_mut().with_config().limit(16 * 1024 * 1024).read_to_string().unwrap_or_default()).unwrap_or(Value::Null);
                if let Some(etag) = etag {
                    self.cache.answers.insert(url, (etag, body.clone()));
                }
                Ok(Some(body))
            }
            404 | 410 => Ok(None),
            401 => Err(SyncError::Login(TOKEN_REFUSED.into())),
            // Too many requests: GitHub says until when.
            403 | 429 if remaining.as_deref() == Some("0") || status == 429 => {
                let until = reset.and_then(|r| r.parse::<i64>().ok()).and_then(|r| jiff::Timestamp::from_second(r).ok()).map(|t| t.to_zoned(jiff::tz::TimeZone::system()).strftime("%H:%M").to_string());
                Err(SyncError::Server(format!("GitHub: {status}, wait until {}", until.unwrap_or_else(|| "later".into()))))
            }
            status => Err(SyncError::Server(format!("GitHub: {status} {path}"))),
        }
    }

    /// Every page of a search (100 a page, 1,000 at most).
    fn search(&mut self, query: &str) -> Result<Vec<Value>, SyncError> {
        let mut items = Vec::new();
        for page in 1..=10 {
            let Some(answer) = self.get(&format!("/search/issues?q={}&per_page=100&page={page}", crate::google::encode(query)))? else { break };
            let found = answer["items"].as_array().cloned().unwrap_or_default();
            let count = found.len();
            items.extend(found);
            if count < 100 {
                break;
            }
        }
        Ok(items)
    }
}

/// An issue from GitHub's JSON; `reason`, why it is yours.
fn issue_of(item: &Value, reason: &str) -> Option<Issue> {
    let repo = item["repository_url"].as_str()?.rsplit("/repos/").next()?.to_string();
    let text = |key: &str| item[key].as_str().unwrap_or("").to_string();
    Some(Issue {
        repo,
        number: item["number"].as_u64()?,
        title: text("title"),
        url: text("html_url"),
        pull: item["pull_request"].is_object(),
        state: text("state"),
        state_reason: text("state_reason"),
        closed_at: text("closed_at"),
        updated_at: text("updated_at"),
        reason: reason.to_string(),
    })
}

/// The open issues and pull requests that are yours, each once, with its
/// strongest reason (an assignment before a review asked, before a mention).
pub fn open_issues(wanted: Wanted) -> Result<Vec<Issue>, SyncError> {
    let mut client = Client::new()?;
    let mut found: BTreeMap<(String, u64), Issue> = BTreeMap::new();
    for (query, reason) in wanted.searches() {
        for issue in client.search(query)?.iter().filter_map(|item| issue_of(item, reason)) {
            let key = (issue.repo.clone(), issue.number);
            if found.get(&key).is_none_or(|known| sioul_core::github::rank(reason) < sioul_core::github::rank(&known.reason)) {
                found.insert(key, issue);
            }
        }
    }
    client.cache.save();
    Ok(found.into_values().collect())
}

/// One issue by its number, as it is now; None when GitHub no longer shows it to you.
pub fn issue(repo: &str, number: u64, reason: &str) -> Result<Option<Issue>, SyncError> {
    let mut client = Client::new()?;
    let answer = client.get(&format!("/repos/{repo}/issues/{number}"))?;
    client.cache.save();
    Ok(answer.as_ref().and_then(|item| issue_of(item, reason)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issues_from_github() {
        let item = serde_json::json!({ "repository_url": "https://api.github.com/repos/someone/project", "number": 12, "title": "Crash", "html_url": "https://github.com/someone/project/pull/12",
            "pull_request": { "url": "…" }, "state": "closed", "state_reason": null, "closed_at": "2026-10-03T10:00:00Z" });
        let issue = issue_of(&item, "review_requested").unwrap();
        assert_eq!((issue.repo.as_str(), issue.number, issue.pull, issue.state_reason.as_str()), ("someone/project", 12, true, ""));
        let wanted = Wanted { assigned: true, reviews: true, created: false, mentioned: false };
        assert_eq!(wanted.searches().len(), 2);
    }
}
