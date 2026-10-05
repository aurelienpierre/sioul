// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Where you stopped: the line left when something interrupted you (a pause,
// a meal, the night, a timer stopped), shown again until you say it is done,
// with the task it was about (docs/tasks.md, "Where you stopped").

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Panel {
    id: card

    required property var sioul
    required property var window
    property var stopped: null

    function reload() {
        card.stopped = JSON.parse(card.sioul.stopped() || "null")
    }

    visible: card.stopped !== null
    Component.onCompleted: card.reload()

    // Left or done elsewhere (the focus window, another page, another device): read again.
    Connections {
        target: card.sioul

        function onTasksChanged() {
            card.reload()
        }

        function onPorchChanged() {
            card.reload()
        }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 6

        Label {
            Layout.fillWidth: true
            text: card.stopped === null ? "" : card.sioul.textWith("stopped-at", "when", card.stopped.when)
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            font.weight: Font.DemiBold
            color: card.theme.text
        }
        Label {
            Layout.fillWidth: true
            text: card.stopped === null ? "" : card.stopped.text
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            font.pixelSize: 16
            color: card.theme.text
        }
        RowLayout {
            Layout.fillWidth: true
            spacing: 8

            Button {
                visible: card.stopped !== null && card.stopped.task !== "" && card.stopped.title !== ""
                Layout.maximumWidth: card.width * 0.6
                flat: true
                text: card.stopped === null ? "" : card.theme.plain(card.stopped.title)
                onClicked: card.window.openTask(card.stopped.task)
            }
            Item {
                Layout.fillWidth: true
            }
            Button {
                text: card.sioul.text("stopped-done")
                onClicked: {
                    card.sioul.setStopped("")
                    card.reload()
                }
            }
        }
    }
}
