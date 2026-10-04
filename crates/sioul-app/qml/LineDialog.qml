// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A line of a budget changed in place: its label, amount and date. Its links,
// its budget and the comment saying where it came from stay.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Dialog {
    id: lineChange

    required property var sioul
    required property var theme
    required property var window

    signal changed

    property string uri: ""
    property string problem: ""

    function begin(line) {
        lineChange.uri = line.uri
        lineChange.problem = ""
        changeLabel.text = line.label
        changeAmount.text = String(line.value).replace(".", lineChange.window.sioulLocale.decimalPoint)
        changeDay.date = line.day
        lineChange.open()
    }

    // Written into the file when the amount is one; else said here, the form kept open.
    function save() {
        const amount = Number(changeAmount.text.replace(/[\s€]/g, "").replace(",", ".").replace("−", "-"))
        if (!amount) {
            lineChange.problem = lineChange.sioul.text("budget-add-bad-amount")
            return
        }
        const problem = lineChange.sioul.changeLine(lineChange.uri, changeLabel.text, amount, changeDay.date)
        if (problem !== "") {
            lineChange.problem = problem
            return
        }
        lineChange.close()
        lineChange.changed()
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    width: Math.min(460, (parent ? parent.width : 460) - 2 * lineChange.theme.gap)
    title: lineChange.sioul.text("budget-line-change")

    contentItem: GridLayout {
        columns: 2
        columnSpacing: 10
        rowSpacing: 8

        Label {
            text: lineChange.sioul.text("budget-add-label")
            color: lineChange.theme.muted
        }
        TextField {
            id: changeLabel

            Layout.fillWidth: true
        }
        Label {
            text: lineChange.sioul.text("budget-add-amount")
            color: lineChange.theme.muted
        }
        TextField {
            id: changeAmount

            Layout.fillWidth: true
            placeholderText: lineChange.sioul.text("budget-add-amount-hint")
            onAccepted: lineChange.save()
        }
        Label {
            text: lineChange.sioul.text("budget-add-date")
            color: lineChange.theme.muted
        }
        DateField {
            id: changeDay

            theme: lineChange.theme
            locale: lineChange.window.sioulLocale
            pickLabel: lineChange.sioul.text("event-pick-day")
        }
        Label {
            visible: lineChange.problem !== ""
            Layout.columnSpan: 2
            Layout.fillWidth: true
            text: lineChange.problem
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: lineChange.theme.warm
        }
    }

    // Sioul's own buttons, in your language (Qt's standard ones are not
    // translated here); inside an Item, so that "Save" closes only what was saved.
    footer: Item {
        implicitWidth: buttons.implicitWidth
        implicitHeight: buttons.implicitHeight

        DialogButtonBox {
            id: buttons

            anchors.fill: parent

            Button {
                text: lineChange.sioul.text("ui-save")
                highlighted: true
                DialogButtonBox.buttonRole: DialogButtonBox.AcceptRole
            }
            Button {
                text: lineChange.sioul.text("ui-cancel")
                DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
            }
            onAccepted: lineChange.save()
            onRejected: lineChange.close()
        }
    }
}
