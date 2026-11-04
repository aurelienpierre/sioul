// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The budgets page, from one income to the next (docs/accounting.md, "The
// budgets page"): each account shown in a few sentences (until when it holds
// every payment, what is left for everyday spending, its lowest point), its
// one chart; what the hour leaves out, in words; each budget in words (what
// is paid, what is still to come, what is left, how its period should end);
// what is coming, week by week; the reserves, the bank accounts, the
// contracts, the mail about money. No percentage, no colour, no bar. Budgets
// show by area with Mail's filter (AreaFilter.qml), as the hours say until
// you tick others; a shortfall on an account always shows.

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
    onVisibleChanged: page.takeShown()
    Component.onCompleted: page.takeShown()
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

    // A movement added by hand, from "New".
    function startNew() {
        newMovement.now().choose()
    }

    // The sections below the first screen, made after it, without holding the
    // window (a phone took over a second for the page); made at once when asked.
    function bankNow() {
        bankLoader.asynchronous = false
        return bankLoader.item
    }

    function contractsNow() {
        contractsLoader.asynchronous = false
        return contractsLoader.item
    }

    readonly property var contracts: contractsLoader.item
    readonly property var bank: bankLoader.item

    // The bank accounts in view: for the window's images.
    function showBank() {
        page.bankNow()
        const flick = scroll.contentItem as Flickable
        if (flick)
            flick.contentY = Math.max(0, Math.min(bankLoader.y - 8, flick.contentHeight - flick.height))
    }

    // What is coming in view, the month beside it on a computer: for the window's images.
    function showComing() {
        const flick = scroll.contentItem as Flickable
        if (flick)
            flick.contentY = Math.max(0, Math.min(coming.y - 8, flick.contentHeight - flick.height))
    }

    function showMonth() {
        coming.month = !page.window.compact
        page.showComing()
    }

    // A contract shown, or started from a mail.
    function openContract(id) {
        page.contractsNow().open(id)
    }

    function startContract(prefill) {
        page.contractsNow().start(prefill)
    }

    function openFirst() {
        if (page.view && page.view.budgets.length > 0)
            page.openId = page.view.budgets[0].id
    }

    // An act on a movement said: its effect, and whether the payee's next ones go there too, asked once.
    property string said: ""
    property var asking: null

    function acted(text, ask) {
        page.said = text
        page.asking = ask
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
                    elide: Text.ElideRight
                    color: page.theme.text
                }
                // Budgets by area, as Mail's: as the hours say, until you tick others.
                AreaFilter {
                    objectName: "budgetAreas"
                    visible: page.view !== null && !!page.view.filter && !page.window.compact
                    sioul: page.sioul
                    theme: page.theme
                    shown: page.view ? page.view.filter : null
                    help: page.sioul.text("budgets-filter-help")
                    onTicked: areas => page.sioul.tickBudgetAreas(areas)
                    onAsHours: page.sioul.budgetAreasAsHours()
                }
                Button {
                    text: page.window.compact ? "" : page.sioul.text("budget-new")
                    icon.name: "list-add"
                    icon.color: page.theme.text
                    Accessible.name: page.sioul.text("budget-new")
                    onClicked: newBudget.now().edit("", null)
                }
            }
            // On a phone, the filter under the title: within 412 points.
            AreaFilter {
                objectName: "budgetAreasPhone"
                visible: page.view !== null && !!page.view.filter && page.window.compact
                sioul: page.sioul
                theme: page.theme
                shown: page.view ? page.view.filter : null
                help: page.sioul.text("budgets-filter-help")
                onTicked: areas => page.sioul.tickBudgetAreas(areas)
                onAsHours: page.sioul.budgetAreasAsHours()
            }
            Label {
                visible: page.view === null
                Layout.fillWidth: true
                text: page.sioul.text("ui-no-budgets")
                wrapMode: Text.Wrap
                color: page.theme.muted
            }

            // Each account shown, from one income to the next, in a few sentences.
            Repeater {
                model: page.view && page.view.accounts ? page.view.accounts : []

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
                        color: page.theme.text
                    }
                    Repeater {
                        model: account.modelData.sentences

                        delegate: Label {
                            required property string modelData

                            Layout.fillWidth: true
                            text: modelData
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            color: page.theme.text
                        }
                    }
                }
            }
            // The one chart: the first account's balance ahead, to its next income.
            AheadChart {
                objectName: "aheadChart"
                visible: page.view !== null && !!page.view.chart
                Layout.fillWidth: true
                Layout.preferredHeight: page.window.compact ? 120 : 150
                theme: page.theme
                shown: page.view ? page.view.chart : null
            }
            // What the hour leaves out, in words; never how many.
            Repeater {
                model: page.view && page.view.left_out ? page.view.left_out : []

                delegate: Label {
                    required property string modelData

                    Layout.fillWidth: true
                    text: modelData
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 13
                    color: page.theme.muted
                }
            }
            // What an act on a movement did, and the question asked once.
            Label {
                visible: page.said !== ""
                Layout.fillWidth: true
                text: page.said
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: page.theme.text
            }
            Flow {
                visible: page.asking !== null
                Layout.fillWidth: true
                spacing: 8

                Label {
                    text: page.asking ? page.sioul.textArgs("moved-ask", JSON.stringify({ payee: page.asking.payee, budget: page.asking.title })) : ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: page.theme.text
                }
                Button {
                    text: page.sioul.text("moved-ask-yes")
                    onClicked: {
                        const a = page.asking
                        page.asking = null
                        page.said = page.sioul.decidePayee(a.payee, a.budget, a.way, true)
                    }
                }
                Button {
                    flat: true
                    text: page.sioul.text("moved-ask-no")
                    onClicked: {
                        const a = page.asking
                        page.asking = null
                        page.said = page.sioul.decidePayee(a.payee, a.budget, a.way, false)
                    }
                }
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

                        // The budget in words: what is paid, what is still to come, what is left.
                        Repeater {
                            model: budgetCard.modelData.words

                            delegate: Label {
                                required property string modelData

                                Layout.fillWidth: true
                                text: modelData
                                textFormat: Text.PlainText
                                wrapMode: Text.Wrap
                                color: page.theme.text
                            }
                        }
                        // How its period should end, in one sentence, with the reserve that covers it.
                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 6

                            Icon {
                                Layout.alignment: Qt.AlignTop
                                iconName: page.moodIcon(budgetCard.modelData.mood)
                                size: 16
                            }
                            Label {
                                Layout.fillWidth: true
                                text: budgetCard.modelData.verdict
                                textFormat: Text.PlainText
                                wrapMode: Text.Wrap
                                color: page.theme.text
                            }
                        }
                    }
                }
            }

            // What is coming, week by week; the month on demand beside it.
            ComingSection {
                id: coming

                objectName: "comingSection"
                visible: page.view !== null
                Layout.fillWidth: true
                sioul: page.sioul
                theme: page.theme
                window: page.window
                view: page.view
                onSaid: (text, ask) => page.acted(text, ask)
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
                    onClicked: page.bankNow().reserveDialog.now().edit(null)
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
                                onClicked: page.bankNow().reserveDialog.now().edit(page.bankNow().shown.reserves.find(r => r.id === reserveCard.modelData.id) || null)
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
                                    // A long label wraps, never under its figure (a phone's 412 points).
                                    wrapMode: reserveCell.index % 2 === 0 ? Text.Wrap : Text.NoWrap
                                    color: reserveCell.index % 2 === 0 ? page.theme.muted : page.theme.text
                                    font.features: { "tnum": 1 }
                                }
                            }
                        }
                    }
                }
            }

            // The bank: what passed, what did not, the month ahead.
            Loader {
                id: bankLoader

                Layout.fillWidth: true
                asynchronous: true
                sourceComponent: BankSection {
                    sioul: page.sioul
                    theme: page.theme
                }
            }

            // What you are bound to: renewals, notices, how to stop each.
            Loader {
                id: contractsLoader

                Layout.fillWidth: true
                asynchronous: true
                sourceComponent: ContractsSection {
                    sioul: page.sioul
                    theme: page.theme
                    window: page.window
                }
            }

            // The mail about money: made after the first screen, as the bank.
            Loader {
                Layout.fillWidth: true
                asynchronous: true
                sourceComponent: ColumnLayout {
                    spacing: 8

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
                                RowLayout {
                                    Layout.fillWidth: true
                                    spacing: 8

                                    Label {
                                        Layout.fillWidth: true
                                        text: mailRow.modelData.date + "  ·  " + mailRow.modelData.subject
                                        textFormat: Text.PlainText
                                        elide: Text.ElideRight
                                        color: page.theme.muted
                                        font.pixelSize: 13
                                    }
                                    // The message the row comes from, in Mail's reader.
                                    Button {
                                        objectName: "budgetOpenMessage"
                                        flat: true
                                        text: page.sioul.text("budget-open-message")
                                        onClicked: page.openMessage(mailRow.modelData.key)
                                    }
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

                                // What waits for you: which budget, then add it, or not; on
                                // more lines when the screen is narrow (a phone).
                                Flow {
                                    visible: mailRow.waiting
                                    Layout.fillWidth: true
                                    spacing: page.theme.gap

                                    PlainComboBox {
                                        id: budgetChoice

                                        visible: mailRow.modelData.can_add
                                        width: Math.min(260, parent.width)
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
                                    Button {
                                        flat: true
                                        text: page.sioul.text("ui-not-payment")
                                        onClicked: page.sioul.ignoreMailLine(mailRow.modelData.key)
                                    }
                                }
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
            }
        }
    }
}
