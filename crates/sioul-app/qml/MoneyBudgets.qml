// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The Budgets page's Budgets tab (the money ledger's design §6): every
// budget, whatever the hour, under its area's heading, the hour's areas
// first, each with its one sentence, opening its detail; the reserves; the
// bank accounts, each with its balance, when its bank's file was last read,
// "Take in an export…", its rules and, for a processor, how what it
// receives reaches the bank; the contracts. Made the first time the tab
// shows.
//
// A list that makes only what is in sight (10 November 2026: on a phone the
// page reached 532 objects once this tab was opened, against Mail's budget
// of 400, and more with many budgets): the budgets are its rows, their areas
// its sections; the reserves, the bank accounts and the contracts follow
// under them, each made on a phone when it comes in sight, or when something
// on it is asked for (`bankNow`, `contractsNow`), and let go with the rows
// when the tab is left. On a computer all three are made with the tab and
// kept, as before.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ListView {
    id: tab

    required property var sioul
    required property var theme
    required property var window
    // The page's view (`view::BudgetsView`).
    property var view: null
    // The parts under the budgets: on a phone each made once in sight or asked for, kept
    // while the tab shows; on a computer made with it.
    property bool reservesMade: !tab.window.compact
    property bool bankMade: !tab.window.compact
    property bool contractsMade: !tab.window.compact
    // Made already, or null: `bankNow` and `contractsNow` make them.
    readonly property var bank: tab.footerItem ? (tab.footerItem as Lower).bankPart.item : null
    readonly property var contracts: tab.footerItem ? (tab.footerItem as Lower).contractsPart.item : null

    signal openBudget(string id)

    function moodIcon(mood) {
        return { "better": "go-up", "as-planned": "dialog-ok-apply", "short": "go-down" }[mood] || "view-financial-budget"
    }

    // The bank accounts, made now when they were not yet.
    function bankNow() {
        const lower = tab.footerItem as Lower
        if (lower === null)
            return null
        lower.bankPart.asynchronous = false
        tab.bankMade = true
        return lower.bankPart.item
    }

    // The contracts, made now when they were not yet.
    function contractsNow() {
        const lower = tab.footerItem as Lower
        if (lower === null)
            return null
        lower.contractsPart.asynchronous = false
        tab.contractsMade = true
        return lower.contractsPart.item
    }

    // An item of the tab in view (the bank accounts, the contracts): the list's
    // end laid first, so that every row above it has its place.
    function showItem(item) {
        if (!item)
            return
        tab.positionViewAtEnd()
        const at = item.mapToItem(tab.contentItem, 0, 0).y
        tab.contentY = Math.max(tab.originY, Math.min(at - 8, tab.originY + tab.contentHeight - tab.height))
    }

    // What has come in sight under the budgets is made (a phone's; a computer's is made already).
    function lookDown() {
        const lower = tab.footerItem as Lower
        if (lower === null || !tab.visible)
            return
        // Rows not laid yet, or the last out of sight (the list makes every row in sight): nothing under them is.
        if (tab.count > 0 && tab.itemAtIndex(tab.count - 1) === null)
            return
        // Each part in its turn: the one above it laid at its own height first.
        const seen = part => part.mapToItem(tab, 0, 0).y <= tab.height
        const laid = part => !part.active || part.status === Loader.Ready
        if (!tab.reservesMade && seen(lower.reservesPart))
            tab.reservesMade = true
        if (!tab.bankMade && laid(lower.reservesPart) && seen(lower.bankPart))
            tab.bankMade = true
        if (!tab.contractsMade && laid(lower.bankPart) && seen(lower.contractsPart))
            tab.contractsMade = true
    }

    clip: true
    spacing: 6
    boundsBehavior: Flickable.StopAtBounds
    // Each budget a row, its area's heading the row's section: only those in sight are made.
    model: tab.view && tab.view.groups ? tab.view.groups.reduce((all, group) => all.concat(group.budgets.map(budget => Object.assign({ area: group.title }, budget))), []) : []
    section.property: "area"
    section.delegate: Label {
        required property string section

        objectName: "budgetGroup"
        width: tab.width
        topPadding: tab.theme.gap
        bottomPadding: 2
        text: section
        textFormat: Text.PlainText
        font.pixelSize: 19
        color: tab.theme.text
    }
    ScrollBar.vertical: ScrollBar {}

    onContentYChanged: tab.lookDown()
    onHeightChanged: tab.lookDown()
    onContentHeightChanged: tab.lookDown()
    // On a phone a tab out of sight lets its parts go, as its rows (BudgetsPage.qml):
    // made again once they come in sight.
    onVisibleChanged: {
        const lower = tab.footerItem as Lower
        if (!tab.visible && tab.window.compact && lower !== null) {
            for (const part of [lower.reservesPart, lower.bankPart, lower.contractsPart])
                part.asynchronous = true
            tab.reservesMade = false
            tab.bankMade = false
            tab.contractsMade = false
        }
        tab.lookDown()
    }
    Component.onCompleted: Qt.callLater(tab.lookDown)

    delegate: ItemDelegate {
        id: line

        required property var modelData

        objectName: "budgetLine"
        width: tab.width
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

    footer: Lower {}

    // Under the budgets: the reserves, the bank accounts, the contracts, each a
    // part made once in sight or asked for; until then a place of about its
    // height, so that each comes in sight in its turn.
    component Lower: ColumnLayout {
        readonly property alias reservesPart: reserves
        readonly property alias bankPart: bankLoader
        readonly property alias contractsPart: contractsLoader

        width: tab.width
        spacing: tab.theme.gap

        // The reserves: savings accounts, made and changed here.
        Loader {
            id: reserves

            Layout.fillWidth: true
            Layout.topMargin: tab.theme.gap
            Layout.preferredHeight: reserves.status === Loader.Ready ? -1 : (tab.view !== null ? 48 + 240 * (tab.view.reserves || []).length : 0)
            active: tab.reservesMade && tab.view !== null
            asynchronous: true
            sourceComponent: ColumnLayout {
                spacing: tab.theme.gap

                RowLayout {
                    Layout.fillWidth: true
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
                        onClicked: tab.bankNow().reserveDialog.now().edit(null)
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
                                    onClicked: {
                                        const bank = tab.bankNow()
                                        bank.reserveDialog.now().edit(bank.shown.reserves.find(r => r.id === reserveCard.modelData.id) || null)
                                    }
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
            }
        }

        // The bank: each account, what passed, what did not.
        Loader {
            id: bankLoader

            Layout.fillWidth: true
            Layout.preferredHeight: bankLoader.status === Loader.Ready ? -1 : 360
            active: tab.bankMade
            asynchronous: true
            sourceComponent: BankSection {
                sioul: tab.sioul
                theme: tab.theme
            }
        }

        // What you are bound to: renewals, notices, how to stop each.
        Loader {
            id: contractsLoader

            Layout.fillWidth: true
            Layout.preferredHeight: contractsLoader.status === Loader.Ready ? -1 : 240
            active: tab.contractsMade
            asynchronous: true
            sourceComponent: ContractsSection {
                sioul: tab.sioul
                theme: tab.theme
                window: tab.window
            }
        }

        Item {
            Layout.preferredHeight: tab.theme.gap
        }
    }
}
