// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The sites' web profile outlives every view that uses it, as Qt asks of a
//! WebEngineProfile: its holder (`sitesWebLoader`, SitesWeb.qml) is the last
//! object declared in the window (main.qml), so it is made after the pages,
//! whose sites, pop-ups and vault dialogs hold the views, and destroyed after
//! them when Sioul quits (a window's objects go in the order they were made).

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
