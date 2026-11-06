// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The Budgets page's Budgets tab (the money ledger's design §6): every
// budget, whatever the hour, under its area's heading, the hour's areas
// first, each with its one sentence, opening its detail; the reserves; the
// bank accounts, each with its balance, when its bank's file was last read,
// "Take in an export…", its rules and, for a processor, how what it
// receives reaches the bank; the contracts. Made the first time the tab
// shows.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ScrollView {
    id: tab

    required property var sioul
    required property var theme
    required property var window
    // The page's view (`view::BudgetsView`).
    property var view: null
    readonly property alias bank: bankSection
    readonly property alias contracts: contractsSection

    signal openBudget(string id)

    contentWidth: availableWidth

    function moodIcon(mood) {
        return { "better": "go-up", "as-planned": "dialog-ok-apply", "short": "go-down" }[mood] || "view-financial-budget"
    }

    // An item of the tab in view (the bank accounts, the contracts).
    function showItem(item) {
        const flick = tab.contentItem as Flickable
        if (flick && item)
            flick.contentY = Math.max(0, Math.min(item.y - 8, flick.contentHeight - flick.height))
    }

    ColumnLayout {
        width: tab.availableWidth
        spacing: tab.theme.gap

        // Every budget, under its area's heading.
        Repeater {
            model: tab.view && tab.view.groups ? tab.view.groups : []

            delegate: ColumnLayout {
                id: group

                required property var modelData

                objectName: "budgetGroup"
                Layout.fillWidth: true
                spacing: 6

                Label {
                    Layout.topMargin: 4
                    text: group.modelData.title
                    textFormat: Text.PlainText
                    font.pixelSize: 19
                    color: tab.theme.text
                }
                Repeater {
                    model: group.modelData.budgets

                    delegate: ItemDelegate {
                        id: line

                        required property var modelData

                        objectName: "budgetLine"
                        Layout.fillWidth: true
                        padding: 6
                        leftPadding: 0
                        Accessible.name: line.modelData.title + ", " + line.modelData.verdict
                        onClicked: tab.openBudget(line.modelData.id)

                        contentItem: ColumnLayout {
                            spacing: 2

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 8

                                Icon {
                                    iconName: tab.moodIcon(line.modelData.mood)
                                    size: 16
                                }
                                Label {
                                    Layout.preferredWidth: 80
                                    Layout.fillWidth: true
                                    text: line.modelData.title
                                    textFormat: Text.PlainText
                                    font.weight: Font.DemiBold
                                    elide: Text.ElideRight
                                    color: tab.theme.text
                                }
                                Label {
                                    text: line.modelData.period
                                    textFormat: Text.PlainText
                                    elide: Text.ElideRight
                                    color: tab.theme.muted
                                }
                            }
                            Label {
                                Layout.fillWidth: true
                                Layout.leftMargin: 24
                                text: line.modelData.verdict
                                textFormat: Text.PlainText
                                wrapMode: Text.Wrap
                                color: tab.theme.text
                            }
                        }
                    }
                }
            }
        }

        // The reserves: savings accounts, made and changed here.
        RowLayout {
            visible: tab.view !== null
            Layout.fillWidth: true
            Layout.topMargin: tab.theme.gap
            spacing: 8

            Label {
                Layout.fillWidth: true
                text: tab.view && tab.view.reserves_title ? tab.view.reserves_title : ""
                textFormat: Text.PlainText
                font.pixelSize: 19
                color: tab.theme.text
            }
            Button {
                text: tab.sioul.text("reserve-new")
                icon.name: "list-add"
                onClicked: bankSection.reserveDialog.now().edit(null)
            }
        }
        Repeater {
            model: tab.view ? tab.view.reserves : []

            delegate: Panel {
                id: reserveCard

                required property var modelData

                Layout.fillWidth: true
                theme: tab.theme

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
                            color: tab.theme.text
                        }
                        Button {
                            flat: true
                            text: tab.sioul.text("ui-edit")
                            onClicked: bankSection.reserveDialog.now().edit(bankSection.shown.reserves.find(r => r.id === reserveCard.modelData.id) || null)
                        }
                    }
                    GridLayout {
                        Layout.fillWidth: true
                        columns: 2
                        columnSpacing: tab.theme.gap
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
                                // A long label wraps, never under its figure (a phone's 412 points).
                                wrapMode: reserveCell.index % 2 === 0 ? Text.Wrap : Text.NoWrap
                                color: reserveCell.index % 2 === 0 ? tab.theme.muted : tab.theme.text
                                font.features: { "tnum": 1 }
                            }
                        }
                    }
                }
            }
        }

        // The bank: each account, what passed, what did not.
        BankSection {
            id: bankSection

            Layout.fillWidth: true
            sioul: tab.sioul
            theme: tab.theme
        }

        // What you are bound to: renewals, notices, how to stop each.
        ContractsSection {
            id: contractsSection

            Layout.fillWidth: true
            sioul: tab.sioul
            theme: tab.theme
            window: tab.window
        }

        Item {
            Layout.preferredHeight: tab.theme.gap
        }
    }
}
