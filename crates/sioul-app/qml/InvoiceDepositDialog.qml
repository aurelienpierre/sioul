// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// "Deposit invoice…" (docs/projects.md, "Invoices in the window"): a
// deposit invoice of an amount, or of a share of a draft's total (excluding
// VAT), for that draft's client and project; opened in the editor. The
// final invoice deducts it once it is issued (its "Deposits" in the editor).

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

SafeDialog {
    id: dialog

    required property var sioul
    required property var theme
    // The draft it is taken from ("" for none: an amount alone, from New).
    property string of: ""
    // By a share of the draft's total rather than an amount.
    property bool byShare: true
    property string problem: ""

    // The deposit's draft made: its id, for the editor.
    signal made(string id)

    // What the amount (or the share) typed reads as, by the window's
    // language (the review of phase 3, F8): {ok, value, words}.
    property var reading: ({ ok: false, value: 0, words: "" })

    function read() {
        const text = dialog.byShare ? share.text.replace("%", "") : amount.text
        dialog.reading = JSON.parse(dialog.sioul.readAmount(text, "") || "{}")
        // A share is no amount of money: what it reads as is not said.
        if (dialog.byShare && dialog.reading.ok)
            dialog.reading = Object.assign({}, dialog.reading, { words: "" })
    }

    function number(text) {
        dialog.read()
        return dialog.reading.ok ? dialog.reading.value : NaN
    }

    function ask(of) {
        dialog.of = of || ""
        dialog.byShare = dialog.of !== ""
        dialog.problem = ""
        share.text = "30"
        amount.text = ""
        dialog.reading = { ok: false, value: 0, words: "" }
        description.text = ""
        dialog.open()
    }

    function save() {
        const value = dialog.number(dialog.byShare ? share.text : amount.text)
        if (isNaN(value) || value <= 0) {
            dialog.problem = dialog.sioul.text("invoices-deposit-no-amount")
            return
        }
        const making = { kind: "deposit", of: dialog.of, description: description.text, amount: dialog.byShare ? 0 : value, share: dialog.byShare ? value : 0 }
        const answer = JSON.parse(dialog.sioul.newInvoiceDraft(JSON.stringify(making)) || "{}")
        if (answer.error) {
            dialog.problem = answer.error
            return
        }
        dialog.close()
        dialog.made(answer.draft)
    }

    objectName: "invoiceDepositDialog"
    parent: Overlay.overlay
    anchors.centerIn: parent
    width: Math.min(420, (parent ? dialog.roomWidth : 420) - 2 * dialog.theme.gap)
    modal: true
    title: dialog.sioul.text("invoices-deposit-title")

    ColumnLayout {
        width: parent ? parent.width : 380
        spacing: 6

        RadioButton {
            visible: dialog.of !== ""
            text: dialog.sioul.text("invoices-deposit-share")
            checked: dialog.byShare
            onToggled: dialog.byShare = checked
        }
        TextField {
            id: share

            objectName: "invoiceDepositShare"
            visible: dialog.byShare
            Layout.fillWidth: true
            inputMethodHints: Qt.ImhFormattedNumbersOnly
            placeholderText: "30 %"
            onTextEdited: dialog.read()
        }
        RadioButton {
            visible: dialog.of !== ""
            text: dialog.sioul.text("invoices-deposit-amount")
            checked: !dialog.byShare
            onToggled: dialog.byShare = !checked
        }
        TextField {
            id: amount

            objectName: "invoiceDepositAmount"
            visible: !dialog.byShare
            Layout.fillWidth: true
            inputMethodHints: Qt.ImhFormattedNumbersOnly
            placeholderText: dialog.sioul.text("invoices-deposit-amount-hint")
            onTextEdited: dialog.read()
        }
        Label {
            objectName: "invoiceDepositReading"
            visible: dialog.reading.words !== ""
            Layout.fillWidth: true
            text: dialog.reading.words
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            font.pixelSize: 12
            color: dialog.reading.ok ? dialog.theme.muted : dialog.theme.warm
        }
        TextField {
            id: description

            Layout.fillWidth: true
            placeholderText: dialog.sioul.text("invoices-deposit-what")
        }
        Label {
            Layout.fillWidth: true
            text: dialog.sioul.text("invoices-deposit-about")
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            font.pixelSize: 12
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

    footer: DialogButtonBox {
        Button {
            objectName: "invoiceDepositSave"
            text: dialog.sioul.text("invoices-deposit-make")
            highlighted: true
            onClicked: dialog.save()
        }
        Button {
            text: dialog.sioul.text("ui-cancel")
            onClicked: dialog.close()
        }
    }
}
