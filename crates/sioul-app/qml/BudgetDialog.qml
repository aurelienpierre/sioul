// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A budget: its name, its period, the balance to reach by the end of each
// period, and what it is for: work, your admin, leisure, any of them together
// (docs/areas.md), which says the hours it is in view. Taking one out keeps
// its lines in the file.

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
        const edit = { title: title.text, period: dialog.periods[period.currentIndex], target: goal, area: dialog.area }
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
    width: Math.min(460, (parent ? parent.width : 460) - 2 * dialog.theme.gap)
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
