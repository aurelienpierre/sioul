// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Android is not necessarily a phone (docs/android.md, "Tablets"), on a
// stand-in for Sioul: the places (Places.qml) list Texts wherever the device
// has no telephony of its own (a computer, a tablet without a SIM), and leave
// it to the device's SMS app where it has (`sioul.telephony`); their rows are a
// finger's on a touch screen or in a narrow window, a pointer's elsewhere,
// whatever the system. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 1280
    height: 800

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        property bool busy: false
        // Whether this system has calls and texts (devicekind.rs).
        property bool telephony: false
        // What the device is: "phone", "tablet" or "computer".
        property string deviceKind: "tablet"

        function text(key) {
            return key
        }
        function syncNow() {
        }
    }

    QtObject {
        id: fakeWindow

        property bool compact: false
        property bool ownTitleBar: false
        property int page: 0
        property bool placesOpen: false
        property string reviewDay: ""

        function sayRailTip(item, shown, ms) {
        }
        function showNewMenu() {
        }
        function togglePlaces(named) {
        }
    }

    Component {
        id: placesComponent

        Places {
            width: 220
            height: 760
            window: fakeWindow
            theme: testTheme
            sioul: mock
            named: true
        }
    }

    TestCase {
        name: "DeviceKind"
        when: windowShown

        function cleanup() {
            mock.telephony = false
            fakeWindow.compact = false
            testTheme.touch = false
        }

        function names(places) {
            const out = []
            for (let i = 0; i < places.repeater.count; i++)
                out.push(places.repeater.itemAt(i).name)
            return out
        }

        // Texts is a place where the device has no telephony: a computer, a
        // tablet without a SIM, which shows the phone's texts; a device with
        // telephony leaves them to its SMS app.
        function test_texts_is_a_place_where_there_is_no_telephony() {
            const places = createTemporaryObject(placesComponent, root)
            verify(names(places).indexOf("texts-page-title") >= 0, "a tablet without a SIM: " + names(places))
            mock.telephony = true
            tryVerify(() => names(places).indexOf("texts-page-title") < 0, 2000, "with telephony, the SMS app's")
            compare(names(places)[3], "ui-sites", "the places after it move up")
        }

        // A row is a finger's on a touch screen (a tablet's, wide as it is) or in
        // a narrow window, a pointer's otherwise: never decided by the system.
        function test_a_row_follows_touch_and_room() {
            const places = createTemporaryObject(placesComponent, root)
            compare(places.row, 36, "a pointer's, wide and without touch")
            testTheme.touch = true
            compare(places.row, 44, "a tablet's touch screen, wide")
            testTheme.touch = false
            fakeWindow.compact = true
            compare(places.row, 44, "a narrow window")
        }
    }
}
