// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The Notes page's own prompt for its folder, in the page's place, when
// this device has none, or has one it cannot read (a drive not plugged in):
// what the folder is, in one sentence, or which folder does not read, by its
// path, and "Choose a folder…", which opens the system's folder chooser
// right there (on Android, Sioul's own, FolderBrowser.qml, which asks for
// "All files access" as the sharing's does). Chosen, it becomes this
// device's notes folder (`case_store`, the Notes page's ⚙:
// docs/notes-folder.md), and the page fills. Never an empty list in its
// place. The state is the backend's `notesFolder` (`Config::notes_folder`),
// read again at each minute, so that a drive plugged in is seen; not known
// yet, the page shows as it is.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs
import QtQuick.Layouts

Item {
    id: needed

    // Sioul's backend (backend.rs): the folder's state, and the setting it writes.
    required property var sioul
    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme
    // The page's name, as a heading (the window's bar says it on a phone).
    property string title: ""
    // {state: "ready" | "unreadable" | "unset", path}; ready while not known yet.
    readonly property var folder: needed.sioul.notesFolder ? JSON.parse(needed.sioul.notesFolder) : ({ state: "ready" })
    // Shown in place of the page: no folder, or one that does not read.
    readonly property bool shown: needed.folder.state !== "ready"
    // What went wrong keeping the folder chosen, in words.
    property string problem: ""

    // A folder chosen and kept: the page reads itself again.
    signal chosen

    // The folder chosen: this device's notes folder, at once.
    function take(path) {
        needed.problem = needed.sioul.setSetting("case_store", JSON.stringify(path))
        if (needed.problem === "")
            needed.chosen()
    }

    // The system's chooser, or on Android Sioul's own (the system's refuses the phone's storage).
    function choose() {
        if (Qt.platform.os === "android") {
            if (browser.item === null)
                browser.setSource("FolderBrowser.qml", { sioul: needed.sioul, theme: needed.theme })
            (browser.item as FolderBrowser).begin(needed.folder.path || "")
        } else {
            dialog.open()
        }
    }

    visible: needed.shown

    ScrollView {
        id: scroll

        anchors.fill: parent
        anchors.margins: needed.theme.gap
        contentWidth: availableWidth
        clip: true

        PageColumn {
            theme: needed.theme
            room: scroll.availableWidth
            spacing: 10

            Label {
                visible: needed.title !== "" && !needed.theme.compact
                Layout.fillWidth: true
                text: needed.title
                textFormat: Text.PlainText
                font.pixelSize: 22
                color: needed.theme.text
            }
            Label {
                objectName: "folderNeededSaid"
                Layout.fillWidth: true
                Layout.topMargin: needed.theme.gap
                text: needed.folder.state === "unreadable" ? needed.sioul.textWith("notes-folder-unreadable", "path", needed.folder.path || "") : needed.sioul.text("notes-folder-what")
                textFormat: Text.PlainText
                wrapMode: Text.WrapAtWordBoundaryOrAnywhere
                lineHeight: 1.25
                color: needed.theme.text
            }
            Button {
                objectName: "folderNeededChoose"
                highlighted: true
                icon.name: "folder-open"
                text: needed.sioul.text(needed.folder.state === "unreadable" ? "notes-folder-choose-again" : "notes-folder-choose")
                onClicked: needed.choose()
            }
            Label {
                visible: needed.problem !== ""
                Layout.fillWidth: true
                text: needed.problem
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: needed.theme.warm
            }
        }
    }

    FolderDialog {
        id: dialog

        title: needed.sioul.text("notes-folder-choose")
        // On Windows the address is file:///C:/…: its path is C:/…, not /C:/…
        onAccepted: {
            const path = needed.theme.localPath(selectedFolder)
            if (path === "")
                needed.problem = needed.sioul.text("folder-not-on-device")
            else
                needed.take(path)
        }
    }

    // On Android: Sioul's own folder browser, made the first time.
    Loader {
        id: browser
    }
    Connections {
        target: browser.item

        function onChosen(path) {
            needed.take(path)
        }
    }
}
