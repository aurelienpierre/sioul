// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The Notes page's left column (NotesPage.qml): the line of two choices
// under the search, Show (By folder, Last changed, Created) and Type (Every
// type, Text, Sound, PDF, Other), and what each lists; the search, the
// names found at once, the texts after a pause, the line that holds the
// words, "still looking" while the texts are not looked in, and the line
// saying that PDFs and sounds are found by their names; the choices kept
// when the page is made again; the list keeping its place when a note
// changes; in French, the line of choices within the column at 412 points
// and beside an open note, and nothing of the list wider than its column.
// On a stand-in for Sioul that answers as notesview.rs does: each answer a
// whole new JSON, later than the call (Rust queues it to the window), the
// texts' answer only when the test lets it come (Rust reads them on a
// thread), and dropped when another question was asked meanwhile. Run by
// tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 412
    height: 891

    Theme {
        id: testTheme
    }

    QtObject {
        id: fakeWindow

        property bool compact: true

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

        property string notes: ""
        property string status: ""
        // What notesview.rs keeps: Show and Type on this device, the words typed, the questions.
        property string show: "folder"
        property string type: "all"
        property string query: ""
        property int question: 0
        // The question whose texts are being read, -1 for none.
        property int reading: -1
        property bool french: false
        property int calls: 0

        // Made up: no real place nor person. Each note's group by when it was
        // last changed and made, as `notes::when` gives them for the stand-in's "now".
        property var vault: {
            const notes = [
                { path: "Projects/Fernhill Library.md", title: "Fernhill Library: the new website", kind: "text", tags: ["client"], changed: "today", created: "2025", text: "# Fernhill Library\n\nThe mock-up first.\n" },
                { path: "Projects/Fernhill meeting.md", title: "Fernhill: first meeting", kind: "text", changed: "week", created: "week", text: "Oskar can give a calendar address for the hours.\n" },
                { path: "Home/Spices.md", title: "Spices", kind: "text", tags: ["cumin"], changed: "month", created: "month", text: "# Spices\n" },
                { path: "Home/Lentil soup.md", title: "Lentil soup", kind: "text", tags: ["recipes"], changed: "month", created: "unknown", text: "# Lentil soup\n\n- 1 tsp cumin, 1 bay leaf\n" },
                { path: "Home/Reading list.md", title: "Reading list", kind: "text", changed: "2025", created: "2024", text: "- [ ] The Hidden Life of Trees\n" },
                { path: "Admin/Income tax 2026.md", title: "Income tax 2026", kind: "text", changed: "week", created: "after", text: "The notice came: cumin is not deductible.\n" },
                { path: "memos/Market call.ogg", title: "Market call", kind: "audio", changed: "today", created: "today" },
                { path: "memos/Choir warm-up.opus", title: "Choir warm-up", kind: "audio", changed: "year", created: "unknown" },
                { path: "Admin/scans/Lease signed.pdf", title: "Lease signed", kind: "pdf", changed: "2025", created: "2024" },
                { path: "Admin/scans/Boiler.png", title: "Boiler", kind: "image", changed: "year", created: "year" },
                { path: "Admin/Budget.ods", title: "Budget", kind: "document", changed: "2024", created: "2024" }
            ]
            for (let i = 1; i <= 30; ++i)
                notes.push({ path: "Ideas/Idea " + String(i).padStart(2, "0") + ".md", title: "Idea " + String(i).padStart(2, "0"), kind: "text", changed: "2024", created: "unknown", text: "An idea.\n" })
            return notes
        }
        readonly property var folders: ["Admin", "Admin/scans", "Empty folder", "Home", "Ideas", "Projects", "memos"]

        readonly property var english: ({
            "note-search": "Search the notes",
            "note-new": "New note",
            "note-recent": "Changed lately",
            "note-all": "Every note",
            "note-show": "Show",
            "note-show-folder": "By folder",
            "note-show-changed": "Last changed",
            "note-show-created": "Created",
            "note-type": "Type",
            "note-type-all": "Every type",
            "note-type-text": "Text",
            "note-type-sound": "Sound",
            "note-type-pdf": "PDF",
            "note-type-other": "Other",
            "note-type-none": "No note of this type.",
            "note-when-after": "After today",
            "note-when-today": "Today",
            "note-when-week": "This week",
            "note-when-month": "This month",
            "note-when-year": "Earlier this year",
            "note-when-unknown": "Not known",
            "note-search-looking": "Still looking in the notes' texts…",
            "note-search-none": "No note holds these words.",
            "note-search-names-only": "PDFs and sounds are found by their name only.",
            "note-none-open": "A note opens here."
        })
        // crates/sioul-core/locales/fr/sioul.ftl, as written there.
        readonly property var frenchWords: ({
            "note-search": "Chercher dans les notes",
            "note-new": "Nouvelle note",
            "note-recent": "Changées récemment",
            "note-all": "Toutes les notes",
            "note-show": "Voir",
            "note-show-folder": "Par dossier",
            "note-show-changed": "Par modification",
            "note-show-created": "Par création",
            "note-type": "Type",
            "note-type-all": "Tous",
            "note-type-text": "Texte",
            "note-type-sound": "Son",
            "note-type-pdf": "PDF",
            "note-type-other": "Autres",
            "note-type-none": "Aucune note de ce type.",
            "note-when-after": "Après aujourd’hui",
            "note-when-today": "Aujourd’hui",
            "note-when-week": "Cette semaine",
            "note-when-month": "Ce mois-ci",
            "note-when-year": "Plus tôt cette année",
            "note-when-unknown": "Date inconnue",
            "note-search-looking": "Recherche dans le texte des notes…",
            "note-search-none": "Aucune note ne contient ces mots.",
            "note-search-names-only": "Les PDF et les sons sont trouvés par leur nom seulement.",
            "note-none-open": "Une note s’ouvre ici."
        })
        function text(key) {
            return (mock.french ? mock.frenchWords : mock.english)[key] || key
        }
        function textWith(key, name, value) {
            return mock.text(key) + ": " + value
        }
        function viewFlag(name) {
            return false
        }
        function setViewFlag(name, on) {}
        function uriOf(kind, key) {
            return "sioul:" + kind + "/" + encodeURIComponent(key)
        }
        function note(path) {
            const n = mock.vault.find(v => v.path === path)
            if (!n)
                return ""
            return JSON.stringify({ path: n.path, title: n.title, kind: n.kind, text: n.text || "", blocks: [{ html: "<p>" + (n.text || "") + "</p>\n", kind: "p" }], tags: n.tags || [], checkboxes: [], outline: [], related: [], base: "", stamp: "1", url: "" })
        }
        // The note's file as it is now: the page reads a note again only when this changed.
        function noteStamp(path) {
            return mock.vault.some(v => v.path === path) ? "1" : ""
        }
        function related(uri) {
            return "[]"
        }

        // As `notes::Kinds::keeps`.
        function keeps(kind) {
            return mock.type === "all" || (mock.type === "text" && kind === "text") || (mock.type === "sound" && kind === "audio") || (mock.type === "pdf" && kind === "pdf") || (mock.type === "other" && (kind === "image" || kind === "document"))
        }
        function row(n) {
            return { path: n.path, title: n.title, kind: n.kind, folder: n.path.split("/").slice(0, -1).join("/"), tags: n.tags || [], modified: 0, created: null }
        }
        // As `notesview::listing`, from the notes' groups.
        function listing(texts) {
            const kept = mock.vault.filter(n => mock.keeps(n.kind))
            const out = { missing: false, show: mock.show, type: mock.type, query: mock.query, question: mock.question, looking: false, namesOnly: false, notes: [], recent: [], folders: [], groups: [] }
            if (mock.query.trim() !== "") {
                const words = mock.query.toLowerCase().split(/\s+/).filter(w => w !== "")
                const name = n => (n.title + " " + n.path + " " + (n.tags || []).join(" ")).toLowerCase()
                const byName = kept.filter(n => words.every(w => name(n).indexOf(w) >= 0))
                const byText = texts ? kept.filter(n => n.kind === "text" && byName.indexOf(n) < 0 && words.every(w => (name(n) + " " + n.text.toLowerCase()).indexOf(w) >= 0)) : []
                const line = n => n.text.split("\n").find(l => words.some(w => l.toLowerCase().indexOf(w) >= 0)) || ""
                out.notes = byName.map(n => Object.assign(mock.row(n), { by: "name" })).concat(byText.map(n => Object.assign(mock.row(n), { by: "text", line: line(n) })))
                out.namesOnly = kept.some(n => n.kind === "pdf" || n.kind === "audio")
                return out
            }
            if (mock.show === "folder") {
                out.recent = kept.slice(0, 3).map(mock.row)
                out.notes = kept.slice().sort((a, b) => a.title.toLowerCase().localeCompare(b.title.toLowerCase())).map(mock.row)
                out.folders = mock.type === "all" ? mock.folders : mock.folders.filter(f => kept.some(n => n.path.startsWith(f + "/")))
                return out
            }
            const order = ["after", "today", "week", "month", "year", "2025", "2024", "unknown"]
            const by = mock.show === "changed" ? "changed" : "created"
            for (const when of order) {
                const notes = kept.filter(n => n[by] === when)
                if (notes.length > 0)
                    out.groups.push({ when: when, notes: notes.map(mock.row) })
            }
            return out
        }
        // An answer comes after the call, as Rust's through the window's queue.
        function publish(texts, looking) {
            const answer = mock.listing(texts)
            answer.looking = looking
            const json = JSON.stringify(answer)
            Qt.callLater(() => mock.notes = json)
        }
        function readsTexts() {
            return mock.query.trim() !== "" && (mock.type === "all" || mock.type === "text")
        }
        function searchNotes(query) {
            mock.calls += 1
            mock.query = query
            mock.question += 1
            mock.publish(false, mock.readsTexts())
        }
        function searchNoteTexts() {
            if (mock.readsTexts())
                mock.reading = mock.question
        }
        // The texts' answer comes: only for the question asked last.
        function textsCome() {
            if (mock.reading === mock.question)
                mock.publish(true, false)
            mock.reading = -1
        }
        function setNotesView(show, kind) {
            mock.show = show
            mock.type = kind
            mock.question += 1
            mock.publish(false, mock.readsTexts())
            mock.searchNoteTexts()
        }
        // A note changed elsewhere: the list made again for the question asked now.
        function changed(edit) {
            edit()
            mock.vault = mock.vault.slice()
            mock.publish(mock.query !== "", false)
        }
        Component.onCompleted: mock.notes = JSON.stringify(mock.listing(false))
    }

    Component {
        id: making

        NotesPage {
            sioul: mock
            theme: testTheme
            window: fakeWindow
        }
    }

    // The page, made again when the test asks (a page is made once per window, kept after).
    Loader {
        id: holder

        width: root.width
        height: root.height
        sourceComponent: making
    }

    TestCase {
        name: "NotesFind"
        when: windowShown

        readonly property var page: holder.item

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
        function rightOf(item) {
            return item.mapToItem(root, item.width, 0).x
        }
        // The list's rows as the view has them: "heading:Today", "note:Home/Spices.md", "folder:Admin".
        function rows() {
            return named(page, "noteList").model.map(r => r.type === "note" ? "note:" + r.note.path : r.type + ":" + (r.type === "heading" ? r.title : r.path || r.title))
        }
        function choose(name, index) {
            const box = named(page, name)
            box.currentIndex = index
            box.activated(index)
        }
        function type(words) {
            const field = named(page, "noteSearch")
            field.text = words
            field.textEdited()
        }

        // The page made again, once what it was doing is over.
        function makeAgain() {
            wait(50)
            holder.active = false
            holder.active = true
            tryVerify(() => page !== null && named(page, "noteList") !== null && named(page, "noteList").count > 0)
        }

        function init() {
            mock.french = false
            mock.show = "folder"
            mock.type = "all"
            mock.query = ""
            mock.reading = -1
            mock.notes = JSON.stringify(mock.listing(false))
            root.width = 412
            fakeWindow.compact = true
            makeAgain()
        }

        // The page as it starts: by folder, every type; what changed lately, then the folders, folded.
        function test_the_page_starts_by_folder_of_every_type() {
            compare(named(page, "noteShow").displayText, "By folder")
            compare(named(page, "noteType").displayText, "Every type")
            compare(rows().slice(0, 3), ["recent:Changed lately", "heading:Every note", "folder:Admin"])
            verify(rows().indexOf("folder:Empty folder") >= 0, "an empty folder, there to be filled")
            verify(!rows().some(r => r.startsWith("folder:Admin/")), "folded")
        }

        // Last changed: one list, newest first, under quiet headings; Created: what is not known last.
        function test_the_dated_lists_under_their_headings() {
            choose("noteShow", 1)
            tryVerify(() => rows()[0] === "heading:Today")
            const headings = rows().filter(r => r.startsWith("heading:"))
            compare(headings, ["heading:Today", "heading:This week", "heading:This month", "heading:Earlier this year", "heading:2025", "heading:2024"])
            compare(rows().slice(0, 3), ["heading:Today", "note:Projects/Fernhill Library.md", "note:memos/Market call.ogg"])
            verify(!rows().some(r => r.startsWith("folder:") || r.startsWith("recent:")), "no folders")
            compare(mock.show, "changed", "kept on this device by Rust")
            choose("noteShow", 2)
            tryVerify(() => rows()[0] === "heading:After today")
            compare(rows().filter(r => r.startsWith("heading:")).pop(), "heading:Not known")
            compare(rows().pop(), "note:Ideas/Idea 30.md")
        }

        // Type narrows the tree (a folder left with nothing of it is not shown), the dated lists and the search.
        function test_a_type_narrows_every_list_and_the_search() {
            choose("noteType", 2)
            tryVerify(() => page.typed === "sound")
            compare(named(page, "noteType").displayText, "Sound")
            // The view takes the rows once the answer's bindings settled (ListPlace.qml).
            tryVerify(() => rows().filter(r => r.startsWith("folder:")).join() === "folder:memos", 1000, rows().join())
            choose("noteShow", 1)
            tryVerify(() => rows()[0] === "heading:Today")
            compare(rows().filter(r => r.startsWith("note:")), ["note:memos/Market call.ogg", "note:memos/Choir warm-up.opus"])
            type("call")
            tryVerify(() => page.searching && rows().join() === "note:memos/Market call.ogg", 1000, rows().join())
            verify(named(page, "noteNamesOnly").visible, "a sound is found by its name only: said")
            verify(!named(page, "noteLooking").visible, "no text to look in")
            choose("noteType", 1)
            tryVerify(() => mock.type === "text" && page.typed === "text")
            verify(!named(page, "noteNamesOnly").visible, "text alone: nothing to say")
            type("")
            choose("noteType", 3)
            tryVerify(() => page.typed === "pdf" && !page.searching && rows().filter(r => r.startsWith("note:")).join() === "note:Admin/scans/Lease signed.pdf", 1000, rows().join())
            choose("noteShow", 2)
            tryVerify(() => page.showing === "created" && rows()[0] === "heading:2024")
        }

        // The names answer at once, still looking in the texts; after a pause the
        // texts' answer comes, each found by its text with its line; the list kept its place.
        function test_a_search_answers_names_then_texts() {
            type("cumin")
            tryVerify(() => page.searching && rows().length === 1)
            compare(rows(), ["note:Home/Spices.md"])
            verify(named(page, "noteLooking").visible, "still looking")
            verify(!named(page, "noteNoneFound").visible)
            compare(mock.reading, -1, "the texts wait for a pause in typing")
            tryCompare(mock, "reading", mock.question, 1000)
            mock.textsCome()
            tryVerify(() => rows().length === 3)
            compare(rows(), ["note:Home/Spices.md", "note:Home/Lentil soup.md", "note:Admin/Income tax 2026.md"])
            verify(!named(page, "noteLooking").visible)
            const list = named(page, "noteList")
            tryVerify(() => list.itemAtIndex(1) !== null && named(list.itemAtIndex(1), "noteFoundLine").visible)
            compare(named(list.itemAtIndex(1), "noteFoundLine").text, "- 1 tsp cumin, 1 bay leaf", "as written, plain text")
            verify(!named(list.itemAtIndex(0), "noteFoundLine").visible, "found by its name: no line")
            // Typed on before the texts came: their answer is not this question's.
            type("bay")
            tryVerify(() => page.shown.query === "bay")
            tryCompare(mock, "reading", mock.question, 1000)
            type("bay leaf")
            mock.reading = mock.question - 1
            mock.textsCome()
            wait(50)
            compare(page.shown.query, "bay leaf")
            verify(named(page, "noteLooking").visible, "still looking for the last question")
            type("saffron")
            tryVerify(() => page.shown.query === "saffron")
            tryCompare(mock, "reading", mock.question, 1000)
            mock.textsCome()
            tryVerify(() => named(page, "noteNoneFound").visible, 1000, "nothing found: said")
        }

        // A box keeps one width whatever is chosen in it: the line does not move.
        function test_a_box_keeps_its_width() {
            const box = named(page, "noteShow")
            const before = box.implicitWidth
            choose("noteShow", 2)
            tryVerify(() => page.showing === "created")
            compare(box.implicitWidth, before)
            compare(box.contentItem.text, "Created")
        }

        // Show and Type stay as they were left when the page is made again (a restart).
        function test_the_choices_are_kept_when_the_page_is_made_again() {
            choose("noteShow", 2)
            choose("noteType", 1)
            tryVerify(() => page.showing === "created" && page.typed === "text")
            makeAgain()
            compare(named(page, "noteShow").displayText, "Created")
            compare(named(page, "noteType").displayText, "Text")
            compare(rows()[0], "heading:After today")
        }

        // A note changed, or one more, while the list is scrolled: the row at its top stays where it was.
        function test_the_list_keeps_its_place_when_a_note_changes() {
            choose("noteShow", 1)
            tryVerify(() => rows()[0] === "heading:Today")
            const list = named(page, "noteList")
            list.contentY = 400
            const top = list.indexAt(10, list.contentY + 1)
            const key = rows()[top]
            const offset = list.itemAtIndex(top).y - list.contentY
            mock.changed(() => {
                mock.vault[1].title = "Fernhill: the first meeting, its notes"
                mock.vault.unshift({ path: "Journal/New today.md", title: "New today", kind: "text", changed: "today", created: "today", text: "" })
            })
            tryVerify(() => rows().indexOf("note:Journal/New today.md") >= 0)
            tryVerify(() => {
                const now = rows().indexOf(key)
                return list.itemAtIndex(now) !== null && Math.abs(list.itemAtIndex(now).y - list.contentY - offset) < 1
            }, 2000, "the row at the top stays where it was")
        }

        // In French, the line of choices fits the phone's 412 points, its names
        // shown; beside an open note in a window 1280 wide, it stays in its column.
        function test_in_french_the_choices_stay_in_the_column() {
            mock.french = true
            makeAgain()
            const line = named(page, "noteChoices")
            const column = line.parent
            // 884: beside the places' names in a window 1100 wide, a note open, the column at its narrowest.
            for (const width of [412, 1280, 884]) {
                root.width = width
                fakeWindow.compact = width < 700
                if (width > 700)
                    page.open("Projects/Fernhill Library.md")
                waitForRendering(page)
                for (const box of [named(page, "noteShow"), named(page, "noteType")]) {
                    verify(box.visible, box.objectName + " shown")
                    verify(rightOf(box) <= rightOf(column) + 0.5, box.objectName + " ends at " + rightOf(box) + ", past its column at " + rightOf(column) + " (" + width + ")")
                    const words = box.contentItem
                    verify(words.width - words.leftPadding - words.rightPadding >= words.contentWidth - 0.5 || words.truncated, box.objectName + "'s words whole, or ended with an ellipsis (" + width + ")")
                }
                verify(named(page, "noteShowName").visible && named(page, "noteTypeName").visible, "Voir and Type said, at every width (" + width + ")")
                for (const name of [named(page, "noteShowName"), named(page, "noteTypeName")])
                    verify(rightOf(name) <= rightOf(column) + 0.5, name.objectName + " within its column (" + width + ")")
                if (width === 412) {
                    for (const box of [named(page, "noteShow"), named(page, "noteType")])
                        verify(box.width >= box.implicitWidth - 0.5, box.objectName + " whole: " + box.width + " for " + box.implicitWidth)
                }
            }
            // The dated list's headings and rows, in French, within the column.
            root.width = 412
            fakeWindow.compact = true
            page.open("")
            choose("noteShow", 1)
            tryVerify(() => rows()[0] === "heading:Aujourd’hui")
            const list = named(page, "noteList")
            waitForRendering(page)
            for (let i = 0; i < 8; ++i)
                verify(rightOf(list.itemAtIndex(i)) <= rightOf(list) + 0.5, rows()[i] + " within the list")
        }
    }
}
