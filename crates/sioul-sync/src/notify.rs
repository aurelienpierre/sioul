// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The one exception to the admin windows: a quiet desktop notification for a
//! verified code, with the code in it (docs/porch.md, "Right now").

#[cfg(not(target_os = "android"))]
use notify_rust::{Notification, Timeout};
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
use notify_rust::{Hint, Urgency};

/// How long the notification stays: about the life of a code.
#[cfg(not(target_os = "android"))]
const SHOWN: u32 = 10 * 60 * 1000;

/// Shows a code: `title` says what and from whom, `body` the code and how long
/// it lasts. No sound. With `copy`, the notification offers a button labelled
/// with its text, and its action runs when the button is pressed; the
/// notification then lives on its own thread, which waits for the button.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
pub fn code(title: &str, body: &str, copy: Option<(String, Box<dyn FnOnce() + Send>)>) -> Result<(), String> {
    let Some((label, on_copy)) = copy else {
        return build(title, body, None).show().map(drop).map_err(|e| e.to_string());
    };
    let (title, body) = (title.to_string(), body.to_string());
    // The D-Bus handle cannot change threads, so it is made where it waits.
    std::thread::spawn(move || {
        let Ok(handle) = build(&title, &body, Some(&label)).show() else { return };
        let mut on_copy = Some(on_copy);
        let _ = handle.wait_for_action(|action: &str| {
            if action == "copy"
                && let Some(run) = on_copy.take()
            {
                run();
            }
        });
    });
    Ok(())
}

/// A gentle reminder (a dose to take): one notification, no sound, with one
/// button whose action runs when pressed.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
pub fn remind(title: &str, body: &str, action: Option<(String, Box<dyn FnOnce() + Send>)>) -> Result<(), String> {
    let Some((label, on_press)) = action else {
        return build(title, body, None).icon("appointment-soon").show().map(drop).map_err(|e| e.to_string());
    };
    let (title, body) = (title.to_string(), body.to_string());
    std::thread::spawn(move || {
        let Ok(handle) = build(&title, &body, Some(&label)).icon("appointment-soon").show() else { return };
        let mut on_press = Some(on_press);
        let _ = handle.wait_for_action(|action: &str| {
            if action == "copy"
                && let Some(run) = on_press.take()
            {
                run();
            }
        });
    });
    Ok(())
}

/// Windows and macOS: the reminder is in the text; no button here.
#[cfg(any(windows, target_os = "macos"))]
pub fn remind(title: &str, body: &str, _action: Option<(String, Box<dyn FnOnce() + Send>)>) -> Result<(), String> {
    Notification::new().appname("Sioul").summary(title).body(body).timeout(Timeout::Milliseconds(SHOWN)).show().map(drop).map_err(|e| e.to_string())
}

/// Windows and macOS: the code is in the text; their notifications carry no button here.
#[cfg(any(windows, target_os = "macos"))]
pub fn code(title: &str, body: &str, _copy: Option<(String, Box<dyn FnOnce() + Send>)>) -> Result<(), String> {
    Notification::new().appname("Sioul").summary(title).body(body).timeout(Timeout::Milliseconds(SHOWN)).show().map(drop).map_err(|e| e.to_string())
}

/// Android: no notifications yet (Android's own are reached through Java);
/// the code shows in the window only.
#[cfg(target_os = "android")]
pub fn code(_title: &str, _body: &str, _copy: Option<(String, Box<dyn FnOnce() + Send>)>) -> Result<(), String> {
    Err("notifications are not made on Android yet".to_string())
}

/// Android: no notifications yet; a reminder is not shown (docs/android.md).
#[cfg(target_os = "android")]
pub fn remind(_title: &str, _body: &str, _action: Option<(String, Box<dyn FnOnce() + Send>)>) -> Result<(), String> {
    Err("notifications are not made on Android yet".to_string())
}

#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn build(title: &str, body: &str, copy_label: Option<&str>) -> Notification {
    let mut notification = Notification::new();
    notification
        .appname("Sioul")
        .summary(title)
        .body(&body_text(body, &notify_rust::get_capabilities().unwrap_or_default()))
        .icon("dialog-password")
        .hint(Hint::SuppressSound(true))
        // Sioul filters its own notifications already: what it sends always gets through.
        .urgency(Urgency::Critical)
        .timeout(Timeout::Milliseconds(SHOWN));
    if let Some(label) = copy_label {
        notification.action("copy", label);
    }
    notification
}

/// A body as the notification server reads it. Most read markup there
/// ("body-markup": `<b>`, `<a href>`, `<img src>`), and bodies carry a mail's
/// or a site's words: there, `&`, `<` and `>` are written as entities, so
/// that the words show as they came and nothing in them is a link or a
/// picture to fetch. A server that reads no markup shows the text as it is.
/// The summary is plain text for every server (the specification), and goes as it is.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn body_text(body: &str, capabilities: &[String]) -> String {
    // A server that could not be asked is taken to read markup: entities at worst.
    if capabilities.is_empty() || capabilities.iter().any(|c| c == "body-markup") {
        body.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
    } else {
        body.to_string()
    }
}

#[cfg(all(test, unix, not(any(target_os = "macos", target_os = "android"))))]
mod tests {
    use super::*;

    #[test]
    fn bodies_are_words_not_markup() {
        let markup = vec!["actions".to_string(), "body-markup".to_string()];
        let body = "Code 482913 <img src=\"https://example.org/t.png\"> & <a href=\"https://example.org\">here</a>";
        let shown = body_text(body, &markup);
        assert!(!shown.contains('<') && !shown.contains('>'), "{shown}");
        assert!(shown.starts_with("Code 482913 &lt;img") && shown.contains(" &amp; "), "{shown}");
        // Not asked, read as markup; a server without markup gets the words as they are.
        assert_eq!(body_text("a < b", &[]), "a &lt; b");
        assert_eq!(body_text("a < b & c", &["actions".to_string()]), "a < b & c");
    }
}
