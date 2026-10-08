// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The commit this Sioul is built from, for `sioul_core::build::COMMIT`
//! (docs/building.md, "Which build"): `git rev-parse --short=12 HEAD`, with
//! "-dirty" when tracked files hold changes not committed; else the commit
//! read from the `.git` folder itself (a build where `git` cannot run, or
//! refuses a folder another user owns: a container); else `SIOUL_COMMIT` from
//! the environment (a build without the repository's history); else
//! "unknown".
//!
//! Run again when the commit changes (HEAD, its branch, packed references),
//! when `SIOUL_COMMIT` changes, and when this crate's own files change. Not
//! when another crate's files change: watching the whole tree would rebuild
//! every crate at each edit, and `.git/index` changes at each `git status`.
//! "-dirty" thus says the tree held changes when this crate was last built.

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default());
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=src");
    println!("cargo::rerun-if-env-changed=SIOUL_COMMIT");
    if let Some(git) = git_dir(&manifest) {
        for watched in watched(&git) {
            println!("cargo::rerun-if-changed={}", watched.display());
        }
    }
    let commit = from_git(&manifest)
        .or_else(|| git_dir(&manifest).and_then(|git| from_files(&git)))
        .or_else(|| std::env::var("SIOUL_COMMIT").ok().map(|c| c.trim().to_string()).filter(|c| !c.is_empty() && c.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '.')))
        .unwrap_or_else(|| "unknown".into());
    println!("cargo::rustc-env=SIOUL_BUILD_COMMIT={commit}");
}

/// The commit from `git` itself, "-dirty" added when tracked files changed.
fn from_git(manifest: &Path) -> Option<String> {
    let run = |args: &[&str]| Command::new("git").arg("-C").arg(manifest).args(args).output().ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
    let commit = run(&["rev-parse", "--short=12", "HEAD"]).filter(|c| is_hash(c))?;
    let dirty = run(&["status", "--porcelain", "--untracked-files=no"]).is_some_and(|s| !s.is_empty());
    Some(if dirty { format!("{commit}-dirty") } else { commit })
}

fn is_hash(text: &str) -> bool {
    text.len() >= 7 && text.chars().all(|c| c.is_ascii_hexdigit())
}

/// The repository's `.git` folder above `manifest` (a worktree's `.git` file followed).
fn git_dir(manifest: &Path) -> Option<PathBuf> {
    let mut at = Some(manifest);
    while let Some(dir) = at {
        let git = dir.join(".git");
        if git.is_dir() {
            return Some(git);
        }
        if let Some(pointed) = std::fs::read_to_string(&git).ok().and_then(|t| t.trim().strip_prefix("gitdir:").map(|p| p.trim().to_string())) {
            let pointed = PathBuf::from(pointed);
            return Some(if pointed.is_absolute() { pointed } else { dir.join(pointed) });
        }
        at = dir.parent();
    }
    None
}

/// Where the references live: a worktree's common folder (`commondir`), else the folder itself.
fn common(git: &Path) -> PathBuf {
    match std::fs::read_to_string(git.join("commondir")) {
        Ok(text) => {
            let path = PathBuf::from(text.trim());
            if path.is_absolute() { path } else { git.join(path) }
        }
        Err(_) => git.to_path_buf(),
    }
}

/// The files whose change means another commit: HEAD, the branch it names, the packed references.
fn watched(git: &Path) -> Vec<PathBuf> {
    let mut out = vec![git.join("HEAD")];
    if let Some(branch) = std::fs::read_to_string(git.join("HEAD")).ok().and_then(|t| t.trim().strip_prefix("ref:").map(|r| r.trim().to_string())) {
        out.push(common(git).join(branch));
    }
    out.push(common(git).join("packed-refs"));
    // A path that does not exist would run this again at every build.
    out.into_iter().filter(|p| p.exists()).collect()
}

/// The commit read from the `.git` folder without `git`: HEAD, the branch it
/// names, or that branch among the packed references. Never "-dirty": that
/// needs `git`.
fn from_files(git: &Path) -> Option<String> {
    let head = std::fs::read_to_string(git.join("HEAD")).ok()?;
    let head = head.trim();
    let full = match head.strip_prefix("ref:").map(str::trim) {
        Some(branch) => std::fs::read_to_string(common(git).join(branch)).ok().map(|t| t.trim().to_string()).or_else(|| {
            let packed = std::fs::read_to_string(common(git).join("packed-refs")).ok()?;
            packed.lines().find_map(|line| line.strip_suffix(branch).map(|hash| hash.trim().to_string()))
        })?,
        None => head.to_string(),
    };
    is_hash(&full).then(|| full.chars().take(12).collect())
}
