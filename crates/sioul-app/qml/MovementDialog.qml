// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A movement added by hand to a budget: once (a date), or recurring (each
// month or year, on a day). Positive is money in, negative money out.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Dialog {
    id: dialog

    // Sioul's backend (backend.rs): its words in your language and what it does.
    required property var sioul
    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme
    // The budget it goes into, by its id.
    property string budget: ""
    // Started from "New": the budget is chosen here, among {id, title}.
    property var budgets: []
    // The budget is chosen in the form (started from "New"), among `budgets`.
    property bool choosing: false
    // Each month or year rather than once.
    property bool recurring: false
    // What went wrong when saving, in words; "" for nothing.
    property string problem: ""

    // The movement was added.
    signal saved

    // Today, as 2026-10-05.
    function today() {
        const d = new Date()
        const pad = n => n < 10 ? "0" + n : String(n)
        return d.getFullYear() + "-" + pad(d.getMonth() + 1) + "-" + pad(d.getDate())
    }

    // From "New": which budget first.
    function choose() {
        dialog.begin(dialog.budgets.length > 0 ? dialog.budgets[0].id : "")
        dialog.choosing = true
        budgetChoice.currentIndex = 0
    }

    // The form emptied and opened for `budget`.
    function begin(budget) {
        dialog.choosing = false
        dialog.budget = budget
        dialog.recurring = false
        dialog.problem = ""
        label.text = ""
        amount.text = ""
        day.date = dialog.today()
        dayOfMonth.value = new Date().getDate()
        every.currentIndex = 0
        estimate.checked = false
        dialog.open()
        label.forceActiveFocus()
    }

    // The form saved: the amount read as typed, then the movement kept by the backend;
    // what went wrong said in the form, which stays open.
    function save() {
        // "−1 234,50 €" as typed; what is no number goes as 0, which the core
        // answers in words (a NaN would reach it as null, and a parser's error).
        const typed = Number(amount.text.replace(/[\s€]/g, "").replace(",", ".").replace("−", "-"))
        const edit = {
            kind: dialog.recurring ? "recurring" : "once",
            budget: dialog.budget,
            label: label.text,
            amount: Number.isNaN(typed) ? 0 : typed,
            date: day.date,
            every: every.currentIndex === 1 ? "year" : "month",
            day: dayOfMonth.value,
            month: every.currentIndex === 1 ? new Date(day.date + "T12:00:00").getMonth() + 1 : null,
            from: dialog.recurring ? day.date : "",
            estimate: estimate.checked
        }
        const problem = dialog.sioul.addMovement(JSON.stringify(edit))
        if (problem !== "") {
            dialog.problem = problem
            return
        }
        dialog.close()
        dialog.saved()
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    width: Math.min(460, (parent ? parent.width : 460) - 2 * dialog.theme.gap)
    title: dialog.sioul.text("budget-add")

    contentItem: GridLayout {
        columns: 2
        columnSpacing: 10
        rowSpacing: 8

        Label {
            visible: dialog.choosing
            text: dialog.sioul.text("budget-add-budget")
            color: dialog.theme.muted
        }
        PlainComboBox {
            id: budgetChoice

            visible: dialog.choosing
            Layout.fillWidth: true
            model: dialog.budgets.map(b => dialog.theme.plain(b.title))
            onActivated: index => dialog.budget = dialog.budgets[index].id
        }
        Item {
            Layout.preferredHeight: 1
        }
        RowLayout {
            // A click on the choice shown unticks it: its binding ticks it again.
            Button {
                id: once

                text: dialog.sioul.text("budget-add-once")
                checkable: true
                checked: !dialog.recurring
                flat: dialog.recurring
                onClicked: {
                    dialog.recurring = false
                    once.checked = Qt.binding(() => !dialog.recurring)
                }
            }
            Button {
                id: recurringButton

                text: dialog.sioul.text("budget-add-recurring")
                checkable: true
                checked: dialog.recurring
                flat: !dialog.recurring
                onClicked: {
                    dialog.recurring = true
                    recurringButton.checked = Qt.binding(() => dialog.recurring)
                }
            }
        }
        Label {
            text: dialog.sioul.text("budget-add-label")
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
            text: dialog.recurring ? dialog.sioul.text("budget-add-from") : dialog.sioul.text("budget-add-date")
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
            visible: dialog.recurring
            text: dialog.sioul.text("budget-add-every")
            color: dialog.theme.muted
        }
        RowLayout {
            visible: dialog.recurring

            PlainComboBox {
                id: every

                Layout.preferredWidth: 190
                model: [dialog.sioul.text("budget-add-every-month"), dialog.sioul.text("budget-add-every-year")]
            }
            SpinBox {
                id: dayOfMonth

                from: 1
                to: 31
                editable: true
            }
        }
        Item {
            visible: dialog.recurring
            Layout.preferredHeight: 1
        }
        CheckBox {
            id: estimate

            visible: dialog.recurring
            text: dialog.sioul.text("budget-add-estimate")
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
