// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The sites' web profile outlives every view that uses it, as Qt asks of a
//! WebEngineProfile: its holder (`sitesWebLoader`, SitesWeb.qml) is the last
//! object declared in the window (main.qml), so it is made after the pages,
//! whose sites, pop-ups and vault dialogs hold the views, and destroyed after
//! them when Sioul quits (a window's objects go in the order they were made).
//! The extensions' sign-in window never shares it: its profile is its own.

use std::path::Path;

#[test]
fn the_sites_web_profile_is_held_by_the_window_s_last_object() {
    let main = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("qml/main.qml")).unwrap();
    let lines: Vec<&str> = main.lines().collect();
    let holder = lines.iter().position(|l| l.trim() == "id: sitesWebLoader").expect("the holder's loader");
    let pages: Vec<usize> = lines.iter().enumerate().filter(|(_, l)| l.trim().starts_with("id: ") && l.trim().ends_with("PageLoader")).map(|(i, _)| i).collect();
    assert!(pages.len() >= 15, "the pages' loaders: {pages:?}");
    assert!(pages.iter().all(|&page| page < holder), "made after every page");
    // Declared in the window itself (four spaces in), and nothing declared there after it.
    assert_eq!(lines[holder - 1], "    Loader {", "a child of the window");
    let after: Vec<&str> = lines[holder + 1..].iter().copied().filter(|l| l.starts_with("    ") && !l.starts_with("     ") && l.trim() != "}").collect();
    assert!(after.is_empty(), "declared after the holder: {after:?}");
}

/// The extensions' sign-in window (SignInWindow.qml) keeps a profile of its
/// own, never the sites' (docs/extensions.md, "A sign-in"): one made for the
/// window, off the record (no storage name: in memory only, gone with it),
/// given to its one view; nothing in it names the sites' profile, its holder,
/// or Qt's default profile. The sites' is the named, persistent one.
#[test]
fn the_sign_in_window_s_profile_is_its_own() {
    let read = |file: &str| std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(file)).unwrap();
    let window = read("qml/SignInWindow.qml");
    let code: String = window.lines().filter(|l| !l.trim_start().starts_with("//")).map(|l| format!("{l}\n")).collect();
    assert_eq!(code.matches("WebEngineProfilePrototype {").count(), 1, "one profile, made by the window");
    assert!(!code.contains("storageName"), "off the record");
    assert!(code.contains("persistentCookiesPolicy: WebEngineProfile.NoPersistentCookies"));
    assert_eq!(code.matches("profile:").count(), 1, "one view given a profile");
    assert!(code.contains("viewKind.createObject(holder, { profile: made, url: open })"), "the view given the window's own");
    for shared in ["sioul-sites", "sitesWeb", "SitesWeb", "defaultProfile", "WebEngine.defaultProfile"] {
        assert!(!code.contains(shared), "the sign-in names {shared}");
    }
    assert!(read("qml/SitesWeb.qml").contains("storageName: \"sioul-sites\""), "the sites' profile is the named one");
}
