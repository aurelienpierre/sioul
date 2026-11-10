// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Where the money is, on the Budgets page (docs/accounting.md, "Bank
// accounts"): each bank account (a current account, PayPal, Stripe) with its
// exports taken in, its balance, the budgets it fills, what tops it up before
// its next income, and its movements, each saying where it went and why, with
// what else tells of it (the mail's invoice, the letter, the contract);
// moved, split or said not the same from its row. Then, in words, what the
// movements found: a payment not seen, an amount that changed. What is coming
// and the balance ahead are the page's own, above.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs
import QtQuick.Layouts

ColumnLayout {
    id: section

    required property var sioul
    required property var theme
    property var shown: ({ store: false, any: false, balance: "", findings: [], coming: [], problem: "", accounts: [], budgets: [], reserves: [] })
    property string said: ""
    property bool saidWell: true
    // Asked once, after a movement was moved: {payee, budget, title, way}; null for nothing to ask.
    property var asking: null
    // The account an export is being taken in for; "" for one named by the export.
    property string importing: ""
    // The accounts whose movements are unfolded.
    property var unfolded: ({})
    readonly property var kindIcons: ({ "bank": "view-bank", "paypal": "view-financial-account", "stripe": "view-financial-account", "other": "view-financial-account-cash" })
    // A narrow screen (a phone): an account's buttons under its name, a movement's budget under it.
    readonly property bool narrow: section.width < 560
    property alias accountDialog: accountDialog
    property alias rulesDialog: rulesDialog
    property alias reserveDialog: reserveDialog

    function reload() {
        section.shown = JSON.parse(section.sioul.bank())
    }

    function say(problem) {
        section.said = problem
        section.saidWell = problem === ""
        section.reload()
    }

    // A movement placed from its row (`placeMovement`): its effect said ("Home holds
    // €12 more for October."), and whether its payee's next ones go there too asked once.
    function placed(answer) {
        const done = JSON.parse(answer)
        section.said = done.problem || done.said || ""
        section.saidWell = !done.problem
        section.asking = done.ask || null
        section.reload()
    }

    // Your answer: a yes is a rule of the budget file, which the rules list shows.
    function answer(next) {
        const a = section.asking
        section.asking = null
        if (a)
            section.said = section.sioul.decidePayee(a.payee, a.budget, a.way, next)
        section.saidWell = true
        section.reload()
    }

    function toggle(id) {
        const next = Object.assign({}, section.unfolded)
        next[id] = !next[id]
        section.unfolded = next
    }

    // The account's movements open: for the window's images.
    function unfold(id) {
        const next = Object.assign({}, section.unfolded)
        next[id] = true
        section.unfolded = next
    }

    spacing: 8
    Component.onCompleted: section.reload()
    onVisibleChanged: if (visible) section.reload()

    // Money changed, here or elsewhere (a movement placed, another device): read again while shown.
    Connections {
        target: section.sioul

        function onMoneyChanged() {
            if (section.visible)
                section.reload()
        }
    }

    RowLayout {
        Layout.fillWidth: true
        Layout.topMargin: section.theme.gap
        spacing: 8

        Label {
            Layout.fillWidth: true
            text: section.sioul.text("bank-accounts")
            elide: Text.ElideRight
            font.pixelSize: 19
            color: section.theme.text
        }
        Button {
            enabled: section.shown.store
            text: section.sioul.text("bank-account-new")
            icon.name: "list-add"
            onClicked: accountDialog.now().edit(null)
        }
        // Before any account is declared: an export taken in as it names itself, for the watch.
        Button {
            visible: section.shown.accounts.length === 0
            enabled: section.shown.store
            flat: true
            text: section.sioul.text("bank-import")
            icon.name: "document-import"
            onClicked: {
                section.importing = ""
                picker.open()
            }
        }
    }
    Label {
        visible: section.shown.accounts.length === 0
        Layout.fillWidth: true
        text: section.sioul.text("bank-accounts-help")
        wrapMode: Text.Wrap
        font.pixelSize: 13
        color: section.theme.muted
    }
    Label {
        visible: section.said !== "" || section.shown.problem !== ""
        Layout.fillWidth: true
        text: section.said !== "" ? section.said : section.shown.problem
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: section.saidWell && section.shown.problem === "" ? section.theme.muted : section.theme.warm
    }
    // Once a movement was moved: whether its payee's next movements go there too, asked once.
    Flow {
        visible: section.asking !== null
        Layout.fillWidth: true
        spacing: 8

        Label {
            width: Math.min(implicitWidth, section.width)
            text: section.asking ? section.sioul.textArgs("moved-ask", JSON.stringify({ payee: section.asking.payee, budget: section.asking.title })) : ""
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: section.theme.text
        }
        Button {
            text: section.sioul.text("moved-ask-yes")
            onClicked: section.answer(true)
        }
        Button {
            flat: true
            text: section.sioul.text("moved-ask-no")
            onClicked: section.answer(false)
        }
    }

    // Each bank account.
    Repeater {
        model: section.shown.accounts

        delegate: Panel {
            id: card

            required property var modelData
            readonly property bool open: section.unfolded[card.modelData.id] === true

            Layout.fillWidth: true
            theme: section.theme

            ColumnLayout {
                anchors.fill: parent
                spacing: 6

                GridLayout {
                    Layout.fillWidth: true
                    columns: section.narrow ? 2 : 3
                    columnSpacing: 8
                    rowSpacing: 0

                    Icon {
                        iconName: section.kindIcons[card.modelData.kind] || "view-bank"
                    }
                    Label {
                        Layout.fillWidth: true
                        text: card.modelData.title
                        textFormat: Text.PlainText
                        font.pixelSize: 17
                        font.weight: Font.DemiBold
                        elide: Text.ElideRight
                        color: section.theme.text
                    }
                    // On more lines where the card is narrow (a phone's 360 points, in French).
                    Flow {
                        Layout.columnSpan: section.narrow ? 2 : 1
                        Layout.fillWidth: section.narrow
                        Layout.preferredWidth: section.narrow ? 0 : implicitWidth
                        spacing: 8

                        // As wide as their names: the style's buttons are 100 pixels at least.
                        Button {
                            flat: true
                            implicitWidth: implicitContentWidth + leftPadding + rightPadding
                            text: section.sioul.text("bank-import")
                            icon.name: "document-import"
                            onClicked: {
                                section.importing = card.modelData.id
                                picker.open()
                            }
                        }
                        Button {
                            flat: true
                            implicitWidth: implicitContentWidth + leftPadding + rightPadding
                            text: section.sioul.text("bank-rules")
                            onClicked: rulesDialog.now().show(card.modelData)
                        }
                        Button {
                            flat: true
                            implicitWidth: implicitContentWidth + leftPadding + rightPadding
                            text: section.sioul.text("ui-edit")
                            onClicked: accountDialog.now().edit(card.modelData)
                        }
                    }
                }
                Label {
                    Layout.fillWidth: true
                    text: card.modelData.balance !== "" ? card.modelData.balance : section.sioul.text("bank-account-no-export")
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: card.modelData.balance !== "" ? section.theme.text : section.theme.muted
                }
                // When its bank's file was last read; once, that movements since may be missing.
                Label {
                    objectName: "bankFresh"
                    visible: (card.modelData.fresh || []).length > 0
                    Layout.fillWidth: true
                    text: (card.modelData.fresh || []).join(" ")
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 13
                    color: section.theme.muted
                }
                // A processor's account: how what it receives reaches your bank.
                Label {
                    objectName: "bankPayout"
                    visible: !!card.modelData.payout
                    Layout.fillWidth: true
                    text: card.modelData.payout || ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 13
                    color: section.theme.muted
                }
                // Two real accounts under one: said, calmly.
                Label {
                    visible: card.modelData.two_accounts === true
                    Layout.fillWidth: true
                    text: section.sioul.text("bank-account-two-in-one")
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 13
                    color: section.theme.muted
                }
                Label {
                    Layout.fillWidth: true
                    text: card.modelData.fills.length > 0 ? section.sioul.textWith("bank-account-fills-line", "budgets", card.modelData.fills.map(b => b.title).join(", ")) : section.sioul.text("bank-account-fills-none")
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 13
                    color: card.modelData.fills.length > 0 ? section.theme.muted : section.theme.warm
                }
                Label {
                    visible: card.modelData.topped_up_by.length > 0
                    Layout.fillWidth: true
                    text: section.sioul.textArgs(card.modelData.floor_text !== "" ? "bank-account-topped-floor" : "bank-account-topped", JSON.stringify({ reserves: card.modelData.topped_up_by.map(r => r.title).join(" → "), floor: card.modelData.floor_text }))
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 13
                    color: section.theme.muted
                }
                // What tops it up this month, in sentences.
                Repeater {
                    model: card.modelData.top_ups

                    delegate: Label {
                        required property string modelData

                        Layout.fillWidth: true
                        text: modelData
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: section.theme.warm
                    }
                }
                Label {
                    visible: card.modelData.unplaced > 0
                    Layout.fillWidth: true
                    text: section.sioul.textArgs("bank-unplaced", JSON.stringify({ count: card.modelData.unplaced }))
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: section.theme.warm
                }
                // Its movements, folded: each where it went and why; changed by hand.
                Button {
                    visible: card.modelData.movements.length > 0
                    flat: true
                    text: (card.open ? "▾  " : "▸  ") + section.sioul.textArgs("bank-movements", JSON.stringify({ count: card.modelData.movements.length }))
                    onClicked: section.toggle(card.modelData.id)
                }
                Repeater {
                    model: card.open ? card.modelData.movements : []

                    // Where it goes, beside it, or below it when the screen is narrow (a phone).
                    delegate: GridLayout {
                        id: row

                        required property var modelData
                        readonly property var choices: [{ id: "", title: section.sioul.text("bank-movement-auto") }].concat(section.shown.budgets.map(b => ({ id: "budget:" + b.id, title: b.title }))).concat([{ id: "none", title: section.sioul.text("bank-place-none") }])

                        Layout.fillWidth: true
                        Layout.leftMargin: 12
                        columns: section.narrow ? 1 : 2
                        columnSpacing: 8
                        rowSpacing: 4

                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 0

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 8

                                Label {
                                    text: row.modelData.date
                                    textFormat: Text.PlainText
                                    font.pixelSize: 13
                                    color: section.theme.muted
                                }
                                Label {
                                    Layout.fillWidth: true
                                    text: row.modelData.label
                                    textFormat: Text.PlainText
                                    elide: Text.ElideRight
                                    color: section.theme.text
                                }
                                Label {
                                    text: row.modelData.amount
                                    textFormat: Text.PlainText
                                    font.features: { "tnum": 1 }
                                    color: section.theme.text
                                }
                            }
                            Label {
                                Layout.fillWidth: true
                                text: row.modelData.place + "  ·  " + section.sioul.text("bank-why-" + row.modelData.why)
                                textFormat: Text.PlainText
                                elide: Text.ElideRight
                                font.pixelSize: 12
                                color: row.modelData.why === "unknown" ? section.theme.warm : section.theme.muted
                            }
                            // What else tells of it, beside the bank's line.
                            Label {
                                visible: row.modelData.sources.length > 1
                                Layout.fillWidth: true
                                text: section.sioul.textWith("coming-from", "list", row.modelData.sources.map(s => s.words).join(", ")) + (row.modelData.probable ? "  " + section.sioul.text("coming-probably") : "")
                                textFormat: Text.PlainText
                                wrapMode: Text.Wrap
                                font.pixelSize: 12
                                color: section.theme.muted
                            }
                        }
                        RowLayout {
                            spacing: 4

                            PlainComboBox {
                                Layout.preferredWidth: section.narrow ? 220 : 190
                                model: row.choices.map(c => section.theme.plain(c.title))
                                currentIndex: Math.max(0, row.choices.findIndex(c => c.id === row.modelData.chosen))
                                Accessible.name: section.sioul.text("bank-movement-where")
                                onActivated: index => section.placed(section.sioul.placeMovement(card.modelData.id, row.modelData.id, row.choices[index].id))
                            }
                            // Split, not the same, the same as…, what tells of it opened.
                            ToolButton {
                                objectName: "bankMovementMore"
                                text: "⋯"
                                Accessible.name: section.sioul.text("movement-split")
                                onClicked: actions.show({ key: row.modelData.key, who: row.modelData.label, amount: row.modelData.amount, cents: row.modelData.cents, budget_id: row.modelData.chosen.startsWith("budget:") ? row.modelData.chosen.slice(7) : "", sources: row.modelData.sources, probable: row.modelData.probable })
                            }
                        }
                    }
                }
            }
        }
    }

    // What the movements found, in words: a payment not seen, an amount that changed.
    Label {
        visible: section.shown.balance !== "" && section.shown.accounts.length === 0
        Layout.fillWidth: true
        text: section.shown.balance
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: section.theme.text
    }
    Repeater {
        model: section.shown.findings

        delegate: Label {
            required property string modelData

            Layout.fillWidth: true
            text: modelData
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: section.theme.warm
        }
    }
    MovementActions {
        id: actions

        sioul: section.sioul
        theme: section.theme
        window: section.Window.window
        budgets: section.shown.budgets
        onSaid: (text, ask) => section.placed(JSON.stringify({ said: text, ask: ask }))
    }

    FileDialog {
        id: picker

        nameFilters: [section.sioul.text("bank-files") + " (*.ofx *.qfx *.xml *.csv *.txt)"]
        onAccepted: {
            const done = JSON.parse(section.importing === "" ? section.sioul.importBank(picker.selectedFile.toString()) : section.sioul.importBankInto(picker.selectedFile.toString(), section.importing))
            section.said = done.said
            section.saidWell = done.ok
            section.reload()
        }
    }

    Later {
        id: accountDialog

        sourceComponent: Component {
            BankAccountDialog {
                id: accountDialogForm

                sioul: section.sioul
                theme: section.theme
                budgets: section.shown.budgets
                reserves: section.shown.reserves
                onDone: section.say("")
            }
        }
    }

    Later {
        id: rulesDialog

        sourceComponent: Component {
            BankRulesDialog {
                id: rulesDialogForm

                sioul: section.sioul
                theme: section.theme
                budgets: section.shown.budgets
                reserves: section.shown.reserves
                accounts: section.shown.accounts
                onChanged: {
                    section.reload()
                    rulesDialogForm.refresh(section.shown.accounts)
                }
            }
        }
    }

    Later {
        id: reserveDialog

        sourceComponent: Component {
            ReserveDialog {
                id: reserveDialogForm

                sioul: section.sioul
                theme: section.theme
                onDone: section.say("")
            }
        }
    }
}
