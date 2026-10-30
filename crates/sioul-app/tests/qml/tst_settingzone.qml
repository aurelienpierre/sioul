// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Settings ▸ Agenda ▸ Time zone (SettingRow.qml's "zone" kind), on a
// stand-in for Sioul: this device's own at first, named on its line;
// "Another time zone…" opens the search (ZonePicker.qml), a zone picked is
// saved; back to this device's saves "". Run by tools/qml-test.sh.

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

        function text(key) {
            return key
        }
        function zoneNames(query) {
            return JSON.stringify(["America/New_York", "Europe/Paris"].filter(n => n.toLowerCase().replace(/_/g, " ").indexOf(query.toLowerCase()) >= 0))
        }
    }

    property var saved: []
    property var setting: ({ key: "agenda.time_zone", kind: "zone", label: "Time zone", help: "", value: "", choices: [{ value: "", label: "Follow this device (Europe/Paris)" }] })

    SettingRow {
        id: row

        width: parent.width
        setting: root.setting
        sioul: mock
        theme: testTheme
        onSave: (key, value, shown) => root.saved = root.saved.concat([[key, value]])
    }

    TestCase {
        name: "SettingZone"
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

        function test_another_zone_is_picked_and_saved() {
            root.setting = { key: "agenda.time_zone", kind: "zone", label: "Time zone", help: "", value: "", choices: [{ value: "", label: "Follow this device (Europe/Paris)" }] }
            const box = find(row, "settingZone")
            verify(box !== null && box.visible)
            compare(box.currentText, "Follow this device (Europe/Paris)")
            const query = find(row, "zoneQuery")
            verify(!query.visible, "no search while this device's own is followed")
            box.activated(1)
            verify(query.visible, "the search shows")
            compare(root.saved.length, 0, "nothing saved before a zone is picked")
            query.text = "new y"
            query.textEdited()
            tryVerify(() => find(row, "zoneMatch") !== null)
            find(row, "zoneMatch").clicked()
            compare(root.saved, [["agenda.time_zone", "America/New_York"]])
        }

        function test_back_to_this_device_saves_none() {
            root.setting = { key: "agenda.time_zone", kind: "zone", label: "Time zone", help: "", value: "America/New_York", choices: [{ value: "", label: "Follow this device (Europe/Paris)" }] }
            const box = find(row, "settingZone")
            compare(box.currentIndex, 1, "another zone chosen")
            compare(find(row, "zoneQuery").text, "America/New_York")
            box.activated(0)
            compare(root.saved, [["agenda.time_zone", ""]])
        }
    }
}
