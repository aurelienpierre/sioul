// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What an AI agent changed through `sioul mcp`, said in the window (25
//! October 2026): each change the line the window says and the files it
//! wrote, each as it was before and after, so that the window offers Undo
//! and puts them back only while they are still as the agent left them.
//! One line per change in `$XDG_STATE_HOME/sioul/mcp/said.jsonl`, yours
//! alone, never shared; the window reads what was added since it last
//! looked, and takes out what is older than a day.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Kept this long, then taken out by the window.
pub const KEPT_MS: i64 = 86_400_000;

/// A file an agent wrote: its text before (None: there was none) and after (None: it is gone).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileChange {
    pub path: PathBuf,
    #[serde(default)]
    pub before: Option<String>,
    #[serde(default)]
    pub after: Option<String>,
}

impl FileChange {
    /// The file as it is now, beside what it was: what the change will say.
    pub fn of(path: &Path, before: Option<String>) -> FileChange {
        FileChange { path: path.to_path_buf(), before, after: std::fs::read_to_string(path).ok() }
    }
}

/// One change: when (ms), the line the window says, the files.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Said {
    pub at: i64,
    pub line: String,
    #[serde(default)]
    pub files: Vec<FileChange>,
}

impl Said {
    /// Its files put back as they were, each only while it is as the agent
    /// left it (a sync, the person, changed none since): whether it was. Nothing
    /// is put back when one changed.
    pub fn put_back(&self) -> bool {
        if self.files.iter().any(|f| std::fs::read_to_string(&f.path).ok() != f.after) {
            return false;
        }
        // An event among them through the one rule first (`agenda::guard`): one refused, none put back.
        if self.files.iter().any(|f| crate::agenda::guard(&f.path, f.before.as_deref().map_or(crate::agenda::Change::Delete, crate::agenda::Change::Write)).is_err()) {
            return false;
        }
        for f in &self.files {
            let done = match &f.before {
                Some(text) => crate::vdir::write_item(&f.path, text),
                // Gone already (made, then moved on by the same change): removed.
                None if !f.path.exists() => Ok(()),
                None => crate::vdir::remove_item(&f.path),
            };
            if done.is_err() {
                return false;
            }
        }
        true
    }
}

pub fn path() -> PathBuf {
    crate::config::state_dir().join("mcp").join("said.jsonl")
}

/// A change written, for the window to say.
pub fn note(said: &Said) -> Result<(), String> {
    let path = path();
    let line = serde_json::to_string(said).map_err(|e| e.to_string())? + "\n";
    crate::filelock::with_lock(&path, || crate::calls::append_private(&path, &line))
}

/// What was added since `offset` bytes, and the new offset; from the start
/// when the file is shorter (taken out meanwhile).
pub fn since(offset: u64) -> (Vec<Said>, u64) {
    let path = path();
    let Ok(text) = std::fs::read_to_string(&path) else { return (Vec::new(), 0) };
    let len = text.len() as u64;
    let from = if offset > len { 0 } else { offset as usize };
    let added = text.get(from..).unwrap_or_default().lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
    (added, len)
}

/// What is older than `KEPT_MS` taken out; how many went.
pub fn trim(now_ms: i64) -> Result<usize, String> {
    crate::calls::rewrite_lines(&path(), |line| serde_json::from_str::<Said>(line).is_ok_and(|s| now_ms - s.at > KEPT_MS))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Put back only while every file is as the agent left it.
    #[test]
    fn a_change_is_put_back_only_as_left() {
        let dir = std::env::temp_dir().join(format!("sioul-agentwrites-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (edited, made) = (dir.join("e.ics"), dir.join("m.ics"));
        std::fs::write(&edited, "after").unwrap();
        std::fs::write(&made, "new").unwrap();
        let said = Said { at: 1, line: "An agent moved it.".into(), files: vec![FileChange { path: edited.clone(), before: Some("before".into()), after: Some("after".into()) }, FileChange { path: made.clone(), before: None, after: Some("new".into()) }] };
        std::fs::write(&made, "changed since").unwrap();
        assert!(!said.put_back(), "one changed since: nothing put back");
        assert_eq!(std::fs::read_to_string(&edited).unwrap(), "after");
        std::fs::write(&made, "new").unwrap();
        assert!(said.put_back());
        assert_eq!(std::fs::read_to_string(&edited).unwrap(), "before");
        assert!(!made.exists(), "one it made: gone");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
