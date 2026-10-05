// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Where you stopped: one line left when something interrupts you (a pause to
//! move, a meal, a nap, the night, the end of the day, a timer stopped),
//! shown again when you come back, on the Porch, the Tasks page and the focus
//! window, until you say it is done. Stopping then costs no fear of
//! forgetting where you were (interruption research: a cue to resume cuts the
//! time to get back into a task, Trafton et al. 2003; Leroy & Glomb 2018).
//! One line, kept in `$XDG_STATE_HOME/sioul/stopped.toml`, carried to your
//! other devices with the sharing.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The line, when it was left, and the task it was about ("" for none).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stopped {
    pub text: String,
    /// Unix seconds.
    pub at: i64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub task: String,
}

impl Stopped {
    pub fn default_path() -> PathBuf {
        crate::config::state_dir().join("stopped.toml")
    }

    /// The line left, if there is one.
    pub fn load(path: &Path) -> Option<Stopped> {
        std::fs::read_to_string(path).ok().and_then(|t| toml::from_str::<Stopped>(&t).ok()).filter(|s| !s.text.trim().is_empty())
    }

    /// A line left (an empty one takes it away: done).
    pub fn keep(path: &Path, text: &str, task: &str, at: i64) -> Result<(), String> {
        let stopped = Stopped { text: text.trim().to_string(), at, task: task.to_string() };
        let fail = |e: std::io::Error| format!("{}: {e}", path.display());
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(fail)?;
        }
        if stopped.text.is_empty() {
            return match std::fs::remove_file(path) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(fail(e)),
                _ => Ok(()),
            };
        }
        crate::filelock::with_lock(path, || std::fs::write(path, toml::to_string(&stopped).map_err(|e| e.to_string())?).map_err(fail))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_left_then_done() {
        let dir = std::env::temp_dir().join(format!("sioul-stopped-{}", std::process::id()));
        let path = dir.join("stopped.toml");
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(Stopped::load(&path), None);
        Stopped::keep(&path, "  the second paragraph, after the quote ", "t1", 100).unwrap();
        assert_eq!(Stopped::load(&path), Some(Stopped { text: "the second paragraph, after the quote".into(), at: 100, task: "t1".into() }));
        Stopped::keep(&path, "", "", 200).unwrap();
        assert_eq!(Stopped::load(&path), None, "done: taken away");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
