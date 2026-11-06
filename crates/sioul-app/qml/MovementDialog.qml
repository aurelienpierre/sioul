// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A movement made by hand (docs/accounting.md, "Movements tab"; the money
// ledger's design §3.1): once (a day), or recurring (each month or year,
// on a day). Positive is money in, negative money out. Once, it can say
// what was paid and its fees (what arrived is what was paid less them), its
// account, the day it was paid at its processor, whether it is still to
// come, and a note: under "More…", so that the form stays short. The same
// form changes a movement, showing only the fields its kind allows (a bank
// line's amount and day are the bank's), and says why the others are not.

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
    // The budget it goes into, by its id.
    property string budget: ""
    // Started from "New": the budget is chosen here, among {id, title}.
    property var budgets: []
    // The bank accounts it may be on, among {id, title}; none: the one that fills its budget.
    property var accounts: []
    // The budget is chosen in the form (started from "New", or changing a movement).
    property bool choosing: false
    // Each month or year rather than once.
    property bool recurring: false
    // What went wrong when saving, in words; "" for nothing.
    property string problem: ""
    // Changing a movement: its row as the Movements tab gave it ({address, allowed, data, …}); null when making one.
    property var editing: null
    // The fields shown when changing: those its kind allows.
    readonly property var fields: dialog.editing ? dialog.editing.allowed.fields : ["label", "gross", "fees", "net", "day", "paid_on", "note", "budget", "planned", "account"]
    // What was paid, the fees, the account, the day paid, the note: shown.
    property bool more: false
    // The values the form opened with, to send only what changed.
    property var started: ({})
    readonly property var feeKinds: ["processor", "commission", "conversion", "contributions", "bank"]

    // The movement was added.
    signal saved
    // A movement was made or changed: the act's answer ({said} or {problem}).
    signal changed(string text)

    function has(field) {
        return dialog.fields.indexOf(field) >= 0
    }

    // Today, as 2026-10-05.
    function today() {
        const d = new Date()
        const pad = n => n < 10 ? "0" + n : String(n)
        return d.getFullYear() + "-" + pad(d.getMonth() + 1) + "-" + pad(d.getDate())
    }

    // "−1 234,50 €" as typed; NaN for nothing or what is no number.
    function parse(text) {
        const plain = text.replace(/[\s€  ]/g, "").replace(",", ".").replace("−", "-")
        return plain === "" ? NaN : Number(plain)
    }

    // An amount as the form shows it: "52.3", "-12".
    function shown(value) {
        return value === null || value === undefined ? "" : String(value)
    }

    // From "New": which budget first.
    function choose() {
        dialog.begin(dialog.budgets.length > 0 ? dialog.budgets[0].id : "")
        dialog.choosing = true
        budgetChoice.currentIndex = 0
    }

    // The form emptied and opened for `budget`.
    function begin(budget) {
        dialog.editing = null
        dialog.choosing = false
        dialog.budget = budget
        dialog.recurring = false
        dialog.problem = ""
        dialog.more = false
        label.text = ""
        amount.text = ""
        gross.text = ""
        fee.text = ""
        feeKind.currentIndex = 0
        accountChoice.currentIndex = 0
        paidElsewhere.checked = false
        paidOn.date = dialog.today()
        planned.checked = false
        note.text = ""
        day.date = dialog.today()
        dayOfMonth.value = new Date().getDate()
        every.currentIndex = 0
        estimate.checked = false
        byHand.checked = false
        dialog.open()
        label.forceActiveFocus()
    }

    // A movement changed: its row from the Movements tab, its fields filled from its data.
    function change(row) {
        dialog.begin(row.budget_id || "")
        dialog.editing = row
        dialog.choosing = dialog.has("budget")
        const data = row.data || {}
        const kept = (data.fees || []).filter(f => !f.about)
        label.text = data.who || ""
        amount.text = dialog.shown(data.amount)
        gross.text = data.gross !== null && data.gross !== undefined ? dialog.shown(Math.abs(data.gross)) : ""
        fee.text = kept.length > 0 ? dialog.shown(Math.round(kept.reduce((all, f) => all + f.amount, 0) * 100) / 100) : ""
        feeKind.currentIndex = Math.max(0, kept.length > 0 ? dialog.feeKinds.indexOf(kept[0].kind) : 0)
        accountChoice.currentIndex = Math.max(0, dialog.accounts.findIndex(a => a.id === data.account) + 1)
        day.date = data.day || dialog.today()
        paidElsewhere.checked = !!data.paid_on
        paidOn.date = data.paid_on || day.date
        planned.checked = data.state === "planned" || data.state === "announced"
        note.text = data.note || ""
        budgetChoice.currentIndex = Math.max(0, dialog.budgets.findIndex(b => b.id === dialog.budget))
        dialog.more = gross.text !== "" || fee.text !== "" || note.text !== "" || paidElsewhere.checked
        dialog.started = dialog.values()
    }

    // What the form says now, as the core reads it.
    function values() {
        const net = dialog.parse(amount.text)
        const paid = dialog.parse(gross.text)
        const cost = dialog.parse(fee.text)
        // What was paid goes the way the amount goes: typed without its sign.
        const sign = !Number.isNaN(net) && net < 0 ? -1 : 1
        return {
            label: label.text.trim(),
            net: Number.isNaN(net) ? null : net,
            gross: Number.isNaN(paid) ? null : (Number.isNaN(net) ? paid : sign * Math.abs(paid)),
            fees: Number.isNaN(cost) || cost === 0 ? [] : [{ kind: dialog.feeKinds[feeKind.currentIndex], amount: Math.abs(cost) }],
            account: accountChoice.currentIndex > 0 ? dialog.accounts[accountChoice.currentIndex - 1].id : "",
            budget: dialog.budget,
            day: day.date,
            paid_on: paidElsewhere.checked ? paidOn.date : "",
            planned: planned.checked,
            note: note.text.trim()
        }
    }

    // The form saved: made or changed by the backend, which says it with
    // Undo; what went wrong said in the form, which stays open.
    function save() {
        if (dialog.editing !== null) {
            const now = dialog.values()
            const edit = {}
            for (const field of dialog.fields)
                if (field !== "net" && field !== "fees" && JSON.stringify(now[field]) !== JSON.stringify(dialog.started[field]))
                    edit[field] = now[field]
            if (dialog.has("net") && now.net !== dialog.started.net)
                edit.net = now.net
            if (dialog.has("fees") && JSON.stringify(now.fees) !== JSON.stringify(dialog.started.fees))
                edit.fees = now.fees
            const answer = dialog.sioul.changeMovement(dialog.editing.address, JSON.stringify(edit))
            if (JSON.parse(answer).problem) {
                dialog.problem = JSON.parse(answer).problem
                return
            }
            dialog.close()
            dialog.changed(answer)
            return
        }
        if (!dialog.recurring) {
            const answer = dialog.sioul.makeMovement(JSON.stringify(dialog.values()))
            if (JSON.parse(answer).problem) {
                dialog.problem = JSON.parse(answer).problem
                return
            }
            dialog.close()
            dialog.saved()
            dialog.changed(answer)
            return
        }
        // Recurring: a recurring payment of the budget file. What is no number
        // goes as 0, which the core answers in words.
        const typed = dialog.parse(amount.text)
        const edit = {
            kind: "recurring",
            budget: dialog.budget,
            label: label.text,
            amount: Number.isNaN(typed) ? 0 : typed,
            date: day.date,
            every: every.currentIndex === 1 ? "year" : "month",
            day: dayOfMonth.value,
            month: every.currentIndex === 1 ? new Date(day.date + "T12:00:00").getMonth() + 1 : null,
            from: day.date,
            estimate: estimate.checked,
            by_hand: !estimate.checked && byHand.checked
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
    width: Math.min(480, (parent ? dialog.roomWidth : 480) - 2 * dialog.theme.gap)
    title: dialog.editing ? dialog.theme.plain(dialog.editing.who + "  ·  " + dialog.editing.amount) : dialog.sioul.text("budget-add")

    contentItem: GridLayout {
        columns: 2
        columnSpacing: 10
        rowSpacing: 8

        // Changing: what its kind keeps as it is, said once.
        Label {
            objectName: "movementFormSays"
            visible: dialog.editing !== null && !!dialog.editing.says
            Layout.columnSpan: 2
            Layout.fillWidth: true
            text: dialog.editing ? dialog.editing.says || "" : ""
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: dialog.theme.muted
        }
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
            visible: dialog.editing === null
            Layout.preferredHeight: 1
        }
        RowLayout {
            visible: dialog.editing === null

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
            visible: dialog.has("label")
            text: dialog.sioul.text("budget-add-label")
            color: dialog.theme.muted
        }
        TextField {
            id: label

            visible: dialog.has("label")
            Layout.fillWidth: true
        }
        Label {
            visible: dialog.has("net")
            text: dialog.sioul.text("budget-add-amount")
            color: dialog.theme.muted
        }
        TextField {
            id: amount

            objectName: "movementAmount"
            visible: dialog.has("net")
            Layout.fillWidth: true
            placeholderText: dialog.sioul.text("budget-add-amount-hint")
            horizontalAlignment: Text.AlignRight
            font.features: { "tnum": 1 }
            onAccepted: dialog.save()
        }
        Label {
            visible: dialog.has("day")
            text: dialog.recurring ? dialog.sioul.text("budget-add-from") : dialog.sioul.text("budget-add-date")
            textFormat: Text.PlainText
            color: dialog.theme.muted
        }
        DateField {
            id: day

            visible: dialog.has("day")
            theme: dialog.theme
            sioul: dialog.sioul
            locale: Qt.locale(dialog.sioul.text("qt-locale"))
            pickLabel: dialog.sioul.text("event-pick-day")
        }
        Item {
            visible: !dialog.recurring && dialog.has("planned")
            Layout.preferredHeight: 1
        }
        CheckBox {
            id: planned

            visible: !dialog.recurring && dialog.has("planned")
            text: dialog.sioul.text("movement-form-planned")
        }

        // Once: what was paid, the fees, the account, the day paid, a note.
        Button {
            objectName: "movementMore"
            visible: !dialog.recurring && !dialog.more && ["gross", "fees", "account", "paid_on", "note"].some(f => dialog.has(f))
            Layout.columnSpan: 2
            flat: true
            text: dialog.sioul.text("movement-form-more")
            onClicked: dialog.more = true
        }
        Label {
            visible: dialog.more && !dialog.recurring && dialog.has("gross")
            text: amount.text.trim().startsWith("-") || amount.text.trim().startsWith("−") ? dialog.sioul.text("movement-form-price") : dialog.sioul.text("movement-form-paid")
            color: dialog.theme.muted
        }
        TextField {
            id: gross

            objectName: "movementGross"
            visible: dialog.more && !dialog.recurring && dialog.has("gross")
            Layout.fillWidth: true
            placeholderText: dialog.sioul.text("movement-form-paid-hint")
            horizontalAlignment: Text.AlignRight
            font.features: { "tnum": 1 }
        }
        Label {
            visible: dialog.more && !dialog.recurring && dialog.has("fees")
            text: dialog.sioul.text("movement-form-fees")
            color: dialog.theme.muted
        }
        RowLayout {
            visible: dialog.more && !dialog.recurring && dialog.has("fees")
            Layout.fillWidth: true
            spacing: 8

            PlainComboBox {
                id: feeKind

                Layout.fillWidth: true
                model: dialog.feeKinds.map(k => dialog.sioul.text("movement-fee-kind-" + k))
            }
            TextField {
                id: fee

                objectName: "movementFee"
                Layout.preferredWidth: 90
                horizontalAlignment: Text.AlignRight
                inputMethodHints: Qt.ImhFormattedNumbersOnly
                font.features: { "tnum": 1 }
            }
        }
        Label {
            visible: dialog.more && !dialog.recurring && dialog.has("account") && dialog.accounts.length > 0
            text: dialog.sioul.text("movement-form-account")
            color: dialog.theme.muted
        }
        PlainComboBox {
            id: accountChoice

            visible: dialog.more && !dialog.recurring && dialog.has("account") && dialog.accounts.length > 0
            Layout.fillWidth: true
            model: [dialog.sioul.text("movement-form-account-budget")].concat(dialog.accounts.map(a => dialog.theme.plain(a.title)))
        }
        Item {
            visible: dialog.more && !dialog.recurring && dialog.has("paid_on")
            Layout.preferredHeight: 1
        }
        CheckBox {
            id: paidElsewhere

            visible: dialog.more && !dialog.recurring && dialog.has("paid_on")
            text: dialog.sioul.text("movement-form-paid-on")
            ToolTip.visible: hovered
            ToolTip.text: dialog.sioul.text("movement-form-paid-on-help")
            ToolTip.delay: 300
        }
        Item {
            visible: dialog.more && !dialog.recurring && paidElsewhere.checked && dialog.has("paid_on")
            Layout.preferredHeight: 1
        }
        DateField {
            id: paidOn

            visible: dialog.more && !dialog.recurring && paidElsewhere.checked && dialog.has("paid_on")
            theme: dialog.theme
            sioul: dialog.sioul
            locale: Qt.locale(dialog.sioul.text("qt-locale"))
            pickLabel: dialog.sioul.text("event-pick-day")
        }
        Label {
            visible: dialog.more && !dialog.recurring && dialog.has("note")
            text: dialog.sioul.text("movement-form-note")
            color: dialog.theme.muted
        }
        TextField {
            id: note

            visible: dialog.more && !dialog.recurring && dialog.has("note")
            Layout.fillWidth: true
        }

        // Recurring: each month or year, on a day.
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
        Item {
            visible: dialog.recurring && !estimate.checked
            Layout.preferredHeight: 1
        }
        // Paid by your hand (or its invoice sent): one reminder a few days before.
        CheckBox {
            id: byHand

            visible: dialog.recurring && !estimate.checked
            text: amount.text.trim().startsWith("-") || amount.text.trim().startsWith("−") ? dialog.sioul.text("movement-by-hand-out") : dialog.sioul.text("movement-by-hand-in")
            ToolTip.visible: hovered
            ToolTip.text: dialog.sioul.text("movement-by-hand-help")
            ToolTip.delay: 300
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
