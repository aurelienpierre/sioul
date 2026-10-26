// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A dose marked more than half an hour past its time, or due while Sioul was
// closed: when it was taken (now, unless you say), or that it was not. For a
// medicine taken every few hours, the next dose comes that many hours after
// the time said: the hours between two doses are kept.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Dialog {
    id: dialog

    required property var sioul
    required property var theme
    property string key: ""
    property var info: null
    property string problem: ""
    // Marked taken already, when ("08:03"): said, and no second Taken offered
    // (review of 36002ea, can-wait 1); "" when not.
    readonly property string marked: dialog.info !== null ? (dialog.info.taken || "") : ""

    // Answered, taken or not: the pages showing doses read them again.
    signal answered

    function ask(key) {
        const info = JSON.parse(dialog.sioul.doseInfo(key))
        if (info === null)
            return
        dialog.key = key
        dialog.info = info
        dialog.problem = ""
        when.text = info.now
        dialog.open()
        when.forceActiveFocus()
        when.selectAll()
    }

    function taken() {
        dialog.problem = dialog.sioul.doseTakenAt(dialog.key, when.text)
        if (dialog.problem !== "")
            return
        dialog.close()
        dialog.answered()
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    width: Math.min(440, (parent ? parent.width : 440) - 2 * dialog.theme.gap)
    title: dialog.sioul.text("dose-taken-title")

    contentItem: ColumnLayout {
        spacing: 10

        Label {
            Layout.fillWidth: true
            text: dialog.info === null ? "" : dialog.sioul.textArgs("dose-taken-due", JSON.stringify({ name: dialog.info.name + (dialog.info.dose !== "" ? " · " + dialog.info.dose : ""), due: dialog.info.due }))
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: dialog.theme.text
        }
        Label {
            objectName: "dose-already-marked"
            visible: dialog.marked !== ""
            Layout.fillWidth: true
            text: dialog.sioul.textWith("dose-already-marked", "time", dialog.marked)
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: dialog.theme.text
        }
        RowLayout {
            visible: dialog.marked === ""
            spacing: 8

            Label {
                text: dialog.sioul.text("dose-taken-when")
                color: dialog.theme.muted
            }
            TextField {
                id: when

                Layout.preferredWidth: 80
                inputMask: "99:99"
                inputMethodHints: Qt.ImhTime
                Accessible.name: dialog.sioul.text("dose-taken-when")
                onAccepted: dialog.taken()
            }
        }
        // Every few hours: the hours between two doses are kept, from the time said.
        Label {
            visible: dialog.marked === "" && dialog.info !== null && dialog.info.hours > 0
            Layout.fillWidth: true
            text: dialog.info === null ? "" : dialog.sioul.textWith("dose-next-after", "hours", String(dialog.info.hours))
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            font.pixelSize: 13
            color: dialog.theme.muted
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

    // The buttons inside an Item: a DialogButtonBox as the footer would close
    // the dialog on "Taken" even when the time does not read.
    footer: Item {
        implicitWidth: buttons.implicitWidth
        implicitHeight: buttons.implicitHeight

        DialogButtonBox {
            id: buttons

            anchors.fill: parent

            Button {
                visible: dialog.marked === ""
                text: dialog.sioul.text("health-taken")
                highlighted: true
                onClicked: dialog.taken()
            }
            Button {
                visible: dialog.marked === ""
                flat: true
                text: dialog.sioul.text("health-not-taken")
                onClicked: {
                    dialog.sioul.doseNotTaken(dialog.key)
                    dialog.close()
                    dialog.answered()
                }
            }
            Button {
                flat: true
                text: dialog.sioul.text("ui-cancel")
                onClicked: dialog.close()
            }
        }
    }
}
