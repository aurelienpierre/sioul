// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Settings ▸ This phone ▸ Calls (CallsSetup.qml): the call screening's own
// section, apart from do-not-disturb: its rows as calls.rs's `setup` gives
// them, each change sent back as "set" with its key and value, the page
// drawn again from the answer. On a stand-in for Sioul: no Sioul started,
// no profile read. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 412
    height: 2400

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        property var changes: []
        property string hidden: "voicemail"
        function text(key) { return key }
        function textWith(key, name, value) { return key + "(" + value + ")" }
        function reminderOpened(link, a, b) {}
        function setup() {
            const choices = [{ value: "voicemail", label: "screening-rule-voicemail" }, { value: "work", label: "screening-rule-work" }, { value: "ring", label: "screening-rule-ring" }]
            return JSON.stringify({
                android: true,
                state: { api: 34, available: true, held: true, contacts: true, table: true, emergency_at: 0 },
                moment: {},
                screening: {
                    title: "screening-title",
                    intro: "screening-intro",
                    hears: "screening-hears",
                    expecting: "screening-expecting",
                    rows: [
                        { key: "screening.on", kind: "bool", label: "screening-on", help: "screening-on-help", value: true, choices: [] },
                        { key: "screening.hidden", kind: "choice", label: "screening-hidden", help: "screening-hidden-help", value: mock.hidden, choices: choices },
                    ],
                },
            })
        }
        function callsSetup() { return mock.setup() }
        function callsSetupChange(verb, json) {
            const asked = JSON.parse(json)
            mock.changes = mock.changes.concat([[verb, asked]])
            if (verb === "set" && asked.key === "screening.hidden")
                mock.hidden = asked.value
            return mock.setup()
        }
    }

    Flickable {
        anchors.fill: parent
        contentHeight: setup.implicitHeight

        CallsSetup {
            id: setup

            width: root.width
            sioul: mock
            theme: testTheme
        }
    }

    TestCase {
        name: "CallsSetup"
        when: windowShown

        function test_the_screening_s_rows_are_drawn_and_saved_through_the_page() {
            tryVerify(() => findChild(setup, "screening-row-screening.hidden") !== null)
            const on = findChild(setup, "screening-row-screening.on")
            const hidden = findChild(setup, "screening-row-screening.hidden")
            verify(on !== null && hidden !== null, "both rows drawn")
            compare(hidden.setting.value, "voicemail")
            verify(hidden.width <= root.width, "within the phone's width")
            // A choice: "set", its key and value; the page drawn again from the answer.
            hidden.save("screening.hidden", "work", "voicemail")
            compare(mock.changes.length, 1)
            compare(mock.changes[0][0], "set")
            compare(mock.changes[0][1].key, "screening.hidden")
            compare(mock.changes[0][1].value, "work")
            tryCompare(findChild(setup, "screening-row-screening.hidden").setting, "value", "work")
            // A switch: its key and a boolean.
            on.save("screening.on", false, true)
            compare(mock.changes[1][1].key, "screening.on")
            compare(mock.changes[1][1].value, false)
        }
    }
}
