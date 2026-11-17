// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// "Credit note…" (docs/projects.md, "Invoices in the window"): a draft
// credit note citing the invoice, all its lines (to cancel it) or the ones
// ticked (to correct some), opened in the editor to be checked and issued.
// An issued invoice is never changed: a credit note answers it.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

SafeDialog {
    id: dialog

    required property var sioul
    required property var theme
    // The invoice, as its pane gives it ({number, title, lines: [{description, amount}]}).
    property var invoice: null
    // The places of the lines ticked; every one: the whole invoice cancelled.
    property var ticked: []
    property string problem: ""

    // The credit note's draft made: its id, for the editor.
    signal made(string id)

    function ask(invoice) {
        dialog.invoice = invoice
        dialog.ticked = invoice.lines.map((_, i) => i)
        dialog.problem = ""
        dialog.open()
    }

    function save() {
        const all = dialog.invoice && dialog.ticked.length === dialog.invoice.lines.length
        if (dialog.ticked.length === 0) {
            dialog.problem = dialog.sioul.text("invoices-credit-none")
            return
        }
        const answer = JSON.parse(dialog.sioul.creditInvoice(dialog.invoice.number, JSON.stringify(all ? [] : dialog.ticked)) || "{}")
        if (answer.error) {
            dialog.problem = answer.error
            return
        }
        dialog.close()
        dialog.made(answer.draft)
    }

    objectName: "invoiceCreditDialog"
    parent: Overlay.overlay
    anchors.centerIn: parent
    width: Math.min(480, (parent ? dialog.roomWidth : 480) - 2 * dialog.theme.gap)
    modal: true
    title: dialog.invoice ? dialog.theme.plain(dialog.sioul.textWith("invoices-credit-title", "title", dialog.invoice.title)) : ""

    ColumnLayout {
        width: parent ? parent.width : 440
        spacing: 6

        Label {
            Layout.fillWidth: true
            text: dialog.sioul.text("invoices-credit-about")
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: dialog.theme.muted
        }
        Repeater {
            model: dialog.invoice ? dialog.invoice.lines : []

            delegate: CheckBox {
                id: line

                required property var modelData
                required property int index

                objectName: "invoiceCreditLine"
                Layout.fillWidth: true
                text: dialog.theme.plain([line.modelData.description, line.modelData.time, line.modelData.amount].filter(t => t !== "").join("  ·  "))
                checked: dialog.ticked.indexOf(line.index) >= 0
                onToggled: dialog.ticked = line.checked ? dialog.ticked.concat([line.index]).sort((a, b) => a - b) : dialog.ticked.filter(i => i !== line.index)
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
            objectName: "invoiceCreditSave"
            text: dialog.sioul.text("invoices-credit-make")
            highlighted: true
            onClicked: dialog.save()
        }
        Button {
            text: dialog.sioul.text("ui-cancel")
            onClicked: dialog.close()
        }
    }
}
