// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The Notes page's prompt for its folder (FolderNeeded.qml), on a stand-in
// for Sioul: shown only when the folder is not set or does not read, never
// while its state is not known; a folder chosen is written as this device's
// notes folder (`case_store`), the page told; refused, the reason said. The
// Notes page's ⚙ opens at the folder's row (its door by key). The system's
// chooser itself is not driven here. Made-up paths. Run by
// tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 900
    height: 700

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        property string notesFolder: ""
        property var written: []
        property string refusal: ""
        // The folders Sioul's own chooser asked for (FolderBrowser.qml).
        property var asked: []

        function text(key) {
            return key
        }
        function filesAccess() {
            return true
        }
        function askFilesAccess() {
        }
        function foldersIn(path) {
            mock.asked = mock.asked.concat([path])
            return JSON.stringify({ path: path, parent: "", folders: [], readable: true })
        }
        function textWith(key, name, value) {
            return key + " " + value
        }
        function viewFlag(name) {
            return false
        }
        function setSetting(key, value) {
            mock.written = mock.written.concat([[key, JSON.parse(value)]])
            if (mock.refusal === "")
                mock.notesFolder = JSON.stringify({ state: "ready", path: JSON.parse(value) })
            return mock.refusal
        }
        function changeSetting(key, shown, value) {
            return mock.setSetting(key, value)
        }
        // The Notes page's ⚙ (`settings::for_view("notes")`): the folder first, then where new notes go.
        function settings(view) {
            const row = (key, kind, label, value) => ({ key: key, kind: kind, label: label, help: label + "-help", value: value, choices: [], group: "set-files-group", section: "", min: 0, max: 0, step: 1, unit: "" })
            return JSON.stringify(view === "notes" ? [row("case_store", "folder", "set-notes-root", ""), row("notes_folder", "text", "set-notes-folder", "notes")] : [])
        }
    }

    FolderNeeded {
        id: needed

        anchors.fill: parent
        sioul: mock
        theme: testTheme
        title: "ui-notes"
    }

    SignalSpy {
        id: chosenSpy

        target: needed
        signalName: "chosen"
    }

    Component {
        id: notesSettingsKind

        SettingsButton {
            sioul: mock
            theme: testTheme
            view: "notes"
        }
    }

    TestCase {
        name: "FolderNeeded"
        when: windowShown

        function walk(item, test, found) {
            if (!item)
                return found
            if (test(item))
                found.push(item)
            const kids = item.children || []
            for (let i = 0; i < kids.length; ++i)
                walk(kids[i], test, found)
            if (item.contentItem && item.contentItem !== item && kids.indexOf(item.contentItem) < 0)
                walk(item.contentItem, test, found)
            return found
        }
        function named(name) {
            return walk(needed, i => i.objectName === name, [])[0]
        }

        function init() {
            mock.written = []
            mock.refusal = ""
            chosenSpy.clear()
            needed.problem = ""
        }

        function test_shown_only_when_the_folder_is_missing_or_unreadable() {
            mock.notesFolder = ""
            verify(!needed.visible, "not known yet: the page as it is")
            mock.notesFolder = JSON.stringify({ state: "ready", path: "/stand-in/Notes" })
            verify(!needed.visible)
            mock.notesFolder = JSON.stringify({ state: "unset" })
            verify(needed.visible)
            compare(named("folderNeededSaid").text, "notes-folder-what")
            compare(named("folderNeededChoose").text, "notes-folder-choose")
            mock.notesFolder = JSON.stringify({ state: "unreadable", path: "/media/stand-in/Notes" })
            compare(named("folderNeededSaid").text, "notes-folder-unreadable /media/stand-in/Notes")
            compare(named("folderNeededChoose").text, "notes-folder-choose-again")
        }

        function test_a_folder_chosen_is_this_device_s_notes_folder() {
            mock.notesFolder = JSON.stringify({ state: "unset" })
            needed.take("/stand-in/Notes")
            compare(mock.written, [["case_store", "/stand-in/Notes"]])
            compare(chosenSpy.count, 1, "the page told")
            verify(!needed.visible, "the page fills")
        }

        // On Android the button opens Sioul's own chooser, at each press: the
        // first press makes it and opens it, the next ones open it again, and
        // the folder chosen there is kept.
        function test_sioul_s_own_chooser_opens_at_each_press() {
            mock.notesFolder = JSON.stringify({ state: "unreadable", path: "/storage/stand-in/Notes" })
            needed.ownChooser = true
            mock.asked = []
            named("folderNeededChoose").clicked()
            verify(needed.ownBrowser !== null, "the chooser is made")
            tryVerify(() => needed.ownBrowser.opened, 2000, "and open, at the first press")
            compare(mock.asked, ["/storage/stand-in/Notes"], "in the folder that does not read")
            needed.ownBrowser.close()
            tryVerify(() => !needed.ownBrowser.visible)
            named("folderNeededChoose").clicked()
            tryVerify(() => needed.ownBrowser.opened, 2000, "and again at the second")
            needed.ownBrowser.chosen("/storage/stand-in/Other")
            compare(mock.written, [["case_store", "/storage/stand-in/Other"]])
            needed.ownBrowser.close()
            needed.ownChooser = false
        }

        function test_a_folder_refused_says_why() {
            mock.notesFolder = JSON.stringify({ state: "unset" })
            mock.refusal = "case_store: refused"
            needed.take("/stand-in/Elsewhere")
            compare(chosenSpy.count, 0)
            verify(needed.visible)
            verify(walk(needed, i => i.visible && i.text === "case_store: refused", []).length === 1, "said")
        }

        // The door by key (SettingDoor.qml, then NotesPage.openSettings): the ⚙ at the folder's row.
        function test_the_notes_settings_open_on_the_folder() {
            const button = createTemporaryObject(notesSettingsKind, root)
            button.show(true)
            button.scrollTo("case_store")
            const panel = () => {
                for (let i = 0; i < button.children.length; i++)
                    if (button.children[i].item)
                        return button.children[i].item
                return null
            }
            tryVerify(() => panel() !== null && panel().opened)
            const rows = walk(panel().contentItem, i => i instanceof SettingRow, [])
            compare(rows.map(r => r.setting.key), ["case_store", "notes_folder"], "the folder first")
            compare(rows[0].setting.kind, "folder", "chosen with the system's chooser there too")
            button.show(false)
        }
    }
}
