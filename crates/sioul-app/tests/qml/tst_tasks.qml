// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The Tasks page asleep (TasksPage.qml; docs/areas.md, "Calm views and full
// views"), on a stand-in for Sioul: the rest cover lies over Now alone, the
// tab row above it in reach and enabled; List shows its tasks, nothing over
// it; back on Now, the cover again. Fiction only. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 1000
    height: 760

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        signal linksChanged
        signal porchChanged
        property string status: ""

        readonly property var row: ({ uid: "figures", title: "Quarterly figures", status: "needs-action", due: "", estimate: "", steps: "", waits: "", stopped: "", list: "Work", color: null, kind: "", folded: false, fold_line: "", comes_back: "", has_steps: false, read_only: false, pinned: false, pinned_time: "", due_soon: false, done_on: "", set_aside: false, depth: 0 })
        readonly property string tasks: JSON.stringify({
            view: { by: "project", done: false, project: "", filter: { kind: "", category: "" } },
            kinds: [],
            categories: [],
            weather: "clear",
            now: { weather: "clear", office_note: "", tied_note: "", weather_note: "", now: null, why: [], picked: "", then: null, others: [], started: [], wip: "", empty: "", loops: [], done_week: [], joy: [], rest: null },
            list: { groups: [{ title: "Studio Lumen", rows: [mock.row] }] },
            board: { columns: [] },
            timeline: { rows: [], days: [] },
            lists: [{ id: "local/work", name: "Work", color: null, pending: false, tasks: true }],
            projects: [],
            no_list: false,
            quiet: true,
            first_step: "",
            first_step_task: "",
            budget: "",
            free: [],
            agendas: { chosen: false, groups: [] },
            agendas_full: { chosen: false, groups: [] }
        })

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
        function setTaskStatus(uid, status) { return "" }
        property var started: []
        function focusStart(uid, minutes) { mock.started = mock.started.concat([uid + " " + minutes]) }
    }

    QtObject {
        id: shell

        property var moment: ({ quiet: true, sleep: true, reason: "sleep", line: "Sleep: nothing disturbs until 07:00." })
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
        name: "TasksAsleep"
        when: windowShown

        function all(item, test, into) {
            const found = into || []
            if (!item)
                return found
            if (test(item))
                found.push(item)
            for (let i = 0; i < (item.children || []).length; ++i)
                all(item.children[i], test, found)
            return found
        }

        // A tab of the row by its name (Tabs.qml: `taskModes:<view>`), shown.
        function tab(page, view) {
            return all(page, i => i.objectName === "taskModes:" + view && i.visible)[0] || null
        }

        // One field to note a thought asleep: the line under the tabs, never the cover's too
        // (the owner's picture of the wave, computer-night-tasks-now.png).
        function test_one_field_to_note_a_thought_asleep() {
            const page = createTemporaryObject(pageComponent, root)
            waitForRendering(page)
            const fields = all(page, i => i.placeholderText === "task-note-for-later" && i.visible)
            compare(fields.length, 1, "one field")
            const cover = all(page, i => i.objectName === "tasksRest")[0]
            verify(fields[0].mapToItem(page, 0, 0).y < cover.mapToItem(page, 0, 0).y, "under the tabs, above the cover")
            compare(all(cover, i => i.placeholderText !== undefined).length, 0, "the cover holds no field")
        }

        // A timer on any task, at any hour, from the full views: the row's menu offers Start,
        // here on a leisure task, asleep, in List (time for every area: the owner's word).
        function test_a_row_s_menu_starts_a_timer_at_any_hour() {
            mock.started = []
            const page = createTemporaryObject(pageComponent, root)
            waitForRendering(page)
            mouseClick(all(page, i => i.objectName === "taskModes:list")[0])
            tryVerify(() => all(page, i => i.task !== undefined && i.task !== null && i.task.uid === "figures" && i.visible).length > 0, 2000)
            const row = all(page, i => i.task !== undefined && i.task !== null && i.task.uid === "figures" && i.visible)[0]
            row.menu(row.task)
            tryVerify(() => all(page.Overlay.overlay, o => o.subMenu !== undefined && o.subMenu !== null && o.subMenu.objectName === "taskStartMenu").length === 1, 2000, "Start in the row's menu")
            const start = all(page.Overlay.overlay, o => o.subMenu !== undefined && o.subMenu !== null && o.subMenu.objectName === "taskStartMenu")[0]
            verify(start.enabled, "offered asleep")
            const twentyFive = start.subMenu.itemAt(2)
            compare(twentyFive.objectName, "taskStart:25")
            twentyFive.triggered()
            compare(mock.started, ["figures 25"])
        }

        function test_the_cover_lies_on_now_alone() {
            const page = createTemporaryObject(pageComponent, root)
            waitForRendering(page)
            compare(page.mode, "now")
            const cover = all(page, i => i.objectName === "tasksRest")[0]
            verify(cover !== undefined && cover.visible, "asleep, the cover over Now")
            const list = tab(page, "list")
            verify(list !== null && list.enabled, "the tab row in reach")
            // The cover starts under the tab row: it covers Now's pane alone.
            const coverTop = cover.mapToItem(page, 0, 0).y
            verify(list.mapToItem(page, 0, list.height).y <= coverTop, "the row above the cover: " + list.mapToItem(page, 0, list.height).y + " ≤ " + coverTop)
            verify(cover.height < page.height, "not the whole page")
            // List: its tasks shown, nothing over them.
            mouseClick(list)
            compare(page.mode, "list")
            tryVerify(() => all(page, i => i.text === "Quarterly figures" && i.visible).length > 0, 2000, "List shows its task")
            verify(!cover.visible, "no cover over List")
            // Back on Now: the cover again.
            mouseClick(tab(page, "now"))
            compare(page.mode, "now")
            verify(cover.visible)
        }
    }
}
