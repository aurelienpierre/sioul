// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! A site opened inside Sioul on Android (docs/android.md, "Sites"): a
//! screen of Sioul's own around the system's web view, one site at a time
//! (android/package/src/com/aurelienpierre/sioul/SiteScreen.java); or the
//! browser, where this device's web view cannot carry it. Each site's line
//! says which, and why (`opens_line`); this device's card in Settings ▸
//! Devices says the web view's version and what it lacks (`card_line`).
//! Rust decides, Java shows: Java says what the web view is (verb
//! "site-webview": its package, its version, and, when asked, each feature
//! it has), and opens the screen with the site and the words it shows in
//! Sioul's language (verb "site-open").
//!
//! On a computer nothing here is used: Qt WebEngine carries every site.

use crate::backend::{load_config, say, tr};
use serde::Deserialize;
use sioul_core::i18n::Translator;
use sioul_core::sites;
use std::collections::BTreeMap;

/// The oldest engine a site's screen takes: Chromium 120, of December 2023.
/// An older web view is no longer mended, and the sites a person keeps
/// (banks, offices, chats) refuse or break under it; a site then opens in
/// the browser, as before. The last web view of Android 9, 138 (2025),
/// passes; a computer's Qt WebEngine 6.11 is Chromium 140.
pub(crate) const OLDEST: u32 = 120;

/// What a web view must have for each help Sioul gives a site beyond its
/// page, by androidx.webkit's names of features (`WebViewFeature`): a site's
/// news, from a stand-in for web notifications set before the page's own
/// scripts and the channel that brings them back; a site silenced; pages
/// darkened. None is built in this phase: the card says what this device
/// could not carry when they come.
pub(crate) const HELPS: [(&str, &[&str]); 3] = [("news", &["DOCUMENT_START_SCRIPT", "WEB_MESSAGE_LISTENER"]), ("silence", &["MUTE_AUDIO"]), ("dark", &["ALGORITHMIC_DARKENING"])];

/// Where a file a site offers may be saved, in the order the question
/// offers them (SiteDownload.PLACES, the same): Downloads, a folder chosen
/// in Android's "Save as". Notes, Papers and Letters join once the screen
/// can hand a file to them (docs/sites.md, "On Android").
pub(crate) const PLACES: [&str; 2] = ["downloads", "choose"];

/// The features Java asks of the web view (SiteWeb.FEATURES, the same, in
/// this order: SiteScreenCheck): every one `HELPS` names, and separate
/// profiles, which would keep another sign-in screen's store apart.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) const FEATURES: [&str; 5] = ["DOCUMENT_START_SCRIPT", "WEB_MESSAGE_LISTENER", "MUTE_AUDIO", "ALGORITHMIC_DARKENING", "MULTI_PROFILE"];

/// This device's web view, as Java says it (SiteScreen.webView).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub(crate) struct WebView {
    /// Its package ("com.android.webview"…); "" when the device has none (disabled or removed).
    #[serde(default)]
    pub package: String,
    /// Its name, as the package gives it: "Android System WebView".
    #[serde(default)]
    pub label: String,
    /// Its version, as the package says it: "138.0.7204.179".
    #[serde(default)]
    pub version: String,
    /// What it can do, feature by feature; empty when not asked.
    #[serde(default)]
    pub features: BTreeMap<String, bool>,
    /// Where it draws pages: "apart" (a process of its own), "one" (Sioul's
    /// own process: a page's crash ends Sioul), "" when not known yet.
    #[serde(default)]
    pub process: String,
    /// Its engine's own version, the first number of "Chrome/…" in its user
    /// agent ("138"); "" when not known yet: the package's version is read then.
    #[serde(default)]
    pub engine: String,
}

/// Where a site opens on this device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Opens {
    /// In Sioul's own screen.
    Here,
    /// In the browser: the device has no web view.
    NoWebView,
    /// In the browser: its web view is older than `OLDEST`.
    TooOld,
    /// In the browser: Java did not answer, or the version says no number.
    Unknown,
    /// In the browser: the site's address is not https (a configuration written by hand).
    NotHttps,
    /// In the browser: the web view draws pages in Sioul's own process,
    /// where a page's crash would end Sioul (review V1).
    OneProcess,
}

/// The engine's version, the first number of the package's: "138.0.7204.179" → 138.
pub(crate) fn major(version: &str) -> Option<u32> {
    version.split('.').next()?.trim().parse().ok()
}

/// The engine's version: its own, from its user agent, when Java said it
/// (review V4: a maker's package may number itself otherwise); else the
/// package's first number.
pub(crate) fn engine_major(view: &WebView) -> Option<u32> {
    major(&view.engine).or_else(|| major(&view.version))
}

/// The version said in a line: the package's, and the engine's beside it
/// when its first number differs ("150.2 (Chromium 108)").
pub(crate) fn shown_version(view: &WebView) -> String {
    match (major(&view.engine), major(&view.version)) {
        (Some(engine), Some(package)) if engine != package => format!("{} (Chromium {engine})", view.version),
        (Some(engine), None) => format!("{} (Chromium {engine})", view.version).trim_start().to_string(),
        _ => view.version.clone(),
    }
}

/// Where a site at `url` opens, with this device's web view as Java said it (none: no answer).
pub(crate) fn opens(view: Option<&WebView>, url: &str) -> Opens {
    if !url.starts_with("https://") || sites::host_of(url).is_empty() {
        return Opens::NotHttps;
    }
    match view {
        None => Opens::Unknown,
        Some(view) if view.package.is_empty() => Opens::NoWebView,
        Some(view) => match engine_major(view) {
            Some(major) if major >= OLDEST && view.process == "one" => Opens::OneProcess,
            Some(major) if major >= OLDEST => Opens::Here,
            Some(_) => Opens::TooOld,
            None => Opens::Unknown,
        },
    }
}

/// A site's line in the list, in words: in Sioul, or in the browser and why.
pub(crate) fn opens_line(opens: Opens, view: Option<&WebView>, tr: &Translator) -> String {
    match opens {
        Opens::Here => tr.text("site-opens-here", None),
        Opens::NoWebView => tr.text("site-opens-none", None),
        Opens::TooOld => {
            let mut args = sioul_core::i18n::args();
            args.set("version", view.map(shown_version).unwrap_or_default());
            tr.text("site-opens-old", Some(&args))
        }
        Opens::Unknown => tr.text("site-opens-unknown", None),
        Opens::NotHttps => tr.text("site-opens-address", None),
        Opens::OneProcess => tr.text("site-opens-one-process", None),
    }
}

/// The helps this web view could not carry (`HELPS`'s names), by what it
/// answered; none when its features were not asked.
pub(crate) fn lacks(view: &WebView) -> Vec<&'static str> {
    if view.features.is_empty() {
        return Vec::new();
    }
    HELPS.iter().filter(|(_, needs)| needs.iter().any(|need| view.features.get(*need) != Some(&true))).map(|(help, _)| *help).collect()
}

/// This device's web view in one line, for its card in Settings ▸ Devices:
/// its name and version, where sites open, and what it cannot carry.
pub(crate) fn card_line(view: Option<&WebView>, tr: &Translator) -> String {
    let named = |id: &str, view: &WebView, lacks: Option<String>| {
        let mut args = sioul_core::i18n::args();
        args.set("name", if view.label.is_empty() { view.package.clone() } else { view.label.clone() });
        args.set("version", shown_version(view));
        if let Some(lacks) = lacks {
            args.set("lacks", lacks);
        }
        tr.text(id, Some(&args))
    };
    match (opens(view, "https://example.org"), view) {
        (Opens::Here, Some(view)) => {
            let lacking = lacks(view);
            if lacking.is_empty() {
                named("webview-line-here", view, None)
            } else {
                let words: Vec<String> = lacking.iter().map(|help| tr.text(&format!("webview-lack-{help}"), None)).collect();
                named("webview-line-lacks", view, Some(words.join(", ")))
            }
        }
        (Opens::TooOld, Some(view)) => named("webview-line-old", view, None),
        (Opens::OneProcess, Some(view)) => named("webview-line-one-process", view, None),
        (Opens::NoWebView, _) => tr.text("webview-line-none", None),
        _ => tr.text("webview-line-unknown", None),
    }
}

/// This device's web view, asked of Java; where it draws and its engine's
/// version when `process`, each feature too when `features` (either loads
/// the web view into the window's process, once); else what Java found
/// before. None on a computer, and when Java does not answer.
pub(crate) fn webview(features: bool, process: bool) -> Option<WebView> {
    if !cfg!(target_os = "android") {
        return None;
    }
    let said = crate::steps::java("site-webview", &serde_json::json!({ "features": features, "process": process }).to_string());
    serde_json::from_value(said).ok()
}

/// Each site's line on this device: in Sioul, or in the browser and why.
/// None on a computer, where every site opens in Sioul.
pub(crate) fn lines_here() -> Option<impl Fn(&str) -> String> {
    if !cfg!(target_os = "android") {
        return None;
    }
    let view = webview(false, false);
    Some(move |url: &str| opens_line(opens(view.as_ref(), url), view.as_ref(), tr()))
}

/// This device's card line (Settings ▸ Devices); "" on a computer.
pub(crate) fn card_line_here() -> String {
    if !cfg!(target_os = "android") {
        return String::new();
    }
    card_line(webview(true, true).as_ref(), tr())
}

/// The words the site's screen shows, in Sioul's language: Java keeps none of its own.
pub(crate) fn words(tr: &Translator) -> serde_json::Value {
    let keys = [
        ("back", "site-back"),
        ("reload", "site-reload"),
        ("more", "ui-more"),
        ("close", "ui-close"),
        ("browser", "site-screen-browser"),
        ("share", "site-screen-share"),
        ("find", "site-screen-find"),
        ("find_none", "site-screen-find-none"),
        ("previous", "site-screen-previous"),
        ("next", "site-screen-next"),
        ("close_window", "site-screen-close-window"),
        ("asked_camera", "site-screen-asked-camera"),
        ("asked_microphone", "site-screen-asked-microphone"),
        ("asked_location", "site-screen-asked-location"),
        ("asked_media", "site-screen-asked-media"),
        ("http_auth", "site-screen-http-auth"),
        ("download_page", "site-screen-download-page"),
        ("save_title", "site-screen-save-title"),
        ("save_name", "site-screen-save-name"),
        ("save_program", "site-screen-save-program"),
        ("save_where", "site-screen-save-where"),
        ("place_downloads", "site-screen-place-downloads"),
        ("place_choose", "site-screen-place-choose"),
        ("save", "ui-save"),
        ("cancel", "ui-cancel"),
        ("too_many", "site-screen-too-many"),
        ("open", "site-screen-open"),
        ("stop", "site-screen-stop"),
        ("no_app", "site-screen-no-app"),
        ("no_chooser", "site-screen-no-chooser"),
        ("why_form", "site-screen-why-form"),
        ("why_elsewhere", "site-screen-why-elsewhere"),
        ("why_page", "site-screen-why-page"),
        ("why_large", "site-screen-why-large"),
        ("why_room", "site-screen-why-room"),
        ("why_network", "site-screen-why-network"),
        ("why_write", "site-screen-why-write"),
        ("why_cancelled", "site-screen-why-cancelled"),
        ("ended", "site-screen-ended"),
        ("no_browser", "site-screen-no-browser"),
        ("not_https", "site-screen-not-https"),
        ("one_process", "site-screen-one-process"),
        ("no_app_link", "site-screen-no-app-link"),
        ("loaded_again", "site-screen-loaded-again"),
        ("ok", "ui-ok"),
        ("close_site", "site-screen-close-site"),
        ("leave_ask", "site-screen-leave-ask"),
        ("leave", "site-screen-leave"),
        ("stay", "site-screen-stay"),
        // The vault's two lines in the menu, and what a fill the screen refuses says (sitefill.rs).
        ("fill", "site-screen-fill"),
        ("choose_login", "site-screen-choose-login"),
        ("fill_changed", "site-screen-fill-changed"),
        ("fill_https", "site-screen-fill-https"),
    ];
    let mut words: serde_json::Map<String, serde_json::Value> = keys.iter().map(|(key, id)| (key.to_string(), tr.text(id, None).into())).collect();
    // Java puts its own words in place of these marks: what was asked, the file's name.
    let mut args = sioul_core::i18n::args();
    args.set("what", "{what}");
    words.insert("asked".into(), tr.text("site-screen-asked", Some(&args)).into());
    // A file's words, its name and where it went filled by Java (review F3).
    for (key, id) in [("saving", "site-screen-saving"), ("saved", "site-screen-saved"), ("saved_choose", "site-screen-saved-choose"), ("saved_own", "site-screen-saved-own"), ("save_failed", "site-screen-save-failed")] {
        let mut args = sioul_core::i18n::args();
        args.set("file", "{file}");
        args.set("place", "{place}");
        args.set("why", "{why}");
        words.insert(key.into(), tr.text(id, Some(&args)).into());
    }
    // The question before a download: its size and host filled by Java (review F4).
    for (key, id) in [("save_from", "site-screen-save-from"), ("save_size_from", "site-screen-save-size-from")] {
        let mut args = sioul_core::i18n::args();
        args.set("size", "{size}");
        args.set("host", "{host}");
        words.insert(key.into(), tr.text(id, Some(&args)).into());
    }
    // A page's dialog: its host as the title, filled by Java (review N5).
    let mut args = sioul_core::i18n::args();
    args.set("host", "{host}");
    words.insert("page_says".into(), tr.text("site-screen-page-says", Some(&args)).into());
    // The bar's second line on another site's page: the site it came from (review N2).
    let mut args = sioul_core::i18n::args();
    args.set("name", "{name}");
    words.insert("from".into(), tr.text("site-screen-from", Some(&args)).into());
    words.into()
}

/// What the screen is given for a site (verb "site-open"): its id, name and
/// address, the registrable domain its links stay in (`sites::domain_of`),
/// Sioul's colours (`look`, from the page: JSON), the words, and whether a
/// Bitwarden account is set on this device (`vault`: the menu's two lines
/// of the vault show only then, sitefill.rs).
pub(crate) fn asked(id: &str, name: &str, url: &str, look: &str, vault: bool, tr: &Translator) -> serde_json::Value {
    let look: serde_json::Value = serde_json::from_str(look).unwrap_or_else(|_| serde_json::json!({}));
    serde_json::json!({
        "id": id,
        "name": name,
        "url": url,
        "domain": sites::domain_of(&sites::host_of(url)),
        "look": look,
        "words": words(tr),
        "places": PLACES,
        "vault": vault,
    })
}

/// A site opened from the Sites page, a link, the Porch or a notice: in
/// Sioul's own screen where this device's web view carries it, else the
/// address for the browser. Returns {"here": true}, or {"browser": address,
/// "problem": words when the screen could not be shown}, or {"problem"} for
/// a site that is gone.
pub(crate) fn open(id: &str, look: &str) -> String {
    let Some(site) = sites::sites(&load_config()).into_iter().find(|s| s.id == id) else {
        return serde_json::json!({ "problem": tr().text("site-gone", None) }).to_string();
    };
    if !cfg!(target_os = "android") {
        return serde_json::json!({ "browser": site.url }).to_string();
    }
    // Where it draws asked now: the screen would load the web view into this process anyway.
    let view = webview(false, true);
    if opens(view.as_ref(), &site.url) != Opens::Here {
        return serde_json::json!({ "browser": site.url }).to_string();
    }
    if crate::steps::java("site-open", &asked(&site.id, &site.name, &site.url, look, crate::sites::account_set(), tr()).to_string()) == true {
        serde_json::json!({ "here": true }).to_string()
    } else {
        serde_json::json!({ "browser": site.url, "problem": say("site-screen-refused", &[]) }).to_string()
    }
}

/// What this device's web view forgot of a site removed (verb
/// "site-forget", SiteWeb.forget): everything stored for its site, its
/// cookies alone, or nothing (Java silent, or the web view not loading).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Forgot {
    All,
    Cookies,
    Nothing,
}

/// Java's answer read: {"forgot": "all" | "cookies" | "none"}.
pub(crate) fn forgot_of(said: &serde_json::Value) -> Forgot {
    match said.get("forgot").and_then(|v| v.as_str()) {
        Some("all") => Forgot::All,
        Some("cookies") => Forgot::Cookies,
        _ => Forgot::Nothing,
    }
}

/// The line a removal says on Android: nothing when the site's sign-in
/// on this device is forgotten whole; else what stays, and how to clear it.
pub(crate) fn forgot_line(forgot: Forgot, name: &str, tr: &Translator) -> String {
    let mut args = sioul_core::i18n::args();
    args.set("name", name.to_string());
    match forgot {
        Forgot::All => String::new(),
        Forgot::Cookies => tr.text("site-forget-cookies", Some(&args)),
        Forgot::Nothing => tr.text("site-forget-none", Some(&args)),
    }
}

/// A site removed on Android forgets its sign-in on this device (review
/// K1): Java asks the web view, on Android's main thread; the line says
/// what stays. "" on a computer, where each site's profile is its own.
pub(crate) fn forget_line(url: &str, name: &str) -> String {
    if !cfg!(target_os = "android") {
        return String::new();
    }
    let said = crate::steps::java("site-forget", &serde_json::json!({ "url": url, "domain": sites::domain_of(&sites::host_of(url)) }).to_string());
    forgot_line(forgot_of(&said), name, tr())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(version: &str, features: &[(&str, bool)]) -> WebView {
        WebView { package: "com.android.webview".into(), label: "Android System WebView".into(), version: version.into(), features: features.iter().map(|(k, v)| (k.to_string(), *v)).collect(), ..WebView::default() }
    }

    /// A web view that draws in Sioul's own process sends sites to the
    /// browser, with that reason in the line and the card (review V1);
    /// one that draws apart, or has not said, carries them.
    #[test]
    fn a_web_view_without_a_process_of_its_own_sends_sites_to_the_browser() {
        let url = "https://app.example.org/inbox";
        let one = WebView { process: "one".into(), ..view("138.0.7204.179", &[]) };
        assert_eq!(opens(Some(&one), url), Opens::OneProcess);
        assert_eq!(opens(Some(&WebView { process: "apart".into(), ..view("138.0.7204.179", &[]) }), url), Opens::Here);
        assert_eq!(opens(Some(&view("138.0.7204.179", &[])), url), Opens::Here, "not said yet");
        assert_eq!(opens(Some(&WebView { process: "one".into(), ..view("108.0.1", &[]) }), url), Opens::TooOld, "too old first");
        for language in ["en", "fr"] {
            let tr = Translator::new(language);
            let line = opens_line(Opens::OneProcess, Some(&one), &tr);
            assert!(!line.starts_with("site-opens") && line != opens_line(Opens::Here, Some(&one), &tr), "{line}");
        }
        assert!(card_line(Some(&one), &Translator::new("en")).contains("browser"));
        assert!(card_line(Some(&one), &Translator::new("en")).contains("crash"));
    }

    /// The engine's own version decides where it says one (review V4): a
    /// package numbered otherwise is read by its engine, and the lines say both.
    #[test]
    fn the_engine_s_own_version_decides() {
        let url = "https://app.example.org/inbox";
        let renumbered = WebView { engine: "108".into(), ..view("150.2.1", &[]) };
        assert_eq!(opens(Some(&renumbered), url), Opens::TooOld);
        assert_eq!(shown_version(&renumbered), "150.2.1 (Chromium 108)");
        assert!(opens_line(Opens::TooOld, Some(&renumbered), &Translator::new("en")).contains("150.2.1 (Chromium 108)"));
        let current = WebView { engine: "142".into(), ..view("2.1.0", &[]) };
        assert_eq!(opens(Some(&current), url), Opens::Here);
        assert_eq!(shown_version(&view("142.0.7444.90", &[])), "142.0.7444.90", "one number, said once");
        assert_eq!(shown_version(&WebView { engine: "142".into(), ..view("142.0.7444.90", &[]) }), "142.0.7444.90");
    }

    /// A site opens in Sioul where this device's web view is Chromium 120 or
    /// later, Android 9's last (138) among them; in the browser where there
    /// is none, where it is older, where Java said nothing or a version
    /// without a number, and for an address that is not https.
    #[test]
    fn a_site_opens_in_sioul_where_the_web_view_carries_it() {
        let url = "https://app.example.org/inbox";
        assert_eq!(opens(Some(&view("138.0.7204.179", &[])), url), Opens::Here, "Android 9's last web view");
        assert_eq!(opens(Some(&view("120.0.6099.0", &[])), url), Opens::Here, "the oldest taken");
        assert_eq!(opens(Some(&view("119.0.6045.193", &[])), url), Opens::TooOld);
        assert_eq!(opens(Some(&view("108.0.5359.128", &[])), url), Opens::TooOld, "a de-Googled system's frozen web view");
        assert_eq!(opens(Some(&WebView::default()), url), Opens::NoWebView, "none, or turned off");
        assert_eq!(opens(None, url), Opens::Unknown, "Java did not answer");
        assert_eq!(opens(Some(&view("dev", &[])), url), Opens::Unknown, "no number");
        assert_eq!(opens(Some(&view("150.0.1.1", &[])), "http://app.example.org"), Opens::NotHttps);
        assert_eq!(opens(Some(&view("150.0.1.1", &[])), "https://"), Opens::NotHttps, "no host");
        assert_eq!(major(" 142.0"), Some(142));
    }

    /// Each line says where the site opens, and why in the browser, in
    /// English and French; the version named when it is too old.
    #[test]
    fn each_site_s_line_says_where_it_opens() {
        let old = view("108.0.5359.128", &[]);
        for language in ["en", "fr"] {
            let tr = Translator::new(language);
            let lines: Vec<String> = [Opens::Here, Opens::NoWebView, Opens::TooOld, Opens::Unknown, Opens::NotHttps].iter().map(|o| opens_line(*o, Some(&old), &tr)).collect();
            for line in &lines {
                assert!(!line.is_empty() && !line.starts_with("site-opens-"), "{language}: {line}");
            }
            assert!(lines[2].contains("108.0.5359.128"), "{}", lines[2]);
            assert_eq!(lines.iter().collect::<std::collections::BTreeSet<_>>().len(), lines.len(), "each reason its own words");
        }
        assert_eq!(opens_line(Opens::Here, None, &Translator::new("en")), "Opens in Sioul.");
    }

    /// The card's line: the web view's name and version, where sites open,
    /// and the helps it could not carry, named by what is lost; nothing said
    /// lacking while its features were not asked; one line for none, for an
    /// old one and for no answer.
    #[test]
    fn the_card_says_the_web_view_and_what_it_lacks() {
        let tr = Translator::new("en");
        let all: Vec<(&str, bool)> = FEATURES.iter().map(|f| (*f, true)).collect();
        assert_eq!(card_line(Some(&view("142.0.7444.90", &all)), &tr), "Web view: Android System WebView 142.0.7444.90. Sites open in Sioul.");
        let some = view("125.0.1", &[("DOCUMENT_START_SCRIPT", true), ("WEB_MESSAGE_LISTENER", false), ("MUTE_AUDIO", true), ("ALGORITHMIC_DARKENING", false)]);
        assert_eq!(lacks(&some), ["news", "dark"]);
        assert_eq!(card_line(Some(&some), &tr), "Web view: Android System WebView 125.0.1. Sites open in Sioul; it cannot carry a site's news, darker pages.");
        assert!(lacks(&view("125.0.1", &[])).is_empty(), "not asked: nothing said lacking");
        assert_eq!(lacks(&view("125.0.1", &[("MUTE_AUDIO", true)])), ["news", "dark"], "a feature not said is lacking");
        assert!(card_line(Some(&view("108.0.1", &all)), &tr).contains("too old"));
        assert_eq!(card_line(Some(&WebView::default()), &tr), "No web view on this device: sites open in your browser.");
        assert!(card_line(None, &tr).starts_with("Sioul could not ask"));
        let unnamed = WebView { label: String::new(), ..view("130.0", &all) };
        assert!(card_line(Some(&unnamed), &tr).contains("com.android.webview 130.0"), "the package when it gives no name");
        let fr = card_line(Some(&some), &Translator::new("fr"));
        assert!(fr.starts_with("Vue web\u{202f}: Android System WebView 125.0.1.") && fr.contains("les nouvelles d’un site"), "{fr}");
    }

    /// The domain a site's links stay in, as the screen reads it too
    /// (SiteWeb.domainOf; SiteScreenCheck holds these same pairs).
    #[test]
    fn the_domain_a_site_s_links_stay_in() {
        for (host, domain) in [("secure.bank.example.co.uk", "example.co.uk"), ("id.example.gouv.fr", "example.gouv.fr"), ("mail.example.me", "example.me"), ("example.me", "example.me"), ("localhost", "localhost"), ("a.b.c.example.org", "example.org"), ("www.example-public.fr", "example-public.fr")] {
            assert_eq!(sites::domain_of(host), domain, "{host}");
        }
    }

    /// A site removed on Android: nothing said when its sign-in on this
    /// device is forgotten whole; what stays, and how to clear it, when
    /// only its cookies were, or nothing; in English and French.
    #[test]
    fn a_removed_site_says_what_of_its_sign_in_stays() {
        assert_eq!(forgot_of(&serde_json::json!({ "forgot": "all" })), Forgot::All);
        assert_eq!(forgot_of(&serde_json::json!({ "forgot": "cookies" })), Forgot::Cookies);
        assert_eq!(forgot_of(&serde_json::json!({ "forgot": "none" })), Forgot::Nothing);
        assert_eq!(forgot_of(&serde_json::Value::Null), Forgot::Nothing, "Java silent");
        for language in ["en", "fr"] {
            let tr = Translator::new(language);
            assert_eq!(forgot_line(Forgot::All, "My bank", &tr), "");
            let cookies = forgot_line(Forgot::Cookies, "My bank", &tr);
            let nothing = forgot_line(Forgot::Nothing, "My bank", &tr);
            for line in [&cookies, &nothing] {
                assert!(line.contains("My bank") && !line.starts_with("site-forget"), "{language}: {line}");
            }
            assert_ne!(cookies, nothing);
        }
        assert_eq!(forget_line("https://bank.example.org", "My bank"), "", "nothing to forget on a computer");
    }

    /// The screen is given the site, the domain its links stay in, Sioul's
    /// colours as the page gave them, and every word it shows, filled, with
    /// the marks Java fills itself; a page's colours that are not JSON give none.
    #[test]
    fn the_screen_is_given_the_site_and_its_words() {
        let tr = Translator::new("fr");
        let given = |look: &str| asked("bank", "My bank", "https://secure.bank.example.co.uk/login", look, true, &tr);
        let asked = given(r##"{"dark":true,"background":"#1e1f1c"}"##);
        assert_eq!(asked["vault"], true, "a Bitwarden account set: the vault's lines show");
        assert_eq!(super::asked("bank", "My bank", "https://secure.bank.example.co.uk/login", "{}", false, &tr)["vault"], false);
        assert_eq!((asked["id"].as_str(), asked["name"].as_str(), asked["url"].as_str()), (Some("bank"), Some("My bank"), Some("https://secure.bank.example.co.uk/login")));
        assert_eq!(asked["domain"], "example.co.uk");
        assert_eq!(asked["look"]["dark"], true);
        let words = asked["words"].as_object().unwrap();
        assert!(words.len() >= 20);
        for (key, value) in words {
            let value = value.as_str().unwrap();
            assert!(!value.is_empty() && !value.starts_with("site-") && !value.starts_with("ui-"), "{key}: {value}");
        }
        assert!(words["asked"].as_str().unwrap().contains("{what}"));
        for key in ["saving", "saved", "saved_choose", "saved_own", "save_failed"] {
            assert!(words[key].as_str().unwrap().contains("{file}"), "{key}");
        }
        assert!(words["saved"].as_str().unwrap().contains("{place}") && words["save_failed"].as_str().unwrap().contains("{why}"));
        assert!(words["save_size_from"].as_str().unwrap().contains("{size}") && words["save_size_from"].as_str().unwrap().contains("{host}") && words["save_from"].as_str().unwrap().contains("{host}"));
        assert_eq!(words["place_choose"], "Un dossier que je choisis\u{2026}");
        assert_eq!(asked["places"], serde_json::json!(["downloads", "choose"]));
        assert!(words["from"].as_str().unwrap().contains("{name}") && words["from"].as_str().unwrap().starts_with("depuis"));
        assert_eq!(words["browser"], "Ouvrir dans le navigateur");
        assert_eq!((words["fill"].as_str(), words["choose_login"].as_str()), (Some("Remplir depuis Bitwarden"), Some("Choisir un identifiant\u{2026}")));
        assert!(words["fill_changed"].as_str().unwrap().contains("rien n\u{2019}a \u{e9}t\u{e9} rempli") && words["fill_https"].as_str().unwrap().contains("https"));
        assert_eq!(given("not json")["look"], serde_json::json!({}));
    }
}
