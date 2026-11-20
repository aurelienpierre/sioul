// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The Notes page (NotesPage.qml) with a note open, in a window 1280 points
// wide and 860 high (the guide's pictures: the page beside the places and
// their names, 216 points), and in a whole 1280: a long title, a line of
// tags, its outline of seven headings (one too long for a line), three
// ties, the card's three buttons (Add, Link to…, Ties) and three lines not
// ticked with their task buttons. Nothing of the column beside the note
// goes past its right edge, the title's row keeps its buttons in the panel,
// and the long title is cut, not the column. A heading of the outline
// pressed brings its place to the top of the note, read or being edited;
// on a phone, 412 points wide, the outline is a fold at the top of the
// note, closed until opened. On a stand-in for Sioul, its English words.
// Run by tools/qml-test.sh.

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
            show: "folder",
            type: "all",
            query: "",
            question: 1,
            looking: false,
            namesOnly: false,
            folders: ["Projects"],
            recent: [],
            groups: [],
            notes: [{ path: "Projects/Fernhill Library.md", title: mock.longTitle, folder: "Projects", kind: "text", tags: [], modified: 1, created: null }]
        })
        property string status: ""

        // The note: its title's heading, then six parts, each a heading and a long paragraph.
        readonly property var parts: ["Where it stands", "The mock-up", "The pages", "Opening hours", "A menu entry for the children's corner of the library, below the photo of the reading room", "Launch"]
        readonly property string paragraph: "Two layouts were sketched, the calm one with the reading room's photo preferred by everyone at the first meeting. Each stage is invoiced at its end, and the site reads well on a phone, where most visitors look for when the library is open. The words stay short, the photos large, and nothing moves by itself on the page."
        readonly property string noteText: "# Fernhill Library\n\nFor Iris Calloway and Oskar.\n\n" + mock.parts.map(p => "## " + p + "\n\n" + mock.paragraph + "\n").join("\n") + "\n- [ ] Opening hours right under the photo\n"
        // The headings as Rust gives them (`notesview::outline_of`): the first line of each part, from 5 on.
        readonly property var outline: [{ level: 1, text: "Fernhill Library", line: 1, drawn: true }].concat(mock.parts.map((p, i) => ({ level: 2, text: p, line: 5 + 4 * i, drawn: true })))

        // The outline of a text being written, as `notesview::outline_json`: its "#" lines.
        function noteOutline(path, text) {
            const out = []
            text.split("\n").forEach((line, i) => {
                const found = /^(#{1,6})\s+(.+)$/.exec(line)
                if (found)
                    out.push({ level: found[1].length, text: found[2].trim(), line: i + 1, drawn: true })
            })
            return JSON.stringify(out)
        }
        function saveNote(path, text, stamp) {
            return JSON.stringify({ problem: "", stamp: "2", path: path, kept: "" })
        }

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
            "note-all": "Every note",
            "note-outline": "Outline",
            "note-show": "Show",
            "note-show-folder": "By folder",
            "note-show-changed": "Last changed",
            "note-show-created": "Created",
            "note-type": "Type",
            "note-type-all": "Every type",
            "note-type-text": "Text",
            "note-type-sound": "Sound",
            "note-type-pdf": "PDF",
            "note-type-other": "Other"
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
        function searchNoteTexts() {}
        function setNotesView(show, kind) {}
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
                text: mock.noteText,
                html: "<h1>Fernhill Library</h1><p>For Iris Calloway and Oskar.</p>" + mock.parts.map(p => "<h2>" + p + "</h2><p>" + mock.paragraph + "</p>").join("") + "<ul><li>Opening hours right under the photo</li></ul>",
                outline: mock.outline,
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

        function cleanup() {
            fakeWindow.compact = false
            page.editing = false
            page.dirty = false
        }

        // The outline's lines shown inside `item`.
        function headings(item) {
            return all(item, "outlineHeading", []).filter(h => h.visible)
        }

        // How far the note's text is from showing `words` at its top: 0 when it does,
        // or as far down as it goes.
        function offTop(words) {
            const reader = named(page, "noteReader")
            const flick = named(page, "noteReadView").contentItem
            const at = reader.getText(0, reader.length).indexOf(words)
            verify(at >= 0, words + " drawn")
            const bottom = flick.contentHeight - flick.height
            return flick.contentY >= bottom - 1 ? 0 : Math.abs(reader.positionToRectangle(at).y - flick.contentY)
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
            // The outline first, every heading in the column, the long one on two lines, cut.
            const outline = named(side, "outlineSide")
            verify(outline.visible && outline.mapToItem(side, 0, 0).y < named(side, "thingTies").mapToItem(side, 0, 0).y, "the outline above what is tied to the note")
            const lines = headings(outline)
            compare(lines.length, 7)
            for (const line of lines)
                verify(rightOf(line) <= edge + 0.5, "a heading ends at " + rightOf(line) + ", past " + edge + " (page " + width + ")")
            verify(lines[5].contentItem.lineCount === 2 && lines[5].contentItem.truncated, "a long heading wraps on two lines, then is cut")
            verify(lines[1].contentItem.x > lines[0].contentItem.x, "a part's heading is indented under the title's")
            verify(!named(page, "outlineFold").visible, "no fold beside the column")
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

        // A heading pressed brings its place to the top of the note as it is read.
        function test_a_heading_pressed_brings_its_place_into_view() {
            openAt(1280)
            const flick = named(page, "noteReadView").contentItem
            compare(flick.contentY, 0)
            const lines = headings(named(page, "outlineSide"))
            mouseClick(lines[4])
            tryVerify(() => flick.contentY > 0 && offTop("Opening hours") < 2, 2000, "the part at the top: " + offTop("Opening hours"))
            mouseClick(lines[2])
            tryVerify(() => offTop("The mock-up") < 2, 2000)
            // A link to one of its headings, written as a web page writes it, or by its words.
            page.follow("#launch")
            tryVerify(() => offTop("Launch") < 2)
            page.follow("#where-it-stands")
            tryVerify(() => offTop("Where it stands") < 2, 1000, "as a web page writes a heading")
            page.follow("sioul:note/Projects/Fernhill%20Library.md#The%20pages")
            tryVerify(() => offTop("The pages") < 2)
            mouseClick(lines[0])
            tryCompare(flick, "contentY", 0)
        }

        // Being edited, a heading pressed puts the cursor on its line, in view;
        // the outline follows the text after a pause.
        function test_editing_a_heading_pressed_puts_the_cursor_on_its_line() {
            openAt(1280)
            mouseClick(named(page, "noteEdit"))
            const editor = named(page, "noteEditor")
            // Laid out: a person presses a heading once the text shows.
            tryVerify(() => editor.visible && editor.text === mock.noteText && editor.width > 300)
            mouseClick(headings(named(page, "outlineSide"))[3])
            const line = mock.noteText.split("\n").slice(0, 12).join("\n").length + 1
            compare(editor.cursorPosition, line, "the cursor at the start of line 13, the third part's heading")
            const flick = named(page, "noteEditView").contentItem
            verify(flick.contentY > 0, "brought into view")
            editor.insert(editor.length, "\n## Added at the end\n")
            compare(headings(named(page, "outlineSide")).length, 7, "not at each key")
            tryVerify(() => headings(named(page, "outlineSide")).length === 8, 3000, "the heading added, after a pause")
        }

        // On a phone, the outline is one fold at the top of the note, closed until opened.
        function test_on_a_phone_the_outline_folds_at_the_top_of_the_note() {
            fakeWindow.compact = true
            openAt(412)
            const fold = named(page, "outlineFold")
            verify(fold.visible && !fold.open, "folded")
            verify(!named(page, "outlineSide").visible)
            compare(headings(fold).length, 0)
            verify(fold.mapToItem(page, 0, 0).y < named(page, "noteReadView").mapToItem(page, 0, 0).y, "above the note's text")
            mouseClick(named(fold, "foldLine"))
            verify(fold.open)
            tryVerify(() => headings(fold).length === 7 && headings(fold)[6].width > 100, 2000, "laid out")
            const lines = headings(fold)
            const edge = rightOf(named(page, "notePanel"))
            for (const line of lines)
                verify(rightOf(line) <= edge + 0.5, "a heading ends at " + rightOf(line) + ", past " + edge)
            // A heading chosen: the fold closes, the note's text takes its room, at that heading.
            mouseClick(lines[4])
            tryVerify(() => !fold.open && named(page, "noteReadView").contentItem.contentY > 0 && offTop("Opening hours") < 2, 2000, "at the heading: " + offTop("Opening hours"))
        }
    }
}
