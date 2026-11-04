// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A budget: its name, its period and the day it starts (the 1st, or an
// income's day: from one pay to the next), the balance to reach by the end
// of each period, what it is for: work, your admin, leisure, any of them
// together (docs/areas.md), which says the hours it is in view; for work, the
// share of each income set aside for taxes and how often it is declared.
// Taking one out keeps its lines in the file.

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
    // The budget changed, by its id; "" for a new one.
    property string budgetId: ""
    // What went wrong when saving, in words; "" for nothing.
    property string problem: ""
    // "Take it out" pressed once: the next press takes it out.
    property bool removing: false
    // The periods offered, in their order.
    readonly property var periods: ["month", "year"]
    // "work", "admin+leisure"…; "" when none is ticked: your admin.
    property string area: ""
    // The incomes a period may start with: [{id, title}] (`view::incomes`), for a new budget the page's.
    property var incomes: []
    // The first choice is the calendar month.
    readonly property var startChoices: [{ id: "", title: dialog.sioul.text("budget-starts-first") }].concat(dialog.incomes)

    // Saved: the budget's id.
    signal saved(string id)
    // The budget was taken out.
    signal removed

    // `shown` is the budget's page, or null for a new one.
    function edit(id, shown) {
        dialog.budgetId = id
        dialog.problem = ""
        dialog.removing = false
        title.text = shown ? shown.card.title : ""
        period.currentIndex = shown ? Math.max(0, dialog.periods.indexOf(shown.period_kind)) : 0
        target.text = shown && shown.target_cents !== 0 ? String(shown.target_cents / 100).replace(".", Qt.locale(dialog.sioul.text("qt-locale")).decimalPoint) : ""
        dialog.area = shown ? shown.area : ""
        if (shown && shown.incomes)
            dialog.incomes = shown.incomes
        starts.currentIndex = Math.max(0, dialog.startChoices.findIndex(c => c.id === (shown ? shown.starts_with : "")))
        aside.text = shown && shown.set_aside > 0 ? String(shown.set_aside).replace(".", Qt.locale(dialog.sioul.text("qt-locale")).decimalPoint) : ""
        declare.currentIndex = shown && shown.declare === "month" ? 0 : 1
        dialog.open()
        title.forceActiveFocus()
    }

    // An amount as typed, "1 234,50 €" or "−650": its number, 0 when nothing
    // is typed, NaN when it is no number (never a silent 0).
    function amount(text) {
        const typed = text.replace(/[\s€]/g, "").replace("−", "-").replace(",", ".")
        return typed === "" ? 0 : Number(typed)
    }

    // The form saved: its amount read first (said when unreadable), then the budget
    // kept by the backend.
    function save() {
        const goal = dialog.amount(target.text)
        if (Number.isNaN(goal)) {
            dialog.problem = dialog.sioul.textWith("amount-unreadable", "text", target.text.trim())
            return
        }
        const share = dialog.amount(aside.text)
        if (Number.isNaN(share)) {
            dialog.problem = dialog.sioul.textWith("amount-unreadable", "text", aside.text.trim())
            return
        }
        const monthly = dialog.periods[period.currentIndex] === "month"
        const edit = {
            title: title.text,
            period: dialog.periods[period.currentIndex],
            target: goal,
            area: dialog.area,
            starts_with: monthly && starts.currentIndex > 0 ? dialog.startChoices[starts.currentIndex].id : "",
            set_aside: dialog.area.split("+").indexOf("work") >= 0 ? share : 0,
            declare: declare.currentIndex === 0 ? "month" : "quarter"
        }
        const answer = JSON.parse(dialog.sioul.saveBudget(dialog.budgetId, JSON.stringify(edit)))
        if (answer.error) {
            dialog.problem = answer.error
            return
        }
        dialog.close()
        dialog.saved(answer.id)
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    width: Math.min(460, (parent ? dialog.roomWidth : 460) - 2 * dialog.theme.gap)
    title: dialog.budgetId === "" ? dialog.sioul.text("budget-new") : dialog.sioul.text("budget-edit")

    contentItem: GridLayout {
        columns: 2
        columnSpacing: 10
        rowSpacing: 8

        Label {
            text: dialog.sioul.text("budget-field-title")
            color: dialog.theme.muted
        }
        TextField {
            id: title

            Layout.fillWidth: true
            onAccepted: dialog.save()
        }
        Label {
            text: dialog.sioul.text("budget-field-period")
            color: dialog.theme.muted
        }
        PlainComboBox {
            id: period

            Layout.fillWidth: true
            model: dialog.periods.map(p => dialog.sioul.text("budget-period-" + p))
        }
        // From one income to the next: a monthly budget may start on its income's day.
        Label {
            visible: period.currentIndex === 0 && dialog.incomes.length > 0
            text: dialog.sioul.text("budget-starts")
            color: dialog.theme.muted
        }
        PlainComboBox {
            id: starts

            visible: period.currentIndex === 0 && dialog.incomes.length > 0
            Layout.fillWidth: true
            model: dialog.startChoices.map(c => dialog.theme.plain(c.title))
        }
        Label {
            text: dialog.sioul.text("budget-field-target")
            color: dialog.theme.muted
        }
        TextField {
            id: target

            Layout.fillWidth: true
            placeholderText: dialog.sioul.text("budget-field-target-hint")
            inputMethodHints: Qt.ImhFormattedNumbersOnly
        }
        Label {
            Layout.alignment: Qt.AlignTop
            Layout.topMargin: 8
            text: dialog.sioul.text("budget-field-area")
            color: dialog.theme.muted
        }
        ColumnLayout {
            spacing: 0

            Flow {
                Layout.fillWidth: true
                spacing: 8

                Repeater {
                    model: ["work", "admin", "leisure"]

                    delegate: CheckBox {
                        id: areaBox

                        required property string modelData

                        text: dialog.sioul.text("area-" + areaBox.modelData)
                        checked: dialog.area.split("+").indexOf(areaBox.modelData) >= 0
                        onToggled: {
                            const was = dialog.area.split("+")
                            dialog.area = ["work", "admin", "leisure"].filter(a => a === areaBox.modelData ? areaBox.checked : was.indexOf(a) >= 0).join("+")
                        }
                    }
                }
            }
            Label {
                Layout.fillWidth: true
                text: dialog.sioul.text("budget-field-area-help")
                wrapMode: Text.Wrap
                font.pixelSize: 12
                color: dialog.theme.muted
            }
        }
        // Work money: a share of each income set aside for the declaration; off until set.
        Label {
            visible: dialog.area.split("+").indexOf("work") >= 0
            Layout.alignment: Qt.AlignTop
            Layout.topMargin: 8
            text: dialog.sioul.text("budget-set-aside")
            color: dialog.theme.muted
        }
        ColumnLayout {
            visible: dialog.area.split("+").indexOf("work") >= 0
            spacing: 4

            RowLayout {
                Layout.fillWidth: true
                spacing: 8

                TextField {
                    id: aside

                    Layout.preferredWidth: 80
                    placeholderText: "0"
                    inputMethodHints: Qt.ImhFormattedNumbersOnly
                    horizontalAlignment: Text.AlignRight
                }
                Label {
                    text: dialog.sioul.text("budget-declare")
                    color: dialog.theme.muted
                }
                PlainComboBox {
                    id: declare

                    Layout.fillWidth: true
                    model: [dialog.sioul.text("budget-declare-month"), dialog.sioul.text("budget-declare-quarter")]
                }
            }
            Label {
                Layout.fillWidth: true
                text: dialog.sioul.text("budget-set-aside-help")
                wrapMode: Text.Wrap
                font.pixelSize: 12
                color: dialog.theme.muted
            }
        }
        // Taking it out: said once, then done.
        Label {
            visible: dialog.removing
            Layout.columnSpan: 2
            Layout.fillWidth: true
            text: dialog.sioul.text("budget-remove-ask")
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
                visible: dialog.budgetId !== ""
                flat: true
                text: dialog.removing ? dialog.sioul.text("budget-remove-yes") : dialog.sioul.text("budget-remove")
                DialogButtonBox.buttonRole: DialogButtonBox.ActionRole
                onClicked: {
                    if (!dialog.removing) {
                        dialog.removing = true
                        return
                    }
                    const problem = dialog.sioul.removeBudget(dialog.budgetId)
                    if (problem !== "") {
                        dialog.problem = problem
                        return
                    }
                    dialog.close()
                    dialog.removed()
                }
            }
            onAccepted: dialog.save()
            onRejected: dialog.close()
        }
    }
}
