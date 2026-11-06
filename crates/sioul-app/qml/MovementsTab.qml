// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The Budgets page's Movements tab (docs/accounting.md, "Movements tab";
// the money ledger's design §4): every movement, newest first, the days as
// the list's sections, in pages of 60 (one ListView, as Mail's). The search
// on top; the filters folded away under "Filters…", those in use said as
// words under the search, each with its ×; the hour's areas as the page's
// filter says, a typed search reaching every area. No running total, no
// count, no red: a sum only when the filters ask a question that has one.
// A row opens on a tap, in a pane of its own with Back: every field, where
// each fee was read, what told of it (each opens), what it is tied to. Its
// menu (a long press, a right click, "⋯") runs the acts, each with Undo.
// New movement sits at the top; For the declaration shows a declaring work
// budget's turnover and its receipts register, saved as CSV and PDF.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs
import QtQuick.Layouts
import com.aurelienpierre.sioul

Item {
    id: tab

    required property var sioul
    required property var theme
    required property var window
    // What the filters ask: {text, from, to, account, budget, way, at_least, at_most, told_by}.
    property var asked: ({ text: "", from: "", to: "", account: "", budget: "", way: "", at_least: null, at_most: null, told_by: [] })
    // The last page's answer without its rows: {page, more, total, chips, filter, accounts, budgets, declares}.
    property var shown: ({ page: 0, more: false, total: "", chips: [], filter: null, accounts: [], budgets: [], declares: [] })
    // The movement open in its pane, as `movementDetail` gives it; null for the list.
    property var detail: null
    // The declaration shown, as `declaration` gives it; null for none.
    property var declaration: null
    property string problem: ""
    readonly property bool compact: tab.window.compact === true

    // The list read again from its first page, as the filters ask.
    function reload() {
        tab.read(0)
        if (tab.detail !== null)
            tab.openMovement(tab.detail.address)
        if (tab.declaration !== null)
            tab.showDeclaration(tab.declaration.budget, tab.declaration.anchor)
    }

    function read(page) {
        const answer = JSON.parse(tab.sioul.movements(JSON.stringify(Object.assign({}, tab.asked, { page: page }))) || "{}")
        tab.problem = answer.problem || ""
        const rows = answer.rows || []
        if (page === 0 && rows.length === 0)
            model.clear()
        for (let i = 0; i < rows.length; ++i) {
            const at = page * 60 + i
            if (at < model.count)
                model.set(at, rows[i])
            else
                model.append(rows[i])
        }
        if (page === 0 && model.count > rows.length)
            model.remove(rows.length, model.count - rows.length)
        delete answer.rows
        tab.shown = Object.assign({ page: 0, more: false, total: "", chips: [], filter: null, accounts: [], budgets: [], declares: [] }, answer)
    }

    // The list's end in sight: its next page, while there is one.
    function moreRows() {
        if (tab.shown.more)
            tab.read(tab.shown.page + 1)
    }

    // A filter changed: {its field: its value}.
    function ask(change) {
        tab.asked = Object.assign({}, tab.asked, change)
        tab.read(0)
    }

    // A filter's × pressed: that filter forgotten.
    function forget(id) {
        const cleared = { account: { account: "" }, budget: { budget: "" }, way: { way: "" }, days: { from: "", to: "" }, amount: { at_least: null, at_most: null }, told: { told_by: [] } }
        tab.ask(cleared[id] || {})
    }

    // A movement opened in its pane, by its address (a row, a link, a notice).
    function openMovement(address) {
        const answer = JSON.parse(tab.sioul.movementDetail(address) || "{}")
        if (answer.problem) {
            tab.problem = answer.problem
            tab.detail = null
            return
        }
        tab.declaration = null
        tab.detail = answer
    }

    // A row's menu, at the finger on a phone: its movement in full first, for what it allows.
    function menuOf(address, item, x, y) {
        const row = JSON.parse(tab.sioul.movementDetail(address) || "{}")
        if (row.problem) {
            tab.problem = row.problem
            return
        }
        if (item && x !== undefined)
            item.Window.window.menuAt = item.mapToItem(null, x, y)
        actions.show(row)
    }

    // "For the declaration": a declaring budget's period holding `anchor` ("" for today's).
    function showDeclaration(budget, anchor) {
        const answer = JSON.parse(tab.sioul.declaration(budget || "", anchor || "") || "{}")
        if (answer.problem) {
            tab.problem = answer.problem
            return
        }
        tab.detail = null
        tab.declaration = answer
    }

    // "New movement".
    function startNew() {
        newForm.now().choose()
    }

    // For the window's pictures: the form's More… unfolded; the first row with fees or a
    // payout (else the first); every form and menu closed.
    function moreNew() {
        if (newForm.item)
            newForm.item.more = true
    }

    function firstTold() {
        for (let i = 0; i < model.count; ++i)
            if (model.get(i).second !== "")
                return model.get(i).address
        return model.count > 0 ? model.get(0).address : ""
    }

    function closeForms() {
        newForm.close()
        actions.closeAll()
    }

    // Opens what a link names: a movement's address, a mail's.
    function open(address) {
        if (address === "")
            return
        if (address.startsWith("sioul:movement/")) {
            tab.openMovement(address)
            return
        }
        tab.window.openThing(JSON.parse(tab.sioul.linkTarget("", address) || "null") || { uri: address, kind: address.startsWith("mid:") ? "mail" : "", key: "" })
    }

    Component.onCompleted: tab.read(0)
    onVisibleChanged: if (tab.visible) tab.reload()

    // Money changed, here or elsewhere: read again while shown.
    Connections {
        target: tab.sioul

        function onMoneyChanged() {
            if (tab.visible)
                tab.reload()
        }

        function onBudgetsChanged() {
            if (tab.visible && tab.detail === null && tab.declaration === null)
                tab.read(0)
        }
    }

    ListModel {
        id: model
    }

    Timer {
        id: typing

        interval: 300
        onTriggered: tab.ask({ text: search.text })
    }

    // The list, its search and its filters.
    ColumnLayout {
        visible: tab.detail === null && tab.declaration === null
        anchors.fill: parent
        spacing: 6

        RowLayout {
            Layout.fillWidth: true
            spacing: 8

            TextField {
                id: search

                objectName: "movementsSearch"
                Layout.fillWidth: true
                Layout.preferredWidth: 120
                placeholderText: tab.sioul.text("movements-search")
                onTextEdited: typing.restart()
                onAccepted: {
                    typing.stop()
                    tab.ask({ text: search.text })
                }
            }
            Button {
                objectName: "movementsFilters"
                // As wide as what it shows: the style's buttons are 100 pixels at least.
                implicitWidth: implicitContentWidth + leftPadding + rightPadding
                text: tab.compact ? "" : tab.sioul.text("movements-filters")
                icon.name: "view-filter"
                icon.color: tab.theme.text
                Accessible.name: tab.sioul.text("movements-filters")
                onClicked: filters.now().edit()
            }
            Button {
                objectName: "movementsNew"
                // As wide as what it shows: the style's buttons are 100 pixels at least.
                implicitWidth: implicitContentWidth + leftPadding + rightPadding
                text: tab.compact ? "" : tab.sioul.text("movements-new")
                icon.name: "list-add"
                icon.color: tab.theme.text
                Accessible.name: tab.sioul.text("movements-new")
                onClicked: tab.startNew()
            }
        }
        // The filters in use, as words, each with its ×; the hour's areas; the declaration.
        Flow {
            Layout.fillWidth: true
            spacing: 6

            AreaFilter {
                objectName: "movementsAreas"
                visible: tab.shown.filter !== null && search.text.trim() === ""
                sioul: tab.sioul
                theme: tab.theme
                shown: tab.shown.filter
                help: tab.sioul.text("movements-areas-help")
                onTicked: areas => {
                    tab.sioul.tickBudgetAreas(areas)
                    tab.read(0)
                }
                onAsHours: {
                    tab.sioul.budgetAreasAsHours()
                    tab.read(0)
                }
            }
            Repeater {
                model: tab.shown.chips

                delegate: Button {
                    required property var modelData

                    objectName: "movementsChip"
                    flat: true
                    text: tab.theme.plain(modelData.words) + "  ×"
                    Accessible.name: tab.sioul.textWith("movements-chip-forget", "filter", modelData.words)
                    onClicked: tab.forget(modelData.id)
                }
            }
            Button {
                objectName: "movementsDeclaration"
                visible: tab.shown.declares.length > 0
                flat: true
                text: tab.sioul.text("receipts-for-declaration")
                icon.name: "taxes-finances"
                icon.color: tab.theme.text
                onClicked: tab.showDeclaration(tab.shown.declares[0].id, "")
            }
        }
        // A sum, only when the filters ask a question that has one.
        Label {
            objectName: "movementsTotal"
            visible: tab.shown.total !== ""
            Layout.fillWidth: true
            text: tab.shown.total
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: tab.theme.text
        }
        Label {
            visible: tab.problem !== ""
            Layout.fillWidth: true
            text: tab.problem
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: tab.theme.warm
        }
        Label {
            visible: model.count === 0 && tab.problem === ""
            Layout.fillWidth: true
            text: tab.sioul.text("movements-none")
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: tab.theme.muted
        }

        ListView {
            id: list

            objectName: "movementsList"
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            model: model
            reuseItems: true
            ScrollBar.vertical: ScrollBar {}
            onAtYEndChanged: {
                if (list.atYEnd)
                    tab.moreRows()
            }

            section.property: "section"
            section.delegate: Label {
                required property string section

                width: list.width - 12
                topPadding: 10
                bottomPadding: 4
                text: section
                textFormat: Text.PlainText
                font.pixelSize: 13
                font.weight: Font.DemiBold
                color: tab.theme.muted
            }

            delegate: ItemDelegate {
                id: line

                required property string address
                required property string who
                required property string amount
                required property string way
                required property string budget
                required property string standing
                required property string second
                required property string note

                objectName: "movementRow"
                width: list.width - 12
                padding: 4
                leftPadding: 0
                Accessible.name: [line.who, line.amount, line.budget, line.standing].join(", ")
                onClicked: tab.openMovement(line.address)
                onPressAndHold: tab.menuOf(line.address, line, line.pressX, line.pressY)

                TapHandler {
                    acceptedButtons: Qt.RightButton
                    acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
                    onTapped: tab.menuOf(line.address)
                }

                contentItem: ColumnLayout {
                    spacing: 0

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 8

                        Label {
                            Layout.preferredWidth: 80
                            Layout.fillWidth: true
                            text: line.who
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            color: tab.theme.text
                        }
                        Label {
                            text: line.amount
                            textFormat: Text.PlainText
                            horizontalAlignment: Text.AlignRight
                            font.features: { "tnum": 1 }
                            font.weight: line.way === "in" ? Font.DemiBold : Font.Normal
                            color: tab.theme.text
                        }
                        ToolButton {
                            visible: !tab.compact
                            text: "⋯"
                            Accessible.name: tab.sioul.text("movements-menu")
                            onClicked: tab.menuOf(line.address)
                        }
                    }
                    Label {
                        Layout.fillWidth: true
                        text: [line.budget, line.standing].filter(t => t !== "").join("  ·  ")
                        textFormat: Text.PlainText
                        elide: Text.ElideRight
                        font.pixelSize: 12
                        color: tab.theme.muted
                    }
                    // Only when there are fees, or a payout: never a line of its own for nothing.
                    Label {
                        visible: line.second !== "" || line.note !== ""
                        Layout.fillWidth: true
                        text: [line.second, line.note !== "" ? "“" + line.note + "”" : ""].filter(t => t !== "").join("  ·  ")
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        font.pixelSize: 12
                        color: tab.theme.muted
                    }
                }
            }
        }
    }

    // A movement opened: every field, its sources, what it carried, its acts.
    ScrollView {
        id: pane

        objectName: "movementDetail"
        visible: tab.detail !== null
        anchors.fill: parent
        contentWidth: availableWidth

        ColumnLayout {
            width: pane.availableWidth
            spacing: 8

            RowLayout {
                Layout.fillWidth: true
                spacing: 8

                Button {
                    objectName: "movementBack"
                    flat: true
                    text: tab.sioul.text("ui-back")
                    icon.name: "go-previous"
                    icon.color: tab.theme.text
                    onClicked: tab.detail = null
                }
                Item {
                    Layout.fillWidth: true
                }
                Button {
                    visible: tab.detail !== null && tab.detail.allowed.fields.length > 0 && !tab.detail.gone
                    text: tab.sioul.text("movement-change")
                    onClicked: actions.changeNow(tab.detail)
                }
                Button {
                    visible: tab.detail !== null && tab.detail.gone === true
                    text: tab.sioul.text("movement-put-back-button")
                    onClicked: actions.done(tab.sioul.putBackMovement(tab.detail.address))
                }
                // Its ties as a chain (ChainPage.qml).
                Button {
                    objectName: "movementChain"
                    visible: tab.detail !== null && (tab.detail.address || "") !== ""
                    flat: true
                    text: tab.sioul.text("ui-ties")
                    onClicked: tab.window.openChain(tab.detail.address)
                }
                ToolButton {
                    visible: tab.detail !== null
                    text: "⋯"
                    Accessible.name: tab.sioul.text("movements-menu")
                    onClicked: actions.show(tab.detail)
                }
            }
            Label {
                Layout.fillWidth: true
                text: tab.detail ? tab.detail.who + "  ·  " + tab.detail.amount : ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 19
                color: tab.theme.text
            }
            Repeater {
                model: tab.detail ? tab.detail.lines : []

                delegate: Label {
                    required property string modelData

                    Layout.fillWidth: true
                    text: modelData
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: tab.theme.text
                }
            }
            // What told of it, each opened.
            Flow {
                Layout.fillWidth: true
                spacing: 6

                Repeater {
                    model: tab.detail ? tab.detail.sources.filter(s => s.address !== "") : []

                    delegate: Button {
                        required property var modelData

                        flat: true
                        text: tab.sioul.text("ui-open") + " · " + tab.theme.plain(modelData.words)
                        onClicked: tab.open(modelData.address)
                    }
                }
                Button {
                    visible: tab.detail !== null && tab.detail.carried_by !== ""
                    flat: true
                    text: tab.sioul.text("movement-open-payout")
                    onClicked: tab.openMovement(tab.detail.carried_by)
                }
            }
            // A payout: the payments it carried, each opened.
            Repeater {
                model: tab.detail ? tab.detail.carried : []

                delegate: ItemDelegate {
                    id: carriedRow

                    required property var modelData

                    objectName: "carriedRow"
                    Layout.fillWidth: true
                    onClicked: tab.openMovement(carriedRow.modelData.address)

                    // On a phone its fee goes to the next line, never cut.
                    contentItem: Label {
                        text: carriedRow.modelData.words
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: tab.theme.text
                    }
                }
            }
        }
    }

    // For the declaration: the turnover, then the register's rows.
    ScrollView {
        id: declared

        objectName: "declarationView"
        visible: tab.declaration !== null
        anchors.fill: parent
        contentWidth: availableWidth

        ColumnLayout {
            width: declared.availableWidth
            spacing: 8

            RowLayout {
                Layout.fillWidth: true
                spacing: 8

                Button {
                    flat: true
                    text: tab.sioul.text("ui-back")
                    icon.name: "go-previous"
                    icon.color: tab.theme.text
                    onClicked: tab.declaration = null
                }
                Item {
                    Layout.fillWidth: true
                }
                ToolButton {
                    text: "‹"
                    Accessible.name: tab.sioul.text("receipts-previous")
                    onClicked: tab.showDeclaration(tab.declaration.budget, tab.declaration.previous)
                }
                ToolButton {
                    enabled: tab.declaration !== null && tab.declaration.next !== ""
                    text: "›"
                    Accessible.name: tab.sioul.text("receipts-next")
                    onClicked: tab.showDeclaration(tab.declaration.budget, tab.declaration.next)
                }
            }
            RowLayout {
                Layout.fillWidth: true
                spacing: 8

                Label {
                    Layout.fillWidth: true
                    text: tab.declaration ? tab.declaration.title : ""
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    font.pixelSize: 19
                    color: tab.theme.text
                }
                PlainComboBox {
                    visible: tab.declaration !== null && tab.declaration.declares.length > 1
                    model: tab.declaration ? tab.declaration.declares.map(b => tab.theme.plain(b.title)) : []
                    currentIndex: tab.declaration ? tab.declaration.declares.findIndex(b => b.id === tab.declaration.budget) : -1
                    onActivated: index => tab.showDeclaration(tab.declaration.declares[index].id, "")
                }
            }
            Repeater {
                model: tab.declaration ? tab.declaration.sentences : []

                delegate: Label {
                    required property string modelData

                    objectName: "declarationSentence"
                    Layout.fillWidth: true
                    text: modelData
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: tab.theme.text
                }
            }
            Button {
                objectName: "receiptsSave"
                text: tab.sioul.text("receipts-save")
                icon.name: "document-save"
                icon.color: tab.theme.text
                onClicked: receiptsFolder.open()
            }
            // The receipts register: day, client, invoice, means of payment, what was paid.
            Repeater {
                model: tab.declaration ? tab.declaration.rows : []

                delegate: ItemDelegate {
                    id: receipt

                    required property var modelData

                    objectName: "receiptRow"
                    Layout.fillWidth: true
                    padding: 4
                    leftPadding: 0
                    onClicked: tab.openMovement(receipt.modelData.address)

                    contentItem: ColumnLayout {
                        spacing: 0

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 8

                            Label {
                                text: receipt.modelData.day
                                textFormat: Text.PlainText
                                font.features: { "tnum": 1 }
                                color: tab.theme.muted
                            }
                            Label {
                                Layout.preferredWidth: 80
                                Layout.fillWidth: true
                                text: receipt.modelData.client
                                textFormat: Text.PlainText
                                elide: Text.ElideRight
                                color: tab.theme.text
                            }
                            Label {
                                text: receipt.modelData.gross
                                textFormat: Text.PlainText
                                horizontalAlignment: Text.AlignRight
                                font.features: { "tnum": 1 }
                                color: receipt.modelData.known ? tab.theme.text : tab.theme.muted
                            }
                        }
                        Label {
                            Layout.fillWidth: true
                            text: [receipt.modelData.invoice, receipt.modelData.means].filter(t => t !== "").join("  ·  ")
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            font.pixelSize: 12
                            color: tab.theme.muted
                        }
                    }
                }
            }
        }
    }

    MovementActions {
        id: actions

        sioul: tab.sioul
        theme: tab.theme
        window: tab.Window.window
        budgets: tab.shown.budgets
        accounts: tab.shown.accounts
        onSaid: (text, ask) => {
            tab.problem = text
            tab.reload()
        }
    }

    // "New movement": who, the amount, its budget, its day; what was paid, its fees… under More.
    Later {
        id: newForm

        sourceComponent: Component {
            MovementDialog {
                sioul: tab.sioul
                theme: tab.theme
                budgets: tab.shown.budgets
                accounts: tab.shown.accounts
            }
        }
    }

    // "Filters…": what the list is narrowed to, folded away; a sheet on a phone.
    Later {
        id: filters

        sourceComponent: Component {
            SafeDialog {
                id: sheet

                readonly property var tellers: ["bank", "mail", "plan", "hand"]
                // What told of it, as ticked in the sheet.
                property var told: []

                function edit() {
                    account.currentIndex = Math.max(0, tab.shown.accounts.findIndex(a => a.id === tab.asked.account) + 1)
                    budget.currentIndex = Math.max(0, tab.shown.budgets.findIndex(b => b.id === tab.asked.budget) + 1)
                    way.currentIndex = Math.max(0, ["", "in", "out"].indexOf(tab.asked.way))
                    fromOn.checked = tab.asked.from !== ""
                    from.date = tab.asked.from || new Date().toISOString().slice(0, 10)
                    toOn.checked = tab.asked.to !== ""
                    to.date = tab.asked.to || new Date().toISOString().slice(0, 10)
                    least.text = tab.asked.at_least === null ? "" : String(tab.asked.at_least)
                    most.text = tab.asked.at_most === null ? "" : String(tab.asked.at_most)
                    sheet.told = tab.asked.told_by.slice()
                    sheet.open()
                }

                function amount(text) {
                    const plain = text.replace(/[\s€\u202f\u00a0]/g, "").replace(",", ".")
                    return plain === "" || Number.isNaN(Number(plain)) ? null : Math.abs(Number(plain))
                }

                function apply() {
                    const told = sheet.tellers.filter(t => sheet.told.indexOf(t) >= 0)
                    tab.ask({
                        account: account.currentIndex > 0 ? tab.shown.accounts[account.currentIndex - 1].id : "",
                        budget: budget.currentIndex > 0 ? tab.shown.budgets[budget.currentIndex - 1].id : "",
                        way: ["", "in", "out"][way.currentIndex],
                        from: fromOn.checked ? from.date : "",
                        to: toOn.checked ? to.date : "",
                        at_least: sheet.amount(least.text),
                        at_most: sheet.amount(most.text),
                        told_by: told
                    })
                    sheet.close()
                }

                parent: Overlay.overlay
                anchors.centerIn: parent
                modal: true
                width: Math.min(460, (parent ? sheet.roomWidth : 460) - 2 * tab.theme.gap)
                title: tab.sioul.text("movements-filters")

                contentItem: GridLayout {
                    columns: 2
                    columnSpacing: 10
                    rowSpacing: 8

                    Label {
                        text: tab.sioul.text("movements-filter-account")
                        color: tab.theme.muted
                    }
                    PlainComboBox {
                        id: account

                        Layout.fillWidth: true
                        model: [tab.sioul.text("movements-filter-any")].concat(tab.shown.accounts.map(a => tab.theme.plain(a.title)))
                    }
                    Label {
                        text: tab.sioul.text("movements-filter-budget")
                        color: tab.theme.muted
                    }
                    PlainComboBox {
                        id: budget

                        Layout.fillWidth: true
                        model: [tab.sioul.text("movements-filter-any")].concat(tab.shown.budgets.map(b => tab.theme.plain(b.title)))
                    }
                    Label {
                        text: tab.sioul.text("movements-filter-way")
                        color: tab.theme.muted
                    }
                    PlainComboBox {
                        id: way

                        Layout.fillWidth: true
                        model: [tab.sioul.text("movements-filter-any"), tab.sioul.text("movements-filter-in"), tab.sioul.text("movements-filter-out")]
                    }
                    CheckBox {
                        id: fromOn

                        text: tab.sioul.text("movements-filter-from-day")
                    }
                    DateField {
                        id: from

                        enabled: fromOn.checked
                        theme: tab.theme
                        sioul: tab.sioul
                        locale: Qt.locale(tab.sioul.text("qt-locale"))
                        pickLabel: tab.sioul.text("event-pick-day")
                    }
                    CheckBox {
                        id: toOn

                        text: tab.sioul.text("movements-filter-to-day")
                    }
                    DateField {
                        id: to

                        enabled: toOn.checked
                        theme: tab.theme
                        sioul: tab.sioul
                        locale: Qt.locale(tab.sioul.text("qt-locale"))
                        pickLabel: tab.sioul.text("event-pick-day")
                    }
                    Label {
                        text: tab.sioul.text("movements-filter-amount")
                        color: tab.theme.muted
                    }
                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 6

                        TextField {
                            id: least

                            Layout.fillWidth: true
                            placeholderText: tab.sioul.text("movements-filter-at-least")
                            horizontalAlignment: Text.AlignRight
                            inputMethodHints: Qt.ImhFormattedNumbersOnly
                        }
                        TextField {
                            id: most

                            Layout.fillWidth: true
                            placeholderText: tab.sioul.text("movements-filter-at-most")
                            horizontalAlignment: Text.AlignRight
                            inputMethodHints: Qt.ImhFormattedNumbersOnly
                        }
                    }
                    Label {
                        Layout.alignment: Qt.AlignTop
                        text: tab.sioul.text("movements-filter-told")
                        color: tab.theme.muted
                    }
                    Flow {
                        Layout.fillWidth: true
                        spacing: 4

                        Repeater {
                            model: sheet.tellers

                            delegate: CheckBox {
                                required property string modelData

                                text: tab.sioul.text("movements-told-" + modelData)
                                checked: sheet.told.indexOf(modelData) >= 0
                                onToggled: sheet.told = checked ? sheet.told.concat([modelData]) : sheet.told.filter(t => t !== modelData)
                            }
                        }
                    }
                }

                footer: DialogButtonBox {
                    Button {
                        text: tab.sioul.text("movements-filters-apply")
                        highlighted: true
                        onClicked: sheet.apply()
                    }
                    Button {
                        text: tab.sioul.text("ui-cancel")
                        onClicked: sheet.close()
                    }
                }
            }
        }
    }

    // "Save the receipts register…": where, then the CSV written there and its PDF beside it.
    PdfWriter {
        id: pdf
    }
    FolderDialog {
        id: receiptsFolder

        onAccepted: {
            // On Windows the address is file:///C:/…, on Android a folder of the phone's storage (Theme.localPath).
            const folder = tab.theme.localPath(receiptsFolder.selectedFolder)
            if (folder === "" || tab.declaration === null) {
                tab.sioul.status = tab.sioul.text("folder-not-on-device")
                return
            }
            const made = JSON.parse(tab.sioul.saveReceipts(tab.declaration.budget, tab.declaration.anchor, folder) || "{}")
            if (made.error) {
                tab.sioul.status = made.error
                return
            }
            const failed = pdf.write(made.html, made.pdf)
            tab.sioul.status = failed !== "" ? tab.sioul.textWith("invoice-not-written", "path", failed) : tab.sioul.textWith("receipts-saved", "path", made.folder)
        }
    }
}
