// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// One line, one task: "Call the CAF tomorrow ~15m #housing {30/10}". What the
// line says besides the title shows as chips while typing, so nothing is
// misread silently; Enter makes the task, Escape empties the line.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: capture

    required property var sioul
    required property var theme
    // Made as a step of this task, when set.
    property string parentUid: ""
    // The list new tasks go into: "account/id"; empty: the usual one.
    property string list: ""
    // A line of steps: nothing is made until the task they are steps of is.
    property bool step: false
    property string placeholder: capture.sioul.text("task-capture-hint")
    readonly property var parsed: line.text.trim() === "" ? ({ chips: [], edit: {} }) : JSON.parse(capture.sioul.capture(line.text))

    signal added(string uid)

    function focusLine() {
        line.forceActiveFocus()
    }

    spacing: 4

    TextField {
        id: line

        Layout.fillWidth: true
        placeholderText: capture.placeholder
        Accessible.name: capture.placeholder
        Keys.onEscapePressed: line.clear()
        onAccepted: {
            if (line.text.trim() === "" || (capture.step && capture.parentUid === ""))
                return
            const answer = JSON.parse(capture.sioul.addTask(line.text, capture.parentUid, capture.list))
            if (answer.uid) {
                line.clear()
                capture.added(answer.uid)
            }
        }
    }

    // What the line says besides its title.
    Flow {
        visible: capture.parsed.chips.length > 0
        Layout.fillWidth: true
        spacing: 6

        Repeater {
            model: capture.parsed.chips

            delegate: Rectangle {
                id: chip

                required property var modelData

                implicitWidth: chipText.implicitWidth + 16
                implicitHeight: chipText.implicitHeight + 6
                radius: height / 2
                color: capture.theme.surface
                border.color: capture.theme.line

                Label {
                    id: chipText

                    anchors.centerIn: parent
                    text: capture.sioul.text("chip-" + chip.modelData.kind) + "  " + (chip.modelData.kind === "kind" ? capture.sioul.text("task-kind-" + chip.modelData.value) : chip.modelData.value)
                    font.pixelSize: 12
                    color: capture.theme.text
                }
            }
        }
    }
}
