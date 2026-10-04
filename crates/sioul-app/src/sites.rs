// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Sites kept in Sioul, for the window: the list, their notifications (shown
//! at once for a site in real time, else kept for the Porch), the site a
//! message announces, and logins filled from Bitwarden.
//!
//! Bitwarden is read here, by Sioul itself (`sioul_sync::bitwarden`): nothing
//! to install, on every system. Your master password is asked when a login is
//! wanted, used to open the vault and dropped; the logins stay in memory until
//! the window closes or the vault is locked.

use crate::backend::{QtThread, Shared, json, load_config, say, tr};
use serde::Serialize;
use sioul_core::sites::{self, Notice, Notices, Site};
use sioul_sync::bitwarden;
use std::sync::{Arc, Mutex};

#[derive(Serialize)]
struct SiteRow {
    #[serde(flatten)]
    site: Site,
    /// It notified something not seen yet: a small dot, never a number.
    news: bool,
    /// What it is for fits the hours now; else it waits, folded under the others.
    in_view: bool,
    /// Its own icon, kept on this computer (a file's address); "" until fetched.
    icon: String,
}

#[derive(Serialize)]
struct PresetRow {
    name: String,
    url: String,
    host: String,
    /// One of `sites::TYPES`.
    kind: String,
    /// Its categories, in your words: "Banque", "Santé".
    categories: Vec<String>,
    area: String,
    about: String,
    announced_by: Vec<String>,
    calls: Option<bool>,
    /// Already in your sites (the same address).
    kept: bool,
}

/// What the Add dialog offers: the countries (and their states or provinces),
/// the types and categories to filter by, the country shown (`country` when
/// given, else your language's region), and its sites with everyone's. Words
/// only: no request goes out.
pub(crate) fn presets(country: &str, region: &str) -> String {
    let presets = sioul_core::presets::load();
    let language = tr().text("qt-locale", None);
    let lang = language.split(['_', '-']).next().unwrap_or("en").to_string();
    let country = if country.is_empty() {
        // "fr_CA" → Canada; the region is chosen by you.
        let guess = language.split(['_', '-']).nth(1).unwrap_or("").to_uppercase();
        if presets.countries.contains_key(&guess) { guess } else { String::new() }
    } else if country == "-" {
        String::new()
    } else {
        country.to_string()
    };
    let kept: Vec<String> = sites::sites(&load_config()).iter().map(|s| sites::place_of(&s.url)).collect();
    let rows: Vec<PresetRow> = presets
        .offered(&country, region)
        .into_iter()
        .map(|(p, _)| PresetRow {
            host: sites::host_of(&p.url),
            kept: kept.contains(&sites::place_of(&p.url)),
            area: presets.area_of(p).id(),
            about: sioul_core::presets::words(&p.about, &lang),
            categories: presets.tag_words(p, &lang),
            name: p.name.clone(),
            url: p.url.clone(),
            kind: sites::type_of(&p.kind).to_string(),
            announced_by: p.announced_by.clone(),
            calls: p.calls,
        })
        .collect();
    let countries: Vec<serde_json::Value> = presets
        .countries
        .iter()
        .map(|(code, c)| {
            let regions: Vec<serde_json::Value> = c.regions.iter().map(|(r, region)| serde_json::json!({ "code": r, "name": sioul_core::presets::words(&region.name, &lang) })).collect();
            serde_json::json!({ "code": code, "name": sioul_core::presets::words(&c.name, &lang), "regions": regions })
        })
        .collect();
    // The categories the shown sites come with, in your words, by name.
    let mut categories: Vec<String> = rows.iter().flat_map(|r| r.categories.iter().cloned()).collect();
    categories.sort_by_key(|c| c.to_lowercase());
    categories.dedup();
    serde_json::json!({ "country": country, "region": region, "countries": countries, "types": sites::TYPES, "categories": categories, "sites": rows }).to_string()
}

/// The usual sites as a menu: each country (and everyone's), its groups
/// (offices, banks, energy…) and its states or provinces apart, each with its
/// own groups: {"countries": [{"name", "groups": [{"name", "sites": [row]}],
/// "regions_name", "regions": [{"name", "groups"}]}]}.
pub(crate) fn presets_tree() -> String {
    let presets = sioul_core::presets::load();
    let language = tr().text("qt-locale", None);
    let lang = language.split(['_', '-']).next().unwrap_or("en").to_string();
    let kept: Vec<String> = sites::sites(&load_config()).iter().map(|s| sites::place_of(&s.url)).collect();
    let row = |p: &sioul_core::presets::Preset| PresetRow {
        host: sites::host_of(&p.url),
        kept: kept.contains(&sites::place_of(&p.url)),
        area: presets.area_of(p).id(),
        about: sioul_core::presets::words(&p.about, &lang),
        categories: presets.tag_words(p, &lang),
        name: p.name.clone(),
        url: p.url.clone(),
        kind: sites::type_of(&p.kind).to_string(),
        announced_by: p.announced_by.clone(),
        calls: p.calls,
    };
    // A personal space goes by its first category (offices, banks, energy…); the rest by type.
    let group_of = |p: &sioul_core::presets::Preset| -> String {
        let kind = sites::type_of(&p.kind);
        if kind == "mailbox" {
            presets.tag_words(p, &lang).into_iter().next().unwrap_or_else(|| tr().text("site-kind-mailbox", None))
        } else {
            tr().text(&format!("site-kind-{kind}"), None)
        }
    };
    let groups = |list: &[sioul_core::presets::Preset]| -> Vec<serde_json::Value> {
        let mut by: Vec<(String, Vec<PresetRow>)> = Vec::new();
        for p in list {
            let name = group_of(p);
            match by.iter_mut().find(|(n, _)| *n == name) {
                Some((_, rows)) => rows.push(row(p)),
                None => by.push((name, vec![row(p)])),
            }
        }
        by.into_iter().map(|(name, mut rows)| {
            rows.sort_by_key(|r| sioul_core::text::fold(&r.name));
            serde_json::json!({ "name": name, "sites": rows })
        }).collect()
    };
    let mut countries: Vec<serde_json::Value> = presets
        .countries
        .values()
        .map(|c| {
            let mut regions: Vec<serde_json::Value> = c.regions.values().map(|r| serde_json::json!({ "name": sioul_core::presets::words(&r.name, &lang), "groups": groups(&r.sites) })).collect();
            regions.sort_by_key(|r| sioul_core::text::fold(r["name"].as_str().unwrap_or("")));
            let regions_name = if c.regions_name.is_empty() { tr().text("site-presets-by-region", None) } else { sioul_core::presets::words(&c.regions_name, &lang) };
            serde_json::json!({ "name": sioul_core::presets::words(&c.name, &lang), "groups": groups(&c.sites), "regions_name": regions_name, "regions": regions })
        })
        .collect();
    countries.sort_by_key(|c| sioul_core::text::fold(c["name"].as_str().unwrap_or("")));
    countries.push(serde_json::json!({ "name": tr().text("site-presets-everyone", None), "groups": groups(&presets.international), "regions_name": "", "regions": [] }));
    serde_json::json!({ "countries": countries }).to_string()
}

/// Several usual sites pinned at once (a JSON list of the tree's rows): those
/// already here left as they are. Returns {"added", "error"}.
pub(crate) fn add_many(rows: &str) -> String {
    let rows: Vec<serde_json::Value> = serde_json::from_str(rows).unwrap_or_default();
    let mut added = 0;
    let mut problems = Vec::new();
    for row in rows {
        let kept: Vec<String> = sites::sites(&load_config()).iter().map(|s| sites::place_of(&s.url)).collect();
        let url = row.get("url").and_then(|u| u.as_str()).unwrap_or("");
        if kept.contains(&sites::place_of(url)) {
            continue;
        }
        let made: serde_json::Value = serde_json::from_str(&add(&row.to_string())).unwrap_or_default();
        match made.get("error").and_then(|e| e.as_str()) {
            Some(e) => problems.push(e.to_string()),
            None => added += 1,
        }
    }
    problems.dedup();
    serde_json::json!({ "added": added, "error": problems.join(" ") }).to_string()
}

/// A site added as the Add dialog gives it ({"name", "url", "kind", "area",
/// "announced_by", "categories"}): its id made from its name, another number
/// when taken. Returns {"id"} or {"error"}.
pub(crate) fn add(edit: &str) -> String {
    let error = |text: String| serde_json::json!({ "error": text }).to_string();
    let edit: serde_json::Value = serde_json::from_str(edit).unwrap_or_default();
    let text = |k: &str| edit.get(k).and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    let (name, url) = (text("name"), text("url"));
    // Only an encrypted address: a site's sign-in never travels in clear.
    if name.is_empty() || !url.starts_with("https://") || sites::host_of(&url).is_empty() {
        return error(tr().text("site-https-only", None));
    }
    let config = load_config();
    let base = sioul_core::config::slug(&name);
    let base = if base.is_empty() { "site".to_string() } else { base };
    let id = (1..).map(|n| if n == 1 { base.clone() } else { format!("{base}-{n}") }).find(|id| config.site(id).is_none()).unwrap_or(base);
    let list = |k: &str| -> Vec<String> { edit.get(k).and_then(|v| v.as_array()).into_iter().flatten().filter_map(|d| d.as_str().map(str::to_string)).collect() };
    match sioul_core::config::add_site_as(&crate::backend::config_path(), &id, &name, &url, &text("kind"), &text("area"), &list("announced_by"), &list("categories")) {
        Ok(()) => serde_json::json!({ "id": id }).to_string(),
        Err(e) => error(e),
    }
}

/// A site moved one place up (-1) or down (1) in the list; returns what went wrong, else "".
pub(crate) fn move_site(id: &str, delta: i64) -> String {
    sioul_core::config::move_site(&crate::backend::config_path(), id, delta).err().unwrap_or_default()
}

/// A site taken out of Sioul; its sign-ins stay in its profile until cleared. Returns what went wrong, else "".
pub(crate) fn remove(id: &str) -> String {
    sioul_core::config::remove_site(&crate::backend::config_path(), id).err().unwrap_or_default()
}

/// Where sites' icons are kept.
fn favicons() -> std::path::PathBuf {
    sioul_core::config::cache_dir().join("favicons")
}

/// One fetching of icons at a time; the hosts that had none, and when they were asked.
static FETCHING: Mutex<()> = Mutex::new(());
static NO_ICON: Mutex<Vec<(String, i64)>> = Mutex::new(Vec::new());

/// Your sites' own icons, asked of each site when missing or a week old, off
/// the window's thread; the list shows them once there. A site that had none
/// is asked again the next day.
pub(crate) fn favicon_tick(qt: &QtThread) {
    if crate::backend::offline() {
        return;
    }
    let qt = qt.clone();
    std::thread::spawn(move || {
        let Some(_busy) = crate::backend::one_at_a_time(&FETCHING) else { return };
        let dir = favicons();
        let now = jiff::Timestamp::now().as_second();
        let mut fetched = false;
        for site in sites::sites(&load_config()) {
            let host = sites::host_of(&site.url);
            let tried = NO_ICON.lock().map(|n| n.iter().any(|(h, at)| *h == host && now - at < 24 * 3600)).unwrap_or(false);
            if host.is_empty() || tried || !sioul_sync::favicon::stale(&dir, &host) {
                continue;
            }
            match sioul_sync::favicon::fetch(&site.url, &dir) {
                Ok(_) => fetched = true,
                Err(_) => {
                    if let Ok(mut none) = NO_ICON.lock() {
                        none.retain(|(h, _)| *h != host);
                        none.push((host, now));
                    }
                }
            }
        }
        if fetched {
            let _ = qt.queue(|mut sioul| sioul.as_mut().sites_changed());
        }
    });
}

/// The last gathered notification sent: the time it was for (Unix seconds).
static GATHERED: Mutex<i64> = Mutex::new(0);

/// What sites notified, gathered in one notification at the times you set
/// (docs/sites.md, "Notifications"): only the sites of these hours, not
/// silenced, on the computer you are at. A site in real time, and a call,
/// came at once already; the rest waits for the Porch as before.
pub(crate) fn gather_tick(qt: &QtThread, shared: &Arc<Shared>) {
    let config = load_config();
    if !config.reminders.gather {
        return;
    }
    let now = jiff::Zoned::now();
    // The time just passed, within two minutes: a window closed then misses it, the Porch keeps all.
    let due = config
        .reminders
        .gathered_times()
        .iter()
        .filter_map(|t| {
            let (h, m) = t.trim().split_once(':')?;
            now.date().at(h.parse().ok()?, m.parse().ok()?, 0, 0).to_zoned(now.time_zone().clone()).ok()
        })
        .filter(|at| at.timestamp() <= now.timestamp() && now.timestamp().as_second() - at.timestamp().as_second() < 120)
        .map(|at| at.timestamp().as_second())
        .max();
    let Some(due) = due else { return };
    match GATHERED.lock() {
        Ok(mut last) if *last != due => *last = due,
        _ => return,
    }
    // Where you are: one computer gathers (`sioul_sync::lease`).
    let active = shared.active.load(std::sync::atomic::Ordering::Relaxed);
    let (keeper, _) = crate::share::keeper("notices", sioul_sync::lease::Rule::FollowsYou, active, false);
    if !keeper.mine {
        return;
    }
    let notices = Notices::load(&Notices::default_path());
    let mode = crate::backend::mode_now();
    let lines: Vec<String> = sites::sites(&config)
        .iter()
        .filter(|s| !s.muted && sioul_core::areas::in_view(s.area, mode.time, mode.week))
        .filter_map(|s| {
            let count = notices.of(&s.id).len();
            (count > 0).then(|| format!("{} ({count})", s.name))
        })
        .collect();
    if lines.is_empty() {
        return;
    }
    let qt_open = qt.clone();
    let open: Box<dyn FnOnce() + Send> = Box::new(move || {
        let _ = qt_open.queue(|mut sioul| sioul.as_mut().reminder_opened(cxx_qt_lib::QString::from("porch"), cxx_qt_lib::QString::default(), cxx_qt_lib::QString::default()));
    });
    if let Err(e) = sioul_sync::notify::remind(&tr().text("sites-gathered", None), &lines.join(" · "), Some((tr().text("sites-gathered-open", None), open))) {
        crate::backend::tell(qt, shared, e);
    }
}

/// The sites, for their list. In quiet time, work sites keep their news to themselves.
pub(crate) fn list() -> String {
    let notices = Notices::load(&Notices::default_path());
    let mode = crate::backend::mode_now();
    let rows: Vec<SiteRow> = sites::sites(&load_config())
        .into_iter()
        .map(|site| {
            let in_view = sioul_core::areas::in_view(site.area, mode.time, mode.week);
            let icon = sioul_sync::favicon::cached(&favicons(), &sites::host_of(&site.url)).map(|p| crate::backend::file_url(&p)).unwrap_or_default();
            SiteRow { news: in_view && !notices.of(&site.id).is_empty(), in_view, icon, site }
        })
        .collect();
    json(&rows)
}

/// A site's notification: shown at once when the site is in real time (or
/// you asked for real time everywhere), else kept for the Porch. In quiet
/// time, a work site's waits for work to come back.
pub(crate) fn notified(qt: &QtThread, shared: &Arc<Shared>, id: &str, title: &str, text: &str) {
    let config = load_config();
    let Some(site) = sites::sites(&config).into_iter().find(|s| s.id == id) else { return };
    let everywhere = shared.realtime.load(std::sync::atomic::Ordering::Relaxed);
    let resting = !crate::backend::in_view_now(site.area) || (site.kind == "chat" && crate::health::chats_covered());
    // A call waits for nobody: shown at once, unless the site is silenced or resting.
    let call = sites::is_call(title, text) && !site.muted;
    if (site.realtime || everywhere || call) && !resting {
        let heading = format!("{} · {}", site.name, title);
        if let Err(e) = sioul_sync::notify::code(&heading, text, None) {
            crate::backend::tell(qt, shared, e);
        }
        return;
    }
    let path = Notices::default_path();
    let mut notices = Notices::load(&path);
    notices.add(Notice { site: site.id, title: title.trim().to_string(), text: text.trim().chars().take(300).collect(), at: jiff::Timestamp::now().as_second() });
    if let Err(e) = notices.save(&path) {
        crate::backend::tell(qt, shared, e);
    }
    let _ = qt.queue(|mut sioul| sioul.as_mut().sites_changed());
}

/// A site opened: what it notified is seen.
pub(crate) fn seen(qt: &QtThread, id: &str) {
    let path = Notices::default_path();
    let mut notices = Notices::load(&path);
    if !notices.of(id).is_empty() {
        notices.seen(id);
        let _ = notices.save(&path);
        let _ = qt.queue(|mut sioul| sioul.as_mut().sites_changed());
    }
}

#[derive(Serialize)]
struct Waiting {
    id: String,
    name: String,
    items: Vec<String>,
}

/// What the sites notified, for the Porch: each site with its lines, oldest
/// first; in quiet time, only your own sites'.
pub(crate) fn waiting() -> String {
    let notices = Notices::load(&Notices::default_path());
    let mode = crate::backend::mode_now();
    let rows: Vec<Waiting> = sites::sites(&load_config())
        .into_iter()
        .filter(|site| sioul_core::areas::in_view(site.area, mode.time, mode.week))
        .filter_map(|site| {
            let items: Vec<String> = notices.of(&site.id).into_iter().map(|n| if n.text.is_empty() { n.title.clone() } else { say("site-news-line", &[("title", n.title.clone()), ("text", n.text.clone())]) }).collect();
            (!items.is_empty()).then(|| Waiting { id: site.id, name: site.name, items })
        })
        .collect();
    json(&rows)
}

/// The site a message's sender announces: {"id", "name"}, else "".
pub(crate) fn announced_by(sender: &str) -> String {
    let all = sites::sites(&load_config());
    sites::announcing(&all, sender).map(|s| serde_json::json!({ "id": s.id, "name": s.name }).to_string()).unwrap_or_default()
}

// Bitwarden, read here: no external program (sioul_sync::bitwarden).

/// The vault, open for the session: its logins, in memory only (wiped when dropped).
pub(crate) struct Vault {
    items: Vec<bitwarden::Item>,
}

/// This computer, as Bitwarden knows it.
fn device() -> String {
    bitwarden::device_id(&sioul_core::config::state_dir().join("bitwarden-device"))
}

fn remembered(email: &str) -> String {
    format!("Bitwarden device token for {}", email.trim().to_lowercase())
}

/// "missing" (no account set in Sites ⚙), "locked", "unlocked".
pub(crate) fn bitwarden_state(shared: &Shared) -> String {
    // A made-up vault for the window's tests, from a JSON list of logins (never in Sioul's builds).
    #[cfg(feature = "insecure-test-tls")]
    if let Some(path) = std::env::var_os("SIOUL_TEST_VAULT")
        && let Ok(mut vault) = shared.bitwarden.lock()
        && vault.is_none()
    {
        let list: Vec<serde_json::Value> = std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        let items = list
            .iter()
            .map(|l| {
                let text = |k: &str| l.get(k).and_then(|v| v.as_str()).unwrap_or_default().to_string();
                let mut item = bitwarden::Item::default();
                item.id = text("id");
                item.name = text("name");
                item.username = text("username");
                item.password = text("password");
                item.uris = l.get("uris").and_then(|u| u.as_array()).into_iter().flatten().filter_map(|u| u.as_str().map(str::to_string)).collect();
                item
            })
            .collect();
        *vault = Some(Vault { items });
    }
    if load_config().bitwarden.email.as_deref().is_none_or(|e| e.trim().is_empty()) {
        return "missing".into();
    }
    if shared.bitwarden.lock().is_ok_and(|v| v.is_some()) { "unlocked".into() } else { "locked".into() }
}

/// Opens the vault with your master password, and the second step's code
/// when one was asked (`provider` as Bitwarden numbers them; -1 for none,
/// 100 for the code of a new device). Returns {"ok"}, {"factor": [providers]},
/// {"new_device"} or {"error"}. The password is never kept.
pub(crate) fn bitwarden_unlock(shared: &Shared, password: &str, provider: i32, code: &str) -> String {
    let config = load_config();
    let Some(email) = config.bitwarden.email.clone().filter(|e| !e.trim().is_empty()) else { return serde_json::json!({ "error": tr().text("bitwarden-missing", None) }).to_string() };
    let server = bitwarden::Server::of(config.bitwarden.server.as_deref().unwrap_or(""));
    let device = device();
    let token = sioul_sync::secret::named(&remembered(&email));
    let factor = (0..100).contains(&provider).then(|| (u8::try_from(provider).unwrap_or(0), code));
    let new_device = (provider == 100).then_some(code);
    let unlocked = bitwarden::unlock(&server, &email, password, &device, factor, token.as_deref().filter(|_| factor.is_none()), new_device);
    let unlocked = match unlocked {
        // A token kept from before that is no longer good: asked again without it.
        Err(bitwarden::Problem::SecondFactor(..)) if token.is_some() && factor.is_none() => bitwarden::unlock(&server, &email, password, &device, None, None, new_device),
        other => other,
    };
    match unlocked.and_then(|u| bitwarden::sync(&server, &u).map(|items| (u, items))) {
        Ok((unlocked, items)) => {
            if let Some(token) = unlocked.remember.as_deref() {
                let _ = sioul_sync::secret::save_named(&remembered(&email), token);
            }
            if let Ok(mut vault) = shared.bitwarden.lock() {
                *vault = Some(Vault { items });
            }
            // The dialog proposes first what opened it last.
            crate::work::set_view_flag(shared, "bitwarden-passkey", false);
            serde_json::json!({ "ok": true }).to_string()
        }
        // The steps offered; for a security key, the vault's page and the script that ask it.
        // The e-mail code is sent when that step is chosen (`bitwarden_send_code`).
        Err(bitwarden::Problem::SecondFactor(providers, webauthn)) => {
            let key = webauthn.map(|options| serde_json::json!({ "page": bitwarden::key_page(&server), "script": bitwarden::key_script(&options, false) }));
            serde_json::json!({ "factor": providers, "key": key }).to_string()
        }
        Err(bitwarden::Problem::NewDevice) => serde_json::json!({ "new_device": true }).to_string(),
        Err(bitwarden::Problem::Network(e) | bitwarden::Problem::Refused(e)) => serde_json::json!({ "error": e }).to_string(),
        Err(bitwarden::Problem::WrongPassword | bitwarden::Problem::NoKeySecret | bitwarden::Problem::PasskeyNotForVault) => serde_json::json!({ "error": tr().text("bitwarden-refused", None) }).to_string(),
    }
}

/// The token of a passkey login begun, until the key answers.
static PASSKEY: Mutex<Option<String>> = Mutex::new(None);

/// Bitwarden's passkey login begun: the vault's page where the key is asked,
/// unseen, and the script that asks it there; {"page", "script"} or {"error"}.
pub(crate) fn bitwarden_passkey_begin() -> String {
    let config = load_config();
    if config.bitwarden.email.as_deref().is_none_or(|e| e.trim().is_empty()) {
        return serde_json::json!({ "error": tr().text("bitwarden-missing", None) }).to_string();
    }
    let server = bitwarden::Server::of(config.bitwarden.server.as_deref().unwrap_or(""));
    match bitwarden::passkey_ask(&server) {
        Ok(ask) => {
            let script = bitwarden::key_script(&ask.options, true);
            if let Ok(mut pending) = PASSKEY.lock() {
                *pending = Some(ask.token);
            }
            serde_json::json!({ "page": bitwarden::key_page(&server), "script": script }).to_string()
        }
        Err(bitwarden::Problem::Network(e) | bitwarden::Problem::Refused(e)) => serde_json::json!({ "error": e }).to_string(),
        Err(_) => serde_json::json!({ "error": tr().text("bitwarden-refused", None) }).to_string(),
    }
}

/// The key's answer, from that page ({"response", "secret"} or {"error"}):
/// the vault opened with it, for the account set here only. Answers as
/// `bitwarden_unlock` does: {"ok"} or {"error"}.
pub(crate) fn bitwarden_passkey(shared: &Shared, answer: &str) -> String {
    let error = |text: String| serde_json::json!({ "error": text }).to_string();
    let answer: serde_json::Value = serde_json::from_str(answer).unwrap_or_default();
    let token = PASSKEY.lock().ok().and_then(|mut pending| pending.take());
    if let Some(said) = answer.get("error").and_then(|e| e.as_str()) {
        return error(if said == "not-allowed" { tr().text("bitwarden-key-not-allowed", None) } else { say("bitwarden-key-failed", &[("error", said.to_string())]) });
    }
    let (Some(token), Some(response)) = (token, answer.get("response")) else { return error(say("bitwarden-key-failed", &[("error", String::new())])) };
    let config = load_config();
    let Some(email) = config.bitwarden.email.clone().filter(|e| !e.trim().is_empty()) else { return error(tr().text("bitwarden-missing", None)) };
    let server = bitwarden::Server::of(config.bitwarden.server.as_deref().unwrap_or(""));
    let secret = answer.get("secret").and_then(|s| s.as_str()).unwrap_or("");
    let opened = bitwarden::unlock_with_passkey(&server, &device(), &token, &response.to_string(), secret).and_then(|unlocked| {
        // A passkey says whose vault it opens: only the account set in Sites ⚙.
        if bitwarden::account(&unlocked).is_some_and(|account| account != email.trim().to_lowercase()) {
            return Err(bitwarden::Problem::Refused(tr().text("bitwarden-passkey-other", None)));
        }
        bitwarden::sync(&server, &unlocked)
    });
    match opened {
        Ok(items) => {
            if let Ok(mut vault) = shared.bitwarden.lock() {
                *vault = Some(Vault { items });
            }
            crate::work::set_view_flag(shared, "bitwarden-passkey", true);
            serde_json::json!({ "ok": true }).to_string()
        }
        Err(bitwarden::Problem::NoKeySecret) => error(tr().text("bitwarden-passkey-no-secret", None)),
        Err(bitwarden::Problem::PasskeyNotForVault) => error(tr().text("bitwarden-passkey-no-vault", None)),
        Err(bitwarden::Problem::WrongPassword) => error(tr().text("bitwarden-passkey-mismatch", None)),
        // Bitwarden's words when it does not know the key as a passkey of an account.
        Err(bitwarden::Problem::Refused(e)) if e.to_lowercase().contains("invalid credential") => error(tr().text("bitwarden-passkey-unknown", None)),
        Err(bitwarden::Problem::Network(e) | bitwarden::Problem::Refused(e)) => error(e),
        Err(bitwarden::Problem::SecondFactor(..) | bitwarden::Problem::NewDevice) => error(tr().text("bitwarden-refused", None)),
    }
}

/// Sends the e-mail step's code (when you choose that step); returns what went wrong, else "".
pub(crate) fn bitwarden_send_code(password: &str) -> String {
    let config = load_config();
    let Some(email) = config.bitwarden.email.clone().filter(|e| !e.trim().is_empty()) else { return tr().text("bitwarden-missing", None) };
    let server = bitwarden::Server::of(config.bitwarden.server.as_deref().unwrap_or(""));
    match bitwarden::send_email_code(&server, &email, password, &device()) {
        Ok(()) => String::new(),
        Err(bitwarden::Problem::Network(e) | bitwarden::Problem::Refused(e)) => e,
        Err(_) => tr().text("bitwarden-refused", None),
    }
}

/// The login last chosen for each site (host → Bitwarden's item id): proposed first next time.
fn chosen_path() -> std::path::PathBuf {
    sioul_core::config::state_dir().join("bitwarden-chosen.json")
}

fn chosen() -> std::collections::BTreeMap<String, String> {
    std::fs::read_to_string(chosen_path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

#[derive(Serialize)]
struct Choice {
    id: String,
    name: String,
    username: String,
    /// Its first site, to tell logins apart.
    site: String,
    /// Made for another domain than the page's: said, so a look-alike site shows.
    elsewhere: bool,
}

impl Choice {
    fn of(item: &bitwarden::Item, host: &str) -> Choice {
        let hosts = bitwarden::hosts_of(item);
        let domain = sioul_core::sites::domain_of(host);
        let elsewhere = !hosts.iter().any(|h| sioul_core::sites::domain_of(h) == domain);
        Choice { id: item.id.clone(), name: item.name.clone(), username: item.username.clone(), site: hosts.into_iter().next().unwrap_or_default(), elsewhere }
    }
}

/// The logins for a site, the one chosen last for it first ("matches"), and
/// when `query` has words, the vault's logins holding them ("found"); names,
/// user names and sites only, never a password. {"site", "matches", "found"} or {"error"}.
pub(crate) fn bitwarden_logins(shared: &Shared, url: &str, query: &str) -> String {
    let Ok(vault) = shared.bitwarden.lock() else { return serde_json::json!({ "error": tr().text("bitwarden-locked", None) }).to_string() };
    let Some(vault) = vault.as_ref() else { return serde_json::json!({ "error": tr().text("bitwarden-locked", None) }).to_string() };
    let host = sioul_core::sites::host_of(url);
    let mut matches = bitwarden::for_site(&vault.items, url);
    if let Some(last) = chosen().get(&host) {
        // Stable: the others keep their order (same host before same domain).
        matches.sort_by_key(|item| item.id != *last);
    }
    let found: Vec<Choice> = bitwarden::search(&vault.items, query).into_iter().take(40).map(|item| Choice::of(item, &host)).collect();
    let matches: Vec<Choice> = matches.into_iter().map(|item| Choice::of(item, &host)).collect();
    serde_json::json!({ "site": host, "matches": matches, "found": found }).to_string()
}

/// One login of the vault, for a site: {"name", "username", "password",
/// "code"} (the one-time code now, when it keeps its secret), or {"error"};
/// kept as the site's choice.
pub(crate) fn bitwarden_login(shared: &Shared, url: &str, id: &str) -> String {
    let error = |id: &str| serde_json::json!({ "error": tr().text(id, None) }).to_string();
    let Ok(vault) = shared.bitwarden.lock() else { return error("bitwarden-locked") };
    let Some(vault) = vault.as_ref() else { return error("bitwarden-locked") };
    let Some(item) = vault.items.iter().find(|item| item.id == id) else { return error("bitwarden-none") };
    let host = sioul_core::sites::host_of(url);
    if !host.is_empty() {
        let mut all = chosen();
        if all.get(&host).map(String::as_str) != Some(id) {
            all.insert(host, id.to_string());
            if let Ok(text) = serde_json::to_string_pretty(&all) {
                let _ = std::fs::create_dir_all(sioul_core::config::state_dir());
                let _ = std::fs::write(chosen_path(), text);
            }
        }
    }
    let code = if item.totp.is_empty() { String::new() } else { bitwarden::totp(&item.totp, jiff::Timestamp::now().as_second()).unwrap_or_default() };
    serde_json::json!({ "name": item.name, "username": item.username, "password": item.password, "code": code }).to_string()
}

/// Locks the vault again: its logins leave memory.
pub(crate) fn bitwarden_lock(shared: &Shared) {
    if let Ok(mut vault) = shared.bitwarden.lock() {
        *vault = None;
    }
}
