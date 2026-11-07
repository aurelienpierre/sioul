// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The budgets page (docs/accounting.md, "The budgets page"; the money
// ledger's design §6), in four tabs: Overview, the one it always opens on
// and the only one the hours filter (each account until its next income,
// the one chart, each budget in words, what is coming); Budgets, every
// budget under its area, the reserves, the bank accounts, the contracts;
// Movements, the full ledger; From mail, every mail about money kept here.
// A notice or a link opens the tab and the row it names. Overview is made
// with the page; Budgets without holding the window, just after it on a
// computer, the first time it shows on a phone, where each object counts
// (tst_coming.qml); the other two the first time they show. A budget opened
// shows its ledger and its balance in place of the tabs.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Item {
    id: page

    required property var sioul
    required property var theme
    required property var window

    // Read while shown: a page out of sight keeps what it showed, and reads
    // again when it comes back; results landing meanwhile cost nothing.
    property string budgetsText: ""
    readonly property var view: page.budgetsText ? JSON.parse(page.budgetsText) : null
    // The tab shown: 0 Overview, 1 Budgets, 2 Movements, 3 From mail.
    property int tab: 0
    // The tabs not made with the page, each kept once made.
    property bool budgetsMade: false
    property bool movementsMade: false
    property bool mailMade: false

    function takeShown() {
        if (page.visible)
            page.budgetsText = page.sioul.budgets
    }

    Connections {
        target: page.sioul

        function onBudgetsChanged() {
            page.takeShown()
        }
    }
    // The page always opens on Overview: facts reach you unasked there (finding 39).
    onVisibleChanged: {
        if (!page.visible)
            page.tab = 0
        page.takeShown()
    }
    onTabChanged: {
        if (page.tab === 1)
            page.budgetsMade = true
        if (page.tab === 2)
            page.movementsMade = true
        if (page.tab === 3)
            page.mailMade = true
    }
    Component.onCompleted: {
        if (!page.window.compact)
            page.budgetsMade = true
        page.takeShown()
    }
    // A budget opened: its ledger and its balance.
    property string openId: ""
    // The budget's detail, made the first time one opens.
    property bool detailMade: false
    onOpenIdChanged: {
        if (page.openId !== "")
            page.detailMade = true
    }

    // A money row's message, in Mail: by its address ("mid:…", the copy that
    // ranks first; "sioul:mail/…" for one without a Message-ID), or, for a
    // row an older Sioul recorded, by its file ("file:…").
    function openMessage(key) {
        if (key.startsWith("file:"))
            page.window.openThing({ kind: "mail", uri: "", key: key.slice("file:".length) })
        else
            page.window.openThing(JSON.parse(page.sioul.linkTarget("", key) || "null") || { uri: key, kind: "mail", key: "" })
    }

    // A tab shown, the budget open closed.
    function showTab(index) {
        page.openId = ""
        page.tab = index
    }

    // A movement added by hand, from "New"; the bank accounts it may be on read
    // as it opens, whether or not the Budgets tab is made.
    function startNew() {
        page.newAccounts = JSON.parse(page.sioul.bank()).accounts.map(a => ({ id: a.id, title: a.title }))
        newMovement.now().choose()
    }

    property var newAccounts: []

    // A movement opened in the Movements tab, by its address (a link, a notice, a reminder).
    function openMovement(address) {
        page.showTab(2)
        Qt.callLater(() => {
            if (movementsTab.item)
                (movementsTab.item as MovementsTab).openMovement(address)
        })
    }

    // The Budgets tab made now, when asked before it was (the bank, the contracts).
    function budgetsNow() {
        budgetsTab.asynchronous = false
        page.budgetsMade = true
        return budgetsTab.item
    }

    function bankNow() {
        return page.budgetsNow().bank
    }

    function contractsNow() {
        return page.budgetsNow().contracts
    }

    readonly property var contracts: budgetsTab.item ? (budgetsTab.item as MoneyBudgets).contracts : null
    readonly property var bank: budgetsTab.item ? (budgetsTab.item as MoneyBudgets).bank : null
    readonly property var movements: movementsTab.item
    readonly property var moneyMail: mailTab.item

    // The bank accounts in view: for the window's images.
    function showBank() {
        page.showTab(1)
        const made = page.budgetsNow()
        made.showItem(made.bank)
    }

    // What is coming in view, the month beside it on a computer: for the window's images.
    function showComing() {
        page.showTab(0)
        overview.showComing()
    }

    function showMonth() {
        overview.coming.month = !page.window.compact
        page.showComing()
    }

    // A contract shown, or started from a mail.
    function openContract(id) {
        page.showTab(1)
        const made = page.budgetsNow()
        made.contracts.open(id)
        made.showItem(made.contracts)
    }

    function startContract(prefill) {
        page.showTab(1)
        page.contractsNow().start(prefill)
    }

    function openFirst() {
        if (page.view && page.view.budgets.length > 0)
            page.openId = page.view.budgets[0].id
    }

    // An act on a movement said: its effect, and whether the payee's next ones go there too, asked once.
    function acted(text, ask) {
        overview.said = text
        overview.asking = ask
    }

    ScrollView {
        id: opened

        visible: page.openId !== ""
        anchors.fill: parent
        anchors.margins: page.theme.gap
        contentWidth: availableWidth

        // Made with the first budget opened, then kept.
        Loader {
            width: opened.availableWidth
            active: page.detailMade

            sourceComponent: Component {
                BudgetDetail {
                    sioul: page.sioul
                    theme: page.theme
                    window: page.window
                    budget: page.openId
                    onBack: page.openId = ""
                }
            }
        }
    }

    ColumnLayout {
        visible: page.openId === ""
        anchors.fill: parent
        anchors.margins: page.theme.gap
        spacing: page.theme.gap

        // The four tabs, as Sioul's other tabs: the one shown pressed, the others flat; in one row on a phone too.
        RowLayout {
            id: tabs

            objectName: "moneyTabs"
            Layout.fillWidth: true
            spacing: 6

            Repeater {
                model: ["money-tab-overview", "money-tab-budgets", "money-tab-movements", "money-tab-mail"]

                delegate: Button {
                    id: tabButton

                    required property string modelData
                    required property int index

                    objectName: "moneyTab" + index
                    // As wide as their names; on a phone the row's room shared out, their paddings narrower.
                    Layout.fillWidth: page.window.compact
                    Layout.preferredWidth: implicitWidth
                    implicitWidth: implicitContentWidth + leftPadding + rightPadding
                    leftPadding: page.window.compact ? 6 : 12
                    rightPadding: page.window.compact ? 6 : 12
                    text: page.sioul.text(modelData)
                    checkable: true
                    checked: page.tab === index
                    flat: page.tab !== index
                    onClicked: {
                        page.tab = index
                        // A click on the tab shown would untick it: it stays the one shown.
                        tabButton.checked = Qt.binding(() => page.tab === tabButton.index)
                    }
                }
            }
        }

        StackLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            currentIndex: page.tab

            MoneyOverview {
                id: overview

                sioul: page.sioul
                theme: page.theme
                window: page.window
                view: page.view
                onOpenBudget: id => page.openId = id
                onActed: (text, ask) => page.acted(text, ask)
                onNewBudget: newBudget.now().edit("", null)
            }
            // Never holds the window while it is made.
            Loader {
                id: budgetsTab

                active: page.budgetsMade
                asynchronous: true
                sourceComponent: MoneyBudgets {
                    sioul: page.sioul
                    theme: page.theme
                    window: page.window
                    view: page.view
                    onOpenBudget: id => page.openId = id
                }
            }
            Loader {
                id: movementsTab

                active: page.movementsMade
                sourceComponent: MovementsTab {
                    sioul: page.sioul
                    theme: page.theme
                    window: page.window
                }
            }
            Loader {
                id: mailTab

                active: page.mailMade
                sourceComponent: MoneyMailTab {
                    sioul: page.sioul
                    theme: page.theme
                    window: page.window
                }
            }
        }
    }

    Later {
        id: newBudget

        sourceComponent: Component {
            BudgetDialog {
                id: newBudgetForm

                sioul: page.sioul
                theme: page.theme
                incomes: page.view ? page.view.incomes : []
                onSaved: id => page.openId = id
            }
        }
    }

    // A movement from "New", its budget chosen in it.
    Later {
        id: newMovement

        sourceComponent: Component {
            MovementDialog {
                id: newMovementForm

                sioul: page.sioul
                theme: page.theme
                budgets: page.view ? page.view.choices : []
                accounts: page.newAccounts
            }
        }
    }
}
