// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A folder's row in Settings (SettingRow.qml's "folder" kind), on a stand-in
// for Sioul: a folder of yours outside Sioul's own says nothing more; one
// inside Sioul's own says, under the row, what that gains and what it loses
// (docs/store.md, "Your folders inside Sioul's"); the folder is saved either
// way. Made-up paths. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 700
    height: 500

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        property string status: ""

        function text(key) {
            return key
        }
        // Inside Sioul's own folders (made-up): the trade-off said.
        function folderSealed(folder) {
            return folder.indexOf("/.local/share/sioul/") >= 0 ? "storage-sealed-tradeoff" : ""
        }
    }

    property var saved: []
    property var setting: ({ key: "papers.folder", kind: "folder", label: "Papers' folder", help: "", value: "/stand-in/Documents/Papers" })

    SettingRow {
        id: row

        width: parent.width
        setting: root.setting
        sioul: mock
        theme: testTheme
        onSave: (key, value, shown) => root.saved = root.saved.concat([[key, value]])
    }

    TestCase {
        name: "SettingFolder"
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
            if (item.contentItem && item.contentItem !== item) {
                const inside = find(item.contentItem, name)
                if (inside)
                    return inside
            }
            return null
        }

        function init() {
            root.saved = []
        }

        function test_a_folder_outside_says_nothing_more() {
            root.setting = { key: "papers.folder", kind: "folder", label: "Papers' folder", help: "", value: "/stand-in/Documents/Papers" }
            const said = find(row, "settingFolderSealed")
            verify(said !== null)
            verify(!said.visible, "outside Sioul's folders: in clear, nothing to say")
        }

        function test_a_folder_inside_sioul_s_says_what_is_gained_and_lost() {
            root.setting = { key: "papers.folder", kind: "folder", label: "Papers' folder", help: "", value: "/stand-in/.local/share/sioul/Papers" }
            const said = find(row, "settingFolderSealed")
            verify(said !== null && said.visible)
            compare(said.text, "storage-sealed-tradeoff")
        }
    }
}
