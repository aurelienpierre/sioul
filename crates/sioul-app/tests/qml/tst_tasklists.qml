// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Tasks ⚙ ▸ Task lists (TaskListsSetup.qml, 24 November 2026), on a stand-in
// for Sioul that gives the agendas as Your agendas reads them: only the
// lists that hold only tasks are listed; yours renamed (`renameAgenda`) and
// removed on the second press, after the sentence that says what goes,
// though it still holds tasks (`agendaRemoval`, then `removeAgenda`, whose
// dated copy the Rust test `a_list_of_tasks_alone_is_removed_as_an_agenda_is`
// checks); someone else's stopped showing the same way; Keep puts the
// question away; at a phone's 412 points nothing passes the edge. Made-up
// lists. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 412
    height: 900

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        signal agendasChanged

        property var calls: []

        function text(key) { return key }
        function agendasSetup() { return JSON.stringify(root.view) }
        function renameAgenda(id, name) { mock.calls = mock.calls.concat([["rename", id, name]]); return "" }
        function agendaRemoval(id) {
            mock.calls = mock.calls.concat([["removal", id]])
            return JSON.stringify(id === "cloud/chores"
                ? { act: "remove", sentence: "Remove “Chores”, 12 tasks, from cloud.example.org?", note: "Nextcloud keeps it in its trash bin." }
                : { act: "leave", sentence: "Stop showing “Sam’s chores”?", note: "" })
        }
        function removeAgenda(id) { mock.calls = mock.calls.concat([["remove", id]]); return "" }
    }

    readonly property var view: ({
        agendas: [
            { id: "cloud/home", name: "Home", place: "cloud.example.org", colour: "#4c6b5c", events: true, tasks: true, server: { server: "caldav", server_name: "cloud.example.org", own: true } },
            { id: "cloud/chores", name: "Chores", place: "cloud.example.org", colour: "#7a5c99", events: false, tasks: true, server: { server: "caldav", server_name: "cloud.example.org", own: true } },
            { id: "cloud/sams-chores", name: "Sam’s chores", place: "cloud.example.org", colour: "#4c6b5c", events: false, tasks: true, server: { server: "caldav", server_name: "cloud.example.org", own: false } },
            { id: "cloud/busy", name: "Busy times", place: "cloud.example.org", events: true, tasks: false, mirror: true, server: { server: "caldav", server_name: "cloud.example.org", own: true } }
        ]
    })

    Component {
        id: listsComponent

        TaskListsSetup {
            width: root.width
            sioul: mock
            theme: testTheme
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

        name: "TaskLists"
        when: windowShown

        function init() {
            mock.calls = []
        }

        function make() {
            const lists = test.createTemporaryObject(listsComponent, root)
            test.waitForRendering(lists)
            return lists
        }

        function test_only_the_lists_of_tasks_alone() {
            const lists = make()
            const lines = root.named(lists, "taskListLine")
            compare(lines.length, 2, "a calendar that takes events too, and a calendar of busy times, are Your agendas'")
            compare(root.named(lines[0], "taskListRename")[0].text, "Chores")
            verify(root.named(lines[0], "taskListRename")[0].visible, "yours: its name to change")
            verify(!root.named(lines[1], "taskListRename")[0].visible && root.named(lines[1], "taskListName")[0].visible, "someone else's: its name said")
        }

        function test_yours_renamed() {
            const lists = make()
            const field = root.named(root.named(lists, "taskListLine")[0], "taskListRename")[0]
            field.text = "House chores"
            field.editingFinished()
            compare(JSON.stringify(mock.calls), JSON.stringify([["rename", "cloud/chores", "House chores"]]))
        }

        // Removed though it holds tasks: the sentence first, the second press removes it (its dated copy Rust's).
        function test_yours_removed_though_it_holds_tasks() {
            const lists = make()
            const chores = root.named(lists, "taskListLine")[0]
            const remove = root.named(chores, "taskListRemove")[0]
            compare(remove.text, "agenda-remove")
            mouseClick(remove)
            compare(JSON.stringify(mock.calls), JSON.stringify([["removal", "cloud/chores"]]), "asked first, nothing removed")
            compare(root.named(chores, "taskListRemoveQuestion")[0].text, "Remove “Chores”, 12 tasks, from cloud.example.org?")
            verify(!remove.visible, "the question in its place")
            const confirm = root.named(chores, "taskListRemoveConfirm")[0]
            compare(confirm.text, "agenda-remove-confirm")
            test.waitForRendering(lists)
            tryVerify(() => confirm.visible && confirm.width > 0)
            mouseClick(confirm)
            compare(JSON.stringify(mock.calls[1]), JSON.stringify(["remove", "cloud/chores"]))
            const wide = root.all(lists, c => c.visible && c.width > 0 && c.mapToItem(lists, 0, 0).x + c.width > lists.width + 1)
            compare(wide.length, 0, "nothing wider than a phone's 412 points")
        }

        // Someone else's: Stop showing, asked the same way; Keep puts the question away.
        function test_someone_elses_stopped_showing() {
            const lists = make()
            const sams = root.named(lists, "taskListLine")[1]
            const leave = root.named(sams, "taskListRemove")[0]
            compare(leave.text, "agenda-stop-showing")
            mouseClick(leave)
            compare(root.named(sams, "taskListRemoveConfirm")[0].text, "agenda-leave-confirm")
            test.waitForRendering(lists)
            mouseClick(root.all(sams, c => c.visible && c.text === "agenda-keep")[0])
            verify(leave.visible, "kept")
            compare(mock.calls.filter(c => c[0] === "remove").length, 0)
        }
    }
}
