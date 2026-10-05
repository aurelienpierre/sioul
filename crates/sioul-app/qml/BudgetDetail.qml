// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A budget opened: how its period goes at its pace, its balance as a line
// (days, weeks, months or years; what is planned dotted), and its ledger:
// every movement of the period, newest first. Movements are added by hand
// here, once or recurring.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: detail

    required property var sioul
    required property var theme
    required property var window
    property string budget: ""
    property string step: "day"
    property date anchor: new Date()
    property var shown: null
    readonly property var steps: ["day", "week", "month", "year"]
    // A narrow screen (a phone): the title under the arrows, each date under its line.
    readonly property bool narrow: detail.width < 560
    readonly property string inOut: detail.shown ? detail.sioul.text("budget-in") + " " + detail.shown.money_in + "   " + detail.sioul.text("budget-out") + " " + detail.shown.money_out : ""

    signal back

    function iso(d) {
        const pad = n => n < 10 ? "0" + n : String(n)
        return d.getFullYear() + "-" + pad(d.getMonth() + 1) + "-" + pad(d.getDate())
    }

    function reload() {
        detail.shown = detail.budget !== "" ? JSON.parse(detail.sioul.budgetDetail(detail.budget, detail.iso(detail.anchor), detail.step) || "null") : null
    }

    // A month or a year away, on the same day or on that month's last: from
    // 31 October, back is 30 September, never 1 October again.
    function move(steps) {
        const a = detail.anchor
        const months = detail.step === "year" || detail.step === "month" ? 12 * steps : steps
        const last = new Date(a.getFullYear(), a.getMonth() + months + 1, 0).getDate()
        detail.anchor = new Date(a.getFullYear(), a.getMonth() + months, Math.min(a.getDate(), last))
        detail.reload()
    }

    spacing: 12
    onBudgetChanged: detail.reload()
    onStepChanged: detail.reload()

    Connections {
        target: detail.sioul

        function onBudgetsChanged() {
            detail.reload()
        }
    }

    RowLayout {
        Layout.fillWidth: true
        spacing: 6

        Button {
            flat: true
            text: "◂  " + detail.sioul.text("budget-back")
            onClicked: detail.back()
        }
        Label {
            visible: !detail.narrow
            Layout.fillWidth: true
            text: detail.shown ? detail.shown.card.title + "  ·  " + detail.shown.period_title : ""
            textFormat: Text.PlainText
            font.pixelSize: 20
            elide: Text.ElideRight
            color: detail.theme.text
        }
        Item {
            visible: detail.narrow
            Layout.fillWidth: true
        }
        Button {
            flat: true
            // The style's buttons are 100 pixels at least, their icon alone too.
            Layout.preferredWidth: detail.narrow ? 40 : -1
            text: detail.sioul.text("ui-edit")
            icon.name: "document-edit"
            icon.color: detail.theme.text
            display: detail.narrow ? AbstractButton.IconOnly : AbstractButton.TextBesideIcon
            Accessible.name: detail.sioul.text("ui-edit")
            onClicked: budgetDialog.edit(detail.budget, detail.shown)
        }
        Button {
            implicitWidth: 40
            text: "◂"
            Accessible.name: detail.sioul.text("agenda-earlier")
            onClicked: detail.move(-1)
        }
        Button {
            text: detail.sioul.text("agenda-today")
            onClicked: {
                detail.anchor = new Date()
                detail.reload()
            }
        }
        Button {
            implicitWidth: 40
            text: "▸"
            Accessible.name: detail.sioul.text("agenda-later")
            onClicked: detail.move(1)
        }
    }

    Label {
        visible: detail.narrow
        Layout.fillWidth: true
        text: detail.shown ? detail.shown.card.title + "  ·  " + detail.shown.period_title : ""
        textFormat: Text.PlainText
        font.pixelSize: 20
        wrapMode: Text.Wrap
        color: detail.theme.text
    }

    // How the period goes, at its pace.
    Label {
        visible: detail.shown !== null && detail.shown.card.verdict !== ""
        Layout.fillWidth: true
        text: detail.shown ? detail.shown.card.verdict : ""
        textFormat: Text.PlainText
        font.weight: Font.DemiBold
        color: detail.shown && detail.shown.card.mood === "short" ? detail.theme.warm : detail.theme.accent
    }
    Label {
        visible: detail.shown !== null && detail.shown.card.pace !== ""
        Layout.fillWidth: true
        text: detail.shown ? detail.shown.card.pace : ""
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: detail.theme.text
    }
    // What it is for, which says the hours it is in view; changed in its form ("Edit").
    Label {
        visible: detail.shown !== null
        Layout.fillWidth: true
        text: detail.shown === null ? "" : detail.sioul.textWith("budget-area-line", "areas", detail.shown.area === "" ? detail.sioul.text("area-admin") : detail.shown.area.split("+").map(a => detail.sioul.text("area-" + a)).join(", "))
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.pixelSize: 13
        color: detail.theme.muted
    }
    GridLayout {
        Layout.fillWidth: true
        columns: 2
        columnSpacing: detail.theme.gap
        rowSpacing: 4

        Repeater {
            model: detail.shown ? detail.shown.card.figures.reduce((all, f) => all.concat([f.label, f.value]), []) : []

            delegate: Label {
                id: cell

                required property string modelData
                required property int index

                Layout.fillWidth: cell.index % 2 === 0
                Layout.alignment: cell.index % 2 === 0 ? Qt.AlignLeft : Qt.AlignRight
                text: cell.modelData
                textFormat: Text.PlainText
                color: cell.index % 2 === 0 ? detail.theme.muted : detail.theme.text
                font.features: { "tnum": 1 }
            }
        }
    }

    // The balance.
    Flow {
        Layout.fillWidth: true
        spacing: 4

        Repeater {
            model: detail.steps

            delegate: Button {
                id: stepButton

                required property string modelData

                text: detail.sioul.text("budget-step-" + stepButton.modelData)
                checkable: true
                checked: detail.step === stepButton.modelData
                flat: detail.step !== stepButton.modelData
                // A click on the step shown unticks it: its binding ticks it again.
                onClicked: {
                    detail.step = stepButton.modelData
                    stepButton.checked = Qt.binding(() => detail.step === stepButton.modelData)
                }
            }
        }
    }
    BalanceChart {
        Layout.fillWidth: true
        Layout.preferredHeight: 200
        theme: detail.theme
        points: detail.shown ? detail.shown.points : []
        low: detail.shown ? detail.shown.low_cents : 0
        high: detail.shown ? detail.shown.high_cents : 0
        target: detail.shown ? detail.shown.target_cents : 0
    }

    // The ledger.
    RowLayout {
        Layout.fillWidth: true
        Layout.topMargin: 6

        Label {
            Layout.fillWidth: true
            text: detail.sioul.text("budget-movements")
            font.weight: Font.DemiBold
            color: detail.theme.text
        }
        Label {
            visible: !detail.narrow
            text: detail.inOut
            textFormat: Text.PlainText
            color: detail.theme.muted
            font.features: { "tnum": 1 }
        }
        Button {
            text: detail.sioul.text("budget-add")
            icon.name: "list-add"
            icon.color: detail.theme.text
            onClicked: movementDialog.begin(detail.budget)
        }
    }
    Label {
        visible: detail.narrow
        Layout.fillWidth: true
        text: detail.inOut
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: detail.theme.muted
        font.features: { "tnum": 1 }
    }
    Repeater {
        model: detail.shown ? detail.shown.movements : []

        delegate: RowLayout {
            id: move

            required property var modelData

            Layout.fillWidth: true
            spacing: 10
            opacity: move.modelData.planned ? 0.75 : 1

            // Right click on a line of the file: tie it to something new or something that exists.
            TapHandler {
                acceptedButtons: Qt.RightButton
                enabled: move.modelData.uri !== ""
                onTapped: {
                    lineMenu.source = { uri: move.modelData.uri, kind: "budget", key: move.modelData.uri, title: move.modelData.label }
                    lineMenu.line = move.modelData
                    lineMenu.popup()
                }
            }

            Label {
                visible: !detail.narrow
                Layout.preferredWidth: 170
                text: move.modelData.date
                textFormat: Text.PlainText
                elide: Text.ElideRight
                font.pixelSize: 13
                color: detail.theme.muted
            }
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 0

                Label {
                    Layout.fillWidth: true
                    text: move.modelData.label + (move.modelData.recurring ? "  ·  " + detail.sioul.text("budget-recurring") : "") + (move.modelData.planned ? "  ·  " + detail.sioul.text("budget-planned") : "")
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    color: detail.theme.text
                }
                Label {
                    visible: detail.narrow
                    Layout.fillWidth: true
                    text: move.modelData.date
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    font.pixelSize: 12
                    color: detail.theme.muted
                }
            }
            Label {
                text: move.modelData.amount
                textFormat: Text.PlainText
                color: move.modelData.incoming ? detail.theme.accent : detail.theme.text
                font.features: { "tnum": 1 }
            }
        }
    }

    SioulMenu {
        id: lineMenu

        property var source: null
        property var line: null

        AddMenu {
            sioul: detail.sioul
            window: detail.window
            source: lineMenu.source
        }
        MenuItem {
            text: detail.sioul.text("ui-link-existing")
            onTriggered: detail.window.linkFrom(lineMenu.source)
        }
        MenuSeparator {}
        MenuItem {
            text: detail.sioul.text("budget-line-change")
            onTriggered: lineChange.begin(lineMenu.line)
        }
        MenuItem {
            text: detail.sioul.text("budget-line-remove")
            onTriggered: lineRemove.ask(lineMenu.source.title, detail.sioul.text("budget-line-remove-ask"), detail.sioul.text("budget-line-remove"))
        }
    }

    // A line changed in place: its label, amount and date; its links stay.
    LineDialog {
        id: lineChange

        sioul: detail.sioul
        theme: detail.theme
        window: detail.window
        onChanged: detail.reload()
    }

    // A line taken out of the file: asked once.
    ConfirmDialog {
        id: lineRemove

        sioul: detail.sioul
        theme: detail.theme
        onConfirmed: {
            const problem = detail.sioul.removeBudget(lineMenu.source.uri)
            if (problem !== "")
                detail.sioul.status = problem
            detail.reload()
        }
    }

    BudgetDialog {
        id: budgetDialog

        sioul: detail.sioul
        theme: detail.theme
        onSaved: detail.reload()
        onRemoved: detail.back()
    }

    MovementDialog {
        id: movementDialog

        sioul: detail.sioul
        theme: detail.theme
        onSaved: detail.reload()
    }
}
