// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Leaving the lists of the mail you called spam (LeaveSpam.qml), on a
// stand-in for Sioul: nothing shows without lists, a row with them, its sheet
// names them as plain text and says a failure under its list; "Leave these"
// asks for those lists alone; at a phone's 412 points nothing passes the
// window's edge and fingers have room. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 412
    height: 732

    Theme {
        id: testTheme

        touch: true
    }

    QtObject {
        id: mock

        // What the window asked of Rust, in order.
        property var asked: []
        signal leaveSpamFound(string found)

        function text(key) {
            return key
        }
        function leaveSpamLook() {
            mock.asked = mock.asked.concat([["look"]])
        }
        function leaveSpamRun(keys) {
            mock.asked = mock.asked.concat([["run", JSON.parse(keys)]])
        }
    }

    readonly property string two: JSON.stringify({
        count: 2,
        row: "Quitter les listes des courriels que vous dites indésirables : 2 listes",
        button: "Quitter ces 2 listes",
        ended: false,
        lists: [
            { key: "list-id:letters.example", name: "<b>Type</b> & Pixels", domain: "letters.example", why: "" },
            { key: "news@old.example", name: "Old News", domain: "old.example", why: "Not unsubscribed from Old News: its server answered 404." }
        ]
    })

    Component {
        id: shown

        LeaveSpam {
            width: root.width
            sioul: mock
            theme: testTheme
            about: "Only mail you said is spam yourself."
            title: "Lists you left"
        }
    }

    TestCase {
        name: "LeaveSpam"
        when: windowShown

        function init() {
            mock.asked = []
        }

        function open() {
            const block = createTemporaryObject(shown, root)
            verify(block)
            return block
        }

        function find(item, wanted) {
            if (item.text === wanted)
                return item
            const kids = item.children || []
            for (let i = 0; i < kids.length; i++) {
                const hit = find(kids[i], wanted)
                if (hit)
                    return hit
            }
            return null
        }

        function test_nothing_shows_without_lists() {
            const block = open()
            compare(mock.asked, [["look"]], "it looks off the window's thread")
            verify(!block.visible)
            mock.leaveSpamFound(JSON.stringify({ count: 0, row: "", button: "", lists: [], ended: false }))
            verify(!block.visible)
        }

        function test_a_row_opens_the_sheet_with_the_names() {
            const block = open()
            mock.leaveSpamFound(root.two)
            verify(block.visible)
            waitForRendering(block)
            const row = find(block, "Quitter les listes des courriels que vous dites indésirables : 2 listes")
            verify(row, "the row says how many")
            verify(row.height >= 48, "room for a finger")
            verify(row.width <= root.width)
            const sheet = block.sheet
            sheet.open()
            tryCompare(sheet, "opened", true)
            verify(sheet.width <= root.width, "within a phone's edge")
            // A name from a message is shown as written: never read as markup.
            const name = find(sheet.contentItem, "<b>Type</b> & Pixels")
            verify(name, "its name, as written")
            compare(name.textFormat, Text.PlainText)
            verify(find(sheet.contentItem, "letters.example"), "its domain")
            verify(find(sheet.contentItem, "Not unsubscribed from Old News: its server answered 404."), "why not, the last time")
            sheet.accept()
            compare(mock.asked[mock.asked.length - 1], ["run", ["list-id:letters.example", "news@old.example"]])
        }

        function test_cancel_asks_nothing() {
            const block = open()
            mock.leaveSpamFound(root.two)
            const sheet = block.sheet
            sheet.open()
            tryCompare(sheet, "opened", true)
            sheet.reject()
            compare(mock.asked, [["look"]])
        }

        function test_the_lists_gone_close_the_sheet() {
            const block = open()
            mock.leaveSpamFound(root.two)
            const sheet = block.sheet
            sheet.open()
            tryCompare(sheet, "opened", true)
            let ended = 0
            block.ended.connect(() => ended += 1)
            mock.leaveSpamFound(JSON.stringify({ count: 0, row: "", button: "", lists: [], ended: true }))
            tryCompare(sheet, "visible", false)
            verify(!block.visible)
            compare(ended, 1, "the rows read again once a run ended")
        }
    }
}
