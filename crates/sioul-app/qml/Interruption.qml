// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Something interrupts you: a meal's, a nap's or the night's notice, or you
// stop of your own accord. For a meal, a nap or the night, today only: a
// few minutes later (as often as you like), at another time, or not today,
// without a word asked. Always: one line on where you stopped, shown again
// when you are back (StoppedCard.qml). Nothing comes again because of it.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Dialog {
    id: dialog

    required property var sioul
    required property var theme
    // A block of today ("meal:1"), or "" to note where you stopped alone.
    property string key: ""
    // Its row, as needsToday() gives it.
    property var block: null
    property string problem: ""

    // The block moved or skipped: the pages showing it read again.
    signal changed

    function ask(key) {
        dialog.key = key || ""
        const rows = JSON.parse(dialog.sioul.needsToday() || "[]")
        dialog.block = rows.find(r => r.key === dialog.key) || null
        const stopped = JSON.parse(dialog.sioul.stopped() || "null")
        line.text = stopped ? stopped.text : ""
        when.text = dialog.block ? dialog.block.from : ""
        dialog.problem = ""
        dialog.open()
        line.forceActiveFocus()
    }

    // The line kept when it changed; an empty one is "done" only when typed so.
    function keepLine() {
        const stopped = JSON.parse(dialog.sioul.stopped() || "null")
        if (line.text.trim() !== (stopped ? stopped.text : ""))
            dialog.sioul.setStopped(line.text)
    }

    function move(minutes, time) {
        dialog.problem = dialog.sioul.moveNeed(dialog.key, minutes, time)
        if (dialog.problem !== "")
            return
        dialog.keepLine()
        dialog.close()
        dialog.changed()
    }

    function skip() {
        dialog.problem = dialog.sioul.skipNeed(dialog.key, !dialog.block.skipped)
        if (dialog.problem !== "")
            return
        dialog.keepLine()
        dialog.close()
        dialog.changed()
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    width: Math.min(460, (parent ? parent.width : 460) - 2 * dialog.theme.gap)
    title: dialog.theme.plain(dialog.block !== null ? dialog.block.name + "  ·  " + dialog.block.from + "–" + dialog.block.to : dialog.sioul.text("stopped-title"))

    contentItem: ColumnLayout {
        spacing: 10

        // Today only: later, at another time, or not today.
        Flow {
            visible: dialog.block !== null
            Layout.fillWidth: true
            spacing: 8

            Button {
                text: dialog.theme.plain(dialog.sioul.textWith("need-later-n", "minutes", String(dialog.sioul.needsLater())))
                onClicked: dialog.move(0, "")
            }
            RowLayout {
                spacing: 6

                Label {
                    text: dialog.sioul.text("need-move-to")
                    color: dialog.theme.muted
                }
                TextField {
                    id: when

                    Layout.preferredWidth: 70
                    inputMask: "99:99"
                    inputMethodHints: Qt.ImhTime
                    Accessible.name: dialog.sioul.text("need-move-to")
                    onAccepted: dialog.move(0, when.text)
                }
                Button {
                    text: dialog.sioul.text("need-move")
                    onClicked: dialog.move(0, when.text)
                }
            }
            Button {
                flat: true
                text: dialog.block !== null && dialog.block.skipped ? dialog.sioul.text("need-unskip") : dialog.sioul.text("needs-not-today")
                onClicked: dialog.skip()
            }
        }
        // Where you stopped: one line, for when you are back.
        Label {
            Layout.fillWidth: true
            text: dialog.sioul.text("stopped-hint")
            wrapMode: Text.Wrap
            color: dialog.theme.muted
        }
        TextField {
            id: line

            Layout.fillWidth: true
            onAccepted: {
                dialog.keepLine()
                dialog.close()
            }
        }
        Label {
            visible: dialog.problem !== ""
            Layout.fillWidth: true
            text: dialog.problem
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: dialog.theme.warm
        }
    }

    footer: DialogButtonBox {
        Button {
            text: dialog.sioul.text("ui-ok")
            highlighted: true
            onClicked: {
                dialog.keepLine()
                dialog.close()
            }
        }
        Button {
            text: dialog.sioul.text("ui-cancel")
            DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
        }
        onRejected: dialog.close()
    }
}
