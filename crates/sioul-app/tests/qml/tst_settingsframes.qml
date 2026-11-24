// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A page's ⚙ (SettingsPanel.qml) draws no empty frame (24 November 2026):
// Mail ⚙'s last group has no heading and holds only the row that leaves
// the lists of the mail you called spam (LeaveSpam.qml), which draws
// nothing while there is none to leave; its frame shows only once there
// are some. Made-up rows, on a stand-in for Sioul. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 900
    height: 800

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        signal leaveSpamFound(string found)

        function text(key) {
            return key
        }
        function textWith(key, name, value) {
            return key + " " + value
        }
        function viewFlag(name) {
            return false
        }
        function leaveSpamLook() {
        }
        function settings(view) {
            return JSON.stringify([
                { key: "mail.threads", kind: "bool", label: "By conversation", help: "", value: false, choices: [], group: "", section: "", min: 0, max: 0, step: 1, unit: "" },
                { key: "topics.list", kind: "bool", label: "Topics", help: "", value: false, choices: [], group: "Topics", section: "", min: 0, max: 0, step: 1, unit: "" },
                { key: "mail.leave_spam", kind: "leavespam", label: "set-leave-spam", help: "Only mail you said is spam yourself.", value: "", choices: [], group: "", section: "", min: 0, max: 0, step: 1, unit: "" }
            ])
        }
    }

    Component {
        id: panelComponent

        SettingsPanel {
            sioul: mock
            theme: testTheme
            view: "mail"
        }
    }

    TestCase {
        name: "SettingsFrames"
        when: windowShown

        function frames(panel) {
            const out = []
            const walk = item => {
                if (item instanceof Section)
                    out.push(item)
                for (let i = 0; i < item.children.length; i++)
                    walk(item.children[i])
            }
            walk(panel.contentItem)
            return out
        }

        function test_no_empty_frame_for_a_row_that_draws_nothing() {
            const panel = createTemporaryObject(panelComponent, root)
            panel.reload()
            panel.open()
            tryCompare(panel, "opened", true)
            tryCompare(frames(panel), "length", 3)
            const last = frames(panel)[2]
            verify(frames(panel)[0].visible && frames(panel)[1].visible, "the others as they are, a heading or not")
            tryVerify(() => !last.visible, 2000, "no lists to leave: no frame")
            mock.leaveSpamFound(JSON.stringify({ count: 1, row: "Leave the lists of the mail you called spam: 1 list", button: "Leave this list", ended: false, lists: [{ key: "news@old.example", name: "Old News", domain: "old.example", why: "" }] }))
            tryVerify(() => last.visible, 2000, "one to leave: its frame")
            panel.close()
        }
    }
}
