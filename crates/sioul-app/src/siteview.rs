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
}

/// The engine's version, the first number of the package's: "138.0.7204.179" → 138.
pub(crate) fn major(version: &str) -> Option<u32> {
    version.split('.').next()?.trim().parse().ok()
}

/// Where a site at `url` opens, with this device's web view as Java said it (none: no answer).
pub(crate) fn opens(view: Option<&WebView>, url: &str) -> Opens {
    if !url.starts_with("https://") || sites::host_of(url).is_empty() {
        return Opens::NotHttps;
    }
    match view {
        None => Opens::Unknown,
        Some(view) if view.package.is_empty() => Opens::NoWebView,
        Some(view) => match major(&view.version) {
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
            args.set("version", view.map(|v| v.version.clone()).unwrap_or_default());
            tr.text("site-opens-old", Some(&args))
        }
        Opens::Unknown => tr.text("site-opens-unknown", None),
        Opens::NotHttps => tr.text("site-opens-address", None),
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
        args.set("version", view.version.clone());
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
        (Opens::NoWebView, _) => tr.text("webview-line-none", None),
        _ => tr.text("webview-line-unknown", None),
    }
}

/// This device's web view, asked of Java; with each feature when `features`
/// (which loads the web view into the window's process, once). None on a
/// computer, and when Java does not answer.
pub(crate) fn webview(features: bool) -> Option<WebView> {
    if !cfg!(target_os = "android") {
        return None;
    }
    let said = crate::steps::java("site-webview", &serde_json::json!({ "features": features }).to_string());
    serde_json::from_value(said).ok()
}

/// Each site's line on this device: in Sioul, or in the browser and why.
/// None on a computer, where every site opens in Sioul.
pub(crate) fn lines_here() -> Option<impl Fn(&str) -> String> {
    if !cfg!(target_os = "android") {
        return None;
    }
    let view = webview(false);
    Some(move |url: &str| opens_line(opens(view.as_ref(), url), view.as_ref(), tr()))
}

/// This device's card line (Settings ▸ Devices); "" on a computer.
pub(crate) fn card_line_here() -> String {
    if !cfg!(target_os = "android") {
        return String::new();
    }
    card_line(webview(true).as_ref(), tr())
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
    // The bar's second line on another site's page: the site it came from (review N2).
    let mut args = sioul_core::i18n::args();
    args.set("name", "{name}");
    words.insert("from".into(), tr.text("site-screen-from", Some(&args)).into());
    words.into()
}

/// What the screen is given for a site (verb "site-open"): its id, name and
/// address, the registrable domain its links stay in (`sites::domain_of`),
/// Sioul's colours (`look`, from the page: JSON) and the words.
pub(crate) fn asked(id: &str, name: &str, url: &str, look: &str, tr: &Translator) -> serde_json::Value {
    let look: serde_json::Value = serde_json::from_str(look).unwrap_or_else(|_| serde_json::json!({}));
    serde_json::json!({
        "id": id,
        "name": name,
        "url": url,
        "domain": sites::domain_of(&sites::host_of(url)),
        "look": look,
        "words": words(tr),
        "places": PLACES,
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
    let view = webview(false);
    if opens(view.as_ref(), &site.url) != Opens::Here {
        return serde_json::json!({ "browser": site.url }).to_string();
    }
    if crate::steps::java("site-open", &asked(&site.id, &site.name, &site.url, look, tr()).to_string()) == true {
        serde_json::json!({ "here": true }).to_string()
    } else {
        serde_json::json!({ "browser": site.url, "problem": say("site-screen-refused", &[]) }).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(version: &str, features: &[(&str, bool)]) -> WebView {
        WebView { package: "com.android.webview".into(), label: "Android System WebView".into(), version: version.into(), features: features.iter().map(|(k, v)| (k.to_string(), *v)).collect() }
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
        for (host, domain) in [("secure.bank.example.co.uk", "example.co.uk"), ("app.franceconnect.gouv.fr", "franceconnect.gouv.fr"), ("mail.proton.me", "proton.me"), ("proton.me", "proton.me"), ("localhost", "localhost"), ("a.b.c.example.org", "example.org"), ("www.service-public.fr", "service-public.fr")] {
            assert_eq!(sites::domain_of(host), domain, "{host}");
        }
    }

    /// The screen is given the site, the domain its links stay in, Sioul's
    /// colours as the page gave them, and every word it shows, filled, with
    /// the marks Java fills itself; a page's colours that are not JSON give none.
    #[test]
    fn the_screen_is_given_the_site_and_its_words() {
        let tr = Translator::new("fr");
        let given = |look: &str| asked("bank", "My bank", "https://secure.bank.example.co.uk/login", look, &tr);
        let asked = given(r##"{"dark":true,"background":"#1e1f1c"}"##);
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
        assert_eq!(given("not json")["look"], serde_json::json!({}));
    }
}
