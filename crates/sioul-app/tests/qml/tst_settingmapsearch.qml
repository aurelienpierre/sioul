// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Contacts ⚙ ▸ The map's search service (SettingRow.qml's "mapsearch" kind),
// on a stand-in for Sioul: what it does said first, what is asked now in
// strong words, then the address in its field and the two buttons after it;
// an address typed is saved as typed, OpenStreetMap's and None by their
// press, the button of what is set now off; None empties the field and says
// that nothing is asked. Within the edge of a phone. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 412
    height: 700

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        function text(key) {
            return key
        }
        function textWith(key, name, value) {
            return key + ":" + name + "=" + value
        }
    }

    readonly property var choices: [{ value: "https://nominatim.openstreetmap.org/search", label: "map-search-default" }, { value: "none", label: "map-search-none" }]
    property var saved: []
    property var setting: ({ key: "map.search", kind: "mapsearch", label: "set-map-search", help: "set-map-search-help", value: "https://nominatim.openstreetmap.org/search", choices: root.choices })

    SettingRow {
        id: row

        width: parent.width
        setting: root.setting
        sioul: mock
        theme: testTheme
        onSave: (key, value, shown) => root.saved = root.saved.concat([[key, value]])
    }

    TestCase {
        name: "SettingMapSearch"
        when: windowShown

        function find(item, name) {
            if (!item)
                return null
            if (item.objectName === name)
                return item
            const kids = item.children || []
            for (let i = 0; i < kids.length; ++i) {
                const found = find(kids[i], name)
                if (found)
                    return found
            }
            return null
        }

        function init() {
            root.saved = []
            root.setting = { key: "map.search", kind: "mapsearch", label: "set-map-search", help: "set-map-search-help", value: "https://nominatim.openstreetmap.org/search", choices: root.choices }
            waitForRendering(row)
        }

        function test_what_is_asked_and_its_field() {
            const field = find(row, "mapSearchField")
            compare(find(row, "mapSearchNow").text, "map-search-now:host=nominatim.openstreetmap.org")
            compare(field.text, "https://nominatim.openstreetmap.org/search")
            verify(!find(row, "mapSearchChoice0").enabled, "OpenStreetMap's is set: its button off")
            verify(find(row, "mapSearchChoice1").enabled)
            // What it does comes before the field, the field before its buttons.
            const y = item => item.mapToItem(row, 0, 0).y
            verify(y(find(row, "mapSearchNow")) < y(field) && y(field) < y(find(row, "mapSearchChoice0")))
            // Its sentence said once, first.
            const said = []
            const walk = item => {
                if (item.visible && item.text === "set-map-search-help")
                    said.push(item)
                for (let i = 0; i < item.children.length; i++)
                    walk(item.children[i])
            }
            walk(row)
            compare(said.length, 1)
            verify(y(said[0]) < y(find(row, "mapSearchNow")))
            // Another address, typed as a person types, saved as typed.
            mouseClick(field)
            field.selectAll()
            keyClick(Qt.Key_Delete)
            for (const c of "https://geo.example.org/search")
                keyClick(c)
            keyClick(Qt.Key_Return)
            compare(root.saved, [["map.search", "https://geo.example.org/search"]])
            // Within the phone's edge.
            for (const item of [field, find(row, "mapSearchChoice0"), find(row, "mapSearchChoice1")])
                verify(item.mapToItem(row, 0, 0).x + item.width <= row.width + 1, item.objectName)
        }

        function test_none_and_back() {
            mouseClick(find(row, "mapSearchChoice1"))
            compare(root.saved, [["map.search", "none"]])
            root.setting = Object.assign({}, root.setting, { value: "none" })
            waitForRendering(row)
            compare(find(row, "mapSearchNow").text, "map-search-off-now")
            compare(find(row, "mapSearchField").text, "")
            verify(!find(row, "mapSearchChoice1").enabled && find(row, "mapSearchChoice0").enabled)
            mouseClick(find(row, "mapSearchChoice0"))
            compare(root.saved[1], ["map.search", "https://nominatim.openstreetmap.org/search"])
        }
    }
}
