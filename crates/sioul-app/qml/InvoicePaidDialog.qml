// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// "Paid…" on an invoice, "Paid back…" on a credit note (docs/projects.md,
// "Invoices in the window"): the day (today), the means (a transfer), the
// amount (what is left); a part of it is a partial payment, the payments
// adding up. Said in the status line with Undo.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

SafeDialog {
    id: dialog

    required property var sioul
    required property var theme
    // The invoice, as its pane gives it ({number, kind, title, left, currency}).
    property var invoice: null
    property string problem: ""
    readonly property var means: ["transfer", "card", "cash", "cheque", "direct-debit", "other"]

    // A payment written: the list reads again.
    signal paid

    // Today, as 2026-10-05.
    function today() {
        const d = new Date()
        const pad = n => n < 10 ? "0" + n : String(n)
        return d.getFullYear() + "-" + pad(d.getMonth() + 1) + "-" + pad(d.getDate())
    }

    // "1 234,5" or "1,234.5" as typed: a number; NaN for none.
    function number(text) {
        const plain = String(text).replace(/[\s  €$£]/g, "")
        const decimal = plain.lastIndexOf(",") > plain.lastIndexOf(".") ? plain.replace(/\./g, "").replace(",", ".") : plain.replace(/,/g, "")
        return decimal === "" ? NaN : Number(decimal)
    }

    function ask(invoice) {
        dialog.invoice = invoice
        dialog.problem = ""
        day.date = dialog.today()
        meansChoice.currentIndex = 0
        amount.text = String(invoice.left)
        dialog.open()
        amount.forceActiveFocus()
    }

    function save() {
        const value = dialog.number(amount.text)
        if (isNaN(value) || value <= 0) {
            dialog.problem = dialog.sioul.text("invoices-pay-no-amount")
            return
        }
        const said = dialog.sioul.payInvoice(dialog.invoice.number, JSON.stringify({ day: day.date, means: dialog.means[meansChoice.currentIndex], amount: value }))
        if (said !== "") {
            dialog.problem = said
            return
        }
        dialog.close()
        dialog.paid()
    }

    objectName: "invoicePaidDialog"
    parent: Overlay.overlay
    anchors.centerIn: parent
    width: Math.min(420, (parent ? dialog.roomWidth : 420) - 2 * dialog.theme.gap)
    modal: true
    title: dialog.invoice ? dialog.theme.plain(dialog.sioul.textWith(dialog.invoice.kind === "credit-note" ? "invoices-paid-back-title" : "invoices-paid-title", "title", dialog.invoice.title)) : ""

    ColumnLayout {
        width: parent ? parent.width : 380
        spacing: 8

        Label {
            text: dialog.sioul.text("invoices-pay-day")
            color: dialog.theme.text
        }
        DateField {
            id: day

            Layout.preferredWidth: 220
            Layout.preferredHeight: 40
            theme: dialog.theme
            sioul: dialog.sioul
        }
        Label {
            text: dialog.sioul.text("invoices-pay-means")
            color: dialog.theme.text
        }
        PlainComboBox {
            id: meansChoice

            Layout.fillWidth: true
            model: dialog.means.map(m => dialog.sioul.text("books-means-" + m))
        }
        Label {
            text: dialog.sioul.text("invoices-pay-amount")
            color: dialog.theme.text
        }
        TextField {
            id: amount

            objectName: "invoicePaidAmount"
            Layout.fillWidth: true
            inputMethodHints: Qt.ImhFormattedNumbersOnly
            onAccepted: dialog.save()
        }
        Label {
            Layout.fillWidth: true
            text: dialog.sioul.text("invoices-pay-partly")
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
            objectName: "invoicePaidSave"
            text: dialog.sioul.text(dialog.invoice && dialog.invoice.kind === "credit-note" ? "invoices-paid-back-save" : "invoices-paid-save")
            highlighted: true
            onClicked: dialog.save()
        }
        Button {
            text: dialog.sioul.text("ui-cancel")
            onClicked: dialog.close()
        }
    }
}
