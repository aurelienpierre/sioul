// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The Budgets page's From mail tab (docs/accounting.md, "From mail"; the
// money ledger's design §5): every mail about money kept on this device,
// Sent mail included, newest first, the days as the list's sections, in
// pages of 60. Each row says its kind, who, the amount (and the fee when
// the mail states one), its subject, and what became of it, in words. The
// search on top, the filters folded away and said as words when in use.
// "Tied or not" is a filter of its own, never the default, never counted.
// A row opens its message; its menu (a long press, a right click, "⋯"):
// Open the message, The same as…, Not the same, A payment…, Not a payment,
// Count it again, each with Undo. Read off the window's thread: a first
// look says calmly that older mail is still being read.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Item {
    id: tab

    required property var sioul
    required property var theme
    required property var window
    // What the filters ask: {text, from, to, address, account, budget, tied, way, at_least, at_most, kind}.
    property var asked: ({ text: "", from: "", to: "", address: "", account: "", budget: "", tied: null, way: "", at_least: null, at_most: null, kind: "" })
    // The last page's answer without its rows.
    property var shown: ({ page: 0, more: false, waiting: "", chips: [], filter: null, accounts: [], budgets: [], addresses: [] })
    property string problem: ""
    // The question last asked, as sent: only its answer is taken.
    property string lastAsked: ""
    readonly property bool compact: tab.window.compact === true

    // The page asked, read off the window's thread; it comes as `moneyMail`.
    // The answer already there, when it is to the same question, is shown at
    // once: an answer that comes the same changes nothing, and says nothing.
    function read(page) {
        tab.lastAsked = JSON.stringify(Object.assign({}, tab.asked, { page: page }))
        tab.sioul.askMoneyMail(tab.lastAsked)
        tab.take(tab.sioul.moneyMail)
    }

    // An answer came: its rows put in the list (its first page in place of the list, a later one after it).
    function take(text) {
        const answer = JSON.parse(text || "{}")
        if (answer.asked !== tab.lastAsked)
            return
        tab.problem = answer.problem || ""
        const rows = answer.rows || []
        const page = answer.page || 0
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
        tab.shown = Object.assign({ page: 0, more: false, waiting: "", chips: [], filter: null, accounts: [], budgets: [], addresses: [] }, answer)
    }

    function moreRows() {
        if (tab.shown.more)
            tab.read(tab.shown.page + 1)
    }

    function ask(change) {
        tab.asked = Object.assign({}, tab.asked, change)
        tab.read(0)
    }

    function forget(id) {
        const cleared = { account: { account: "" }, budget: { budget: "" }, way: { way: "" }, days: { from: "", to: "" }, amount: { at_least: null, at_most: null }, address: { address: "" }, tied: { tied: null }, kind: { kind: "" } }
        tab.ask(cleared[id] || {})
    }

    // A row as plain data, kept by a menu or a form while the list's rows are reused.
    function plain(m) {
        return { key: m.key, file: m.file, day: m.day, cents: m.cents, outcome: m.outcome, movement: m.movement, domain: m.domain, sender: m.sender, subject: m.subject, doubtful: m.doubtful, shape: m.shape }
    }

    // A message in Mail's reader, by its address, else its file.
    function openMessage(row) {
        const found = JSON.parse(tab.sioul.linkTarget("", row.key) || "null")
        tab.window.openThing(found || { kind: "mail", uri: row.key, key: row.file })
    }

    // An act's answer ({said} or {problem}): its line is the status line's, with Undo.
    function done(text) {
        const answer = JSON.parse(text || "{}")
        tab.problem = answer.problem || ""
        tab.read(0)
    }

    // A row's menu, at the finger on a phone.
    function menuOf(row, item, x, y) {
        if (item && x !== undefined)
            item.Window.window.menuAt = item.mapToItem(null, x, y)
        menu.now().show(tab.plain(row))
    }

    // What names the row for its acts.
    function named(row) {
        return JSON.stringify({ key: row.key, domain: row.domain, sender: row.sender, subject: row.subject, movement: row.movement })
    }

    // "A payment…": the form filled from the message; mail kept out by its shape counts again first.
    function payment(row) {
        if (row.outcome === "ignored")
            tab.sioul.countAgain(tab.named(row))
        tab.window.askPayment("mail", row.file, row.doubtful ? "" : "verified")
    }

    Component.onCompleted: tab.read(0)
    onVisibleChanged: if (tab.visible) tab.read(0)

    Connections {
        target: tab.sioul

        function onMoneyMailChanged() {
            tab.take(tab.sioul.moneyMail)
        }

        function onMoneyChanged() {
            if (tab.visible)
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

    ColumnLayout {
        anchors.fill: parent
        spacing: 6

        RowLayout {
            Layout.fillWidth: true
            spacing: 8

            TextField {
                id: search

                objectName: "moneyMailSearch"
                Layout.fillWidth: true
                Layout.preferredWidth: 120
                placeholderText: tab.sioul.text("money-mail-search")
                onTextEdited: typing.restart()
                onAccepted: {
                    typing.stop()
                    tab.ask({ text: search.text })
                }
            }
            Button {
                objectName: "moneyMailFilters"
                // As wide as what it shows: the style's buttons are 100 pixels at least.
                implicitWidth: implicitContentWidth + leftPadding + rightPadding
                text: tab.compact ? "" : tab.sioul.text("movements-filters")
                icon.name: "view-filter"
                icon.color: tab.theme.text
                Accessible.name: tab.sioul.text("movements-filters")
                onClicked: filters.now().edit()
            }
        }
        Flow {
            Layout.fillWidth: true
            spacing: 6

            AreaFilter {
                objectName: "moneyMailAreas"
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

                    objectName: "moneyMailChip"
                    flat: true
                    text: tab.theme.plain(modelData.words) + "  ×"
                    Accessible.name: tab.sioul.textWith("movements-chip-forget", "filter", modelData.words)
                    onClicked: tab.forget(modelData.id)
                }
            }
        }
        // Older mail still being read: said in one calm line, never a count.
        Label {
            objectName: "moneyMailWaiting"
            visible: tab.shown.waiting !== ""
            Layout.fillWidth: true
            text: tab.shown.waiting
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            font.pixelSize: 13
            color: tab.theme.muted
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
            visible: model.count === 0 && tab.problem === "" && tab.shown.waiting === ""
            Layout.fillWidth: true
            text: tab.sioul.text("money-mail-none")
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: tab.theme.muted
        }

        ListView {
            id: list

            objectName: "moneyMailList"
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

                required property var model
                required property string kind
                required property string who
                required property string amount
                required property string fee
                required property string subject
                required property string became
                required property string outcome
                required property bool shape
                required property bool doubtful

                objectName: "moneyMailRow"
                width: list.width - 12
                padding: 4
                leftPadding: 0
                Accessible.name: [line.kind, line.who, line.amount, line.subject, line.became].filter(t => t !== "").join(", ")
                onClicked: tab.openMessage(line.model)
                onPressAndHold: tab.menuOf(line.model, line, line.pressX, line.pressY)

                TapHandler {
                    acceptedButtons: Qt.RightButton
                    acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
                    onTapped: tab.menuOf(line.model)
                }

                contentItem: ColumnLayout {
                    spacing: 0

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 8

                        Label {
                            text: line.kind
                            textFormat: Text.PlainText
                            color: tab.theme.muted
                        }
                        Label {
                            Layout.preferredWidth: 80
                            Layout.fillWidth: true
                            text: line.who
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            font.weight: Font.DemiBold
                            color: tab.theme.text
                        }
                        Label {
                            text: [line.amount, line.fee].filter(t => t !== "").join("  ·  ")
                            textFormat: Text.PlainText
                            horizontalAlignment: Text.AlignRight
                            font.features: { "tnum": 1 }
                            color: tab.theme.text
                        }
                        ToolButton {
                            visible: !tab.compact
                            text: "⋯"
                            Accessible.name: tab.sioul.text("movements-menu")
                            onClicked: tab.menuOf(line.model)
                        }
                    }
                    Label {
                        Layout.fillWidth: true
                        text: "“" + line.subject + "”"
                        textFormat: Text.PlainText
                        elide: Text.ElideRight
                        font.pixelSize: 12
                        color: tab.theme.muted
                    }
                    // What became of it; mail kept out by its shape says so once, with Count it again.
                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 6

                        Label {
                            Layout.fillWidth: true
                            text: line.became + (line.doubtful && line.outcome === "untied" ? "  ·  " + tab.sioul.text("note-doubtful") : "")
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            font.pixelSize: 12
                            color: tab.theme.muted
                        }
                        Button {
                            objectName: "moneyMailCountAgain"
                            visible: line.outcome === "ignored" && line.shape
                            flat: true
                            text: tab.sioul.text("money-mail-count-again")
                            onClicked: tab.done(tab.sioul.countAgain(tab.named(line.model)))
                        }
                        Button {
                            objectName: "moneyMailAdd"
                            visible: line.outcome === "untied" && line.amount !== ""
                            flat: true
                            text: tab.sioul.text("ui-add-line")
                            onClicked: tab.payment(line.model)
                        }
                        Button {
                            objectName: "moneyMailNot"
                            visible: line.outcome === "untied"
                            flat: true
                            text: tab.sioul.text("ui-not-payment")
                            onClicked: tab.done(tab.sioul.notAPayment(tab.named(line.model)))
                        }
                    }
                }
            }
        }
    }

    // A row's menu.
    Later {
        id: menu

        sourceComponent: Component {
            SioulMenu {
                id: rowMenu

                property var row: null

                function show(row) {
                    rowMenu.row = row
                    rowMenu.popup()
                }

                MenuItem {
                    text: tab.sioul.text("budget-open-message")
                    onTriggered: tab.openMessage(rowMenu.row)
                }
                MenuItem {
                    objectName: "moneyMailSame"
                    visible: rowMenu.row !== null && rowMenu.row.outcome === "untied"
                    height: visible ? implicitHeight : 0
                    text: tab.sioul.text("movement-same")
                    onTriggered: same.now().edit(rowMenu.row)
                }
                MenuItem {
                    objectName: "moneyMailNotSame"
                    visible: rowMenu.row !== null && ["joined", "carried", "fees"].indexOf(rowMenu.row.outcome) >= 0
                    height: visible ? implicitHeight : 0
                    text: tab.sioul.text("movement-not-same")
                    onTriggered: tab.done(tab.sioul.mailNotSame(tab.named(rowMenu.row)))
                }
                MenuItem {
                    visible: rowMenu.row !== null && ["untied", "ignored"].indexOf(rowMenu.row.outcome) >= 0
                    height: visible ? implicitHeight : 0
                    text: tab.sioul.text("payment-from")
                    onTriggered: tab.payment(rowMenu.row)
                }
                MenuItem {
                    visible: rowMenu.row !== null && rowMenu.row.outcome === "untied"
                    height: visible ? implicitHeight : 0
                    text: tab.sioul.text("ui-not-payment")
                    onTriggered: tab.done(tab.sioul.notAPayment(tab.named(rowMenu.row)))
                }
                MenuItem {
                    visible: rowMenu.row !== null && rowMenu.row.outcome === "ignored"
                    height: visible ? implicitHeight : 0
                    text: tab.sioul.text("money-mail-count-again")
                    onTriggered: tab.done(tab.sioul.countAgain(tab.named(rowMenu.row)))
                }
                MenuItem {
                    visible: rowMenu.row !== null && rowMenu.row.movement !== ""
                    height: visible ? implicitHeight : 0
                    text: tab.sioul.text("money-mail-open-movement")
                    onTriggered: tab.window.openThing({ kind: "movement", uri: rowMenu.row.movement, key: "" })
                }
            }
        }
    }

    // "The same as…": the movements of its way near its day, to choose from.
    Later {
        id: same

        sourceComponent: Component {
            SafeDialog {
                id: sameForm

                property var row: null
                property var candidates: []
                property int chosen: -1
                property string problem: ""

                function edit(row) {
                    sameForm.row = row
                    sameForm.candidates = JSON.parse(tab.sioul.mailCandidates(row.day, row.cents) || "[]")
                    sameForm.chosen = -1
                    sameForm.problem = ""
                    sameForm.open()
                }

                parent: Overlay.overlay
                anchors.centerIn: parent
                modal: true
                width: Math.min(440, (parent ? sameForm.roomWidth : 440) - 2 * tab.theme.gap)
                title: tab.sioul.text("movement-same-ask")

                contentItem: ColumnLayout {
                    spacing: 4

                    Repeater {
                        model: sameForm.candidates

                        delegate: RadioButton {
                            required property var modelData
                            required property int index

                            Layout.fillWidth: true
                            checked: sameForm.chosen === index
                            text: tab.theme.plain(modelData.date + "  ·  " + modelData.label + "  ·  " + modelData.amount)
                            onClicked: sameForm.chosen = index
                        }
                    }
                    Label {
                        visible: sameForm.candidates.length === 0
                        text: tab.sioul.text("coming-none")
                        color: tab.theme.muted
                    }
                    Label {
                        visible: sameForm.problem !== ""
                        Layout.fillWidth: true
                        text: sameForm.problem
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: tab.theme.warm
                    }
                }

                footer: DialogButtonBox {
                    Button {
                        enabled: sameForm.chosen >= 0
                        text: tab.sioul.text("movement-same")
                        highlighted: true
                        onClicked: {
                            const answer = JSON.parse(tab.sioul.mailSame(sameForm.row.key, sameForm.candidates[sameForm.chosen].key) || "{}")
                            if (answer.problem) {
                                sameForm.problem = answer.problem
                                return
                            }
                            sameForm.close()
                            tab.read(0)
                        }
                    }
                    Button {
                        text: tab.sioul.text("ui-cancel")
                        onClicked: sameForm.close()
                    }
                }
            }
        }
    }

    // "Filters…": what the list is narrowed to, folded away.
    Later {
        id: filters

        sourceComponent: Component {
            SafeDialog {
                id: sheet

                readonly property var kinds: ["", "received", "paid", "order", "bill", "refund"]

                function edit() {
                    address.currentIndex = Math.max(0, tab.shown.addresses.findIndex(a => a.id === tab.asked.address) + 1)
                    account.currentIndex = Math.max(0, tab.shown.accounts.findIndex(a => a.id === tab.asked.account) + 1)
                    budget.currentIndex = Math.max(0, tab.shown.budgets.findIndex(b => b.id === tab.asked.budget) + 1)
                    tied.currentIndex = tab.asked.tied === null ? 0 : (tab.asked.tied ? 1 : 2)
                    way.currentIndex = Math.max(0, ["", "in", "out"].indexOf(tab.asked.way))
                    kind.currentIndex = Math.max(0, sheet.kinds.indexOf(tab.asked.kind))
                    fromOn.checked = tab.asked.from !== ""
                    from.date = tab.asked.from || new Date().toISOString().slice(0, 10)
                    toOn.checked = tab.asked.to !== ""
                    to.date = tab.asked.to || new Date().toISOString().slice(0, 10)
                    least.text = tab.asked.at_least === null ? "" : String(tab.asked.at_least)
                    most.text = tab.asked.at_most === null ? "" : String(tab.asked.at_most)
                    sheet.open()
                }

                function amount(text) {
                    const plain = text.replace(/[\s€  ]/g, "").replace(",", ".")
                    return plain === "" || Number.isNaN(Number(plain)) ? null : Math.abs(Number(plain))
                }

                function apply() {
                    tab.ask({
                        address: address.currentIndex > 0 ? tab.shown.addresses[address.currentIndex - 1].id : "",
                        account: account.currentIndex > 0 ? tab.shown.accounts[account.currentIndex - 1].id : "",
                        budget: budget.currentIndex > 0 ? tab.shown.budgets[budget.currentIndex - 1].id : "",
                        tied: [null, true, false][tied.currentIndex],
                        way: ["", "in", "out"][way.currentIndex],
                        kind: sheet.kinds[kind.currentIndex],
                        from: fromOn.checked ? from.date : "",
                        to: toOn.checked ? to.date : "",
                        at_least: sheet.amount(least.text),
                        at_most: sheet.amount(most.text)
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
                        text: tab.sioul.text("money-mail-filter-address")
                        color: tab.theme.muted
                    }
                    PlainComboBox {
                        id: address

                        Layout.fillWidth: true
                        model: [tab.sioul.text("movements-filter-any")].concat(tab.shown.addresses.map(a => tab.theme.plain(a.title)))
                    }
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
                        text: tab.sioul.text("money-mail-filter-tied-or-not")
                        color: tab.theme.muted
                    }
                    PlainComboBox {
                        id: tied

                        Layout.fillWidth: true
                        model: [tab.sioul.text("movements-filter-any"), tab.sioul.text("money-mail-filter-tied"), tab.sioul.text("money-mail-filter-untied")]
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
                    Label {
                        text: tab.sioul.text("money-mail-filter-kind")
                        color: tab.theme.muted
                    }
                    PlainComboBox {
                        id: kind

                        Layout.fillWidth: true
                        model: sheet.kinds.map(k => k === "" ? tab.sioul.text("movements-filter-any") : tab.sioul.text("kind-" + k))
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
}
