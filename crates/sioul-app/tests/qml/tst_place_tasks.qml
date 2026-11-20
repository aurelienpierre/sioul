// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Tasks keeps its place (TasksPage.qml, ListPlace.qml; docs/qt-quick.md, "A
// list keeps its place"), on a stand-in for Sioul that hands the whole
// answer again after each change, as Rust does. List, scrolled to its
// middle: a task ticked done from its row leaves the row at the top where it
// was; the top one ticked, the next takes its place; a task captured in the
// line above, out of sight, comes into view by the smallest move. The board
// on a computer: a card ticked moves to Done, and its column keeps its
// place. Made-up tasks. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 1100
    height: 700

    Theme {
        id: testTheme
    }

    // The stand-in's tasks: three projects of twenty, a few titles long enough to wrap.
    property var tasks: []

    function task(n, project) {
        return { uid: "t" + n, key: "/lists/work/t" + n + ".ics", title: "Task " + n + (n % 4 === 0 ? ", with a longer title that takes more room than one line in the list" : ""),
                 status: "needs-action", due: "", estimate: "", steps: "", waits: "", stopped: "", list: "Work", color: null, kind: "", folded: false, fold_line: "",
                 comes_back: "", has_steps: false, read_only: false, pinned: false, pinned_time: "", due_soon: false, done_on: "", set_aside: false, depth: 0, project: project }
    }

    QtObject {
        id: mock

        signal linksChanged
        signal porchChanged
        property string status: ""
        property string tasks: "{}"

        // The whole answer made again, as `tasks` is after every change.
        function tell() {
            const open = root.tasks.filter(t => t.status !== "completed")
            const projects = ["Studio Lumen", "Garden club", "Home"]
            const groups = projects.map(p => ({ title: p, rows: open.filter(t => t.project === p) })).filter(g => g.rows.length > 0)
            const columns = [{ id: "ready", title: "Free to start", cards: open }, { id: "doing", title: "Started", cards: [] },
                             { id: "waiting", title: "Waiting", cards: [] }, { id: "done", title: "Done", cards: root.tasks.filter(t => t.status === "completed") }]
            mock.tasks = JSON.stringify({
                view: { by: "project", done: false, project: "", filter: { kind: "", category: "" } },
                kinds: [], categories: [], weather: "clear",
                now: { weather: "clear", office_note: "", tied_note: "", weather_note: "", now: null, why: [], picked: "", then: null, others: [], started: [], wip: "", empty: "", loops: [], done_week: [], joy: [], rest: null },
                list: { groups: groups },
                board: { columns: columns, wip: "" },
                timeline: { rows: [], days: [] },
                lists: [{ id: "local/work", name: "Work", color: null, pending: false, tasks: true }],
                projects: [], no_list: false, quiet: false, first_step: "", first_step_task: "", budget: "", free: [],
                agendas: { chosen: false, groups: [] }, agendas_full: { chosen: false, groups: [] }
            })
        }

        function text(key) { return key }
        function textWith(key, name, value) { return key + ": " + value }
        function textArgs(key, args) { return key }
        function textCounted(key, n) { return key }
        function stopped() { return "null" }
        function forgetNewTasks() {}
        function showTasksAnyway(on) {}
        function showTasks(by, done, query, project) {}
        function listAccounts() { return "[]" }
        function foldTask(uid, open) {}
        function capture(line) { return JSON.stringify({ chips: [], edit: {} }) }
        function setTaskStatus(uid, status) {
            root.tasks = root.tasks.map(t => t.uid === uid ? Object.assign({}, t, { status: status }) : t)
            mock.tell()
            return ""
        }
        function addTask(title, parentUid, list) {
            const made = root.task(100 + root.tasks.length, "Home")
            made.title = title
            root.tasks = root.tasks.concat([made])
            mock.tell()
            return JSON.stringify({ uid: made.uid })
        }
    }

    QtObject {
        id: shell

        property bool compact: false
        property var moment: ({ quiet: false, sleep: false, reason: "work", line: "" })
        property var cardHost: null
        property var cardRef: null
        property string today: "2099-01-05"
        function openCard(ref) {}
    }

    Component {
        id: pageComponent

        TasksPage {
            width: root.width
            height: root.height
            sioul: mock
            theme: testTheme
            window: shell
        }
    }

    TestCase {
        name: "PlaceTasks"
        when: windowShown

        property var page: null

        function all(item, test, into) {
            const found = into || []
            if (!item)
                return found
            if (test(item) && found.indexOf(item) < 0)
                found.push(item)
            for (let i = 0; i < (item.children || []).length; ++i)
                all(item.children[i], test, found)
            return found
        }

        function frame() {
            root.Window.window.update()
            waitForRendering(root)
        }

        // The task rows shown in `view`, with their tops in its view, top first.
        function rows(view) {
            return all(view.contentItem, i => i.visible && i.task !== undefined && i.task !== null && typeof i.tick === "function")
                .map(r => ({ uid: r.task.uid, y: r.mapToItem(view, 0, 0).y, height: r.height, row: r }))
                .filter(r => r.y + r.height > 0 && r.y < view.height)
                .sort((a, b) => a.y - b.y)
        }

        function toMiddle(view) {
            view.contentY = view.originY + (view.contentHeight - view.height) / 2
            frame()
            view.contentY += rows(view)[0].y + 7
            frame()
            return rows(view)[0]
        }

        // A row's own circle, Done.
        function tick(row) {
            const circle = all(row.row, i => i.Accessible.name === "task-done" && i.visible)[0]
            verify(circle !== undefined, "its circle")
            mouseClick(circle)
            wait(0)
            frame()
        }

        function init() {
            root.tasks = Array.from({ length: 60 }, (_, i) => root.task(i, ["Studio Lumen", "Garden club", "Home"][Math.floor(i / 20)]))
            mock.tell()
            page = createTemporaryObject(pageComponent, root)
            frame()
        }

        function showList() {
            mouseClick(all(page, i => i.objectName === "taskModes:list" && i.visible)[0])
            tryVerify(() => all(page, i => i.task !== undefined && i.task !== null && i.task.uid === "t0" && i.visible).length > 0, 2000, "List shows its tasks")
            frame()
            return all(page, i => i.spacing === 14 && i.contentY !== undefined && i.positionViewAtIndex !== undefined && i.visible)[0]
        }

        function test_a_task_ticked_in_the_list_keeps_the_place() {
            const list = showList()
            const top = toMiddle(list)
            verify(top.uid !== "t0")
            tick(rows(list)[3])
            compare(rows(list)[0].uid, top.uid, "the same task at the top")
            compare(rows(list)[0].y, top.y, "at the same pixel")
            // The top one ticked: the next takes its place.
            const next = rows(list)[1]
            tick(rows(list)[0])
            compare(rows(list)[0].uid, next.uid, "the next one at the top")
            compare(rows(list)[0].y, top.y, "in its place")
        }

        // Captured in the line above the list, filed at the end of Home: brought up until it shows.
        function test_a_task_captured_comes_into_view() {
            const list = showList()
            toMiddle(list)
            const line = all(page, i => i.placeholderText === "task-capture-hint" && i.visible)[0]
            line.text = "Call the plumber"
            line.accepted()
            wait(0)
            frame()
            const made = rows(list).find(r => r.row.task.title === "Call the plumber")
            verify(made !== undefined, "in view")
            verify(Math.abs(made.y + made.height - list.height) < 1, "its bottom at the list's bottom: " + (made.y + made.height) + " / " + list.height)
        }

        // The board on a computer: a card ticked goes to Done; its column keeps its place.
        function test_a_card_ticked_on_the_board_keeps_its_column_s_place() {
            mouseClick(all(page, i => i.objectName === "taskModes:board" && i.visible)[0])
            tryVerify(() => all(page, i => i.task !== undefined && i.task !== null && i.task.uid === "t0" && i.visible).length > 0, 2000, "the board")
            frame()
            const column = all(page, i => i.spacing === 4 && i.contentY !== undefined && i.positionViewAtIndex !== undefined && i.visible && i.count === 60)[0]
            verify(column !== undefined, "Free to start, its sixty cards")
            const top = toMiddle(column)
            tick(rows(column)[2])
            verify(column.count === 59 && column.visible, "the same column, one card fewer")
            compare(rows(column)[0].uid, top.uid, "the same card at its top")
            compare(rows(column)[0].y, top.y, "at the same pixel")
        }
    }
}
