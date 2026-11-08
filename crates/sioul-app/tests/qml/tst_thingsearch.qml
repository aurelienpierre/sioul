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

        function text(key) { return key }
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
