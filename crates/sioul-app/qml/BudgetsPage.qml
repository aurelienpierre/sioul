// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Budgets and reserves at a glance, itemized: each figure on its own line, the
// verdict apart with its icon. Then the mail about money, newest first: what is
// counted, what is the same payment told twice, and what waits for you.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Item {
    id: page

    required property var sioul
    required property var theme
    required property var window

    readonly property var view: page.sioul.budgets ? JSON.parse(page.sioul.budgets) : null
    // A budget opened: its ledger and its balance.
    property string openId: ""

    // A movement added by hand, from "New".
    function startNew() {
        newMovement.choose()
    }

    property alias contracts: contractsSection
    property alias bank: bankSection

    // The bank accounts in view: for the window's images.
    function showBank() {
        const flick = scroll.contentItem as Flickable
        if (flick)
            flick.contentY = Math.max(0, Math.min(bankSection.y - 8, flick.contentHeight - flick.height))
    }

    // A contract shown, or started from a mail.
    function openContract(id) {
        contractsSection.open(id)
    }

    function startContract(prefill) {
        contractsSection.start(prefill)
    }

    function openFirst() {
        if (page.view && page.view.budgets.length > 0)
            page.openId = page.view.budgets[0].id
    }

    function moodColor(mood) {
        return mood === "short" ? theme.warm : mood === "reserve" ? theme.muted : theme.accent
    }

    function moodIcon(mood) {
        return { "better": "go-up", "as-planned": "dialog-ok-apply", "short": "go-down" }[mood] || "view-financial-budget"
    }

    function kindIcon(kind) {
        return { "received": "go-down", "refund": "go-down", "paid": "go-up", "order": "go-up", "bill": "x-office-document" }[kind] || "view-financial-budget"
    }

    ScrollView {
        id: opened

        visible: page.openId !== ""
        anchors.fill: parent
        anchors.margins: page.theme.gap
        contentWidth: availableWidth

        BudgetDetail {
            width: opened.availableWidth
            sioul: page.sioul
            theme: page.theme
            window: page.window
            budget: page.openId
            onBack: page.openId = ""
        }
    }

    ScrollView {
        id: scroll

        visible: page.openId === ""
        anchors.fill: parent
        anchors.margins: page.theme.gap
        contentWidth: availableWidth

        ColumnLayout {
            width: scroll.availableWidth
            spacing: page.theme.gap

            RowLayout {
                Layout.fillWidth: true

                Label {
                    Layout.fillWidth: true
                    text: page.view ? page.view.title : page.sioul.text("ui-budgets")
                    textFormat: Text.PlainText
                    font.pixelSize: 22
                    color: page.theme.text
                }
                Button {
                    text: page.sioul.text("budget-new")
                    icon.name: "list-add"
                    icon.color: page.theme.text
                    onClicked: newBudget.edit("", null)
                }
            }
            Label {
                visible: page.view === null
                Layout.fillWidth: true
                text: page.sioul.text("ui-no-budgets")
                wrapMode: Text.Wrap
                color: page.theme.muted
            }

            // The budgets.
            Repeater {
                model: page.view ? page.view.budgets : []

                delegate: Panel {
                    id: budgetCard

                    required property var modelData

                    Layout.fillWidth: true
                    theme: page.theme

                    ColumnLayout {
                        anchors.fill: parent
                        spacing: 10

                        RowLayout {
                            spacing: 8

                            Icon {
                                iconName: "view-financial-budget"
                            }
                            Label {
                                text: budgetCard.modelData.title
                                textFormat: Text.PlainText
                                font.pixelSize: 17
                                font.weight: Font.DemiBold
                                color: page.theme.text
                            }
                            Label {
                                Layout.fillWidth: true
                                text: budgetCard.modelData.period
                                textFormat: Text.PlainText
                                color: page.theme.muted
                            }
                            // Its ledger and its balance.
                            Button {
                                flat: true
                                text: page.sioul.text("ui-open")
                                icon.name: "view-financial-account"
                                icon.color: page.theme.text
                                onClicked: page.openId = budgetCard.modelData.id
                            }
                        }

                        // The verdict, apart.
                        Rectangle {
                            Layout.preferredWidth: verdictRow.implicitWidth + 20
                            Layout.preferredHeight: verdictRow.implicitHeight + 10
                            radius: page.theme.radius
                            color: "transparent"
                            border.color: page.moodColor(budgetCard.modelData.mood)

                            RowLayout {
                                id: verdictRow

                                anchors.centerIn: parent
                                spacing: 6

                                Icon {
                                    iconName: page.moodIcon(budgetCard.modelData.mood)
                                    size: 16
                                }
                                Label {
                                    text: budgetCard.modelData.verdict
                                    textFormat: Text.PlainText
                                    color: page.moodColor(budgetCard.modelData.mood)
                                    font.weight: Font.DemiBold
                                }
                            }
                        }

                        Label {
                            visible: budgetCard.modelData.pace !== ""
                            Layout.fillWidth: true
                            text: budgetCard.modelData.pace
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            color: page.theme.text
                        }

                        GridLayout {
                            Layout.fillWidth: true
                            columns: 2
                            columnSpacing: page.theme.gap
                            rowSpacing: 4

                            Repeater {
                                model: budgetCard.modelData.figures.reduce((all, f) => all.concat([f.label, f.value]), [])

                                delegate: Label {
                                    id: cell

                                    required property string modelData
                                    required property int index

                                    Layout.fillWidth: cell.index % 2 === 0
                                    Layout.alignment: cell.index % 2 === 0 ? Qt.AlignLeft : Qt.AlignRight
                                    text: cell.modelData
                                    textFormat: Text.PlainText
                                    color: cell.index % 2 === 0 ? page.theme.muted : page.theme.text
                                    font.features: { "tnum": 1 }
                                }
                            }
                        }

                        Repeater {
                            model: budgetCard.modelData.notes

                            delegate: RowLayout {
                                id: note

                                required property string modelData

                                Layout.fillWidth: true
                                spacing: 8

                                Icon {
                                    iconName: "view-financial-account-savings"
                                    size: 16
                                }
                                Label {
                                    Layout.fillWidth: true
                                    text: note.modelData
                                    textFormat: Text.PlainText
                                    wrapMode: Text.Wrap
                                    color: page.theme.muted
                                }
                            }
                        }
                    }
                }
            }

            // The reserves: savings accounts, made and changed here.
            RowLayout {
                visible: page.view !== null
                Layout.fillWidth: true
                Layout.topMargin: page.theme.gap
                spacing: 8

                Label {
                    Layout.fillWidth: true
                    text: page.view ? page.view.reserves_title : ""
                    textFormat: Text.PlainText
                    font.pixelSize: 19
                    color: page.theme.text
                }
                Button {
                    text: page.sioul.text("reserve-new")
                    icon.name: "list-add"
                    onClicked: bankSection.reserveDialog.edit(null)
                }
            }
            Repeater {
                model: page.view ? page.view.reserves : []

                delegate: Panel {
                    id: reserveCard

                    required property var modelData

                    Layout.fillWidth: true
                    theme: page.theme

                    ColumnLayout {
                        anchors.fill: parent
                        spacing: 10

                        RowLayout {
                            spacing: 8

                            Icon {
                                iconName: "view-financial-account-savings"
                            }
                            Label {
                                Layout.fillWidth: true
                                text: reserveCard.modelData.title
                                textFormat: Text.PlainText
                                font.pixelSize: 17
                                font.weight: Font.DemiBold
                                wrapMode: Text.Wrap
                                color: page.theme.text
                            }
                            Button {
                                flat: true
                                text: page.sioul.text("ui-edit")
                                onClicked: bankSection.reserveDialog.edit(bankSection.shown.reserves.find(r => r.id === reserveCard.modelData.id) || null)
                            }
                        }
                        GridLayout {
                            Layout.fillWidth: true
                            columns: 2
                            columnSpacing: page.theme.gap
                            rowSpacing: 4

                            Repeater {
                                model: reserveCard.modelData.figures.reduce((all, f) => all.concat([f.label, f.value]), [])

                                delegate: Label {
                                    id: reserveCell

                                    required property string modelData
                                    required property int index

                                    Layout.fillWidth: reserveCell.index % 2 === 0
                                    Layout.alignment: reserveCell.index % 2 === 0 ? Qt.AlignLeft : Qt.AlignRight
                                    text: reserveCell.modelData
                                    textFormat: Text.PlainText
                                    color: reserveCell.index % 2 === 0 ? page.theme.muted : page.theme.text
                                    font.features: { "tnum": 1 }
                                }
                            }
                        }
                    }
                }
            }

            // The bank: what passed, what did not, the month ahead.
            BankSection {
                id: bankSection

                Layout.fillWidth: true
                sioul: page.sioul
                theme: page.theme
            }

            // What you are bound to: renewals, notices, how to stop each.
            ContractsSection {
                id: contractsSection

                Layout.fillWidth: true
                sioul: page.sioul
                theme: page.theme
                window: page.window
            }

            // The mail about money.
            Label {
                visible: page.view !== null
                Layout.topMargin: page.theme.gap
                text: page.view ? page.view.mail_title : ""
                textFormat: Text.PlainText
                font.pixelSize: 19
                color: page.theme.text
            }
            Label {
                visible: page.view !== null && page.view.mail.length === 0
                text: page.sioul.text("mail-none")
                color: page.theme.muted
            }
            Repeater {
                model: page.view ? page.view.mail : []

                delegate: Panel {
                    id: mailRow

                    required property var modelData
                    readonly property bool waiting: mailRow.modelData.state === "proposed"

                    Layout.fillWidth: true
                    theme: page.theme
                    opacity: mailRow.modelData.state === "duplicate" ? 0.7 : 1

                    ColumnLayout {
                        anchors.fill: parent
                        spacing: 6

                        RowLayout {
                            spacing: 8

                            Icon {
                                iconName: page.kindIcon(mailRow.modelData.kind)
                                size: 16
                            }
                            Label {
                                text: mailRow.modelData.kind_label
                                textFormat: Text.PlainText
                                color: page.theme.muted
                            }
                            Label {
                                Layout.fillWidth: true
                                text: mailRow.modelData.party
                                textFormat: Text.PlainText
                                font.weight: Font.DemiBold
                                elide: Text.ElideRight
                                color: page.theme.text
                            }
                            Label {
                                text: mailRow.modelData.amount
                                textFormat: Text.PlainText
                                font.weight: Font.DemiBold
                                font.features: { "tnum": 1 }
                                color: mailRow.modelData.credit ? page.theme.accent : page.theme.text
                            }
                        }
                        Label {
                            Layout.fillWidth: true
                            text: mailRow.modelData.date + "  ·  " + mailRow.modelData.subject
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            color: page.theme.muted
                            font.pixelSize: 13
                        }
                        RowLayout {
                            visible: mailRow.modelData.note !== ""
                            Layout.fillWidth: true
                            spacing: 8

                            Icon {
                                iconName: mailRow.modelData.state === "recorded" ? "dialog-ok-apply" : mailRow.modelData.state === "duplicate" ? "edit-copy" : "view-financial-budget"
                                size: 16
                            }
                            Label {
                                Layout.fillWidth: true
                                text: mailRow.modelData.note
                                textFormat: Text.PlainText
                                wrapMode: Text.Wrap
                                color: page.theme.muted
                            }
                        }
                        RowLayout {
                            visible: mailRow.modelData.doubtful && mailRow.waiting
                            Layout.fillWidth: true
                            spacing: 8

                            Icon {
                                iconName: "dialog-warning"
                                size: 16
                            }
                            Label {
                                Layout.fillWidth: true
                                text: page.sioul.text("note-doubtful")
                                wrapMode: Text.Wrap
                                color: page.theme.warm
                            }
                        }

                        // What waits for you: which budget, then add it, or not.
                        RowLayout {
                            visible: mailRow.waiting
                            Layout.fillWidth: true
                            spacing: page.theme.gap

                            ComboBox {
                                id: budgetChoice

                                visible: mailRow.modelData.can_add
                                Layout.preferredWidth: 260
                                model: page.view ? page.view.choices.map(c => page.theme.plain(c.title)) : []
                                currentIndex: page.view ? page.view.choices.findIndex(c => c.id === mailRow.modelData.budget) : -1
                                displayText: currentIndex < 0 ? "…" : currentText
                            }
                            Button {
                                visible: mailRow.modelData.can_add
                                enabled: budgetChoice.currentIndex >= 0
                                text: page.sioul.text("ui-add-line")
                                onClicked: page.sioul.addMailLine(mailRow.modelData.key, page.view.choices[budgetChoice.currentIndex].id)
                            }
                            Item {
                                Layout.fillWidth: true
                            }
                            Button {
                                flat: true
                                text: page.sioul.text("ui-not-payment")
                                onClicked: page.sioul.ignoreMailLine(mailRow.modelData.key)
                            }
                        }
                    }
                }
            }

            Item {
                Layout.preferredHeight: page.theme.gap
            }
        }
    }

    BudgetDialog {
        id: newBudget

        sioul: page.sioul
        theme: page.theme
        onSaved: id => page.openId = id
    }

    // A movement from "New", its budget chosen in it.
    MovementDialog {
        id: newMovement

        sioul: page.sioul
        theme: page.theme
        budgets: page.view ? page.view.choices : []
    }
}
