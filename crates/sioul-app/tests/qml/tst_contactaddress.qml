// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A contact's postal address in its card (AddressLine.qml, ContactsPage.qml's
// details), on a stand-in for Sioul: its Directions opens a `geo:` link with
// the point the contacts' map placed it at, else its words, when the system
// has an app for such links (asked at each press); else OpenStreetMap's
// directions, or Apple's maps on macOS; the label's cell offers nothing.
// The links are made by the event card's own helper (Directions.qml), so the
// two never differ. Nothing is opened for real: the opener is watched. Run
// by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 412
    height: 400

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        // Whether this system has an app for geo: links, as asked at each press.
        property bool handler: true
        property int asked: 0
        // Made up: no real person, no real place.
        readonly property var placed: ({ "3 rue de l'Exemple, 75000 Paris": { lat: 48.88, lon: 2.33 } })

        function text(key) {
            return key
        }
        function placeOf(address) {
            return mock.placed[address] ? JSON.stringify(mock.placed[address]) : ""
        }
        function geoHandler() {
            mock.asked += 1
            return mock.handler
        }
    }

    property var opened: []

    AddressLine {
        id: placedLine

        width: root.width
        theme: testTheme
        sioul: mock
        text: "3 rue de l'Exemple, 75000 Paris"
        address: "3 rue de l'Exemple, 75000 Paris"
        way.opener: url => { root.opened.push(url); return true }
    }

    AddressLine {
        id: wordsLine

        y: 120
        width: root.width
        theme: testTheme
        sioul: mock
        text: "1 Example Road, Springfield"
        address: "1 Example Road, Springfield"
        way.opener: url => { root.opened.push(url); return mock.handler }
    }

    AddressLine {
        id: labelLine

        y: 240
        theme: testTheme
        sioul: mock
        text: "contact-address · home"
    }

    TestCase {
        name: "ContactAddress"
        when: windowShown

        function button(line) {
            return findChild(line, "contactDirections")
        }

        function init() {
            root.opened = []
            mock.handler = true
            mock.asked = 0
        }

        // Placed by the contacts' map: its point, and its words.
        function test_a_placed_address_opens_its_point() {
            verify(button(placedLine).visible)
            compare(button(placedLine).text, "map-directions")
            mouseClick(button(placedLine))
            compare(root.opened, ["geo:48.88,2.33?q=3%20rue%20de%20l'Exemple%2C%2075000%20Paris"])
            compare(mock.asked, 1, "asked at the press, never assumed")
        }

        // Not placed: geo:0,0 and its words.
        function test_an_address_not_placed_opens_its_words() {
            mouseClick(button(wordsLine))
            compare(root.opened, ["geo:0,0?q=1%20Example%20Road%2C%20Springfield"])
        }

        // No app for geo: links: OpenStreetMap's directions; on macOS, Apple's maps.
        function test_no_app_for_geo_links_opens_a_page() {
            mock.handler = false
            mouseClick(button(wordsLine))
            compare(root.opened, ["https://www.openstreetmap.org/directions?to=1%20Example%20Road%2C%20Springfield"])
            mock.handler = true
            compare(placedLine.way.open({ lat: 48.88, lon: 2.33 }, "3 rue de l'Exemple", "osx"), "geo:48.88,2.33?q=3%20rue%20de%20l'Exemple", "an app there: its link")
            mock.handler = false
            root.opened = []
            compare(wordsLine.way.open(null, "1 Example Road", "osx"), "https://maps.apple.com/?daddr=1%20Example%20Road")
            compare(mock.asked, 3, "asked again at each press")
        }

        // The label's cell: no Directions; an empty address opens nothing.
        function test_a_label_offers_nothing() {
            verify(!button(labelLine).visible)
            compare(labelLine.way.open(null, " ", "linux"), "")
            compare(root.opened, [])
        }
    }
}
