// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The Search tab of Tasks and the Agenda (ThingSearch.qml, 8 November 2026)
// on a stand-in for Sioul: the query it asks of the core's engine as the
// filters change (project, kind and category each with "none", dates, an
// event's guests and status, a task's done and date asked), its rows grouped
// as what comes, the past and what has no date, a row opening its card, and
// its filters within a phone's width. Fiction only. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 412
    height: 860

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        property var asked: []
        // The page's tasks, as Sioul says them: their change asks again.
        property string tasks: ""
        property var opened: null

        property var changed: null
        property string status: ""
        function text(key) { return key }
        function textCounted(key, n) { return key + ":" + n }
        function changeThings(things, asked) {
            mock.changed = { things: JSON.parse(things), asked: JSON.parse(asked) }
            return JSON.stringify({ said: "batch-changed", left: [{ title: "Team meeting", why: "a meeting", line: "“Team meeting”: a meeting" }] })
        }
        function textWith(key, name, value) { return key + ":" + value }
        function findThings(query) {
            mock.asked = mock.asked.concat([JSON.parse(query)])
            return JSON.stringify({
                projects: [{ id: "home", label: "Home" }],
                kinds: [{ id: "call", label: "Call" }],
                categories: ["Papers"],
                found: [
                    { thing: "event", key: "/c/dentist.ics", uid: "dentist", title: "Dentist", place: "Clinique du Parc", at: 100, when: "coming", date: "Tue 10 Nov, 14:00", projects: ["Home"], status: "tentative", done: false },
                    { thing: "task", key: "/t/caf.ics", uid: "caf", title: "Fill the form", place: "", at: 200, when: "coming", date: "Thu 12 Nov", projects: [], status: "needs-action", done: false },
                    { thing: "task", key: "/t/plants.ics", uid: "plants", title: "Water the plants", place: "", at: 50, when: "past", date: "Mon 2 Nov", projects: [], status: "completed", done: true },
                    { thing: "task", key: "/t/book.ics", uid: "book", title: "Read the book", place: "", at: null, when: "undated", date: "", projects: [], status: "needs-action", done: false }
                ]
            })
        }
    }

    QtObject {
        id: fakeWindow

        function openCard(ref) {
            mock.opened = ref
        }
    }

    Component {
        id: searchComponent

        ThingSearch {
            width: root.width
            height: root.height
            sioul: mock
            theme: testTheme
            window: fakeWindow
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
        return root.all(item, c => c.objectName === name)[0]
    }

    TestCase {
        id: test

        name: "ThingSearch"
        when: windowShown

        function init() {
            mock.asked = []
            mock.opened = null
            mock.changed = null
        }

        function make(which) {
            const search = test.createTemporaryObject(searchComponent, root, { which: which })
            test.waitForRendering(search)
            return search
        }

        // Each filter asks the engine at once, with "none" as its own choice; what
        // only events have is not asked of tasks alone, and the other way.
        function test_the_filters_make_the_query() {
            const search = make("tasks")
            let last = mock.asked[mock.asked.length - 1]
            compare(last.which, "tasks")
            compare(last.guests, null, "tasks alone: no guests asked")
            const project = root.named(search, "thingsProject")
            project.activated(1)
            last = mock.asked[mock.asked.length - 1]
            compare(last.project, ":none", "no project")
            root.named(search, "thingsKind").activated(2)
            compare(mock.asked[mock.asked.length - 1].kind, "call")
            root.named(search, "thingsCategory").activated(2)
            compare(mock.asked[mock.asked.length - 1].category, "Papers")
            root.named(search, "thingsDone").activated(1)
            compare(mock.asked[mock.asked.length - 1].done, false, "not done")
            root.named(search, "thingsDeadline").activated(1)
            compare(mock.asked[mock.asked.length - 1].deadline, true, "with a date asked")
            verify(!root.named(search, "thingsGuests").visible, "guests: events' alone")
            root.named(search, "thingsWhich").activated(2)
            last = mock.asked[mock.asked.length - 1]
            compare([last.which, last.done, last.deadline], ["events", null, null], "events alone: done and date asked not asked")
            verify(root.named(search, "thingsGuests").visible)
            root.named(search, "thingsGuests").activated(1)
            compare(mock.asked[mock.asked.length - 1].guests, true)
            test.waitForRendering(search)
            const tentative = root.named(search, "thingsStatus-tentative")
            tryVerify(() => tentative.width > 0)
            wait(50)
            mouseClick(tentative)
            compare(mock.asked[mock.asked.length - 1].status, ["tentative"])
            // Dates: anytime, or between two days.
            verify(!root.named(search, "thingsFrom").visible)
            root.named(search, "thingsDates").activated(1)
            verify(root.named(search, "thingsFrom").visible && root.named(search, "thingsTo").visible)
        }

        // Words wait for a pause in the typing, then ask once.
        function test_words_ask_after_a_pause() {
            const search = make("both")
            const before = mock.asked.length
            const field = root.named(search, "thingsWords")
            field.forceActiveFocus()
            keyClick(Qt.Key_C)
            keyClick(Qt.Key_A)
            keyClick(Qt.Key_F)
            compare(mock.asked.length, before, "not at each key")
            tryVerify(() => mock.asked.length === before + 1, 2000)
            compare(mock.asked[mock.asked.length - 1].words, "caf")
        }

        // The rows as found, under what comes, the past and no date; a row opens its card.
        function test_rows_grouped_and_opened() {
            const search = make("both")
            const list = root.named(search, "thingsFound")
            tryVerify(() => root.all(list.contentItem, c => c.objectName === "thingRow").length === 4)
            const rows = root.all(list.contentItem, c => c.objectName === "thingRow")
            mouseClick(rows[0])
            compare(JSON.stringify(mock.opened), JSON.stringify({ kind: "event", srcKey: "/c/dentist.ics", srcStart: 100, title: "Dentist" }))
            mouseClick(rows[1])
            compare(JSON.stringify(mock.opened), JSON.stringify({ kind: "task", owner: "caf" }))
            compare(search.line(search.found[0]), "Tue 10 Nov, 14:00  ·  Home  ·  Clinique du Parc  ·  things-status-tentative")
            compare(search.line(search.found[2]), "Mon 2 Nov  ·  things-row-done")
        }

        // Ctrl with a click picks a row, Shift every row from the last; the bar
        // acts on all of them at once, says what it left out, and the selection empties.
        function test_rows_picked_and_changed_together() {
            const search = make("both")
            const list = root.named(search, "thingsFound")
            tryVerify(() => root.all(list.contentItem, c => c.objectName === "thingRow").length === 4)
            const rows = root.all(list.contentItem, c => c.objectName === "thingRow")
            const bar = root.named(search, "thingsBatch")
            verify(!bar.visible, "nothing picked: no bar")
            mouseClick(rows[0], 10, 10, Qt.LeftButton, Qt.ControlModifier)
            compare(mock.opened, null, "picked, not opened")
            compare(search.pickedList.length, 1)
            mouseClick(rows[2], 10, 10, Qt.LeftButton, Qt.ShiftModifier)
            compare(search.pickedList.length, 3, "a range")
            verify(bar.visible)
            compare(root.named(bar, "batchCount").text, "batch-selected:3")
            mouseClick(rows[1], 10, 10, Qt.LeftButton, Qt.ControlModifier)
            compare(search.pickedList.length, 2, "picked again: let go")
            bar.change({ status: "completed" })
            compare(mock.changed.asked.status, "completed")
            compare(mock.changed.things.map(t => t.uid).sort(), ["dentist", "plants"])
            compare(mock.status, "batch-changed")
            compare(search.pickedList.length, 0, "the act done: nothing picked")
            verify(bar.visible, "what was left out still said")
            compare(root.all(bar, c => c.objectName === "batchLeft")[0].text, "“Team meeting”: a meeting")
        }

        // Select (10 November 2026): the bar at once, its acts waiting for a first row ticked;
        // a box on each row; a plain click picks and never opens; Select again, or the bar's ×,
        // lets everything go. Something picked with Ctrl shows the boxes and turns Select on.
        function test_select_shows_the_boxes_and_the_bar() {
            const search = make("tasks")
            const list = root.named(search, "thingsFound")
            tryVerify(() => root.all(list.contentItem, c => c.objectName === "thingRow").length === 4)
            const select = root.named(search, "thingsSelect")
            const bar = root.named(search, "thingsBatch")
            verify(select.visible && !select.checked && !bar.visible)
            compare(root.all(list.contentItem, c => c.objectName === "thingPick").length, 0, "no box before Select")
            mouseClick(select)
            verify(select.checked && bar.visible, "Select: the bar at once")

            compare(root.named(bar, "batchCount").text, "batch-pick")
            verify(!root.named(bar, "batchDone").enabled && !root.named(bar, "batchDelete").enabled, "acts wait for a row")
            verify(root.named(bar, "batchKind").visible && root.named(bar, "batchDone").visible, "a task's acts shown for a list of tasks")
            tryVerify(() => root.all(list.contentItem, c => c.objectName === "thingPick" && c.visible).length === 4, 1000, "a box on each row")
            let rows = root.all(list.contentItem, c => c.objectName === "thingRow")
            mouseClick(rows[1])
            compare(mock.opened, null, "picked, not opened")
            compare(search.pickedList.map(t => t.uid), ["caf"])
            const boxes = root.all(list.contentItem, c => c.objectName === "thingPick")
            mouseClick(boxes[3])
            compare(search.pickedList.map(t => t.uid).sort(), ["book", "caf"])
            verify(boxes[1].checked && boxes[3].checked && !boxes[0].checked, "ticked as picked")
            verify(root.named(bar, "batchDone").enabled)
            compare(root.named(bar, "batchCount").text, "batch-selected:2")
            mouseClick(boxes[1])
            compare(search.pickedList.map(t => t.uid), ["book"], "ticked again: let go")
            // Past the double-click time since the box's click just before.
            wait(600)
            mouseClick(select)
            compare([select.checked, bar.visible, search.pickedList.length, search.selecting], [false, false, 0, false], "Select again: all let go")
            tryVerify(() => root.all(list.contentItem, c => c.objectName === "thingPick" && c.visible).length === 0)
            rows = root.all(list.contentItem, c => c.objectName === "thingRow")
            mouseClick(rows[0], 10, 10, Qt.LeftButton, Qt.ControlModifier)
            verify(select.checked && bar.visible, "picked with Ctrl: Select on")
            mouseClick(root.named(bar, "batchClear"))
            verify(!select.checked && !bar.visible && search.pickedList.length === 0, "the bar's ×: all let go")
            mouseClick(select)
            bar.change({ status: "completed" })
            verify(!select.checked && search.pickedList.length === 0, "an act ends the choosing")
        }

        // On a phone, every filter within the screen's width.
        function test_filters_within_a_phone() {
            const search = make("both")
            const boxes = root.all(search, c => c.objectName && c.objectName.indexOf("things") === 0 && c.visible && c.width > 0)
            for (const box of boxes) {
                const at = box.mapToItem(root, 0, 0)
                verify(at.x >= 0 && at.x + box.width <= root.width + 0.5, box.objectName + " within the screen")
            }
        }
    }
}
