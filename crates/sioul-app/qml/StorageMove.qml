// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The window of the move of Sioul's lists out of the notes folder
// (docs/notes-folder.md, "What lives where"): what moves and where, the
// papers' and the letters' folders to choose, what stays (the notes) and
// the copy kept, how the lists travel afterwards, and "Move now" or
// "Later". Shown by itself once, when every device that counts knows the
// new places (`storageMove.offer`, the view flag `storage-move-said`);
// opened again from Settings ▸ Sharing. Nothing moves before the press;
// while a device runs an older Sioul, it says which and what to do, and
// "Move now" waits. The move runs off the window's thread (`moveLists`):
// copied, checked, a dated copy kept, then switched; what went wrong is said
// in one sentence, and the lists stay where they were.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs
import QtQuick.Layouts

SafeDialog {
    id: move

    // Sioul's backend (backend.rs): its words in your language and what it does.
    required property var sioul
    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme
    // Where Sioul's lists are and the move stands (`storage::view_json`).
    readonly property var view: move.sioul.storageMove ? JSON.parse(move.sioul.storageMove) : ({})
    // The folders chosen here, "" for the ones the view proposes.
    property string papers: ""
    property string letters: ""
    // The part "projects" switched on here with the move, when it is off: ticked unless you untick it.
    property bool carry: true
    // Which folder the chooser is for: "papers" or "letters".
    property string choosing: ""
    property bool ownChooser: Qt.platform.os === "android"
    // The press made: the window stays to say how it went.
    property bool pressed: false

    // Opened (by itself once, main.qml's `showStorageMove`, or from Settings ▸ Sharing).
    function show() {
        if (move.opened)
            return
        move.papers = ""
        move.letters = ""
        move.carry = true
        move.pressed = false
        move.open()
    }

    function later() {
        move.sioul.setViewFlag("storage-move-said", true)
        move.close()
    }

    function moveNow() {
        move.sioul.setViewFlag("storage-move-said", true)
        move.pressed = true
        move.sioul.moveLists(move.papers, move.letters, move.carry && !move.view.carries && move.view.sharing === true)
    }

    function choose(which) {
        move.choosing = which
        if (!move.ownChooser) {
            chooser.open()
            return
        }
        if (browser.item === null) {
            browser.setSource("FolderBrowser.qml", { sioul: move.sioul, theme: move.theme })
        }
        const own = browser.item as FolderBrowser
        if (own !== null) {
            own.begin(which === "papers" ? move.papersShown : move.lettersShown)
        }
    }

    function take(path) {
        if (move.choosing === "papers")
            move.papers = path
        else if (move.choosing === "letters")
            move.letters = path
    }

    readonly property string papersShown: move.papers !== "" ? move.papers : (move.view.papers || "")
    readonly property string lettersShown: move.letters !== "" ? move.letters : (move.view.letters || "")

    objectName: "storageMove"
    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    width: Math.min(560, move.roomWidth - 2 * move.safeGap)
    title: move.sioul.text("storage-move-title")

    contentItem: ScrollView {
        id: scroll

        contentWidth: availableWidth
        clip: true
        implicitHeight: Math.min(column.implicitHeight, move.roomHeight - 200)

        ColumnLayout {
            id: column

            width: scroll.availableWidth
            spacing: 10

            // Done: where everything is now.
            Label {
                objectName: "storageMoveDone"
                visible: move.view.moved === true
                Layout.fillWidth: true
                text: move.sioul.text("storage-move-done")
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: move.theme.text
            }
            // Where the lists are is not known now (the state of the move lost): that alone, and no move.
            Label {
                objectName: "storageMoveLost"
                visible: (move.view.lost || "") !== ""
                Layout.fillWidth: true
                text: move.view.lost || ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: move.theme.warm
            }
            // Waiting for a device on an older Sioul: which, and what to do.
            Label {
                objectName: "storageMoveWaiting"
                visible: move.view.moved !== true && move.view.open === false
                Layout.fillWidth: true
                // Why it waits: the devices on an older Sioul, or the sharing that cannot be read now (`held`).
                text: (move.view.held || "") !== "" ? move.view.held : move.sioul.textWith("storage-waiting", "devices", move.view.waiting || "")
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: move.theme.warm
            }
            Label {
                objectName: "storageMoveWhat"
                visible: move.view.moved !== true
                Layout.fillWidth: true
                text: move.sioul.textArgs("storage-move-what", JSON.stringify({ lists: move.view.lists || 0 }))
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: move.theme.text
            }
            // The two folders, each with its chooser.
            Repeater {
                model: move.view.moved === true ? [] : ["papers", "letters"]

                delegate: ColumnLayout {
                    id: folder

                    required property string modelData

                    Layout.fillWidth: true
                    spacing: 4

                    Label {
                        Layout.fillWidth: true
                        text: move.sioul.textArgs("storage-move-" + folder.modelData, JSON.stringify({ count: (folder.modelData === "papers" ? move.view.papers_files : move.view.letters_files) || 0 }))
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: move.theme.text
                    }
                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 8

                        TextField {
                            objectName: "storageMoveFolder:" + folder.modelData
                            Layout.fillWidth: true
                            enabled: !move.pressed
                            text: folder.modelData === "papers" ? move.papersShown : move.lettersShown
                            onEditingFinished: {
                                if (folder.modelData === "papers")
                                    move.papers = text
                                else
                                    move.letters = text
                            }
                        }
                        Button {
                            enabled: !move.pressed
                            icon.name: "folder-open"
                            text: move.sioul.text("storage-move-choose")
                            onClicked: move.choose(folder.modelData)
                        }
                    }
                }
            }
            // What stays under papers/ and letters/: no list of Sioul's names it (a note, a scan not read yet).
            Label {
                objectName: "storageMoveLeft"
                visible: move.view.moved !== true && (move.view.left || []).length > 0
                Layout.fillWidth: true
                text: move.sioul.textWith("storage-move-left", "files", (move.view.left || []).join(", "))
                textFormat: Text.PlainText
                wrapMode: Text.WrapAtWordBoundaryOrAnywhere
                color: move.theme.muted
            }
            Label {
                objectName: "storageMoveNotes"
                visible: move.view.moved !== true
                Layout.fillWidth: true
                // The notes folder in its three states: ready, set and not read now (no move), none set.
                text: move.view.notes_state === "unreadable" ? move.sioul.textWith("storage-notes-unreadable", "what", move.view.notes || "") : ((move.view.notes || "") !== "" ? move.sioul.textWith("storage-move-notes", "notes", move.view.notes) : move.sioul.text("storage-move-no-notes"))
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: move.view.notes_state === "unreadable" ? move.theme.warm : move.theme.text
            }
            Label {
                visible: move.view.moved !== true
                Layout.fillWidth: true
                text: move.sioul.text("storage-move-kept")
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: move.theme.muted
            }
            // How the lists travel afterwards: no sharing here, or its part "projects" off here.
            Label {
                objectName: "storageMoveNoSharing"
                visible: move.view.moved !== true && move.view.sharing !== true
                Layout.fillWidth: true
                text: move.sioul.text("storage-move-no-sharing")
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: move.theme.warm
            }
            CheckBox {
                objectName: "storageMoveCarry"
                visible: move.view.moved !== true && move.view.sharing === true && move.view.carries !== true
                Layout.fillWidth: true
                enabled: !move.pressed
                checked: move.carry
                text: move.sioul.text("storage-move-carry")
                onToggled: move.carry = checked
            }
            Label {
                visible: move.view.moved !== true && move.view.sharing === true && move.view.carries !== true
                Layout.fillWidth: true
                text: move.sioul.text("storage-move-carry-help")
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: move.theme.muted
            }
            Label {
                objectName: "storageMoveOthers"
                visible: move.view.moved !== true && (move.view.unmoved || "") !== ""
                Layout.fillWidth: true
                text: move.sioul.textWith("storage-move-others", "devices", move.view.unmoved || "")
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: move.theme.muted
            }
            Label {
                objectName: "storageMoveMoving"
                visible: move.view.moving === true
                Layout.fillWidth: true
                text: move.sioul.text("storage-move-moving")
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: move.theme.text
            }
            // Sioul's own folder chooser, on Android (FolderBrowser.qml).
            Loader {
                id: browser

                Layout.maximumHeight: 0
            }
            Label {
                objectName: "storageMoveSaid"
                visible: move.pressed && move.view.moving !== true && (move.view.said || "") !== ""
                Layout.fillWidth: true
                text: move.view.said || ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: move.theme.warm
            }
        }
    }

    footer: DialogButtonBox {
        Button {
            objectName: "storageMoveNow"
            visible: move.view.moved !== true
            enabled: move.view.open === true && (move.view.lost || "") === "" && move.view.notes_state !== "unreadable" && move.view.moving !== true && !(move.pressed && (move.view.said || "") === "")
            highlighted: true
            text: move.sioul.text("storage-move-now")
            // An action, never the dialog's accept: the window stays to say how the move went.
            DialogButtonBox.buttonRole: DialogButtonBox.ActionRole
            onClicked: move.moveNow()
        }
        Button {
            objectName: "storageMoveLater"
            enabled: move.view.moving !== true
            text: move.sioul.text(move.view.moved === true ? "ui-close" : "storage-move-later")
            DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
            onClicked: move.later()
        }
    }

    FolderDialog {
        id: chooser

        title: move.sioul.text("storage-move-choose")
        onAccepted: {
            const path = move.theme.localPath(selectedFolder)
            if (path !== "")
                move.take(path)
        }
    }

    Connections {
        target: browser.item

        function onChosen(path) {
            move.take(path)
        }
    }
}
