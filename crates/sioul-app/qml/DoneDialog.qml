// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// After "Done for today": the day is already closed ("Undo" waits in the
// status line). Read in ten seconds: where work went and when it comes back,
// what got done if anything, the first step, what still gets through. No
// count of what was not done, no question, no judgement (docs/tasks.md).

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Dialog {
    id: dialog

    required property var sioul
    required property var theme
    required property var window
    property var closing: null
    property string firstStep: ""
    property bool changing: false
    property bool deadlineLeft: false

    function show(closing) {
        if (!closing)
            return
        dialog.closing = closing
        dialog.firstStep = closing.first.text
        dialog.changing = false
        dialog.deadlineLeft = false
        dialog.open()
    }

    function keepStep() {
        dialog.firstStep = stepField.text.trim()
        dialog.sioul.setFirstStep(dialog.firstStep)
        dialog.changing = false
    }

    // For the window's tests: the dialog as an image, its frame included.
    function grab(path) {
        const item = dialog.contentItem.parent || dialog.contentItem
        item.grabToImage(result => result.saveToFile(path))
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    width: Math.min(540, (parent ? parent.width : 540) - 2 * dialog.theme.gap)
    title: dialog.closing ? dialog.theme.plain(dialog.closing.put_away) : ""

    contentItem: ColumnLayout {
        spacing: 10

        // What got done, if anything: never a zero.
        Repeater {
            model: dialog.closing ? dialog.closing.did : []

            delegate: Label {
                required property string modelData

                Layout.fillWidth: true
                text: modelData
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: dialog.theme.text
            }
        }
        Label {
            visible: dialog.closing !== null && dialog.closing.rest !== ""
            Layout.fillWidth: true
            text: dialog.closing ? dialog.closing.rest : ""
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: dialog.theme.text
        }
        // The first step, as the plan has it: one tap to say it your way.
        ColumnLayout {
            visible: dialog.firstStep !== "" || dialog.changing
            Layout.fillWidth: true
            spacing: 2

            Label {
                text: dialog.closing ? dialog.closing.starts_with : ""
                textFormat: Text.PlainText
                color: dialog.theme.muted
            }
            RowLayout {
                visible: !dialog.changing
                Layout.fillWidth: true
                spacing: 8

                Label {
                    Layout.fillWidth: true
                    text: dialog.firstStep
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 17
                    color: dialog.theme.text
                }
                Button {
                    flat: true
                    text: dialog.sioul.text("closing-change-step")
                    onClicked: {
                        stepField.text = dialog.firstStep
                        dialog.changing = true
                        stepField.forceActiveFocus()
                    }
                }
            }
            TextField {
                id: stepField

                visible: dialog.changing
                Layout.fillWidth: true
                placeholderText: dialog.sioul.text("closing-step-hint")
                onAccepted: dialog.keepStep()
            }
        }
        // A date asked before work comes back: said once, with a way to protect it and a way to leave it.
        ColumnLayout {
            visible: dialog.closing !== null && dialog.closing.deadline !== null && !dialog.deadlineLeft
            Layout.fillWidth: true
            spacing: 4

            Label {
                Layout.fillWidth: true
                text: dialog.closing && dialog.closing.deadline ? dialog.closing.deadline.line : ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: dialog.theme.text
            }
            Flow {
                Layout.fillWidth: true
                spacing: 6

                Button {
                    text: dialog.sioul.text("closing-ask-time")
                    onClicked: {
                        const id = dialog.sioul.draftForTask(dialog.closing.deadline.uid)
                        if (id !== "")
                            dialog.window.openDraft(id)
                        dialog.deadlineLeft = true
                    }
                }
                Button {
                    flat: true
                    text: dialog.closing ? dialog.theme.plain(dialog.closing.leave) : ""
                    onClicked: dialog.deadlineLeft = true
                }
            }
        }
        // What rests, and what still gets through: a known silence, not a total one.
        Label {
            Layout.fillWidth: true
            text: dialog.sioul.text("closing-resting")
            wrapMode: Text.Wrap
            color: dialog.theme.muted
        }
        Label {
            Layout.fillWidth: true
            text: dialog.sioul.text("closing-plan")
            wrapMode: Text.Wrap
            color: dialog.theme.muted
        }
    }

    footer: DialogButtonBox {
        Button {
            text: dialog.sioul.text("ui-close")
            highlighted: true
            DialogButtonBox.buttonRole: DialogButtonBox.AcceptRole
        }
    }

    onAccepted: {
        if (dialog.changing)
            dialog.keepStep()
    }
}
