// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The Contacts page's selection bar (ContactSelection.qml, wave D) on a
// stand-in for Sioul, at a computer's width and at a phone's 412 points:
// how many are selected, "Select the 132 shown"; an action asks first, its
// count and what cannot change said, the title's before and after shown,
// its group box sent with it; Go runs it; while it runs, its progress and
// Stop; at its end, what it could not do, card by card. Fiction only. Run
// by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 1200
    height: 900

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        property string contactsBatch: ""
        property string status: ""
        property var asked: []
        property var ran: []
        property int stopped: 0

        function text(key) { return key }
        function textArgs(key, args) { return key + ":" + args }
        function contactsBatchPlan(text) {
            mock.asked = mock.asked.concat([JSON.parse(text)])
            const a = JSON.parse(text)
            if (a.action === "title")
                return JSON.stringify({ count: 1, sentence: "Move the title of one contact into its field?", note: "", losses: "", cannot_line: "One stays as it is:", cannot: [{ name: "Lou Exemple", why: "its title field holds Dr already" }], previews: [["Mme Jeanne Martin", "Jeanne Martin"]] })
            return JSON.stringify({ count: a.keys.length, sentence: "Move " + a.keys.length + " contacts to “Professional”?", note: "Each keeps its identifier.", losses: "", cannot_line: "", cannot: [], previews: [] })
        }
        function runContactsBatch(text) { mock.ran = mock.ran.concat([JSON.parse(text)]); return "" }
        function stopContactsBatch() { mock.stopped++ }
    }

    Component {
        id: barComponent

        ContactSelection {
            sioul: mock
            theme: testTheme
            keys: ["/a.vcf", "/b.vcf"]
            shownCount: 132
            books: [{ place: "cloud/contacts", name: "Contacts · example.org" }, { place: "cloud/pro", name: "Professional · example.org" }]
            groups: ["Clients", "Professional"]
        }
    }

    function all(item, test, out) {
        out = out || []
        for (let i = 0; i < item.children.length; i++) {
            const child = item.children[i]
            if (test(child))
                out.push(child)
            root.all(child, test, out)
        }
        return out
    }

    function named(item, name) {
        return root.all(item, c => c.objectName === name)
    }

    TestCase {
        id: test

        name: "ContactSelection"
        when: windowShown

        function init() {
            mock.asked = []
            mock.ran = []
            mock.stopped = 0
            mock.contactsBatch = ""
        }

        function make(width) {
            const bar = test.createTemporaryObject(barComponent, root, { width: width })
            test.waitForRendering(bar)
            return bar
        }

        function test_how_many_and_all_shown() {
            for (const width of [1000, 412]) {
                const bar = make(width)
                compare(root.named(bar, "contactSelectionCount")[0].text, "contacts-selected:{\"n\":2}")
                const shown = root.named(bar, "contactSelectShown")[0]
                compare(shown.text, testTheme.plain("contacts-select-shown:{\"n\":132}"))
                let asked = 0
                bar.selectShown.connect(() => asked++)
                mouseClick(shown)
                compare(asked, 1)
                for (const item of root.all(bar, c => c.visible && c.width > 0 && c.objectName.startsWith("contactSelect"))) {
                    const x = item.mapToItem(bar, 0, 0).x
                    verify(x >= 0 && x + item.width <= width + 1, item.objectName + " at " + x + " + " + item.width + " in " + width)
                }
            }
        }

        // A move: its book chosen, its count said first, then Go.
        function test_a_move_asks_first_then_runs() {
            const bar = make(1000)
            const dialog = bar.act("move")
            wait(30)
            compare(mock.asked[0].action, "move")
            compare(mock.asked[0].book, "cloud/contacts", "the first book, until another is chosen")
            const book = root.named(dialog.contentItem, "batchBook")[0]
            book.currentIndex = 1
            book.activated(1)
            compare(mock.asked[1].book, "cloud/pro")
            compare(root.named(dialog.contentItem, "batchSentence")[0].text, "Move 2 contacts to “Professional”?")
            compare(mock.ran.length, 0, "nothing runs before Go")
            dialog.accept()
            compare(JSON.stringify(mock.ran[0]), JSON.stringify({ keys: ["/a.vcf", "/b.vcf"], action: "move", book: "cloud/pro", group: "" }))
        }

        // The title: before and after for each card, what cannot move, its group box.
        function test_the_title_shows_before_and_after() {
            const bar = make(412)
            const dialog = bar.act("title")
            wait(30)
            const previews = root.named(dialog.contentItem, "batchPreview")
            compare(previews.length, 1)
            compare(previews[0].text, "Mme Jeanne Martin  →  Jeanne Martin")
            compare(root.named(dialog.contentItem, "batchCannot")[0].text, "•  Lou Exemple — its title field holds Dr already")
            const also = root.named(dialog.contentItem, "batchAlsoGroup")[0]
            verify(also.visible)
            also.checked = true
            also.toggled()
            root.named(dialog.contentItem, "batchGroup")[0].text = "Professional"
            dialog.accept()
            compare(mock.ran[0].action, "title")
            compare(mock.ran[0].group, "Professional", "added to the group in the same step")
        }

        // Running: its progress and Stop; ended: what it could not do, card by card.
        function test_progress_stop_and_what_could_not() {
            const bar = make(1000)
            mock.contactsBatch = JSON.stringify({ running: true, done: 40, total: 132, line: "40 of 132…", failed: [] })
            wait(30)
            const progress = root.named(bar, "contactBatchProgress")[0]
            verify(progress.visible)
            compare([progress.value, progress.to], [40, 132])
            verify(!root.named(bar, "contactSelectionActions")[0].enabled, "one batch at a time")
            mouseClick(root.named(bar, "contactBatchStop")[0])
            compare(mock.stopped, 1)
            mock.contactsBatch = JSON.stringify({ running: false, line: "118 contacts moved. 14 could not: its address book can only be read.", failed: [{ name: "Sam Exemple", why: "its address book can only be read" }] })
            wait(30)
            verify(!progress.visible)
            const failed = root.named(bar, "contactBatchFailed")
            compare(failed.length, 1)
            compare(failed[0].text, "•  Sam Exemple — its address book can only be read")
        }
    }
}
