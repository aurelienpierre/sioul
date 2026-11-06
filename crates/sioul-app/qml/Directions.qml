// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The way to a place, one helper for every card that offers Directions (an
// event's place, 5 November 2026; a contact's postal address, 6 November
// 2026): a `geo:` link (RFC 5870), with its point when one is known, else its
// words; opened by the app this system has for such links, asked at each
// press and never assumed (`geoHandler`: Linux's default handler, Windows'
// URL protocol, macOS's LaunchServices; on Android, an intent that resolves);
// else a page: Apple's maps on macOS, OpenStreetMap's directions elsewhere.
// Nothing is fetched here, and no address leaves the device but to the app
// or the page you open.

pragma ComponentBehavior: Bound

import QtQuick

QtObject {
    id: directions

    // Sioul's backend: `geoHandler`.
    required property var sioul
    // How an address is opened (the tests watch it): Qt's own, false when nothing took it.
    property var opener: url => Qt.openUrlExternally(url)

    // The links to `place` for `os` (Qt.platform.os): {geo, page}; `point`
    // ({lat, lon}) when its place is known, else null.
    function uris(point, place, os) {
        const words = encodeURIComponent(place)
        const at = point ? point.lat + "," + point.lon : ""
        const geo = point ? "geo:" + at + "?q=" + words : "geo:0,0?q=" + words
        const page = os === "osx" || os === "macos" ? "https://maps.apple.com/?daddr=" + (point ? at : words) : "https://www.openstreetmap.org/directions?to=" + (point ? at : words)
        return { geo: geo, page: page }
    }

    // Directions pressed: the `geo:` link when this system has an app for
    // it, else the page. `point`: {lat, lon}, or anything else when not
    // known. What was opened, "" for no place.
    function open(point, place, os) {
        if ((place || "").trim() === "")
            return ""
        const known = point && point.lat !== undefined && point.lon !== undefined ? point : null
        const links = directions.uris(known, place.trim(), os === undefined ? Qt.platform.os : os)
        if (directions.sioul.geoHandler() && directions.opener(links.geo))
            return links.geo
        directions.opener(links.page)
        return links.page
    }
}
