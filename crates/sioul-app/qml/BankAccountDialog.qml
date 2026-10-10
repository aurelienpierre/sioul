// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A bank account (docs/accounting.md, "Bank accounts"): its name and kind (a
// bank, PayPal, Stripe), the budgets it fills (the first takes what nothing
// else places), the balance kept on it at least, and the reserves that top it
// up, in their order. Taking one out keeps its movements in sioul-bank.toml.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Dialog {
    id: dialog

    required property var sioul
    required property var theme
    property var budgets: []
    property var reserves: []
    property string accountId: ""
    property string problem: ""
    property bool removing: false
    readonly property var kinds: ["bank", "paypal", "stripe", "other"]
    // In their order: the first budget takes what nothing else places; the first reserve tops up first.
    property var fills: []
    property var toppedUpBy: []

    signal done

    // `account` is a card of the bank view, or null for a new one.
    function edit(account) {
        dialog.accountId = account ? account.id : ""
        dialog.problem = ""
        dialog.removing = false
        title.text = account ? account.title : ""
        kind.currentIndex = account ? Math.max(0, dialog.kinds.indexOf(account.kind)) : 0
        dialog.fills = account ? account.fills.map(b => b.id) : dialog.budgets.map(b => b.id)
        dialog.toppedUpBy = account ? account.topped_up_by.map(r => r.id) : []
        floor.text = account && account.floor > 0 ? String(account.floor).replace(".", Qt.locale(dialog.sioul.text("qt-locale")).decimalPoint) : ""
        dialog.open()
        title.forceActiveFocus()
    }

    // An amount as typed, "1 500,50 €": its number, 0 when nothing is typed,
    // NaN when it is no number (never a silent 0).
    function amount(text) {
        const typed = text.replace(/[\s€]/g, "").replace("−", "-").replace(",", ".")
        return typed === "" ? 0 : Number(typed)
    }

    // Ticked: added at the end; unticked: taken out.
    function toggled(list, id, on) {
        const kept = list.filter(x => x !== id)
        return on ? kept.concat([id]) : kept
    }

    // One place earlier in its list.
    function earlier(list, id) {
        const at = list.indexOf(id)
        if (at <= 0)
            return list
        const next = list.slice()
        next.splice(at - 1, 0, next.splice(at, 1)[0])
        return next
    }

    function save() {
        const kept = dialog.amount(floor.text)
        if (Number.isNaN(kept)) {
            dialog.problem = dialog.sioul.textWith("amount-unreadable", "text", floor.text.trim())
            return
        }
        const edit = { title: title.text, kind: dialog.kinds[kind.currentIndex], fills: dialog.fills, floor: kept, topped_up_by: dialog.toppedUpBy }
        const problem = dialog.sioul.saveBankAccount(dialog.accountId, JSON.stringify(edit))
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
    width: Math.min(520, (parent ? parent.width : 520) - 2 * dialog.theme.gap)
    title: dialog.accountId === "" ? dialog.sioul.text("bank-account-new") : dialog.sioul.text("bank-account-edit")

    contentItem: GridLayout {
        columns: 2
        columnSpacing: 10
        rowSpacing: 8

        Label {
            text: dialog.sioul.text("bank-account-title")
            color: dialog.theme.muted
        }
        TextField {
            id: title

            Layout.fillWidth: true
            placeholderText: dialog.sioul.text("bank-account-title-hint")
            onAccepted: dialog.save()
        }
        Label {
            text: dialog.sioul.text("bank-account-kind")
            color: dialog.theme.muted
        }
        PlainComboBox {
            id: kind

            Layout.fillWidth: true
            model: dialog.kinds.map(k => dialog.sioul.text("bank-kind-" + k))
        }
        // The budgets it fills, in order.
        Label {
            Layout.alignment: Qt.AlignTop
            Layout.topMargin: 8
            text: dialog.sioul.text("bank-account-fills")
            color: dialog.theme.muted
        }
        ColumnLayout {
            Layout.fillWidth: true
            spacing: 0

            Repeater {
                model: dialog.fills.map(id => dialog.budgets.find(b => b.id === id)).filter(b => b !== undefined).concat(dialog.budgets.filter(b => dialog.fills.indexOf(b.id) < 0))

                delegate: RowLayout {
                    id: budgetRow

                    required property var modelData
                    required property int index
                    readonly property bool on: dialog.fills.indexOf(budgetRow.modelData.id) >= 0

                    spacing: 4

                    CheckBox {
                        Layout.fillWidth: true
                        text: dialog.theme.plain(budgetRow.modelData.title) + (budgetRow.on && dialog.fills[0] === budgetRow.modelData.id ? "  · " + dialog.sioul.text("bank-account-first") : "")
                        checked: budgetRow.on
                        onToggled: dialog.fills = dialog.toggled(dialog.fills, budgetRow.modelData.id, checked)
                    }
                    ToolButton {
                        visible: budgetRow.on && dialog.fills.indexOf(budgetRow.modelData.id) > 0
                        text: "↑"
                        Accessible.name: dialog.sioul.text("bank-account-earlier")
                        ToolTip.visible: hovered
                        ToolTip.text: dialog.sioul.text("bank-account-earlier")
                        onClicked: dialog.fills = dialog.earlier(dialog.fills, budgetRow.modelData.id)
                    }
                }
            }
            Label {
                Layout.fillWidth: true
                text: dialog.sioul.text("bank-account-fills-help")
                wrapMode: Text.Wrap
                font.pixelSize: 12
                color: dialog.theme.muted
            }
        }
        Label {
            text: dialog.sioul.text("bank-account-floor")
            color: dialog.theme.muted
        }
        TextField {
            id: floor

            Layout.fillWidth: true
            placeholderText: "0"
            inputMethodHints: Qt.ImhFormattedNumbersOnly
        }
        // The reserves that top it up, in order.
        Label {
            visible: dialog.reserves.length > 0
            Layout.alignment: Qt.AlignTop
            Layout.topMargin: 8
            text: dialog.sioul.text("bank-account-topped-by")
            color: dialog.theme.muted
        }
        ColumnLayout {
            visible: dialog.reserves.length > 0
            Layout.fillWidth: true
            spacing: 0

            Repeater {
                model: dialog.toppedUpBy.map(id => dialog.reserves.find(r => r.id === id)).filter(r => r !== undefined).concat(dialog.reserves.filter(r => dialog.toppedUpBy.indexOf(r.id) < 0))

                delegate: RowLayout {
                    id: reserveRow

                    required property var modelData
                    readonly property bool on: dialog.toppedUpBy.indexOf(reserveRow.modelData.id) >= 0

                    spacing: 4

                    CheckBox {
                        Layout.fillWidth: true
                        text: dialog.theme.plain(reserveRow.modelData.title) + (reserveRow.modelData.delay_days > 0 ? "  · " + dialog.sioul.textArgs("reserve-delay-short", JSON.stringify({ days: Number(reserveRow.modelData.delay_days) })) : "  · " + dialog.sioul.text("reserve-at-once"))
                        checked: reserveRow.on
                        onToggled: dialog.toppedUpBy = dialog.toggled(dialog.toppedUpBy, reserveRow.modelData.id, checked)
                    }
                    ToolButton {
                        visible: reserveRow.on && dialog.toppedUpBy.indexOf(reserveRow.modelData.id) > 0
                        text: "↑"
                        Accessible.name: dialog.sioul.text("bank-account-earlier")
                        ToolTip.visible: hovered
                        ToolTip.text: dialog.sioul.text("bank-account-earlier")
                        onClicked: dialog.toppedUpBy = dialog.earlier(dialog.toppedUpBy, reserveRow.modelData.id)
                    }
                }
            }
        }
        // Taking it out: said once, then done.
        Label {
            visible: dialog.removing
            Layout.columnSpan: 2
            Layout.fillWidth: true
            text: dialog.sioul.text("bank-account-remove-ask")
            wrapMode: Text.Wrap
            color: dialog.theme.text
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
            Button {
                visible: dialog.accountId !== ""
                flat: true
                text: dialog.removing ? dialog.sioul.text("bank-account-remove-yes") : dialog.sioul.text("bank-account-remove")
                DialogButtonBox.buttonRole: DialogButtonBox.ActionRole
                onClicked: {
                    if (!dialog.removing) {
                        dialog.removing = true
                        return
                    }
                    const problem = dialog.sioul.removeBankAccount(dialog.accountId)
                    if (problem !== "") {
                        dialog.problem = problem
                        return
                    }
                    dialog.close()
                    dialog.done()
                }
            }
            onAccepted: dialog.save()
            onRejected: dialog.close()
        }
    }
}
