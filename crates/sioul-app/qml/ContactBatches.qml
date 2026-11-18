// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Contacts ⚙'s changes to many contacts of the last thirty days (review F2),
// under their own heading (the panel's Section), the newest first: what each
// did and when, how far it went when it was stopped, and Undo… asked first,
// or why it can no longer be undone. Read off the window's thread
// (`readContactsBatches`, the list in `contactsBatches`); its sentence is the
// row's own, said above it.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: setup

    required property var sioul
    required property var theme
    // The batches of the last thirty days (`contactsBatches`), and the one whose Undo is asked.
    readonly property var batches: setup.sioul.contactsBatches ? JSON.parse(setup.sioul.contactsBatches) : []
    property string undoing: ""
    property string problem: ""

    objectName: "contactBatches"
    spacing: 4
    Component.onCompleted: setup.sioul.readContactsBatches()

    Label {
        visible: setup.batches.length === 0
        Layout.fillWidth: true
        text: setup.sioul.text("books-batches-none")
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.muted
    }
    Label {
        visible: setup.problem !== ""
        Layout.fillWidth: true
        text: setup.problem
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.warm
    }
    Repeater {
        model: setup.batches

        delegate: ColumnLayout {
            id: batch

            required property var modelData

            objectName: "contactsBatch"
            Layout.fillWidth: true
            spacing: 2

            Label {
                Layout.fillWidth: true
                text: [batch.modelData.line, batch.modelData.partly].filter(l => l !== "").join(" ")
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: setup.theme.text
            }
            Label {
                visible: batch.modelData.why_not !== ""
                Layout.fillWidth: true
                text: batch.modelData.why_not
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: setup.theme.muted
            }
            Button {
                objectName: "contactsBatchUndo"
                visible: batch.modelData.why_not === "" && setup.undoing !== batch.modelData.id
                flat: true
                text: setup.sioul.text("batch-undo")
                onClicked: setup.undoing = batch.modelData.id
            }
            Label {
                visible: setup.undoing === batch.modelData.id
                Layout.fillWidth: true
                text: batch.modelData.ask
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: setup.theme.text
            }
            Flow {
                visible: setup.undoing === batch.modelData.id
                Layout.fillWidth: true
                spacing: 6

                Button {
                    objectName: "contactsBatchUndoGo"
                    text: setup.sioul.text("batch-undo-go")
                    onClicked: {
                        setup.problem = setup.sioul.undoContactsBatch(batch.modelData.id)
                        setup.undoing = ""
                    }
                }
                Button {
                    flat: true
                    text: setup.sioul.text("ui-cancel")
                    onClicked: setup.undoing = ""
                }
            }
        }
    }
}
