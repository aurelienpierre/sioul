// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The Notes page (NotesPage.qml) with a note open, in a window 1280 points
// wide and 860 high (the guide's pictures: the page beside the places and
// their names, 216 points), and in a whole 1280: a long title, a line of
// tags, three ties, the card's three buttons (Add, Link to…, Ties) and
// three lines not ticked with their task buttons. Nothing of the column
// beside the note goes past its right edge, the title's row keeps its
// buttons in the panel, and the long title is cut, not the column. On a
// stand-in for Sioul, its English words. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 1280
    height: 860

    Theme {
        id: testTheme
    }

    // The main window's part the page calls.
    QtObject {
        id: fakeWindow

        property bool compact: false
        property var thingShown: null

        function openThing(item) {}
        function openChain(uri) {}
        function linkFrom(source) {}
        function openTask(uid) {}
        function openDraft(id) {}
    }

    QtObject {
        id: mock

        signal sharedIn(var stores)
        signal linksChanged
        signal tasksChanged

        // Made up: no real place nor person.
        readonly property string longTitle: "Fernhill Library: the new website, its pages, its opening hours and the children's corner"
        property string notes: JSON.stringify({
            missing: false,
            tree: false,
            folders: ["Projects"],
            recent: [],
            notes: [{ path: "Projects/Fernhill Library.md", title: mock.longTitle, folder: "Projects", kind: "text" }]
        })
        property string status: ""

        // The English words of what the page shows, the rest by their keys.
        readonly property var words: ({
            "ui-add-new": "Add",
            "ui-link-existing": "Link to…",
            "ui-ties": "Ties",
            "related-title": "Tied to it",
            "note-open-lines": "Lines not ticked",
            "note-make-task": "Make it a task",
            "note-send-guests": "Send to the guests",
            "ui-edit": "Edit",
            "note-read": "Read",
            "chain-see-all": "See them all",
            "note-search": "Search the notes",
            "note-new": "New note",
            "note-as-list": "List",
            "note-as-tree": "Folders",
            "note-all": "Every note"
        })
        function text(key) {
            return mock.words[key] || key
        }
        function textWith(key, name, value) {
            return mock.text(key) + ": " + value
        }
        function viewFlag(name) {
            return false
        }
        function setViewFlag(name, on) {}
        function searchNotes(query) {}
        function setNotesTree(tree) {}
        function uriOf(kind, key) {
            return "sioul:" + kind + "/" + encodeURIComponent(key)
        }
        function note(path) {
            if (path !== "Projects/Fernhill Library.md")
                return ""
            return JSON.stringify({
                path: path,
                title: mock.longTitle,
                kind: "text",
                text: "# Fernhill Library\n\n- [ ] Opening hours right under the photo\n",
                html: "<p>For Iris Calloway and Oskar. Three stages, invoiced at the end of each: mock-up, pages, launch.</p>",
                tags: ["client", "website", "library", "children"],
                checkboxes: [
                    { text: "Opening hours right under the photo", done: false, line: 14 },
                    { text: "A menu entry for the children's corner of the library, below the opening hours", done: false, line: 15 },
                    { text: "Check the contrast of the footer", done: false, line: 16 },
                    { text: "Gather the library's photos", done: true, line: 13 }
                ],
                related: [],
                base: "",
                stamp: "1"
            })
        }
        function related(uri) {
            return JSON.stringify([
                { uri: "sioul:project/fernhill", kind: "project", how: "Project", title: "Fernhill Library: the new website and its opening hours", detail: "", when: "", found: true },
                { uri: "sioul:contact/iris", kind: "contact", how: "Person", title: "Iris Calloway, head librarian of the Fernhill Library", detail: "iris@example.org", when: "", found: true },
                { uri: "mid:quote@example.org", kind: "mail", how: "Message", title: "Re: the signed quote for the website, with the three stages", detail: "Iris Calloway", when: "Monday 9 November", found: true }
            ])
        }
    }

    Item {
        id: room

        height: root.height
        width: root.width

        NotesPage {
            id: page

            anchors.fill: parent
            sioul: mock
            theme: testTheme
            window: fakeWindow
        }
    }

    TestCase {
        name: "NotesPage"
        when: windowShown

        function named(item, name) {
            if (!item)
                return null
            if (item.objectName === name)
                return item
            const kids = item.children || []
            for (let i = 0; i < kids.length; ++i) {
                const found = named(kids[i], name)
                if (found)
                    return found
            }
            if (item.contentItem && kids.indexOf(item.contentItem) < 0)
                return named(item.contentItem, name)
            return null
        }
        function all(item, name, found) {
            if (!item)
                return found
            if (item.objectName === name)
                found.push(item)
            const kids = item.children || []
            for (let i = 0; i < kids.length; ++i)
                all(kids[i], name, found)
            return found
        }
        function rightOf(item) {
            return item.mapToItem(page, item.width, 0).x
        }

        // The note open at this width, laid out.
        function openAt(width) {
            room.width = width
            page.open("")
            page.open("Projects/Fernhill Library.md")
            tryVerify(() => named(page, "noteSide") !== null && named(page, "thingTies") !== null)
            waitForRendering(page)
            wait(50)
        }

        function check(width) {
            openAt(width)
            const side = named(page, "noteSide")
            const edge = rightOf(side)
            verify(side.visible && side.width > 100, "the column beside the note is shown, " + side.width + " wide")
            verify(edge <= rightOf(named(page, "notePanel")) + 0.5, "the column stays in the note's panel")
            // The card's buttons, all three, each whole.
            for (const name of ["thingAdd", "thingLink", "thingTies"]) {
                const button = named(side, name)
                verify(button !== null && button.visible, name + " shown")
                verify(rightOf(button) <= edge + 0.5, name + " ends at " + rightOf(button) + ", past the column's edge at " + edge + " (page " + width + ")")
                verify(button.width >= button.implicitWidth - 0.5, name + " drawn whole")
            }
            // The ties: each row, its title cut at the column's edge, not past it.
            const ties = all(side, "relatedRow", [])
            compare(ties.length, 3)
            for (const tie of ties)
                verify(rightOf(tie) <= edge + 0.5, "a tie ends at " + rightOf(tie) + ", past " + edge)
            // The lines not ticked, each with its task button in the column.
            const tasks = all(side, "noteLineTask", [])
            compare(tasks.length, 3)
            for (const task of tasks)
                verify(rightOf(task) <= edge + 0.5, "a line's task button ends at " + rightOf(task) + ", past " + edge + " (page " + width + ")")
            // The title's row: its buttons in the panel, the long title cut before them.
            const title = named(page, "noteTitle")
            verify(title.truncated, "the long title is cut")
            verify(rightOf(named(page, "noteEdit")) <= rightOf(named(page, "notePanel")) + 0.5, "Edit stays in the panel")
            verify(rightOf(title) <= named(page, "noteEdit").mapToItem(page, 0, 0).x, "the title stops before Edit")
        }

        // The page beside the places and their names, in a window 1280 wide.
        function test_beside_the_named_places_nothing_passes_the_column() {
            check(1280 - 216)
        }

        // The page beside the places' icons alone.
        function test_beside_the_places_icons_nothing_passes_the_column() {
            check(1280 - 56)
        }

        // The page on the whole 1280.
        function test_on_the_whole_width_nothing_passes_the_column() {
            check(1280)
        }
    }
}
