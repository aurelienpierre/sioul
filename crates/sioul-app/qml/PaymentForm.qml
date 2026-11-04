// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// "A payment…" from a message or a paper letter (docs/accounting.md, "A
// payment from any mail"): a short form filled from what it says (who, the
// amount, the day), any budget, whatever the hour; the message or the letter
// stays its source. From a sender not verified, it says to check it first.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

SafeDialog {
    id: dialog

    // Sioul's backend (backend.rs): its words in your language and what it does.
    required property var sioul
    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme
    // Every budget, {id, title}: any may be chosen, whatever the hour.
    property var budgets: []
    // The message or the letter it comes from: its address, and its subject or sender.
    property string source: ""
    property string origin: ""
    // From a sender not verified: to check before it counts.
    property bool doubtful: false
    // What went wrong when saving, in words; "" for nothing.
    property string problem: ""

    // The form filled from what `paymentFrom` read (`payments::from`), and opened;
    // what could not be read said in the status line.
    function ask(read) {
        if (!read || read.problem) {
            dialog.sioul.status = read ? read.problem : ""
            return
        }
        const p = read.payment
        dialog.budgets = read.budgets || []
        dialog.source = p.source
        dialog.origin = read.title || ""
        dialog.doubtful = p.doubtful === true
        dialog.problem = ""
        label.text = p.label || ""
        const point = Qt.locale(dialog.sioul.text("qt-locale")).decimalPoint
        amount.text = p.amount ? p.amount.toFixed(2).replace(".", point).replace("-", "−") : ""
        day.date = p.day || ""
        const chosen = dialog.budgets.findIndex(b => b.id === p.budget)
        budgetChoice.currentIndex = chosen >= 0 ? chosen : 0
        dialog.open()
        amount.forceActiveFocus()
    }

    // Saved into the budget file; what went wrong said in the form, which stays open.
    function save() {
        // "−1 234,50 €" as typed; what is no number goes as 0, which the core answers in words.
        const typed = Number(amount.text.replace(/[\s€  ]/g, "").replace(",", ".").replace("−", "-"))
        const budget = dialog.budgets.length > 0 ? dialog.budgets[Math.max(0, budgetChoice.currentIndex)].id : ""
        const edit = {
            payment: { amount: Number.isNaN(typed) ? 0 : typed, day: day.date, label: label.text, source: dialog.source, budget: budget, doubtful: dialog.doubtful },
            title: dialog.origin
        }
        const answer = JSON.parse(dialog.sioul.addPayment(JSON.stringify(edit)))
        if (answer.problem) {
            dialog.problem = answer.problem
            return
        }
        dialog.sioul.status = answer.said
        dialog.close()
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    width: Math.min(460, (parent ? dialog.roomWidth : 460) - 2 * dialog.theme.gap)
    title: dialog.sioul.text("payment-title")

    contentItem: GridLayout {
        columns: 2
        columnSpacing: 10
        rowSpacing: 8

        Label {
            visible: dialog.origin !== ""
            Layout.columnSpan: 2
            Layout.fillWidth: true
            text: dialog.origin
            textFormat: Text.PlainText
            elide: Text.ElideRight
            color: dialog.theme.muted
        }
        Label {
            visible: dialog.doubtful
            Layout.columnSpan: 2
            Layout.fillWidth: true
            text: dialog.sioul.text("note-doubtful")
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: dialog.theme.warm
        }
        Label {
            text: dialog.sioul.text("payment-who")
            color: dialog.theme.muted
        }
        TextField {
            id: label

            Layout.fillWidth: true
        }
        Label {
            text: dialog.sioul.text("budget-add-amount")
            color: dialog.theme.muted
        }
        TextField {
            id: amount

            Layout.fillWidth: true
            placeholderText: dialog.sioul.text("budget-add-amount-hint")
            onAccepted: dialog.save()
        }
        Label {
            text: dialog.sioul.text("budget-add-date")
            textFormat: Text.PlainText
            color: dialog.theme.muted
        }
        DateField {
            id: day

            theme: dialog.theme
            sioul: dialog.sioul
            locale: Qt.locale(dialog.sioul.text("qt-locale"))
            pickLabel: dialog.sioul.text("event-pick-day")
        }
        Label {
            text: dialog.sioul.text("budget-add-budget")
            color: dialog.theme.muted
        }
        PlainComboBox {
            id: budgetChoice

            Layout.fillWidth: true
            model: dialog.budgets.map(b => dialog.theme.plain(b.title))
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
    // the dialog on "Add" even when saving fails, and what went wrong is never read.
    footer: Item {
        implicitWidth: buttons.implicitWidth
        implicitHeight: buttons.implicitHeight

        DialogButtonBox {
            id: buttons

            anchors.fill: parent

            Button {
                text: dialog.sioul.text("ui-add")
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
