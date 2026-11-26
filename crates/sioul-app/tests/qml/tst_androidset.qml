// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The window's QML as the phone's build has it (qml-test: android): each file
// of qml/android/ in place of its namesake, the files for computers only left
// out (crates/sioul-app/build.rs, `desktop_only`). From 3 to 26 November 2026
// the Notes page gave the PDF view three properties that the phone's own
// PdfView.qml lacked: QML refused the whole page on the phone alone, which
// showed nothing under the page's name, no list, no ⚙, no folder asked for;
// on a computer every test passed.
//
// Here: every file of that set is made (but main.qml and TitleBar.qml, which
// make the window's C++ objects, not stood in for here); then the Notes page,
// made as the window makes it (a Loader's setSource), at a phone's 412 by
// 860 points, in each state it can be in, never shows nothing: the list with
// its head and ⚙ (notes, none read yet, a folder holding none), the folder
// asked for in its place with the ⚙ in the window's bar (none set, one that
// does not read), a note open with a way back (text, a PDF on the phone's
// PdfView, a note gone meanwhile). On a computer, the folder asked for keeps
// the ⚙ beside its title. On a stand-in for Sioul, its words as their keys.
// Run by tools/qml-test.sh, which makes the phone's set for it.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import Qt.labs.folderlistmodel
import "../../qml"

Item {
    id: root

    width: 412
    height: 860

    Theme {
        id: testTheme
    }

    // The main window's part the page calls; a phone's unless a test says otherwise.
    QtObject {
        id: fakeWindow

        property bool compact: true
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

        // The folder as `Config::notes_folder` gives it; the lists as `notesview::listing` does.
        property string notesFolder: JSON.stringify({ state: "ready", path: "/storage/emulated/0/Documents" })
        property string notes: ""
        property string status: ""

        // Made up: no real place nor person.
        function listed(notes) {
            return JSON.stringify({ missing: false, show: "folder", type: "all", query: "", question: 1, looking: false, namesOnly: false, folders: notes.length > 0 ? ["admin"] : [], recent: notes, groups: [], notes: notes })
        }
        readonly property var plan: ({ path: "admin/plan.md", title: "Plan for the week", folder: "admin", kind: "text", tags: [], modified: 2, created: null })
        readonly property var lease: ({ path: "admin/lease.pdf", title: "lease", folder: "admin", kind: "pdf", tags: [], modified: 1, created: null })

        function note(path) {
            if (path === mock.plan.path)
                return JSON.stringify({ path: path, title: mock.plan.title, kind: "text", text: "# Plan for the week\n\nThe dentist on Tuesday.\n", blocks: [{ html: "<p>The dentist on Tuesday.</p>\n", kind: "p", picture: false, key: "b0" }], outline: [], tags: [], checkboxes: [], related: [], base: "", stamp: "1" })
            if (path === mock.lease.path)
                return JSON.stringify({ path: path, title: "lease", kind: "pdf", url: "file:///storage/emulated/0/Documents/admin/lease.pdf", text: "", blocks: [], outline: [], tags: [], checkboxes: [], related: [], base: "", stamp: "1" })
            // Gone, or not readable: as Rust says it.
            return ""
        }
        function noteStamp(path) {
            return mock.note(path) === "" ? "" : "1"
        }
        function related(uri) {
            return "[]"
        }
        function uriOf(kind, key) {
            return "sioul:" + kind + "/" + encodeURIComponent(key)
        }
        function text(key) {
            return key
        }
        function textWith(key, name, value) {
            return key + " " + value
        }
        function textCounted(key, count) {
            return key + " " + count
        }
        function viewFlag(name) {
            return false
        }
        function setViewFlag(name, on) {}
        function searchNotes(query) {}
        function searchNoteTexts() {}
        function setNotesView(show, kind) {}
        function copyText(text) {
            return true
        }
        function memoUrl() {
            return ""
        }
        function filesAccess() {
            return true
        }
        function askFilesAccess() {}
        function foldersIn(path) {
            return JSON.stringify({ path: path, parent: "", folders: [], readable: true })
        }
        function setSetting(key, value) {
            return ""
        }
        function changeSetting(key, shown, value) {
            return ""
        }
        // The Notes page's ⚙ (`settings::for_view("notes")`): the folder first, then where new notes go.
        function settings(view) {
            const row = (key, kind, label, value) => ({ key: key, kind: kind, label: label, help: label + "-help", value: value, choices: [], group: "set-files-group", section: "", min: 0, max: 0, step: 1, unit: "" })
            return JSON.stringify(view === "notes" ? [row("case_store", "folder", "set-notes-root", "/storage/emulated/0/Documents"), row("notes_folder", "text", "set-notes-folder", "notes")] : [])
        }
    }

    // Every QML file of the set, as the phone's module holds it.
    FolderListModel {
        id: files

        folder: Qt.resolvedUrl("../../qml")
        nameFilters: ["*.qml"]
        showDirs: false
    }

    // The page, made as main.qml makes it: setSource, with its required properties.
    Item {
        id: room

        width: root.width
        height: root.height

        Loader {
            id: loader

            anchors.fill: parent
        }
    }

    TestCase {
        name: "AndroidSet"
        when: windowShown

        readonly property var page: loader.item

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
        // Shown on the page: visible (its parents too), with a size, inside the page.
        function shows(name) {
            const item = named(page, name)
            if (item === null || !item.visible || item.width <= 0 || item.height <= 0)
                return false
            const at = item.mapToItem(page, 0, 0)
            return at.x >= -0.5 && at.y >= -0.5 && at.x < page.width && at.y < page.height
        }
        // Something a person can act on is shown: never a page with nothing on it.
        function somethingShown() {
            return ["noteSearch", "notesSettings", "folderNeededChoose", "noteTitle"].some(name => shows(name))
        }

        function made(notes) {
            mock.notes = notes
            const url = Qt.resolvedUrl("../../qml/NotesPage.qml")
            const kind = Qt.createComponent(url)
            verify(kind.status === Component.Ready, "the Notes page made from the phone's files: " + kind.errorString())
            loader.setSource(url, { sioul: mock, theme: testTheme, window: fakeWindow })
            verify(loader.status === Loader.Ready && page !== null)
            waitForRendering(page)
        }

        function init() {
            fakeWindow.compact = true
            room.width = 412
            mock.notesFolder = JSON.stringify({ state: "ready", path: "/storage/emulated/0/Documents" })
            mock.notes = ""
        }
        // The page let go once what it put off to later has run.
        function unload() {
            wait(50)
            loader.source = ""
        }
        function cleanup() {
            unload()
        }

        // Every file of the phone's set is made: none gives a property, nor names a
        // type, that the phone's files lack.
        function test_every_file_of_the_phone_s_set_is_made() {
            tryVerify(() => files.status === FolderListModel.Ready && files.count > 200, 5000, "the folder listed")
            const refused = []
            for (let i = 0; i < files.count; i++) {
                const name = files.get(i, "fileName")
                if (name === "main.qml" || name === "TitleBar.qml")
                    continue
                const kind = Qt.createComponent(files.get(i, "fileUrl"))
                if (kind.status !== Component.Ready)
                    refused.push(kind.errorString().trim())
            }
            verify(refused.length === 0, refused.join(" | "))
            // The phone's own PDF view, not the computer's (which draws with Qt PDF).
            const pdf = Qt.createComponent(Qt.resolvedUrl("../../qml/PdfView.qml")).createObject(root, { source: "" })
            verify(pdf !== null && pdf.document === null, "the phone's set: qml/android/PdfView.qml in place")
            pdf.destroy()
        }

        // Notes listed: the search, the head and its ⚙, and the notes.
        function test_with_notes_the_list_its_head_and_its_settings() {
            made(mock.listed([mock.plan]))
            verify(shows("noteSearch") && shows("notesSettings"), "the search and the ⚙")
            verify(!page.barSettings, "the ⚙ in the page's head, not in the bar")
            tryVerify(() => named(page, "noteList").count > 0, 2000, "the notes listed")
            verify(!shows("noteFolderEmpty") && !shows("noteReading"))
        }

        // The folder not read yet (a large folder on a phone): the head, and a line saying it is read.
        function test_not_read_yet_the_head_and_a_line_saying_so() {
            made("")
            verify(shows("noteSearch") && shows("notesSettings"))
            tryVerify(() => shows("noteReading"), 2000, "said that the folder is being read")
            // Read: the line goes.
            mock.notes = mock.listed([mock.plan])
            tryVerify(() => !shows("noteReading"))
        }

        // A folder read and holding no note: said, with its path, under the head.
        function test_an_empty_folder_says_so_with_its_path() {
            made(mock.listed([]))
            verify(shows("noteSearch") && shows("notesSettings"))
            tryVerify(() => shows("noteFolderEmpty"), 2000)
            compare(named(page, "noteFolderEmpty").text, "note-folder-empty /storage/emulated/0/Documents")
        }

        // No folder set, or one that does not read: asked for in the page's place, the ⚙ in the bar.
        function test_no_folder_the_prompt_and_the_settings_in_the_bar() {
            for (const state of [{ state: "unset" }, { state: "unreadable", path: "/storage/emulated/0/Gone" }]) {
                mock.notesFolder = JSON.stringify(state)
                made("")
                verify(shows("folderNeededChoose"), state.state + ": the folder asked for")
                verify(!shows("noteSearch"), state.state + ": not an empty list")
                verify(page.barSettings, state.state + ": the ⚙ in the window's bar")
                verify(!shows("folderNeededSettings"), state.state + ": not twice")
                // The bar's ⚙ (main.qml: `shownPage.openSettings(true, "")`) opens the page's panel, its head hidden.
                page.openSettings(true, "")
                const button = named(page, "notesSettings")
                tryVerify(() => {
                    for (let i = 0; i < button.children.length; i++) {
                        const panel = button.children[i].item
                        if (panel && panel.opened)
                            return true
                    }
                    return false
                }, 3000, state.state + ": the panel open")
                page.openSettings(false, "")
                unload()
            }
        }

        // A note open on a phone: the note, with a way back to the list and its ⚙.
        function test_a_note_open_shows_with_a_way_back() {
            made(mock.listed([mock.plan]))
            page.open(mock.plan.path)
            tryVerify(() => shows("noteTitle") && shows("notePanel"))
            verify(page.canGoBack, "Back in the bar")
            verify(!shows("noteSearch"), "the list gives the note its room")
            page.back()
            tryVerify(() => shows("noteSearch") && shows("notesSettings"), 2000, "back to the list and its ⚙")
            verify(!page.canGoBack)
        }

        // A PDF open on a phone: the phone's PdfView, with Open with… under its title.
        function test_a_pdf_open_shows_its_title_and_open_with() {
            made(mock.listed([mock.lease]))
            page.open(mock.lease.path)
            tryVerify(() => shows("noteTitle") && shows("noteOpenElsewhere"))
            verify(page.canGoBack)
        }

        // A note gone meanwhile (deleted on another device) or not readable: the list, not nothing.
        function test_a_note_gone_leaves_the_list() {
            made(mock.listed([mock.plan]))
            page.open("admin/gone.md")
            verify(page.note === null && !page.canGoBack)
            verify(shows("noteSearch") && shows("notesSettings"))
        }

        // Whatever the state, at a phone's width something is shown.
        function test_never_nothing() {
            const states = [[{ state: "ready", path: "/storage/emulated/0/Documents" }, ""], [{ state: "ready", path: "/storage/emulated/0/Documents" }, mock.listed([])], [{ state: "ready", path: "/storage/emulated/0/Documents" }, mock.listed([mock.plan])], [{ state: "unset" }, ""], [{ state: "unreadable", path: "/x" }, ""]]
            for (const [folder, notes] of states) {
                mock.notesFolder = JSON.stringify(folder)
                made(notes)
                verify(somethingShown(), JSON.stringify(folder) + " " + notes)
                for (const path of ["", mock.plan.path, mock.lease.path, "admin/gone.md"]) {
                    page.open(path)
                    verify(somethingShown(), JSON.stringify(folder) + " with " + (path || "nothing") + " open")
                }
                unload()
            }
        }

        // On a computer, the folder asked for keeps the page's ⚙ beside its title.
        function test_on_a_computer_the_prompt_keeps_the_settings() {
            fakeWindow.compact = false
            room.width = 1000
            mock.notesFolder = JSON.stringify({ state: "unset" })
            made("")
            verify(shows("folderNeededChoose"))
            verify(!page.barSettings)
            verify(shows("folderNeededSettings"), "the ⚙ beside the title")
            mouseClick(named(page, "folderNeededSettings"))
            const button = named(page, "notesSettings")
            tryVerify(() => {
                for (let i = 0; i < button.children.length; i++) {
                    const panel = button.children[i].item
                    if (panel && panel.opened)
                        return true
                }
                return false
            }, 3000, "the panel open")
            page.openSettings(false, "")
        }
    }
}
