// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The late question for a dose (DoseTaken.qml). An open dose starts from now,
// and Enter marks it then. One marked already starts from its own time, with
// Cancel the default: Enter as it is rewrites nothing; a time changed is
// said (the check of 1b6795e, M4). Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml" as App

Item {
    id: root
    width: 480
    height: 640

    QtObject {
        id: mock
        property var asked: []
        property bool marked: false
        function text(key) { return key }
        function textWith(key, name, value) { return key + ":" + value }
        function textArgs(key, args) { return key }
        function doseInfo(key) {
            return JSON.stringify({ name: "Corvan", dose: "1 g", due: "08:00", now: "13:00", hours: 6, taken: mock.marked ? "08:03" : "", taken_at: mock.marked ? "08:03" : "" })
        }
        function doseTakenAt(key, time) {
            mock.asked = mock.asked.concat([key + " " + time])
            return ""
        }
        function doseNotTaken(key) {
            mock.asked = mock.asked.concat(["not " + key])
        }
    }
    QtObject {
        id: theme
        property color text: "#2d2a26"
        property color muted: "#6b655c"
        property color warm: "#8f6330"
        property int gap: 16
    }

    App.DoseTaken {
        id: dialog
        sioul: mock
        theme: theme
    }

    TestCase {
        name: "DoseTaken"
        when: windowShown

        function walk(item, test, found) {
            if (!item)
                return found
            if (test(item))
                found.push(item)
            const children = item.children || []
            for (let i = 0; i < children.length; ++i)
                walk(children[i], test, found)
            if (item.contentItem && item.contentItem !== item && children.indexOf(item.contentItem) < 0)
                walk(item.contentItem, test, found)
            return found
        }
        function field() {
            return walk(dialog.contentItem, i => typeof i.inputMask === "string" && i.inputMask.indexOf("99:99") === 0, [])[0]
        }
        function button(text) {
            return walk(dialog.footer, i => i.text === text && i.clicked !== undefined && i.highlighted !== undefined, [])[0]
        }
        function init() {
            mock.asked = []
            dialog.close()
            tryVerify(() => !dialog.visible, 2000)
        }

        function test_an_open_dose_starts_from_now() {
            mock.marked = false
            dialog.ask("corvan@1")
            tryVerify(() => dialog.opened, 2000)
            compare(field().text, "13:00")
            keyClick(Qt.Key_Return)
            compare(mock.asked, ["corvan@1 13:00"])
        }

        function test_a_marked_dose_starts_from_its_time_and_enter_rewrites_nothing() {
            mock.marked = true
            dialog.ask("corvan@1")
            tryVerify(() => dialog.opened, 2000)
            compare(field().text, "08:03", "its own time, never now")
            verify(button("ui-cancel") !== undefined && button("health-taken") !== undefined, "Cancel beside Taken")
            keyClick(Qt.Key_Return)
            tryVerify(() => !dialog.visible, 2000)
            compare(mock.asked, [], "nothing rewritten")
            // A time changed, said.
            dialog.ask("corvan@1")
            tryVerify(() => dialog.opened, 2000)
            field().text = "09:30"
            field().forceActiveFocus()
            keyClick(Qt.Key_Return)
            compare(mock.asked, ["corvan@1 09:30"])
        }
    }
}
