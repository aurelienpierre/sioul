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

        // A tab of the row by its words, shown.
        function tab(page, key) {
            return all(page, i => i.text === key && i.visible && i.clicked !== undefined)[0] || null
        }

        function test_the_cover_lies_on_now_alone() {
            const page = createTemporaryObject(pageComponent, root)
            waitForRendering(page)
            compare(page.mode, "now")
            const cover = all(page, i => i.objectName === "tasksRest")[0]
            verify(cover !== undefined && cover.visible, "asleep, the cover over Now")
            const list = tab(page, "task-mode-list")
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
            mouseClick(tab(page, "task-mode-now"))
            compare(page.mode, "now")
            verify(cover.visible)
        }
    }
}
