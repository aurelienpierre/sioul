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
    /// Android: where it opens, in words (in Sioul, or in the browser and why: `siteview`); none on a computer.
    #[serde(skip_serializing_if = "Option::is_none")]
    opens_line: Option<String>,
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
/// own groups: `{"countries": [{"name", "groups": [{"name", "sites": [row]}],
/// "regions_name", "regions": [{"name", "groups"}]}]}`.
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
    // The order travels as its own setting once each site travels apart (format 2 of the sharing).
    sioul_core::config::move_site(&crate::backend::config_path(), id, delta, crate::share::writes_format_2()).err().unwrap_or_default()
}

/// A site taken out of Sioul. On a computer its sign-ins stay in its
/// profile until cleared; on Android its sign-in on this device is
/// forgotten as far as the web view can (`siteview::forget_line`, review
/// K1). Returns what went wrong, or what stays on this device, else "".
pub(crate) fn remove(id: &str) -> String {
    let site = sites::sites(&load_config()).into_iter().find(|s| s.id == id);
    match sioul_core::config::remove_site(&crate::backend::config_path(), id) {
        Err(e) => e,
        Ok(()) => site.map(|s| crate::siteview::forget_line(&s.url, &s.name)).unwrap_or_default(),
    }
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
/// came at once already; the rest waits for the Porch as before. A gathered
/// time the matrix of what reaches you holds (as usual: sleep, a pause, Free
/// time, a slot of time for you, do-not-disturb) is skipped: what the sites
/// said waits for the next one. A phone makes no such notice (docs/android.md,
/// "Notifications"): what its sites said waits for the Porch.
pub(crate) fn gather_tick(qt: &QtThread, shared: &Arc<Shared>) {
    let config = load_config();
    if cfg!(target_os = "android") || !config.reminders.gather || crate::hours::level(sioul_core::attention::Kind::Sites) != sioul_core::attention::Level::Gathered {
        return;
    }
    let now = sioul_core::zone::now();
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
    // Where you are: one computer gathers (`sioul_sync::lease`), the sites that come forward on it
    // now (docs/areas.md, "Which device tells"); a site for another device's time waits under
    // "Other hours", its news gathered once that device is the one used.
    let active = shared.active.load(std::sync::atomic::Ordering::Relaxed);
    let (keeper, _) = crate::share::keeper("notices", sioul_sync::lease::Rule::FollowsYou, active, false);
    if !keeper.mine {
        return;
    }
    let path = Notices::default_path();
    let notices = Notices::load(&path);
    let mode = crate::backend::mode_now();
    // While you are Reachable, no device keeps a site back: the hours alone.
    let reachable = crate::hours::reachable_at(&sioul_core::zone::now());
    let named: Vec<(String, String)> = sites::sites(&config).iter().filter(|s| !s.muted && if reachable { sioul_core::areas::in_view(s.area, mode.time, mode.week) } else { mode.brings(s.area) }).map(|s| (s.id.clone(), s.name.clone())).collect();
    // What is new since the last time it rang: each notification named once (the Porch keeps them all).
    let lines: Vec<String> = named
        .iter()
        .filter_map(|(id, name)| {
            let count = notices.untold(id).len();
            (count > 0).then(|| format!("{name} ({count})"))
        })
        .collect();
    if lines.is_empty() {
        return;
    }
    let _ = (qt, shared);
    // Named once posted: news not told (nobody at the computer, a do-not-disturb) waits for the next gathering.
    let ids: Vec<String> = named.into_iter().map(|(id, _)| id).collect();
    let claim: crate::tell::Claim = Box::new(move || {
        let mut notices = Notices::load(&path);
        notices.mark_told(&ids.iter().map(String::as_str).collect::<Vec<_>>());
        if let Err(e) = notices.save(&path) {
            eprintln!("{e}");
        }
        true
    });
    use sioul_core::notice::{Act, Kind, Notice, address};
    let notice = Notice::new(address::PORCH, Kind::Own(sioul_core::attention::Kind::Sites), tr().text("sites-gathered", None), lines.join(" · ")).action(tr().text("sites-gathered-open", None), Act::Open { address: address::PORCH.to_string(), hint: String::new() });
    crate::tell::tell_with(notice, sioul_core::notice::told(Some(sioul_core::attention::Kind::Sites)), None, Some(claim), true);
}

/// The sites, for their list. In quiet time, work sites keep their news to
/// themselves. On Android each says where it opens (`siteview`).
pub(crate) fn list() -> String {
    let notices = Notices::load(&Notices::default_path());
    let mode = crate::backend::mode_now();
    // On Android, where each opens: this device's web view asked once for the list.
    let opens = crate::siteview::lines_here();
    let rows: Vec<SiteRow> = sites::sites(&load_config())
        .into_iter()
        .map(|site| {
            let in_view = mode.brings(site.area);
            let icon = sioul_sync::favicon::cached(&favicons(), &sites::host_of(&site.url)).map(|p| crate::backend::file_url(&p)).unwrap_or_default();
            let opens_line = opens.as_ref().map(|line| line(&site.url));
            SiteRow { news: in_view && !notices.of(&site.id).is_empty(), in_view, icon, opens_line, site }
        })
        .collect();
    json(&rows)
}

/// How long a call in a site may ring before its notice goes, when the site
/// does not withdraw its own first (a ringing call ends within a minute or two).
const CALL_RINGS: i64 = 3 * 60;

/// The calls in a site told, by their notice's address: a site withdrawing
/// one (the call answered or over) takes Sioul's away (`withdrawn`).
static CALLS: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// A site withdrew a notification: Sioul's notice of it goes when it was a
/// call (answered, or over). A message's stays its minutes: some sites close
/// their own after a few seconds, which would take Sioul's away before it
/// could be read.
pub(crate) fn withdrawn(id: &str, key: &str) {
    let about = sioul_core::notice::address::site(id, key);
    let mut calls = CALLS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(at) = calls.iter().position(|a| *a == about) {
        calls.remove(at);
        crate::tell::settle(&about);
    }
}

/// A site's notification (`key`: the web notice's own, its tag; "" for one
/// without, so that the next takes its place): kept for the Porch and the
/// next gathering whether told or not, and told at once when the site is in
/// real time (or you asked for real time everywhere) or it is a call.
/// Told (docs/notifications.md), it is about the site and that key
/// (`sioul:site/<id>#<key>`): the same key takes the last one's place (a
/// conversation's next message); a click opens the site, then the web
/// notice's own click (the site opens the conversation). A chat's message is
/// to know; a call is to do, until the site withdraws it (`withdrawn`) or a
/// few minutes pass (`CALL_RINGS`). With the site's own icon. Whether a
/// desktop notice was posted or waits, for the Sites page to keep the web
/// notice it would click. In quiet
/// time, a work site's waits for work to come back; at the times the matrix
/// of what reaches you holds them (as usual: sleep, a pause, Free time,
/// do-not-disturb, and but for a call a slot of time for you), every site's.
/// A chat site's that names someone your cards know follows their Messages
/// row too, which may only hold more (docs/attention.md, Q13): a blocked
/// person's is not even kept.
pub(crate) fn notified(qt: &QtThread, shared: &Arc<Shared>, id: &str, key: &str, title: &str, text: &str) -> bool {
    use sioul_core::attention::{Event, Level, Source};
    let config = load_config();
    let Some(site) = sites::sites(&config).into_iter().find(|s| s.id == id) else { return false };
    let everywhere = shared.realtime.load(std::sync::atomic::Ordering::Relaxed);
    // A call waits for nobody: shown at once, unless the site is silenced or
    // resting, or the matrix holds calls in sites now.
    let call = sites::is_call(title, text) && !site.muted;
    let chat = site.kind == "chat";
    let named = if chat { sioul_core::porch::Senders::load(&config).judge_name(title.trim()).map(|j| j.row()) } else { None };
    let mut now = crate::hours::attention_here();
    now.realtime = everywhere;
    now.holds.chats = chat && crate::health::chats_covered();
    let out = crate::hours::attention().decide(&Event::of(Source::Site { call, live: site.realtime, named }).for_area(Some(site.area)), &now);
    if out.level == Level::Never {
        return false;
    }
    // Kept for the Porch and the next gathering, told or not.
    let path = Notices::default_path();
    let mut notices = Notices::load(&path);
    notices.add(Notice { site: site.id.clone(), title: title.trim().to_string(), text: text.trim().chars().take(300).collect(), at: jiff::Timestamp::now().as_second(), told: false });
    if let Err(e) = notices.save(&path) {
        crate::backend::tell(qt, shared, e);
    }
    let _ = qt.queue(|mut sioul| sioul.as_mut().sites_changed());
    // A phone makes no notification of its own for a site yet (docs/android.md, "Notifications").
    // A site's news comes at once in real time, for a call, or where its row says so (What reaches
    // you ▸ Sites and apps ▸ At once: `behaviours::Group::Sites`); else it waits for the gathering.
    let at_once = site.realtime || everywhere || call || out.level == Level::Now;
    if !at_once || !out.told || cfg!(target_os = "android") {
        return false;
    }
    use sioul_core::notice::{Kind, Notice as Told, address};
    let kind = if call { sioul_core::attention::Kind::SiteCalls } else { sioul_core::attention::Kind::SitesLive };
    let icon = sioul_sync::favicon::cached(&favicons(), &sites::host_of(&site.url)).map(|p| p.display().to_string()).unwrap_or_default();
    let until = call.then(|| jiff::Timestamp::now().as_second() + CALL_RINGS);
    let about = address::site(&site.id, key);
    let notice = Told::new(about.clone(), Kind::Own(kind), format!("{} · {}", site.name, title), text).icon(icon).until(until);
    let fate = crate::tell::tell_as(notice, out, None);
    let told = matches!(fate, crate::tell::Fate::Posted | crate::tell::Fate::Waits);
    if told && call {
        let mut calls = CALLS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        calls.retain(|a| *a != about);
        calls.push(about);
        let excess = calls.len().saturating_sub(32);
        calls.drain(..excess);
    }
    told
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
        .filter(|site| mode.brings(site.area))
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

/// The script that writes a login into a page's form, called with the user
/// name, the password and the one-time code: the password field and the
/// name field before it set as typing would, and a page asking for the
/// one-time code given the code the vault computes now. One source for every
/// device, run only inside the call Rust builds around it (`fill_call`): a
/// computer's Sites page and a phone's site screen both take that call.
/// Word for word the script the computer's page held until 21 November
/// 2026, but for its first line, `'use strict'` (review of 22 November 2026,
/// finding 6): a page's own function that the script calls (a
/// `querySelector` the page replaced) can then read neither its caller nor
/// its arguments, the password and the one-time code.
pub(crate) const FILL_SCRIPT: &str = r#"function (user, password, code) {
        'use strict';
        const set = (field, value) => {
            const setter = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(field), 'value').set;
            setter.call(field, value);
            field.dispatchEvent(new Event('input', { bubbles: true }));
            field.dispatchEvent(new Event('change', { bubbles: true }));
        };
        const secret = document.querySelector('input[type=password]');
        const names = Array.from(document.querySelectorAll('input[type=email], input[type=text], input:not([type])'));
        const before = secret ? names.filter(f => f.compareDocumentPosition(secret) & Node.DOCUMENT_POSITION_FOLLOWING) : names;
        const name = before.length ? before[before.length - 1] : null;
        if (name && user) set(name, user);
        if (secret && password) set(secret, password);
        // A page asking for the one-time code: the code the vault computes now.
        const otp = document.querySelector('input[autocomplete="one-time-code"]') || Array.from(document.querySelectorAll('input:not([type=password]):not([type=hidden])')).find(f => /otp|totp|2fa|mfa|code|token/i.test((f.name || '') + ' ' + (f.id || '') + ' ' + (f.getAttribute('aria-label') || '')));
        if (otp && code && !secret) set(otp, code);
        return !!(name || secret);
    }"#;

/// A text as a JavaScript string: JSON's quoting (quotes, backslashes,
/// control characters, line breaks escaped), then the two line separators
/// (U+2028, U+2029), which older engines end a line at, and `<`, so that
/// nothing in it can close a script it would be written into (`</script>`,
/// `<!--`). Inside the quotes each escape reads back as the character itself.
fn js_text(text: &str) -> String {
    serde_json::to_string(text).unwrap_or_else(|_| "\"\"".into()).replace('\u{2028}', "\\u2028").replace('\u{2029}', "\\u2029").replace('<', "\\u003c")
}

/// The whole call a page runs to fill a login, built here for every device:
/// the page's document itself first compares its own origin
/// (`location.origin`, which no page can redefine) with `origin`, the one
/// the login was asked for, and returns false without touching anything
/// when they differ; only then `FILL_SCRIPT` runs (review of 22 November
/// 2026, finding 1: a phone's screen and a computer's page both know the
/// page's address from a copy that may be late, never the document that
/// runs the call). Nothing of the page is called before the comparison.
/// The four values are quoted by Rust (`js_text`), so that nothing in an
/// origin, a password, a user name or a code can change the script. The
/// guard's own arguments are out of a page's reach: the only function that
/// calls into the page is `FILL_SCRIPT`, which is strict, so a page's
/// function sees no caller above it.
pub(crate) fn fill_call(origin: &str, user: &str, password: &str, code: &str) -> String {
    format!("(function (origin, user, password, code) {{ if (location.origin !== origin) return false; return ({FILL_SCRIPT})(user, password, code); }})({}, {}, {}, {})", js_text(origin), js_text(user), js_text(password), js_text(code))
}

/// An http or https address's scheme, host and port: the host in lower
/// case, without a user, an IPv6 address in its brackets; the port written,
/// else the scheme's own (443, 80). As a browser reads it: the first of
/// `/ ? # \` ends the host, the last `@` before it ends a user. None for any
/// other address, a host missing, or a port that does not read.
pub(crate) fn origin_parts(url: &str) -> Option<(&'static str, String, u16)> {
    let (scheme, default, rest) = if url.get(..8).is_some_and(|s| s.eq_ignore_ascii_case("https://")) {
        ("https", 443, &url[8..])
    } else if url.get(..7).is_some_and(|s| s.eq_ignore_ascii_case("http://")) {
        ("http", 80, &url[7..])
    } else {
        return None;
    };
    let authority = &rest[..rest.find(['/', '?', '#', '\\']).unwrap_or(rest.len())];
    let authority = &authority[authority.rfind('@').map_or(0, |at| at + 1)..];
    let (host, port) = match authority.strip_prefix('[') {
        Some(inside) => {
            let close = inside.find(']')?;
            (&authority[..close + 2], &inside[close + 1..])
        }
        None => authority.split_at(authority.find(':').unwrap_or(authority.len())),
    };
    let port = match port.strip_prefix(':') {
        None if port.is_empty() => default,
        Some(digits) if (1..=5).contains(&digits.len()) && digits.bytes().all(|b| b.is_ascii_digit()) => digits.parse().ok()?,
        _ => return None,
    };
    (!host.is_empty()).then(|| (scheme, host.to_lowercase(), port))
}

/// The origin of a page at `url` as its own `location.origin` says it: the
/// scheme, the host, and the port unless it is the scheme's own
/// (`https://example.org`, `https://example.org:8443`, `http://192.168.1.1`).
/// The address as a web view commits it: a name in other alphabets in its
/// "xn--" form. None for an address that is not http or https.
pub(crate) fn web_origin(url: &str) -> Option<String> {
    let (scheme, host, port) = origin_parts(url)?;
    let own = if scheme == "https" { 443 } else { 80 };
    Some(if port == own { format!("{scheme}://{host}") } else { format!("{scheme}://{host}:{port}") })
}

/// The vault, open for the session: its logins, in memory only (wiped when
/// dropped), with what each is searched by (`bitwarden::Logins`).
pub(crate) struct Vault {
    logins: bitwarden::Logins,
    /// When it was last used (opened, a site's or a login's lookup, a fill),
    /// Unix seconds: what "Lock Bitwarden after" counts from.
    used: std::sync::atomic::AtomicI64,
}

impl Vault {
    pub(crate) fn of(items: Vec<bitwarden::Item>) -> Vault {
        Vault { logins: bitwarden::Logins::new(items), used: std::sync::atomic::AtomicI64::new(jiff::Timestamp::now().as_second()) }
    }

    /// Used now: its time without use starts again.
    fn touch(&self) {
        self.used.store(jiff::Timestamp::now().as_second(), std::sync::atomic::Ordering::Relaxed);
    }

    /// Whether `minutes` without use have passed at `now` (Unix seconds); never with 0.
    fn idle(&self, minutes: u32, now: i64) -> bool {
        minutes > 0 && now - self.used.load(std::sync::atomic::Ordering::Relaxed) >= i64::from(minutes) * 60
    }
}

/// The vault locked once it went unused as long as chosen (`bitwarden.lock_after`,
/// decided 3 November 2026), as Bitwarden's own apps do: asked at the window's
/// minute. Whether it was locked now.
pub(crate) fn bitwarden_lock_if_idle(shared: &Shared, minutes: u32, now: i64) -> bool {
    let Ok(mut vault) = shared.bitwarden.lock() else { return false };
    if vault.as_ref().is_some_and(|v| v.idle(minutes, now)) {
        *vault = None;
        return true;
    }
    false
}

/// This computer, as Bitwarden knows it.
fn device() -> String {
    bitwarden::device_id(&sioul_core::config::state_dir().join("bitwarden-device"))
}

fn remembered(email: &str) -> String {
    format!("Bitwarden device token for {}", email.trim().to_lowercase())
}

/// Whether a Bitwarden account is set on this device (Accounts' password manager card).
pub(crate) fn account_set() -> bool {
    load_config().bitwarden.email.as_deref().is_some_and(|e| !e.trim().is_empty())
}

/// "missing" (no account set: Accounts' password manager card), "locked", "unlocked".
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
        *vault = Some(Vault::of(items));
    }
    if !account_set() {
        return "missing".into();
    }
    if shared.bitwarden.lock().is_ok_and(|v| v.is_some()) { "unlocked".into() } else { "locked".into() }
}

/// Bitwarden's calls that wait for its server (an unlock, a passkey, the
/// e-mail's code), made off the window's thread so that it never freezes
/// meanwhile: a ticket at once, then `work`'s answer with
/// `bitwarden_answered(ticket, answer)` on the window's thread. The dialog
/// waits for its own ticket, saying so (VaultUnlock.qml).
pub(crate) fn bitwarden_later(qt: &QtThread, work: impl FnOnce() -> String + Send + 'static) -> i32 {
    static ASKED: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);
    let ticket = ASKED.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
    let qt = qt.clone();
    std::thread::spawn(move || {
        let answer = work();
        let _ = qt.queue(move |mut sioul| sioul.as_mut().bitwarden_answered(ticket, cxx_qt_lib::QString::from(&answer)));
    });
    ticket
}

/// Opens the vault with your master password, and the second step's code
/// when one was asked (`provider` as Bitwarden numbers them; -1 for none,
/// 100 for the code of a new device). Returns `{"ok"}`, `{"factor": [providers]}`,
/// `{"new_device"}` or `{"error"}`. The password is never kept.
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
                *vault = Some(Vault::of(items));
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
        // A passkey says whose vault it opens: only the account set in Accounts (the password manager's card).
        if bitwarden::account(&unlocked).is_some_and(|account| account != email.trim().to_lowercase()) {
            return Err(bitwarden::Problem::Refused(tr().text("bitwarden-passkey-other", None)));
        }
        bitwarden::sync(&server, &unlocked)
    });
    match opened {
        Ok(items) => {
            if let Ok(mut vault) = shared.bitwarden.lock() {
                *vault = Some(Vault::of(items));
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
    /// Its site: the one that matched the search, else the page's, else its first.
    site: String,
    /// Made for another domain than the page's: said, so a look-alike site shows.
    elsewhere: bool,
}

impl Choice {
    fn of(found: bitwarden::Found<'_>) -> Choice {
        Choice { id: found.item.id.clone(), name: found.item.name.clone(), username: found.item.username.clone(), site: found.site, elsewhere: !found.own }
    }
}

/// How many logins the chooser lists; the others are counted, to narrow the search.
const LISTED: usize = 50;

fn locked() -> String {
    serde_json::json!({ "error": tr().text("bitwarden-locked", None) }).to_string()
}

/// The chooser's search: the vault's logins by site and by user name, each
/// optional, both matching when both are given (`bitwarden::Logins::find`);
/// the one chosen last for `url` first, then the best matches, `url`'s own
/// before others. Names, user names and sites only, never a password.
/// {"found": [{"id", "name", "username", "site", "elsewhere"}], "more"} or {"error"}.
fn bitwarden_logins(shared: &Shared, url: &str, site: &str, user: &str) -> String {
    let Ok(vault) = shared.bitwarden.lock() else { return locked() };
    let Some(vault) = vault.as_ref() else { return locked() };
    vault.touch();
    let last = chosen().get(&sioul_core::sites::host_of(url)).cloned();
    let (found, total) = vault.logins.find(site, user, url, last.as_deref(), LISTED);
    let more = total - found.len();
    let found: Vec<Choice> = found.into_iter().map(Choice::of).collect();
    serde_json::json!({ "found": found, "more": more }).to_string()
}

/// Above this many logins, the chooser searches off the window's thread:
/// 20,000 made-up logins took 1 to 5.5 ms a search on a desktop computer
/// busy with builds (`a_large_vault_is_searched_quickly`), a phone being
/// several times slower; below, at once, so the chooser opens filled.
const LARGE: usize = 5_000;

/// The chooser's search as the window asks it: the answer at once
/// (`bitwarden_logins`); for a large vault {"later": n}, the answer brought
/// by `bitwarden_found(n, …)` once searched off the window's thread.
pub(crate) fn bitwarden_search(qt: &QtThread, shared: &Arc<Shared>, url: &str, site: &str, user: &str) -> String {
    let large = shared.bitwarden.lock().is_ok_and(|vault| vault.as_ref().is_some_and(|vault| vault.logins.items().len() > LARGE));
    if !large {
        return bitwarden_logins(shared, url, site, user);
    }
    static ASKED: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);
    let ticket = ASKED.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
    let (qt, shared, url, site, user) = (qt.clone(), Arc::clone(shared), url.to_string(), site.to_string(), user.to_string());
    std::thread::spawn(move || {
        let found = bitwarden_logins(&shared, &url, &site, &user);
        let _ = qt.queue(move |mut sioul| sioul.as_mut().bitwarden_found(ticket, cxx_qt_lib::QString::from(&found)));
    });
    serde_json::json!({ "later": ticket }).to_string()
}

/// What the vault holds for an address, as the chooser opens on it and as
/// "Fill the login" begins: {"domain": the address's registrable domain;
/// "host": its host, as the chooser names the page it fills; "chosen": the
/// domain of the login chosen last for it, "" when none; "only": the login
/// filled at once (`bitwarden::Logins::opening`: its only one, made for this
/// very host or chosen last on it), "" when there is a choice to make} or
/// {"error"}.
pub(crate) fn bitwarden_site(shared: &Shared, url: &str) -> String {
    let Ok(vault) = shared.bitwarden.lock() else { return locked() };
    let Some(vault) = vault.as_ref() else { return locked() };
    vault.touch();
    let host = sioul_core::sites::host_of(url);
    let last = chosen().get(&host).cloned();
    let opening = vault.logins.opening(url, last.as_deref());
    serde_json::json!({ "domain": opening.domain, "host": host, "chosen": opening.chosen, "only": opening.only.unwrap_or_default() }).to_string()
}

/// The login "Fill the login" fills at once for an address, as
/// `bitwarden_site`'s "only" says. None when there is a choice to make, or
/// the vault is locked.
pub(crate) fn fill_at_once(shared: &Shared, url: &str) -> Option<String> {
    let held = shared.bitwarden.lock().ok()?;
    let vault = held.as_ref()?;
    vault.touch();
    let last = chosen().get(&sioul_core::sites::host_of(url)).cloned();
    vault.logins.opening(url, last.as_deref()).only
}

/// A login taken out of the vault for one fill; its password and code wiped when it goes.
pub(crate) struct Login {
    pub(crate) name: String,
    pub(crate) username: String,
    pub(crate) password: String,
    /// The one-time code now, when the login keeps its secret; else "".
    pub(crate) code: String,
}

impl Drop for Login {
    fn drop(&mut self) {
        wipe(&mut self.password);
        wipe(&mut self.code);
    }
}

/// A secret's text overwritten before its memory goes back, then emptied.
/// Best effort: a copy made on the way (a string grown, a JSON text, Java's
/// own strings) is not reached.
pub(crate) fn wipe(text: &mut String) {
    // SAFETY: only zeros are written, which leave the text valid UTF-8.
    let bytes = unsafe { text.as_mut_vec() };
    for byte in bytes.iter_mut() {
        // SAFETY: a valid, aligned byte of the vector, written through its own reference.
        unsafe { std::ptr::write_volatile(byte, 0) };
    }
    std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
    text.clear();
}

/// One login of the vault by its id, for the page at `url`, kept as the
/// site's choice (the login chosen last comes first there next time). Else
/// the id of the words that say why not: "bitwarden-locked", "bitwarden-none".
pub(crate) fn login(shared: &Shared, url: &str, id: &str) -> Result<Login, &'static str> {
    let Ok(held) = shared.bitwarden.lock() else { return Err("bitwarden-locked") };
    let Some(vault) = held.as_ref() else { return Err("bitwarden-locked") };
    vault.touch();
    let Some(item) = vault.logins.items().iter().find(|item| item.id == id) else { return Err("bitwarden-none") };
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
    Ok(Login { name: item.name.clone(), username: item.username.clone(), password: item.password.clone(), code })
}

/// The whole call that fills one login of the vault into the page at `url`
/// (`fill_call`), for that page's origin alone, or the id of the words that
/// say why not: "site-screen-fill-https" for an address that is not a web
/// page's, else `login`'s.
pub(crate) fn fill_for(shared: &Shared, url: &str, id: &str) -> Result<String, &'static str> {
    fill_at(shared, url, url, id)
}

/// `fill_for`, the origin read from `committed`, the page's address with
/// its host in ASCII (a computer's page hands the address as Qt shows it,
/// a name in other alphabets in its own letters, which the login's memory
/// is kept by).
fn fill_at(shared: &Shared, url: &str, committed: &str, id: &str) -> Result<String, &'static str> {
    let origin = web_origin(committed).ok_or("site-screen-fill-https")?;
    let login = login(shared, url, id)?;
    Ok(fill_call(&origin, &login.username, &login.password, &login.code))
}

/// A computer's Sites page fills one login of the vault into its page
/// (SitesPage.qml, `fillChosen`), with the same guarded call as a phone's
/// screen: {"call"}, run as it is by the page's web view, or {"error"}; kept
/// as the site's choice. `committed`: the page's address with its host in
/// ASCII (QUrl's encoded form). The call's own text is wiped once quoted.
pub(crate) fn bitwarden_fill(shared: &Shared, url: &str, committed: &str, id: &str) -> String {
    match fill_at(shared, url, committed, id) {
        Ok(mut call) => {
            let answer = serde_json::json!({ "call": call }).to_string();
            wipe(&mut call);
            answer
        }
        Err(id) => serde_json::json!({ "error": tr().text(id, None) }).to_string(),
    }
}

/// One login of the vault, for a site: {"name", "username", "password",
/// "code"} (the one-time code now, when it keeps its secret), or {"error"};
/// kept as the site's choice.
pub(crate) fn bitwarden_login(shared: &Shared, url: &str, id: &str) -> String {
    match login(shared, url, id) {
        Ok(login) => serde_json::json!({ "name": login.name, "username": login.username, "password": login.password, "code": login.code }).to_string(),
        Err(id) => serde_json::json!({ "error": tr().text(id, None) }).to_string(),
    }
}

/// Locks the vault again: its logins leave memory.
pub(crate) fn bitwarden_lock(shared: &Shared) {
    if let Ok(mut vault) = shared.bitwarden.lock() {
        *vault = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The vault locks once unused as long as chosen, as Bitwarden's own apps
    /// do (decided 3 November 2026): kept while used, never with "when Sioul
    /// closes" (0); a lookup counts as use. A stand-in vault: nothing asks Bitwarden.
    #[test]
    fn the_vault_locks_after_its_time_without_use() {
        let shared = Shared::default();
        let open = |used: i64| {
            let vault = Vault::of(Vec::new());
            vault.used.store(used, std::sync::atomic::Ordering::Relaxed);
            *shared.bitwarden.lock().unwrap() = Some(vault);
        };
        let now = jiff::Timestamp::now().as_second();
        open(now - 10 * 60);
        assert!(!bitwarden_lock_if_idle(&shared, 0, now), "never by default: until Sioul closes");
        assert!(!bitwarden_lock_if_idle(&shared, 15, now), "ten minutes of fifteen");
        assert!(bitwarden_lock_if_idle(&shared, 5, now), "ten minutes of five: locked");
        assert!(shared.bitwarden.lock().unwrap().is_none(), "its logins gone from memory");
        assert!(!bitwarden_lock_if_idle(&shared, 5, now), "locked already: nothing more");
        // Used meanwhile: a site's lookup starts its time again.
        open(now - 10 * 60);
        let _ = bitwarden_site(&shared, "https://bank.example.org/login");
        assert!(!bitwarden_lock_if_idle(&shared, 5, jiff::Timestamp::now().as_second()));
    }

    /// One fill call for every device (22 November 2026): the computer's
    /// Sites page takes Rust's whole guarded call (`bitwardenFill`), keeps
    /// no script of its own, and runs the call as given.
    #[test]
    fn the_computer_s_page_takes_the_whole_call_from_rust() {
        let page = include_str!("../qml/SitesPage.qml");
        assert!(!page.contains("querySelector") && !page.contains("fillScript"), "no copy of the script in the page");
        assert!(page.contains("const fill = JSON.parse(page.sioul.bitwardenFill(view.url.toString(), item))"));
        assert!(page.contains("view.runJavaScript(fill.call)"), "run as Rust built it");
        assert_eq!(page.matches("view.runJavaScript(").count(), 1, "the page runs no other script in a site's page");
        let backend = include_str!("backend.rs");
        assert!(!backend.contains("fn fill_script("), "the bare script is handed to no page");
        assert!(backend.contains("crate::sites::bitwarden_fill(&self.shared(), &url.to_string(), &encoded, &id.to_string())"));
    }

    /// The script is strict from its first statement (review of 22 November
    /// 2026, finding 6), so that a page's function it calls cannot read its
    /// caller's arguments (the password, the one-time code); otherwise word
    /// for word the computer's script of 21 November 2026.
    #[test]
    fn the_script_is_strict_from_its_first_statement() {
        assert!(FILL_SCRIPT.starts_with("function (user, password, code) {\n        'use strict';\n        const set = "), "'use strict' first, as the directive of the function");
        assert!(FILL_SCRIPT.ends_with("return !!(name || secret);\n    }"));
        assert_eq!(FILL_SCRIPT.matches("'use strict'").count(), 1);
        assert_eq!(FILL_SCRIPT.len(), 1335 + "\n        'use strict';".len(), "the computer's 1335 bytes and the one line");
    }

    /// Each page asked for is held to its own origin (review of 22 November
    /// 2026, finding 1): the call begins with the guard, its first argument
    /// the origin of the address the login was asked for, as the page's
    /// `location.origin` says it: the scheme's own port left out, another
    /// kept, an IPv6 host in its brackets, a user dropped, lower case; http
    /// too (a computer's page on a router), nothing for another scheme.
    #[test]
    fn the_call_runs_only_at_the_origin_asked() {
        let call = fill_call("https://bank.example.org", "jane", "pass", "");
        let guard = format!("(function (origin, user, password, code) {{ if (location.origin !== origin) return false; return ({FILL_SCRIPT})(user, password, code); }})(");
        assert!(call.starts_with(&guard), "the guard first, the script inside it");
        assert_eq!(&call[guard.len()..], "\"https://bank.example.org\", \"jane\", \"pass\", \"\")");
        for (url, origin) in [
            ("https://bank.example.org/login?next=%2F", Some("https://bank.example.org")),
            ("HTTPS://Bank.Example.org:443/x", Some("https://bank.example.org")),
            ("https://jane@bank.example.org/", Some("https://bank.example.org")),
            ("https://bank.example.org:8443/login", Some("https://bank.example.org:8443")),
            ("https://bank.example.org\\@evil.example/", Some("https://bank.example.org")),
            ("https://[::1]:8443/a", Some("https://[::1]:8443")),
            ("https://[::1]/a", Some("https://[::1]")),
            ("https://xn--bnk-qla.example/", Some("https://xn--bnk-qla.example")),
            ("http://192.168.1.1/admin", Some("http://192.168.1.1")),
            ("http://192.168.1.1:80/", Some("http://192.168.1.1")),
            ("http://192.168.1.1:8080/", Some("http://192.168.1.1:8080")),
            ("about:blank", None),
            ("", None),
            ("file:///home/someone/a.html", None),
            ("javascript:alert(1)", None),
            ("https://", None),
            ("https://bank.example.org:99999/", None),
            ("https://bank.example.org:/", None),
        ] {
            assert_eq!(web_origin(url).as_deref(), origin, "{url}");
        }
        // The login read for the address asked, its origin in the call.
        sioul_core::config::stand_ins_only();
        let shared = Shared::default();
        let mut item = bitwarden::Item::default();
        item.id = "a".into();
        item.username = "jane".into();
        item.password = "pass-a".into();
        item.uris = vec!["https://bank.example.org:8443".into()];
        *shared.bitwarden.lock().unwrap() = Some(Vault::of(vec![item]));
        assert_eq!(fill_for(&shared, "https://bank.example.org:8443/login", "a"), Ok(fill_call("https://bank.example.org:8443", "jane", "pass-a", "")));
        assert_eq!(fill_for(&shared, "about:blank", "a"), Err("site-screen-fill-https"), "no origin: nothing read from the vault");
        // A computer's page: the memory by the address as Qt shows it, the origin by its ASCII form.
        let answer: serde_json::Value = serde_json::from_str(&bitwarden_fill(&shared, "https://bänk.example/", "https://xn--bnk-qla.example/", "a")).unwrap();
        assert_eq!(answer["call"].as_str(), Some(fill_call("https://xn--bnk-qla.example", "jane", "pass-a", "").as_str()));
        let refused: serde_json::Value = serde_json::from_str(&bitwarden_fill(&shared, "about:blank", "about:blank", "a")).unwrap();
        assert!(refused["call"].is_null() && refused["error"].as_str().is_some_and(|e| !e.is_empty()));
    }

    /// The call is Rust's whole: whatever a password holds (quotes, a
    /// backslash, an apostrophe, `</script>`, the line separators, line
    /// breaks, a null, a template's marks), the four values come back intact
    /// when the call's arguments are read as JSON, and none of it is left raw
    /// to end a string, a line or a script.
    #[test]
    fn nothing_in_a_password_can_change_the_call() {
        let password = "a\"b\\c'd</script><!--e\u{2028}f\u{2029}g\nh\ri\u{0}j${k}`l";
        let user = "jane\"</script>@example.org";
        let call = fill_call("https://bank.example.org", user, password, "123456");
        let prefix = format!("(function (origin, user, password, code) {{ if (location.origin !== origin) return false; return ({FILL_SCRIPT})(user, password, code); }})(");
        assert!(call.starts_with(&prefix) && call.ends_with(')'));
        let args = &call[prefix.len()..call.len() - 1];
        let read: Vec<String> = serde_json::from_str(&format!("[{args}]")).expect("the arguments are four JSON strings");
        assert_eq!(read, ["https://bank.example.org", user, password, "123456"]);
        for raw in ["\n", "\r", "\u{2028}", "\u{2029}", "\u{0}", "<"] {
            assert!(!args.contains(raw), "{raw:?} left raw in the arguments");
        }
        // Each value one literal: its quotes are the only ones not escaped.
        let unescaped = args.char_indices().filter(|(at, c)| *c == '"' && args[..*at].chars().rev().take_while(|b| *b == '\\').count() % 2 == 0).count();
        assert_eq!(unescaped, 8);
        assert_eq!(fill_call("", "", "", ""), format!("{prefix}\"\", \"\", \"\", \"\")"));
    }

    /// The calls tests/qml/tst_fillcall.qml runs in Qt's JavaScript engine,
    /// with stand-ins for a page's `location` and `document`, as Rust builds
    /// them now: invented logins, each with the address it was asked for.
    fn calls_for_qml() -> String {
        let cases = [
            ("right", "https://jane@Bank.Example.org:443/login?next=%2F", "jane@example.org", "p\"a'ss\\</script>\u{2028}x\ny", "123456"),
            ("port", "https://bank.example.org:8443/sign-in", "jane", "pass-port", ""),
            ("ipv6", "https://[::1]:8443/sign-in", "jane", "pass-six", ""),
            ("code", "https://bank.example.org/second-step", "jane", "pass-code", "654321"),
        ];
        let mut text = String::from(
            "// SPDX-License-Identifier: GPL-3.0-or-later\n// Copyright © 2026 Aurélien Pierre\n\n// Made by crates/sioul-app/src/sites.rs (tests::the_page_s_engine_runs_rust_s_own_calls),\n// which fails while this file is not the calls Rust builds now: invented logins,\n// each with the address it was asked for and its call (sites::fill_call), run\n// by tst_fillcall.qml with stand-ins for a page.\n.pragma library\n\nvar cases = [\n",
        );
        for (name, url, user, password, code) in cases {
            let origin = web_origin(url).unwrap_or_default();
            let call = fill_call(&origin, user, password, code);
            text.push_str(&format!(
                "    {{ name: {}, url: {}, user: {}, password: {}, code: {}, call: {} }},\n",
                js_text(name),
                js_text(url),
                js_text(user),
                js_text(password),
                js_text(code),
                js_text(&call)
            ));
        }
        text.push_str("];\n");
        text
    }

    /// The guarded call as a page's engine runs it (tst_fillcall.qml): the
    /// file it runs is the calls Rust builds now.
    #[test]
    fn the_page_s_engine_runs_rust_s_own_calls() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/qml/fillcalls.js");
        let made = calls_for_qml();
        let on_disk = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(on_disk == made, "tests/qml/fillcalls.js is not the calls Rust builds now: put the text between the lines in it, then run tools/qml-test.sh tst_fillcall.qml\n-----\n{made}-----");
    }
}
