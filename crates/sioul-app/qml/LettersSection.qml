// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Paper letters, in the Porch's window: each scan read, as a card. Who sent
// it, what it is, how much, by when (legal delays counted into a date); a task
// for that date, its appointment in the agenda, its scan one click away; done,
// it is filed with its project. Letters wait outside like mail.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: section

    required property var sioul
    required property var theme
    required property var window
    property var shown: ({ inbox: "", letters: [], projects: [], missing: "" })
    property string problem: ""

    function reload() {
        section.shown = JSON.parse(section.sioul.letters())
    }

    spacing: 8
    visible: section.shown.letters.length > 0 || section.shown.missing !== ""
    Component.onCompleted: section.reload()
    // Read again once shown, after the binding that showed it: read at once,
    // what it reads would change whether it shows while that is being decided.
    onVisibleChanged: if (visible) Qt.callLater(section.reload)

    Connections {
        target: section.sioul

        function onLettersChanged() {
            section.reload()
        }
    }

    Label {
        Layout.fillWidth: true
        text: section.sioul.text("letters")
        font.pixelSize: 17
        font.weight: Font.DemiBold
        color: section.theme.text
    }
    // What installs the reader: its command, if any, to copy.
    CommandText {
        visible: section.shown.missing !== ""
        Layout.fillWidth: true
        sioul: section.sioul
        theme: section.theme
        text: section.sioul.textWith("letters-no-ocr", "hint", section.shown.missing)
        color: section.theme.warm
    }
    Label {
        visible: section.problem !== ""
        Layout.fillWidth: true
        text: section.problem
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: section.theme.warm
    }

    Repeater {
        model: section.shown.letters

        delegate: Panel {
            id: card

            required property var modelData

            Layout.fillWidth: true
            theme: section.theme

            ColumnLayout {
                anchors.fill: parent
                spacing: 6

                RowLayout {
                    spacing: 8

                    Icon {
                        iconName: "mail-mark-unread"
                    }
                    // What the scan says (its letterhead, its words) is plain text.
                    Label {
                        Layout.fillWidth: true
                        text: card.modelData.sender + " · " + card.modelData.kind_label
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        font.pixelSize: 16
                        color: section.theme.text
                    }
                    Label {
                        text: card.modelData.received
                        font.pixelSize: 12
                        color: section.theme.muted
                    }
                }
                Repeater {
                    model: card.modelData.lines

                    delegate: Label {
                        required property string modelData

                        Layout.fillWidth: true
                        text: modelData
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: section.theme.text
                    }
                }
                Label {
                    visible: card.modelData.problem !== ""
                    Layout.fillWidth: true
                    text: section.sioul.textWith("letters-unread", "problem", card.modelData.problem)
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: section.theme.muted
                }
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 6

                    Button {
                        text: section.sioul.text("letters-scan")
                        // Its path made an address: a "#" or "%" in a scan's name stays in it; Windows' paths too.
                        onClicked: Qt.openUrlExternally(section.theme.fileUrl(card.modelData.file))
                    }
                    Button {
                        visible: card.modelData.deadline !== "" && card.modelData.task === ""
                        highlighted: true
                        text: section.sioul.text("letters-task")
                        onClicked: {
                            section.problem = section.sioul.letterTask(card.modelData.id)
                            section.reload()
                        }
                    }
                    Button {
                        visible: card.modelData.task !== ""
                        flat: true
                        text: section.sioul.text("letters-task-made")
                        onClicked: section.window.openTask(card.modelData.task)
                    }
                    Button {
                        visible: card.modelData.appointment
                        text: section.sioul.text("letters-event")
                        onClicked: section.problem = section.sioul.letterEvent(card.modelData.id)
                    }
                    Item {
                        Layout.fillWidth: true
                    }
                    ComboBox {
                        id: projectChoice

                        readonly property var choices: [{ id: "", title: section.sioul.text("letters-no-project") }].concat(section.shown.projects)

                        Layout.preferredWidth: 170
                        model: choices.map(c => c.title)
                        currentIndex: Math.max(0, choices.findIndex(c => c.id === card.modelData.project))
                    }
                    Button {
                        text: section.sioul.text("letters-done")
                        onClicked: {
                            section.problem = section.sioul.letterDone(card.modelData.id, projectChoice.choices[projectChoice.currentIndex].id)
                            section.reload()
                        }
                    }
                }
            }
        }
    }
}
