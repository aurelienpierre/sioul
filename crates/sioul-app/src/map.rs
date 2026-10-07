// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Contacts on a map: where each postal address is, from the local cache of
//! places; placing the others, once you allow it, one address a second, on
//! a thread, the map told as they come.

use crate::backend::{QtThread, Shared, json, load_config, tell, tr};
use crate::work::loaded;
use serde::Serialize;
use sioul_core::places::{Place, Places};
use std::sync::Arc;
use std::sync::atomic::Ordering;

#[derive(Serialize)]
struct Pin {
    key: String,
    name: String,
    address: String,
    lat: f64,
    lon: f64,
}

#[derive(Serialize)]
struct MapView {
    /// Addresses may be placed (you allowed it).
    allowed: bool,
    /// The tile server, as Qt's map wants it: `https://tile.openstreetmap.org/`.
    tiles: String,
    pins: Vec<Pin>,
    /// Addresses not placed yet.
    waiting: usize,
    /// Placing is under way.
    locating: bool,
}

/// The tile server's address before "{z}/{x}/{y}.png", OpenStreetMap's by default.
fn tiles() -> String {
    let configured = load_config().map.tiles.unwrap_or_default();
    let host = configured.split("{z}").next().unwrap_or("").trim().to_string();
    if host.is_empty() { "https://tile.openstreetmap.org/".to_string() } else { host }
}

/// Every contact with a postal address placed, and how many are not yet.
pub(crate) fn view(shared: &Shared) -> String {
    let loaded = loaded(shared);
    let places = Places::load(&Places::default_path());
    let mut pins = Vec::new();
    let mut waiting = 0;
    for contact in &loaded.contacts {
        for address in contact.addresses.iter().filter(|a| !a.value.trim().is_empty()) {
            match places.get(&address.value) {
                Some(Place { lat, lon }) => pins.push(Pin { key: contact.key.clone(), name: contact.name.clone(), address: address.value.clone(), lat, lon }),
                None if !places.asked(&address.value) => waiting += 1,
                None => {}
            }
        }
    }
    json(&MapView { allowed: load_config().map.geocode, tiles: tiles(), pins, waiting, locating: shared.locating.load(Ordering::Relaxed) })
}

/// Where one address is, when placed: {"lat", "lon"}, else "".
pub(crate) fn place_of(address: &str) -> String {
    Places::load(&Places::default_path()).get(address).map(|p| serde_json::json!({ "lat": p.lat, "lon": p.lon }).to_string()).unwrap_or_default()
}

/// Places the addresses not asked yet, when you allowed it: one a second, on a
/// thread; the map is told every few, and at the end.
pub(crate) fn locate(qt: &QtThread, shared: &Arc<Shared>) {
    if !load_config().map.geocode || crate::backend::offline() || shared.locating.swap(true, Ordering::Relaxed) {
        return;
    }
    let loaded = loaded(shared);
    let mut addresses: Vec<String> = loaded.contacts.iter().flat_map(|c| c.addresses.iter().map(|a| a.value.clone())).filter(|a| !a.trim().is_empty()).collect();
    addresses.sort();
    addresses.dedup();
    let (qt, shared) = (qt.clone(), Arc::clone(shared));
    let changed = |qt: &QtThread| {
        let _ = qt.queue(|mut sioul| sioul.as_mut().places_changed());
    };
    changed(&qt);
    std::thread::spawn(move || {
        let path = Places::default_path();
        let language = tr().text("qt-locale", None).replace('_', "-");
        let mut asked = 0;
        for address in addresses {
            let mut places = Places::load(&path);
            if places.asked(&address) {
                continue;
            }
            if asked > 0 {
                std::thread::sleep(sioul_sync::geocode::PAUSE);
            }
            asked += 1;
            match sioul_sync::geocode::locate(&address, &language) {
                Ok(place) => {
                    places.set(&address, place);
                    if let Err(e) = places.save(&path) {
                        tell(&qt, &shared, e);
                        break;
                    }
                }
                // The network or the service is away: what is left waits for the next time.
                Err(e) => {
                    tell(&qt, &shared, e.sentence(tr(), "OpenStreetMap"));
                    break;
                }
            }
            if asked % 5 == 0 {
                changed(&qt);
            }
        }
        shared.locating.store(false, Ordering::Relaxed);
        changed(&qt);
    });
}
