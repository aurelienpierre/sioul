// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! A store refuses a folder a sync app carries (docs/store.md, "Failsafe"):
//! a sync app would carry its blobs half written, bring back older ones, and
//! make copies "in conflict". Recognised as the window recognises the folders
//! a sync carries for the sharing (sioul-app's `share::synced_roots`): those
//! named so in your home, Nextcloud's, Syncthing's and Dropbox's own lists;
//! and, wherever it is, a folder that holds a sync app's own mark.

use std::path::{Path, PathBuf};

/// The folders this system's sync apps carry.
pub fn synced_roots() -> Vec<PathBuf> {
    let Some(home) = directories::BaseDirs::new().map(|dirs| dirs.home_dir().to_path_buf()) else { return Vec::new() };
    let mut roots: Vec<PathBuf> = ["Nextcloud", "Dropbox", "ownCloud", "Sync", "Syncthing", "OneDrive", "pCloudDrive", "Seafile"].iter().map(|n| home.join(n)).filter(|p| p.is_dir()).collect();
    let data = |name: &str, file: &str| std::env::var_os(name).map(|a| PathBuf::from(a).join(file)).unwrap_or_default();
    let read = |paths: &[PathBuf]| paths.iter().filter_map(|c| std::fs::read_to_string(c).ok()).collect::<Vec<_>>();
    let mut found: Vec<PathBuf> = Vec::new();
    for text in read(&[home.join(".config/Nextcloud/nextcloud.cfg"), home.join("Library/Preferences/Nextcloud/nextcloud.cfg"), data("APPDATA", "Nextcloud/nextcloud.cfg")]) {
        found.extend(text.lines().filter_map(|line| line.split_once("localPath=")).map(|(_, path)| PathBuf::from(path.trim().trim_end_matches('/'))));
    }
    for text in read(&[home.join(".config/syncthing/config.xml"), home.join(".local/state/syncthing/config.xml"), home.join("Library/Application Support/Syncthing/config.xml"), data("LOCALAPPDATA", "Syncthing/config.xml")]) {
        found.extend(syncthing_folders(&text).iter().map(|path| path.strip_prefix("~/").map_or_else(|| PathBuf::from(path), |rest| home.join(rest))));
    }
    for text in read(&[home.join(".dropbox/info.json"), data("APPDATA", "Dropbox/info.json"), data("LOCALAPPDATA", "Dropbox/info.json")]) {
        if let Ok(serde_json::Value::Object(accounts)) = serde_json::from_str::<serde_json::Value>(&text) {
            found.extend(accounts.values().filter_map(|account| account.get("path").and_then(serde_json::Value::as_str).map(PathBuf::from)));
        }
    }
    for path in found {
        if path.is_dir() && !roots.contains(&path) {
            roots.push(path);
        }
    }
    roots
}

fn syncthing_folders(xml: &str) -> Vec<String> {
    xml.split("<folder ")
        .skip(1)
        .filter_map(|element| {
            let tag = element.split('>').next()?;
            let (_, rest) = tag.split_once(" path=\"").or_else(|| tag.strip_prefix("path=\"").map(|rest| ("", rest)))?;
            let path = rest.split('"').next()?;
            Some(path.replace("&amp;", "&").replace("&apos;", "'").replace("&quot;", "\"").replace("&lt;", "<").replace("&gt;", ">"))
        })
        .collect()
}

/// The folder a sync app carries that holds `path`, by the list or by a mark of its own.
pub(crate) fn carried(path: &Path, roots: &[PathBuf]) -> Option<PathBuf> {
    if let Some(root) = roots.iter().find(|root| path.starts_with(root)) {
        return Some(root.clone());
    }
    path.ancestors().find(|dir| marked(dir)).map(Path::to_path_buf)
}

/// A sync app's own mark in a folder: Syncthing's `.stfolder`, Nextcloud's
/// and ownCloud's journal (`.sync_*.db`, `._sync_*.db`), Dropbox's `.dropbox`.
fn marked(dir: &Path) -> bool {
    if dir.join(".stfolder").exists() || dir.join(".dropbox").exists() || dir.join(".dropbox.cache").is_dir() {
        return true;
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return false };
    entries.filter_map(Result::ok).any(|entry| {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        (name.starts_with(".sync_") || name.starts_with("._sync_")) && name.ends_with(".db")
    })
}
