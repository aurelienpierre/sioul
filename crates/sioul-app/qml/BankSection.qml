// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Where the money is, on the Budgets page (docs/accounting.md, "Bank
// accounts"): each bank account (a current account, PayPal, Stripe) with its
// exports taken in, its balance, the budgets it fills, what tops it up, and
// its movements, each saying where it went and why, changed by hand when the
// rules got it wrong. Then the watch: in words what passed and what did not,
// the week's payments, and the balance carried forward a month as a plain
// line (zero marked, no red).

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs
import QtQuick.Layouts

ColumnLayout {
    id: section

    required property var sioul
    required property var theme
    property var shown: ({ store: false, any: false, balance: "", findings: [], coming: [], forecast: [], problem: "", accounts: [], budgets: [], reserves: [] })
    property string said: ""
    property bool saidWell: true
    // The account an export is being taken in for; "" for one named by the export.
    property string importing: ""
    // The accounts whose movements are unfolded.
    property var unfolded: ({})
    readonly property var kindIcons: ({ "bank": "view-bank", "paypal": "view-financial-account", "stripe": "view-financial-account", "other": "view-financial-account-cash" })
    property alias accountDialog: accountDialog
    property alias rulesDialog: rulesDialog
    property alias reserveDialog: reserveDialog

    function reload() {
        section.shown = JSON.parse(section.sioul.bank())
        curve.requestPaint()
    }

    function say(problem) {
        section.said = problem
        section.saidWell = problem === ""
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

    RowLayout {
        Layout.fillWidth: true
        Layout.topMargin: section.theme.gap
        spacing: 8

        Label {
            Layout.fillWidth: true
            text: section.sioul.text("bank-accounts")
            font.pixelSize: 19
            color: section.theme.text
        }
        Button {
            enabled: section.shown.store
            text: section.sioul.text("bank-account-new")
            icon.name: "list-add"
            onClicked: accountDialog.edit(null)
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

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8

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
                    Button {
                        flat: true
                        text: section.sioul.text("bank-import")
                        icon.name: "document-import"
                        onClicked: {
                            section.importing = card.modelData.id
                            picker.open()
                        }
                    }
                    Button {
                        flat: true
                        text: section.sioul.text("bank-rules")
                        onClicked: rulesDialog.show(card.modelData)
                    }
                    Button {
                        flat: true
                        text: section.sioul.text("ui-edit")
                        onClicked: accountDialog.edit(card.modelData)
                    }
                }
                Label {
                    Layout.fillWidth: true
                    text: card.modelData.balance !== "" ? card.modelData.balance : section.sioul.text("bank-account-no-export")
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: card.modelData.balance !== "" ? section.theme.text : section.theme.muted
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

                    delegate: RowLayout {
                        id: row

                        required property var modelData
                        readonly property var choices: [{ id: "", title: section.sioul.text("bank-movement-auto") }].concat(section.shown.budgets.map(b => ({ id: "budget:" + b.id, title: b.title }))).concat([{ id: "none", title: section.sioul.text("bank-place-none") }])

                        Layout.fillWidth: true
                        Layout.leftMargin: 12
                        spacing: 8

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
                        }
                        ComboBox {
                            Layout.preferredWidth: 190
                            model: row.choices.map(c => section.theme.plain(c.title))
                            currentIndex: Math.max(0, row.choices.findIndex(c => c.id === row.modelData.chosen))
                            Accessible.name: section.sioul.text("bank-movement-where")
                            onActivated: index => section.say(section.sioul.placeMovement(card.modelData.id, row.modelData.id, row.choices[index].id))
                        }
                    }
                }
            }
        }
    }

    // The watch: what passed and what did not, the week, the month ahead.
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
    Label {
        visible: section.shown.coming.length > 0
        Layout.fillWidth: true
        Layout.topMargin: 4
        text: section.sioul.text("bank-coming")
        font.pixelSize: 13
        color: section.theme.muted
    }
    Repeater {
        model: section.shown.coming

        delegate: Label {
            required property string modelData

            Layout.fillWidth: true
            text: modelData
            textFormat: Text.PlainText
            elide: Text.ElideRight
            color: section.theme.text
            font.features: { "tnum": 1 }
        }
    }
    // The month ahead: the balance as a line, zero dashed.
    Label {
        visible: section.shown.forecast.length > 1
        Layout.fillWidth: true
        Layout.topMargin: 4
        text: section.sioul.text("bank-forecast")
        font.pixelSize: 13
        color: section.theme.muted
    }
    Canvas {
        id: curve

        visible: section.shown.forecast.length > 1
        Layout.fillWidth: true
        Layout.preferredHeight: 90
        onWidthChanged: requestPaint()

        onPaint: {
            const ctx = getContext("2d")
            ctx.reset()
            const points = section.shown.forecast
            if (points.length < 2)
                return
            const values = points.map(p => p.cents)
            const low = Math.min(0, ...values)
            const high = Math.max(0, ...values)
            const span = Math.max(1, high - low)
            const y = v => height - 6 - (v - low) / span * (height - 12)
            const x = i => i / (points.length - 1) * width
            ctx.strokeStyle = section.theme.line
            ctx.setLineDash([4, 3])
            ctx.beginPath()
            ctx.moveTo(0, y(0))
            ctx.lineTo(width, y(0))
            ctx.stroke()
            ctx.setLineDash([])
            ctx.strokeStyle = section.theme.accent
            ctx.lineWidth = 1.5
            ctx.beginPath()
            points.forEach((p, i) => i === 0 ? ctx.moveTo(x(i), y(p.cents)) : ctx.lineTo(x(i), y(p.cents)))
            ctx.stroke()
        }
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

    BankAccountDialog {
        id: accountDialog

        sioul: section.sioul
        theme: section.theme
        budgets: section.shown.budgets
        reserves: section.shown.reserves
        onDone: section.say("")
    }

    BankRulesDialog {
        id: rulesDialog

        sioul: section.sioul
        theme: section.theme
        budgets: section.shown.budgets
        reserves: section.shown.reserves
        accounts: section.shown.accounts
        onChanged: {
            section.reload()
            rulesDialog.refresh(section.shown.accounts)
        }
    }

    ReserveDialog {
        id: reserveDialog

        sioul: section.sioul
        theme: section.theme
        onDone: section.say("")
    }
}
