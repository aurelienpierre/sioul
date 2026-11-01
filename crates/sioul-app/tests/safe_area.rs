// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Every popup drawn over the whole window (`parent: Overlay.overlay`) keeps
//! to the window's safe area: it is a SafeDialog or a SafePopup, which take
//! their margins from Qt's SafeArea (Android's navigation buttons and status
//! bar, a notch), so that a new dialog cannot forget it (review of 4c77d0c,
//! L9). DoseTaken.qml, the dose path, is left as it is.

use std::path::Path;

#[test]
fn every_dialog_over_the_window_keeps_to_its_safe_area() {
    let qml = Path::new(env!("CARGO_MANIFEST_DIR")).join("qml");
    let mut found = 0;
    let mut wrong = Vec::new();
    for entry in std::fs::read_dir(&qml).unwrap().filter_map(Result::ok) {
        let path = entry.path();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if !name.ends_with(".qml") || name == "DoseTaken.qml" {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            if line.trim() != "parent: Overlay.overlay" {
                continue;
            }
            found += 1;
            let indent = line.len() - line.trim_start().len();
            // The object it is in: the nearest declaration above it, less indented.
            let enclosing = lines[..i].iter().rev().find_map(|l| {
                let trimmed = l.trim();
                let declares = trimmed.ends_with('{') && trimmed.chars().next().is_some_and(char::is_uppercase) && !trimmed.contains(':');
                (declares && l.len() - l.trim_start().len() < indent).then(|| trimmed.trim_end_matches('{').trim().to_string())
            });
            // Qt's own popups over the window: one of Sioul's safe ones instead (an item, the bars stood in for, is no popup).
            if matches!(enclosing.as_deref(), Some("Dialog" | "Popup" | "Drawer" | "Menu" | "ToolTip") | None) {
                wrong.push(format!("{name}:{}: {}", i + 1, enclosing.unwrap_or_default()));
            }
        }
    }
    assert!(found >= 40, "{found} popups over the window found");
    assert!(wrong.is_empty(), "over the window, outside the safe area: {wrong:?}");
}
