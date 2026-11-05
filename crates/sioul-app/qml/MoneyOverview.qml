// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The Budgets page's Overview (docs/accounting.md, "The budgets page"; the
// money ledger's design §6), the tab the page always opens on and the only
// one that follows the hours: each account in a few sentences until its next
// income, when its bank's file was last read; what "left for everyday
// spending" means, once; the one chart, named after its account, whose name
// chooses another; what the hour leaves out, in words; each budget in words,
// a declaring one with its turnover; what is coming, week by week. No
// percentage, no colour, no bar; a shortfall on an account always shows.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ScrollView {
    id: overview

    required property var sioul
    required property var theme
    required property var window
    // The page's view (`view::BudgetsView`).
    property var view: null
    // What an act on a movement did, and the question asked once ({payee, budget, title, way}).
    property string said: ""
    property var asking: null
    // The account whose chart is drawn; "" for the first.
    property string chartAccount: ""
    readonly property var chart: {
        const charts = overview.view && overview.view.charts ? overview.view.charts : []
        return charts.find(c => c.account === overview.chartAccount) || (charts.length > 0 ? charts[0] : null)
    }
    readonly property alias coming: coming

    // A budget opened: its ledger and its balance.
    signal openBudget(string id)
    // An act's effect, and its question, said here.
    signal acted(string text, var ask)
    signal newBudget

    contentWidth: availableWidth

    function moodIcon(mood) {
        return { "better": "go-up", "as-planned": "dialog-ok-apply", "short": "go-down" }[mood] || "view-financial-budget"
    }

    // What is coming in view, for the window's images.
    function showComing() {
        const flick = overview.contentItem as Flickable
        if (flick)
            flick.contentY = Math.max(0, Math.min(coming.y - 8, flick.contentHeight - flick.height))
    }

    ColumnLayout {
        width: overview.availableWidth
        spacing: overview.theme.gap

        RowLayout {
            Layout.fillWidth: true

            Label {
                Layout.fillWidth: true
                text: overview.view ? overview.view.title : overview.sioul.text("ui-budgets")
                textFormat: Text.PlainText
                font.pixelSize: 22
                elide: Text.ElideRight
                color: overview.theme.text
            }
            // Budgets by area, as Mail's: as the hours say, until you tick others.
            AreaFilter {
                objectName: "budgetAreas"
                visible: overview.view !== null && !!overview.view.filter && !overview.window.compact
                sioul: overview.sioul
                theme: overview.theme
                shown: overview.view ? overview.view.filter : null
                help: overview.sioul.text("budgets-filter-help")
                onTicked: areas => overview.sioul.tickBudgetAreas(areas)
                onAsHours: overview.sioul.budgetAreasAsHours()
            }
            Button {
                text: overview.window.compact ? "" : overview.sioul.text("budget-new")
                icon.name: "list-add"
                icon.color: overview.theme.text
                Accessible.name: overview.sioul.text("budget-new")
                onClicked: overview.newBudget()
            }
        }
        // On a phone, the filter under the title: within 412 points.
        AreaFilter {
            objectName: "budgetAreasPhone"
            visible: overview.view !== null && !!overview.view.filter && overview.window.compact
            sioul: overview.sioul
            theme: overview.theme
            shown: overview.view ? overview.view.filter : null
            help: overview.sioul.text("budgets-filter-help")
            onTicked: areas => overview.sioul.tickBudgetAreas(areas)
            onAsHours: overview.sioul.budgetAreasAsHours()
        }
        Label {
            visible: overview.view === null
            Layout.fillWidth: true
            text: overview.sioul.text("ui-no-budgets")
            wrapMode: Text.Wrap
            color: overview.theme.muted
        }

        // Each account shown, from one income to the next, in a few sentences,
        // then when its bank's file was last read.
        Repeater {
            model: overview.view && overview.view.accounts ? overview.view.accounts : []

            delegate: ColumnLayout {
                id: account

                required property var modelData

                objectName: "aheadAccount"
                Layout.fillWidth: true
                spacing: 2

                Label {
                    Layout.fillWidth: true
                    text: account.modelData.title
                    textFormat: Text.PlainText
                    font.weight: Font.DemiBold
                    elide: Text.ElideRight
                    color: overview.theme.text
                }
                Repeater {
                    model: account.modelData.sentences

                    delegate: Label {
                        required property string modelData

                        Layout.fillWidth: true
                        text: modelData
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: overview.theme.text
                    }
                }
                Label {
                    objectName: "accountFresh"
                    visible: (account.modelData.fresh || []).length > 0
                    Layout.fillWidth: true
                    text: (account.modelData.fresh || []).join(" ")
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 13
                    color: overview.theme.muted
                }
            }
        }
        // What "left for everyday spending" means, once.
        Label {
            objectName: "leftMeaning"
            visible: overview.view !== null && !!overview.view.left_meaning
            Layout.fillWidth: true
            text: overview.view && overview.view.left_meaning ? overview.view.left_meaning : ""
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            font.pixelSize: 13
            color: overview.theme.muted
        }
        // The one chart, named after its account; its name chooses another.
        Button {
            id: chartName

            objectName: "chartName"
            visible: overview.chart !== null
            flat: true
            leftPadding: 0
            text: overview.chart ? overview.theme.plain(overview.chart.name) + ((overview.view.charts || []).length > 1 ? "  ▾" : "") : ""
            enabled: overview.view !== null && (overview.view.charts || []).length > 1
            Accessible.name: overview.sioul.text("ahead-chart-choose")
            ToolTip.visible: hovered && enabled
            ToolTip.text: overview.sioul.text("ahead-chart-choose")
            ToolTip.delay: 300
            onClicked: chartMenu.now().popup(chartName, 0, chartName.height)
        }
        AheadChart {
            objectName: "aheadChart"
            visible: overview.chart !== null
            Layout.fillWidth: true
            Layout.preferredHeight: overview.window.compact ? 120 : 150
            theme: overview.theme
            shown: overview.chart
        }
        // What the hour leaves out, in words; never how many.
        Repeater {
            model: overview.view && overview.view.left_out ? overview.view.left_out : []

            delegate: Label {
                required property string modelData

                Layout.fillWidth: true
                text: modelData
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: overview.theme.muted
            }
        }
        // What an act on a movement did, and the question asked once.
        Label {
            visible: overview.said !== ""
            Layout.fillWidth: true
            text: overview.said
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: overview.theme.text
        }
        Flow {
            visible: overview.asking !== null
            Layout.fillWidth: true
            spacing: 8

            Label {
                width: Math.min(implicitWidth, parent.width)
                text: overview.asking ? overview.sioul.textArgs("moved-ask", JSON.stringify({ payee: overview.asking.payee, budget: overview.asking.title })) : ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: overview.theme.text
            }
            Button {
                text: overview.sioul.text("moved-ask-yes")
                onClicked: {
                    const a = overview.asking
                    overview.asking = null
                    overview.said = overview.sioul.decidePayee(a.payee, a.budget, a.way, true)
                }
            }
            Button {
                flat: true
                text: overview.sioul.text("moved-ask-no")
                onClicked: {
                    const a = overview.asking
                    overview.asking = null
                    overview.said = overview.sioul.decidePayee(a.payee, a.budget, a.way, false)
                }
            }
        }

        // The budgets the hour shows, each in words.
        Repeater {
            model: overview.view ? overview.view.budgets : []

            delegate: Panel {
                id: budgetCard

                required property var modelData

                objectName: "overviewBudget"
                Layout.fillWidth: true
                theme: overview.theme

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
                            color: overview.theme.text
                        }
                        Label {
                            Layout.fillWidth: true
                            text: budgetCard.modelData.period
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            color: overview.theme.muted
                        }
                        // Its ledger and its balance.
                        Button {
                            flat: true
                            text: overview.window.compact ? "" : overview.sioul.text("ui-open")
                            icon.name: "view-financial-account"
                            icon.color: overview.theme.text
                            Accessible.name: overview.sioul.text("ui-open")
                            onClicked: overview.openBudget(budgetCard.modelData.id)
                        }
                    }

                    // The budget in words: what is paid, what is still to come, what is left.
                    Repeater {
                        model: budgetCard.modelData.words

                        delegate: Label {
                            required property string modelData

                            Layout.fillWidth: true
                            text: modelData
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            color: overview.theme.text
                        }
                    }
                    // A work budget that declares: its turnover this quarter, and when to declare it.
                    Label {
                        objectName: "budgetDeclare"
                        visible: !!budgetCard.modelData.declare
                        Layout.fillWidth: true
                        text: budgetCard.modelData.declare || ""
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: overview.theme.text
                    }
                    // How its period should end, in one sentence, with the reserve that covers it.
                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 6

                        Icon {
                            Layout.alignment: Qt.AlignTop
                            iconName: overview.moodIcon(budgetCard.modelData.mood)
                            size: 16
                        }
                        Label {
                            Layout.fillWidth: true
                            text: budgetCard.modelData.verdict
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            color: overview.theme.text
                        }
                    }
                }
            }
        }

        // What is coming, week by week; the month on demand beside it.
        ComingSection {
            id: coming

            objectName: "comingSection"
            visible: overview.view !== null
            Layout.fillWidth: true
            sioul: overview.sioul
            theme: overview.theme
            window: overview.window
            view: overview.view
            onSaid: (text, ask) => overview.acted(text, ask)
        }

        Item {
            Layout.preferredHeight: overview.theme.gap
        }
    }

    // The accounts whose balance ahead can be drawn.
    Later {
        id: chartMenu

        sourceComponent: Component {
            SioulMenu {
                id: chooser

                Instantiator {
                    model: overview.view && overview.view.charts ? overview.view.charts : []

                    delegate: MenuItem {
                        required property var modelData

                        text: overview.theme.plain(modelData.name)
                        checkable: true
                        checked: overview.chart !== null && overview.chart.account === modelData.account
                        onTriggered: overview.chartAccount = modelData.account
                    }
                    onObjectAdded: (index, item) => chooser.insertItem(index, item)
                    onObjectRemoved: (index, item) => chooser.removeItem(item)
                }
            }
        }
    }
}
