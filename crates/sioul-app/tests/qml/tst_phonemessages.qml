// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The phone's messages on a computer's Porch (PhoneMessagesSection.qml) and
// the phone's tab for them (PhoneMessagesSetup.qml), on a stand-in for
// Sioul. What travels and when a line shows are tested in sioul-core
// (phonemsgs.rs) and sioul-sync (share.rs); here the window's side: the
// lines and their messages, Text back only for a text from a number, Seen
// and its ten seconds to undo, the part's switch and an app's choice handed
// to Rust. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 900
    height: 1200

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        // What the window asked, in order: [verb, args].
        property var asked: []
        property var seen: []
        property bool partOn: false
        property string pharmacy: "off"
        // Made up: no real person, fiction numbers only.
        readonly property var lines: [
            { ids: ["1-a"], head: "While you slept, Dr Elena Varga wrote:", said: [{ time: "23:10", from: "", text: "Could you call me back tomorrow morning?" }], note: "", number: "04 65 71 47 70", dial: "+33465714770", known: true, texted: true, code: false },
            { ids: ["2-b"], head: "A code came from My Bank at 11:05. It stays on your phone.", said: [], note: "", number: "", dial: "", known: false, texted: false, code: true },
            { ids: ["3-c"], head: "In your leisure time, Sam wrote on Signal:", said: [{ time: "13:02", from: "", text: "" }], note: "Their words stay on your phone: Signal sends who and when only.", number: "", dial: "", known: false, texted: false, code: false }
        ]

        signal porchChanged()

        function text(key) {
            return ({ "calls-seen": "Seen", "calls-text-back": "Text back", "calls-call-back": "Call back", "ui-undo": "Undo", "phonemsgs-title": "From your phone" })[key] || key
        }
        function viewFlag(flag) {
            return false
        }
        function textWith(key, name, value) {
            return key + ":" + value
        }
        function phoneMessages(verb, json) {
            const args = JSON.parse(json)
            mock.asked = mock.asked.concat([[verb, args]])
            if (verb === "view")
                return JSON.stringify({ lines: mock.lines.filter(l => mock.seen.indexOf(l.ids[0]) < 0), links: { tel: true, sms: true } })
            if (verb === "seen") {
                mock.seen = mock.seen.concat(args.ids)
                return JSON.stringify({ said: "Marked seen.", shared: true, at: 1791446400000 })
            }
            if (verb === "unseen") {
                mock.seen = []
                return JSON.stringify({ said: "", shared: true, at: 0 })
            }
            if (verb === "part")
                mock.partOn = args.on
            if (verb === "set")
                mock.pharmacy = args.value
            return JSON.stringify({
                android: true,
                sharing: true,
                said: "",
                part: { key: "part", kind: "bool", label: "Messages from your phone", help: "", value: mock.partOn, choices: [] },
                apps: [{ key: "app.foundation.e.message", kind: "choice", label: "Message", help: "The phone's SMS app", value: "words", choices: [{ value: "off", label: "Not on your computers" }, { value: "who", label: "Who and when" }, { value: "words", label: "Who, when and the words" }] }]
            })
        }
    }

    QtObject {
        id: stand

        property var now: 0
        property bool compact: false
    }

    Column {
        width: root.width - 32
        x: 16
        spacing: 12

        PhoneMessagesSection {
            id: section

            width: parent.width
            sioul: mock
            theme: testTheme
            window: stand
        }
        PhoneMessagesSetup {
            id: setup

            width: parent.width
            sioul: mock
            theme: testTheme
        }
    }

    TestCase {
        name: "PhoneMessages"
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
        function labels(under, text) {
            return walk(under, i => i.visible && i.text === text && i.clicked === undefined, [])
        }
        function buttons(under, text) {
            return walk(under, i => i.visible && i.text === text && i.clicked !== undefined, [])
        }

        function init() {
            mock.asked = []
            mock.seen = []
            section.reload()
            wait(50)
        }

        function test_lines_and_their_messages() {
            compare(labels(section, "While you slept, Dr Elena Varga wrote:").length, 1)
            compare(labels(section, "Could you call me back tomorrow morning?").length, 1)
            compare(labels(section, "A code came from My Bank at 11:05. It stays on your phone.").length, 1)
            compare(labels(section, "Their words stay on your phone: Signal sends who and when only.").length, 1)
            // A text from a number: Text back and Call back on its line only.
            compare(buttons(section, "Text back").length, 1)
            compare(buttons(section, "Call back").length, 1)
            compare(buttons(section, "Seen").length, 3)
        }

        function test_seen_then_undone() {
            mouseClick(buttons(section, "Seen")[0])
            const last = mock.asked.filter(a => a[0] === "seen")
            compare(last.length, 1)
            compare(last[0][1].ids, ["1-a"])
            wait(50)
            compare(labels(section, "While you slept, Dr Elena Varga wrote:").length, 0, "gone at once")
            compare(labels(section, "Marked seen.").length, 1)
            const undo = buttons(section, "Undo")
            compare(undo.length, 1, "ten seconds to undo")
            mouseClick(undo[0])
            const back = mock.asked.filter(a => a[0] === "unseen")
            compare(back.length, 1)
            compare(back[0][1].at, 1791446400000)
            wait(50)
            compare(labels(section, "While you slept, Dr Elena Varga wrote:").length, 1, "back")
        }

        function test_the_phone_s_tab_hands_its_choices_to_rust() {
            const rows = walk(setup, i => i.visible && i.setting !== undefined && i.save !== undefined, [])
            const part = rows.filter(r => r.setting.key === "part")[0]
            const app = rows.filter(r => r.setting.key === "app.foundation.e.message")[0]
            verify(part && app)
            part.save("part", true, part.setting.value)
            verify(mock.partOn)
            compare(mock.asked[mock.asked.length - 1][0], "part")
            app.save("app.foundation.e.message", "off", app.setting.value)
            compare(mock.pharmacy, "off")
            compare(mock.asked[mock.asked.length - 1][1].key, "app.foundation.e.message")
        }
    }
}
