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
        // Whether Android lets Sioul read the call history (`Calls.state`'s "history").
        property bool history: false
        function text(key) { return key }
        function textWith(key, name, value) { return key + "(" + value + ")" }
        function reminderOpened(link, a, b) {}
        function setup() {
            const choices = [{ value: "voicemail", label: "screening-rule-voicemail" }, { value: "work", label: "screening-rule-work" }, { value: "ring", label: "screening-rule-ring" }]
            return JSON.stringify({
                android: true,
                state: { api: 34, available: true, held: true, contacts: true, history: mock.history, table: true, emergency_at: 0 },
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
            if (verb === "ask-history")
                mock.history = true
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

        function all(item, test, into) {
            const found = into || []
            if (!item)
                return found
            if (test(item) && found.indexOf(item) < 0)
                found.push(item)
            const kids = item.children || []
            for (let i = 0; i < kids.length; ++i)
                all(kids[i], test, found)
            return found
        }

        // The call history not read yet: said, with Android's question one press away; read, nothing said.
        function test_the_call_history_is_asked_for_once() {
            mock.changes = []
            mock.history = false
            setup.reload()
            const ask = () => all(setup, i => i.visible && i.text === "calls-setup-history-allow" && typeof i.clicked === "function")
            tryCompare(ask(), "length", 1)
            verify(all(setup, i => i.visible && i.text === "calls-setup-history-off").length === 1, "what is missing, said")
            mouseClick(ask()[0])
            compare(mock.changes.length, 1)
            compare(mock.changes[0][0], "ask-history")
            tryCompare(ask(), "length", 0)
            mock.changes = []
        }

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
