// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A reserve, a savings account (docs/accounting.md, "Reserves"): its name, its
// balance on a day, the floor it is never planned below, and how many days
// money asked from it takes to arrive (a Livret A at once, an assurance vie
// in about ten).

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Dialog {
    id: dialog

    required property var sioul
    required property var theme
    property string reserveId: ""
    property string problem: ""

    signal done

    // `reserve` is one of the bank view's reserves, or null for a new one.
    function edit(reserve) {
        dialog.reserveId = reserve ? reserve.id : ""
        dialog.problem = ""
        title.text = reserve ? reserve.title : ""
        const point = Qt.locale(dialog.sioul.text("qt-locale")).decimalPoint
        balance.text = reserve ? String(reserve.balance).replace(".", point) : ""
        asOf.text = reserve ? reserve.as_of : ""
        floor.text = reserve && reserve.floor > 0 ? String(reserve.floor).replace(".", point) : ""
        delay.value = reserve ? reserve.delay_days : 0
        dialog.open()
        title.forceActiveFocus()
    }

    // An amount as typed, "12 345,67 €" or "−650": its number, 0 when nothing
    // is typed, NaN when it is no number (never a silent 0).
    function amount(text) {
        const typed = text.replace(/[\s€]/g, "").replace("−", "-").replace(",", ".")
        return typed === "" ? 0 : Number(typed)
    }

    function save() {
        for (const field of [balance, floor]) {
            if (Number.isNaN(dialog.amount(field.text))) {
                dialog.problem = dialog.sioul.textWith("amount-unreadable", "text", field.text.trim())
                return
            }
        }
        const edit = { title: title.text, balance: dialog.amount(balance.text), as_of: asOf.text.trim(), floor: dialog.amount(floor.text), delay_days: delay.value }
        const problem = dialog.sioul.saveReserve(dialog.reserveId, JSON.stringify(edit))
        if (problem !== "") {
            dialog.problem = problem
            return
        }
        dialog.close()
        dialog.done()
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    width: Math.min(460, (parent ? parent.width : 460) - 2 * dialog.theme.gap)
    title: dialog.reserveId === "" ? dialog.sioul.text("reserve-new") : dialog.sioul.text("reserve-edit")

    contentItem: GridLayout {
        columns: 2
        columnSpacing: 10
        rowSpacing: 8

        Label {
            text: dialog.sioul.text("reserve-title")
            color: dialog.theme.muted
        }
        TextField {
            id: title

            Layout.fillWidth: true
            placeholderText: dialog.sioul.text("reserve-title-hint")
            onAccepted: dialog.save()
        }
        Label {
            text: dialog.sioul.text("reserve-balance")
            color: dialog.theme.muted
        }
        TextField {
            id: balance

            Layout.fillWidth: true
            inputMethodHints: Qt.ImhFormattedNumbersOnly
        }
        Label {
            text: dialog.sioul.text("reserve-as-of")
            color: dialog.theme.muted
        }
        TextField {
            id: asOf

            Layout.fillWidth: true
            placeholderText: dialog.sioul.text("reserve-as-of-hint")
            inputMethodHints: Qt.ImhDate
        }
        Label {
            text: dialog.sioul.text("reserve-floor")
            color: dialog.theme.muted
        }
        TextField {
            id: floor

            Layout.fillWidth: true
            placeholderText: "0"
            inputMethodHints: Qt.ImhFormattedNumbersOnly
        }
        Label {
            text: dialog.sioul.text("reserve-delay")
            color: dialog.theme.muted
        }
        SpinBox {
            id: delay

            from: 0
            to: 90
            editable: true
        }
        Label {
            Layout.columnSpan: 2
            Layout.fillWidth: true
            text: dialog.sioul.text("reserve-delay-help")
            wrapMode: Text.Wrap
            font.pixelSize: 12
            color: dialog.theme.muted
        }
        Label {
            visible: dialog.problem !== ""
            Layout.columnSpan: 2
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
