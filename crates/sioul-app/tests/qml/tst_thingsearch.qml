// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The Search tab of Tasks and the Agenda (ThingSearch.qml, 8 November 2026)
// on a stand-in for Sioul: the query it asks of the core's engine as the
// filters change (project, kind and category each with "none", dates, an
// event's guests and status, a task's done and date asked), what is searched
// given by the page alone, its rows grouped as what comes, the past and what
// has no date, a row opening its card, its start on the page's own choices,
// the quiet line of what the agenda filter leaves out, and its filters folded
// within a phone's width. Fiction only. Run by tools/qml-test.sh.

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
        // What the agenda filter leaves out, as the window's search answers it.
        property int leftOut: 0
        property var off: []
        function text(key) { return key }
        function textCounted(key, n) { return key + ":" + n }
        function changeThings(things, asked) {
            mock.changed = { things: JSON.parse(things), asked: JSON.parse(asked) }
            return JSON.stringify({ said: "batch-changed", left: [{ title: "Team meeting", why: "a meeting", line: "“Team meeting”: a meeting" }] })
        }
        function textWith(key, name, value) { return key + ":" + value }
        // As the window's search answers: what its page searches alone, tasks or events.
        readonly property var things: [
            { thing: "event", key: "/c/dentist.ics", uid: "dentist", title: "Dentist", place: "Clinique du Parc", at: 100, when: "coming", date: "Tue 10 Nov, 14:00", projects: ["Home"], status: "tentative", done: false },
            { thing: "event", key: "/c/choir.ics", uid: "choir", title: "Choir", place: "", at: 150, when: "coming", date: "Wed 11 Nov, 19:00", projects: [], status: "confirmed", done: false },
            { thing: "event", key: "/c/hairdresser.ics", uid: "hairdresser", title: "Hairdresser", place: "", at: 40, when: "past", date: "Thu 1 Oct, 11:00", projects: [], status: "cancelled", done: false },
            { thing: "event", key: "/c/fair.ics", uid: "fair", title: "Book fair", place: "", at: 30, when: "past", date: "Sat 26 Sep", projects: [], status: "confirmed", done: false },
            { thing: "task", key: "/t/report.ics", uid: "report", title: "Send the report", place: "", at: 150, when: "coming", date: "Wed 11 Nov", projects: ["Home"], status: "needs-action", done: false },
            { thing: "task", key: "/t/caf.ics", uid: "caf", title: "Fill the form", place: "", at: 200, when: "coming", date: "Thu 12 Nov", projects: [], status: "needs-action", done: false },
            { thing: "task", key: "/t/plants.ics", uid: "plants", title: "Water the plants", place: "", at: 50, when: "past", date: "Mon 2 Nov", projects: [], status: "completed", done: true },
            { thing: "task", key: "/t/book.ics", uid: "book", title: "Read the book", place: "", at: null, when: "undated", date: "", projects: [], status: "needs-action", done: false }
        ]
        function findThings(query) {
            const asked = JSON.parse(query)
            mock.asked = mock.asked.concat([asked])
            return JSON.stringify({
                projects: [{ id: "home", label: "Home" }],
                kinds: [{ id: "call", label: "Call" }],
                categories: ["Papers"],
                left_out: JSON.parse(query).every_agenda ? 0 : mock.leftOut,
                agendas: { chosen: mock.off.length > 0, groups: [{ key: "yours:", title: "Yours", agendas: [{ id: "dav/home", name: "Home", on: mock.off.indexOf("dav/home") < 0 }, { id: "dav/work", name: "Work", on: mock.off.indexOf("dav/work") < 0 }] }] },
                found: mock.things.filter(t => t.thing === (asked.which === "events" ? "event" : "task"))
            })
        }
    }

    // Nothing found in the agendas ticked; one match in an agenda not ticked.
    QtObject {
        id: emptyMock

        property string tasks: ""
        function text(key) { return key }
        function textCounted(key, n) { return key + ":" + n }
        function textWith(key, name, value) { return key + ":" + value }
        function findThings(query) {
            return JSON.stringify({ projects: [], kinds: [], categories: [], left_out: JSON.parse(query).every_agenda ? 0 : 1, agendas: { chosen: true, groups: [] }, found: [] })
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
            mock.leftOut = 0
            mock.off = []
            root.width = 412
        }

        function make(which, more) {
            const search = test.createTemporaryObject(searchComponent, root, Object.assign({ which: which, width: root.width }, more || {}))
            test.waitForRendering(search)
            return search
        }

        // Each filter asks the engine at once, with "none" as its own choice; the
        // page decides what is searched: no choice of tasks, events or both, and
        // what only events have is never asked of tasks, nor the other way.
        function test_the_filters_make_the_query() {
            const search = make("tasks")
            mouseClick(root.named(search, "thingsFilters"))
            compare(root.named(search, "thingsWhich"), undefined, "no choice of tasks, events or both")
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
            verify(!root.named(search, "thingsGuests").visible && !root.named(search, "thingsStatus-tentative"), "guests and status: events' alone")
            // The Agenda's: events alone, their guests and status, never a task's done or date asked.
            const events = make("events")
            mouseClick(root.named(events, "thingsFilters"))
            compare(root.named(events, "thingsWhich"), undefined)
            last = mock.asked[mock.asked.length - 1]
            compare([last.which, last.done, last.deadline], ["events", null, null], "events alone: done and date asked not asked")
            verify(!root.named(events, "thingsDone").visible && !root.named(events, "thingsDeadline").visible, "a task's own: not shown")
            verify(root.named(events, "thingsGuests").visible)
            root.named(events, "thingsGuests").activated(1)
            compare(mock.asked[mock.asked.length - 1].guests, true)
            test.waitForRendering(events)
            const tentative = root.named(events, "thingsStatus-tentative")
            tryVerify(() => tentative.width > 0)
            wait(50)
            mouseClick(tentative)
            compare(mock.asked[mock.asked.length - 1].status, ["tentative"])
            // Dates: anytime, or between two days.
            verify(!root.named(events, "thingsFrom").visible)
            root.named(events, "thingsDates").activated(1)
            verify(root.named(events, "thingsFrom").visible && root.named(events, "thingsTo").visible)
        }

        // The search starts on the project, type and category the page gives (Tasks'
        // other tabs), shown as set; what is changed here stays here when the tab is
        // left and shown again, unless the page's own choice changed meanwhile, which
        // the search then takes, that one alone.
        function test_it_starts_on_the_page_s_choices() {
            root.width = 1000
            const page = { project: "home", kind: "call", category: "Papers" }
            const search = make("tasks", { upstream: page })
            const first = mock.asked[mock.asked.length - 1]
            compare([first.project, first.kind, first.category], ["home", "call", "Papers"], "asked at once on the page's choices")
            compare(root.named(search, "thingsProject").currentText, "Home", "shown as set")
            compare(root.named(search, "thingsKind").currentText, "Call")
            compare(root.named(search, "thingsCategory").currentText, "Papers")
            // Changed here: the search's own.
            root.named(search, "thingsKind").activated(0)
            compare(mock.asked[mock.asked.length - 1].kind, "")
            compare(page.kind, "call", "the page's own untouched")
            // The tab left and shown again, the page as it was: the search as it was left.
            search.visible = false
            search.visible = true
            let last = mock.asked[mock.asked.length - 1]
            compare([last.project, last.kind, last.category], ["home", "", "Papers"], "what was changed here stays")
            // The page's project changed in another tab: that one taken, the type left as here.
            search.visible = false
            search.upstream = { project: "work", kind: "call", category: "Papers" }
            search.visible = true
            last = mock.asked[mock.asked.length - 1]
            compare([last.project, last.kind, last.category], ["work", "", "Papers"], "the new choice upstream, the others as here")
        }

        // What the page's agenda filter leaves out and the search would have found: one
        // quiet line under the results, with a link that shows it all for this visit of
        // the tab, then the link back. Nothing found: said "in the agendas ticked".
        function test_the_agendas_left_out_are_counted_under_the_results() {
            root.width = 1000
            mock.leftOut = 2
            mock.off = ["dav/work"]
            const search = make("events")
            const list = root.named(search, "thingsFound")
            tryVerify(() => root.all(list.contentItem, c => c.objectName === "thingRow").length === 4)
            const line = root.all(list, c => c.objectName === "thingsLeftOut")[0]
            verify(line !== undefined && line.visible && line.height > 0, "the quiet line")
            compare(root.all(line, c => c.objectName === "thingsLeftOutLine")[0].text, "things-left-out:2")
            const rows = root.all(list.contentItem, c => c.objectName === "thingRow")
            const lastRow = rows[rows.length - 1]
            verify(line.mapToItem(list, 0, 0).y >= lastRow.mapToItem(list, 0, lastRow.height).y - 0.5, "under the results")
            compare(search.agendas.groups[0].agendas.map(a => a.on), [true, false], "the page's filter, as the search read it")
            const link = root.all(line, c => c.objectName === "thingsShowAll")[0]
            compare(link.text, "things-show-all")
            mouseClick(link)
            compare(mock.asked[mock.asked.length - 1].every_agenda, true, "every agenda, asked again")
            compare(root.all(line, c => c.objectName === "thingsLeftOutLine")[0].text, "things-every-agenda")
            compare(link.text, "things-agendas-shown")
            mouseClick(link)
            compare(mock.asked[mock.asked.length - 1].every_agenda, false, "back to the agendas ticked")
            // Every agenda again, then the filter turned elsewhere: its word followed.
            mouseClick(link)
            compare(search.everyAgenda, true)
            mock.off = ["dav/home", "dav/work"]
            search.run()
            compare(search.everyAgenda, false, "the filter changed: followed again")
            compare(mock.asked[mock.asked.length - 1].every_agenda, false)
            // Left and shown again: the filter followed.
            mouseClick(link)
            search.visible = false
            search.visible = true
            compare(mock.asked[mock.asked.length - 1].every_agenda, false, "for this visit of the tab alone")
            // Nothing left out: no line, no room.
            mock.leftOut = 0
            search.run()
            verify(!line.visible && line.height === 0)
            compare(root.named(search, "thingsNone").visible, false)
        }

        // Nothing found in the agendas ticked, something elsewhere: said so, the line under it.
        function test_nothing_found_but_in_agendas_left_out() {
            root.width = 1000
            const search = make("tasks", { sioul: emptyMock })
            const none = root.named(search, "thingsNone")
            verify(none.visible)
            compare(none.text, "things-none-found-filtered")
            const list = root.named(search, "thingsFound")
            const line = root.all(list, c => c.objectName === "thingsLeftOut")[0]
            verify(line.visible)
            verify(line.mapToItem(search, 0, 0).y < none.mapToItem(search, 0, none.height).y + 40, "right under it: " + line.mapToItem(search, 0, 0).y)
        }

        // On a phone and a narrow window, the filters fold under one button on the words'
        // line, which says how many are set: closed, taking no room, until opened, unless one
        // is set (the page's choices among them); open, every filter within the width.
        function test_the_filters_fold_on_a_phone() {
            const search = make("tasks")
            const fold = root.named(search, "thingsFilters")
            const words = root.named(search, "thingsWords")
            const select = root.named(search, "thingsSelect")
            verify(fold.visible && !fold.checked, "folded")
            compare(fold.text, "task-filters:0")
            verify(Math.abs(fold.mapToItem(search, 0, 0).y - words.mapToItem(search, 0, 0).y) < 1 && Math.abs(select.mapToItem(search, 0, 0).y - words.mapToItem(search, 0, 0).y) < 1, "on the words' line, with Select")
            verify(!root.named(search, "thingsProject").visible, "the filters out of sight")
            const list = root.named(search, "thingsFound")
            verify(list.mapToItem(search, 0, 0).y < words.mapToItem(search, 0, words.height).y + 40, "the list right under the words' line")
            mouseClick(fold)
            verify(fold.checked && root.named(search, "thingsProject").visible, "opened")
            root.named(search, "thingsProject").activated(2)
            root.named(search, "thingsDone").activated(1)
            compare(fold.text, "task-filters:2", "how many are set")
            // Past the double-click time since the click before.
            wait(600)
            mouseClick(fold)
            verify(!fold.checked && !root.named(search, "thingsProject").visible, "closed by its button, two set")
            // The page's choices: open at once, with their count.
            const given = make("tasks", { upstream: { project: "home", kind: "", category: "Papers" } })
            const givenFold = root.named(given, "thingsFilters")
            verify(givenFold.checked && root.named(given, "thingsProject").visible, "one set: open")
            compare(givenFold.text, "task-filters:2")
            // Wide: no fold, every filter shown.
            root.width = 1000
            const wide = make("events")
            verify(!root.named(wide, "thingsFilters").visible && root.named(wide, "thingsProject").visible)
        }

        // Words wait for a pause in the typing, then ask once.
        function test_words_ask_after_a_pause() {
            const search = make("tasks")
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
            const events = make("events")
            let list = root.named(events, "thingsFound")
            tryVerify(() => root.all(list.contentItem, c => c.objectName === "thingRow").length === 4)
            mouseClick(root.all(list.contentItem, c => c.objectName === "thingRow")[0])
            compare(JSON.stringify(mock.opened), JSON.stringify({ kind: "event", srcKey: "/c/dentist.ics", srcStart: 100, title: "Dentist" }))
            compare(events.line(events.found[0]), "Tue 10 Nov, 14:00  ·  Home  ·  Clinique du Parc  ·  things-status-tentative")
            const tasks = make("tasks")
            list = root.named(tasks, "thingsFound")
            tryVerify(() => root.all(list.contentItem, c => c.objectName === "thingRow").length === 4)
            const sections = root.all(list.contentItem, c => c.section !== undefined && c.text !== undefined && c.visible).map(c => c.text)
            compare(sections, ["things-coming", "things-past", "things-undated"], "what comes, the past, no date")
            mouseClick(root.all(list.contentItem, c => c.objectName === "thingRow")[1])
            compare(JSON.stringify(mock.opened), JSON.stringify({ kind: "task", owner: "caf" }))
            compare(tasks.line(tasks.found[2]), "Mon 2 Nov  ·  things-row-done")
        }

        // Ctrl with a click picks a row, Shift every row from the last; the bar
        // acts on all of them at once, says what it left out, and the selection empties.
        function test_rows_picked_and_changed_together() {
            const search = make("events")
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
            compare(mock.changed.things.map(t => t.uid).sort(), ["dentist", "hairdresser"])
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

        // On a phone, every filter within the screen's width, the fold opened, in each page.
        function test_filters_within_a_phone() {
            for (const which of ["tasks", "events"]) {
                const search = make(which)
                mouseClick(root.named(search, "thingsFilters"))
                root.named(search, "thingsDates").activated(1)
                test.waitForRendering(search)
                const boxes = root.all(search, c => c.objectName && c.objectName.indexOf("things") === 0 && c.visible && c.width > 0)
                verify(boxes.length > 8, which + ": " + boxes.length)
                for (const box of boxes) {
                    const at = box.mapToItem(root, 0, 0)
                    verify(at.x >= 0 && at.x + box.width <= root.width + 0.5, which + ": " + box.objectName + " within the screen")
                }
            }
        }

    }
}
