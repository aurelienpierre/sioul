// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A task just done, in one optional press from the status line (docs/supplies.md):
// what it really used ("Change…", or "Supplies used…" for a task that planned
// nothing), its lines each changeable, "Add a supply"; or, for a task to
// resupply, "How many came in?", filled with what it is usually bought by.
// Never asked by itself: the tick applied what was planned already.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

SafeDialog {
    id: dialog

    required property var sioul
    required property var theme
    property string uid: ""
    // "used": what it used; "arrived": how many came in.
    property string mode: "used"
    // The task's supplies (`supplies::task_supplies`).
    property var info: null
    // The lines as changed here, until Save.
    property var lines: []
    property string problem: ""

    // What a task done used, its lines to change.
    function ask(uid) {
        const text = dialog.sioul.taskSupplies(uid)
        if (text === "null")
            return
        dialog.uid = uid
        dialog.info = JSON.parse(text)
        dialog.mode = "used"
        dialog.problem = ""
        const from = dialog.info.used.length > 0 ? dialog.info.used : dialog.info.planned
        dialog.lines = from.map(l => ({ id: l.id, quantity: l.quantity }))
        dialog.open()
    }

    // How many came in, for a task to resupply done: filled with what it is bought by.
    function arrive(uid) {
        const text = dialog.sioul.taskSupplies(uid)
        if (text === "null")
            return
        dialog.uid = uid
        dialog.info = JSON.parse(text)
        if (!dialog.info.resupply)
            return
        dialog.mode = "arrived"
        dialog.problem = ""
        count.text = dialog.info.resupply.text
        dialog.open()
        count.forceActiveFocus()
        count.selectAll()
    }

    function save() {
        const problem = dialog.mode === "arrived" ? dialog.sioul.supplyArrived(dialog.uid, count.text) : dialog.sioul.setSuppliesUsed(dialog.uid, JSON.stringify(dialog.lines))
        dialog.problem = problem
        if (problem === "")
            dialog.close()
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    width: Math.min(460, (parent ? dialog.roomWidth : 460) - 2 * dialog.theme.gap)
    title: dialog.mode === "arrived" ? dialog.sioul.text("supplies-arrived-title") : dialog.sioul.text("supplies-used-title")

    contentItem: ColumnLayout {
        spacing: 8

        SupplyLines {
            visible: dialog.mode === "used"
            Layout.fillWidth: true
            sioul: dialog.sioul
            theme: dialog.theme
            choices: dialog.info ? dialog.info.choices : []
            value: dialog.lines
            onEdited: value => dialog.lines = value
        }
        RowLayout {
            visible: dialog.mode === "arrived"
            Layout.fillWidth: true
            spacing: 8

            Label {
                Layout.fillWidth: true
                text: dialog.info && dialog.info.resupply ? dialog.info.resupply.name : ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: dialog.theme.text
            }
            TextField {
                id: count

                objectName: "suppliesArrivedCount"
                Layout.preferredWidth: 90
                horizontalAlignment: Text.AlignRight
                inputMethodHints: Qt.ImhFormattedNumbersOnly
                onAccepted: dialog.save()
            }
            Label {
                visible: text !== ""
                text: dialog.info && dialog.info.resupply ? dialog.info.resupply.unit : ""
                textFormat: Text.PlainText
                color: dialog.theme.muted
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

    // The buttons inside an Item: a DialogButtonBox as the footer itself closes
    // the dialog on "Save" even when saving fails, and what went wrong is never read.
    footer: Item {
        implicitWidth: buttons.implicitWidth
        implicitHeight: buttons.implicitHeight

        DialogButtonBox {
            id: buttons

            anchors.fill: parent

            Button {
                objectName: "suppliesUsedSave"
                text: dialog.sioul.text("ui-save")
                highlighted: true
                DialogButtonBox.buttonRole: DialogButtonBox.AcceptRole
            }
            Button {
                text: dialog.sioul.text("ui-cancel")
                DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
            }
            onAccepted: dialog.save()
            onRejected: dialog.close()
        }
    }
}
