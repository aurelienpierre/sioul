// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A setting opened by its key (SettingDoor.qml, main.qml's
// `showParameters`): each lands where it lives now, on a stand-in for the
// window that records what it is asked, and a stand-in for Sioul that
// says where each key lives as the core's `settings::page_of` does (its
// own test, `a_link_by_key_opens_the_page_that_holds_it`, checks the core's
// side). Run by tools/qml-test.sh.

import QtQuick
import QtTest
import "../../qml"

TestCase {
    id: root

    name: "SettingDoor"

    // What the core says of each key (`settings::page_of`): "" for Settings.
    QtObject {
        id: mock

        readonly property var places: ({
            "screen_colours": "sites",
            "calmer_colours": "sites",
            "sites": "sites"
        })

        function settingPage(key) {
            return mock.places[key] || ""
        }
    }

    // A page that records its ⚙ opened, at which setting.
    component FakePage: QtObject {
        property var asked: []

        function openSettings(open, key) {
            asked = asked.concat([[open, key]])
        }
    }

    QtObject {
        id: fakeWindow

        property int page: 0
        property var parametersPage: QtObject {
            property var asked: []

            function showSetting(key) {
                asked = asked.concat([key])
            }
        }
        property var sitesPage: FakePage {}
    }

    SettingDoor {
        id: door

        window: fakeWindow
        sioul: mock
    }

    function init() {
        fakeWindow.page = 0
        fakeWindow.parametersPage.asked = []
        fakeWindow.sitesPage = Qt.createQmlObject("import QtQuick; QtObject { property var asked: []; function openSettings(open, key) { asked = asked.concat([[open, key]]) } }", root)
    }

    // The sites' colours: the Sites page, its ⚙ open at the row.
    function test_the_sites_colours_open_the_sites_page_settings() {
        door.open("calmer_colours")
        compare(fakeWindow.page, 3)
        compare(fakeWindow.sitesPage.asked, [[true, "calmer_colours"]])
        compare(fakeWindow.parametersPage.asked, [], "not Settings")
        door.open("sites")
        compare(fakeWindow.sitesPage.asked[1], [true, ""], "the page's own name: its ⚙ at its top")
    }

    // What Settings holds: Settings, at its tab.
    function test_settings_keys_open_settings() {
        for (const key of ["theme", "window", "time_off", "attention.pause", "words.languages", "files"]) {
            fakeWindow.page = 0
            door.open(key)
            compare(fakeWindow.page, 12, key)
        }
        compare(fakeWindow.parametersPage.asked, ["theme", "window", "time_off", "attention.pause", "words.languages", "files"])
        compare(fakeWindow.sitesPage.asked, [])
    }

    // A page without a ⚙ of its own on this device (Android's Sites page): the page alone.
    function test_a_page_without_its_settings_shows_alone() {
        fakeWindow.sitesPage = Qt.createQmlObject("import QtQuick; QtObject {}", root)
        door.open("screen_colours")
        compare(fakeWindow.page, 3)
    }
}
