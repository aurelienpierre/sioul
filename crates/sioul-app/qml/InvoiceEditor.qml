// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A draft in its editor (docs/projects.md, "The editor"; the invoicing
// design §9), saved as it is typed (`invoice::edit`): its client, chosen
// among the client profiles and the contacts, with the kind and the country
// shown and changeable (written in the client's profile, which the draft
// follows until it is issued); its days, the issue's (today, when you press
// Issue) and the service's (the issue's unless said); its lines, those from
// time folded, each with its quantity, unit, price and discount, its own
// tax, its activity and its words under "More"; the deposits a final
// invoice deducts; its terms and notes; its totals; what it lacks (required
// items wait before Issue, the others are said); its preview, in Qt's HTML.
// Keep as draft, Issue, Discard; a deposit invoice made from it. On a phone
// the same page, one column, every row within its width.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Item {
    id: editor

    required property var sioul
    required property var theme
    required property var window
    // The draft open, by its id.
    property string draftId: ""
    // The draft as the core shows it (`invoices::draft_view`); null before it reads.
    property var view: null
    // The lines as typed, sent whole at each change; `from` their place in the draft as kept.
    property var lines: []
    // How many lines the page shows: made again when lines come or go, never while one is typed.
    property int shownLines: 0
    // The lines from time unfolded.
    property bool timeOpen: false
    property bool previewOpen: false
    property bool moreOpen: false
    // The client's kind and country being changed.
    property bool tracing: false
    property string problem: ""
    // Invoices are numbered on another computer: offered here, on purpose.
    property bool elsewhere: false
    // Who the client may be, as typed: [{ref, name, detail}].
    property var found: []
    readonly property bool compact: editor.window.compact === true
    readonly property int timeLines: editor.lines.filter(l => l.time).length

    // Back to the list (Keep as draft: it is kept already).
    signal closed
    // Issued: the core's answer ({number, pdf, said}).
    signal issued(var made)
    // A deposit invoice asked of this draft.
    signal depositAsked(string id)

    function load(id) {
        editor.draftId = id
        editor.elsewhere = false
        editor.problem = ""
        editor.tracing = false
        editor.previewOpen = false
        editor.moreOpen = false
        editor.found = []
        if (editor.take(editor.sioul.invoiceDraft(id)))
            editor.rebuild()
    }

    // The core's answer taken: the draft, or what went wrong.
    function take(answer) {
        const made = JSON.parse(answer || "{}")
        if (made.error) {
            editor.problem = made.error
            return false
        }
        editor.view = made
        editor.problem = ""
        return true
    }

    // The lines as the draft holds them, the page made again with them.
    function rebuild() {
        editor.lines = (editor.view ? editor.view.lines : []).map(l => Object.assign({}, l))
        editor.shownLines = 0
        editor.shownLines = editor.lines.length
    }

    // A change sent; the draft read back (its totals, its checks, its preview).
    function send(change) {
        return editor.take(editor.sioul.changeInvoiceDraft(editor.draftId, JSON.stringify(change)))
    }

    // "1 234,5" or "1,234.5" as typed: a number; NaN for none.
    function number(text) {
        const plain = String(text).replace(/[\s  €$£]/g, "").replace("−", "-")
        const decimal = plain.lastIndexOf(",") > plain.lastIndexOf(".") ? plain.replace(/\./g, "").replace(",", ".") : plain.replace(/,/g, "")
        return decimal === "" ? NaN : Number(decimal)
    }

    // Every line sent; `again`: the page made again (a line came or went).
    function sendLines(again) {
        typing.stop()
        const sent = editor.lines.map(l => ({ from: l.from === null || l.from === undefined ? null : l.from, description: l.description, details: l.details, quantity: editor.number(l.quantity) || 0, unit: l.unit, price: editor.number(l.price) || 0, discount: editor.number(l.discount) || 0, nature: l.nature, activity: l.activity, own_tax: l.own_tax === true, category: l.category, rate: editor.number(l.rate) || 0, exemption: l.exemption }))
        if (!editor.send({ lines: sent }))
            return
        // Each line is now at its place in the draft.
        editor.lines = editor.lines.map((l, i) => Object.assign({}, l, { from: i }))
        if (again)
            editor.rebuild()
    }

    // A line's fields changed in a copy of the list, the list given again (a
    // list held by a property is changed by giving it again, never in place).
    function changeLine(index, change) {
        editor.lines = editor.lines.map((l, i) => i === index ? Object.assign({}, l, change) : l)
    }

    // One field of a line typed: sent a moment after the typing stops.
    function setLine(index, field, value) {
        const change = {}
        change[field] = value
        editor.changeLine(index, change)
        typing.restart()
    }

    // One field of a line chosen: sent at once.
    function chooseLine(index, change) {
        editor.changeLine(index, change)
        editor.sendLines(false)
    }

    function addLine() {
        editor.lines = editor.lines.concat([{ from: null, description: "", details: "", quantity: 1, unit: "C62", price: 0, discount: 0, nature: "service", activity: "", own_tax: false, category: "S", rate: 0, exemption: "", time: false }])
        editor.sendLines(true)
    }

    function removeLine(index) {
        editor.lines = editor.lines.filter((_, i) => i !== index)
        editor.sendLines(true)
    }

    // The client chosen: its profile, its contact, or a name as typed.
    function chooseClient(reference) {
        editor.found = []
        clientField.text = ""
        if (editor.send({ client: reference }))
            editor.rebuild()
    }

    // Its kind and its country, written in its profile.
    function saveTraits(kind, country) {
        if (editor.take(editor.sioul.setInvoiceClientTraits(editor.draftId, kind, country)))
            editor.tracing = false
    }

    // Issue: numbered, kept, its file copied where invoices go.
    function issueNow() {
        typing.stop()
        if (typing.running || editor.lines.some(l => l.from === null))
            editor.sendLines(false)
        const made = JSON.parse(editor.sioul.issueInvoiceDraft(editor.draftId) || "{}")
        editor.elsewhere = made.elsewhere === true
        if (made.error) {
            editor.problem = made.error
            return
        }
        editor.issued(made)
    }

    function discard() {
        const said = editor.sioul.discardInvoiceDraft(editor.draftId)
        if (said !== "") {
            editor.problem = said
            return
        }
        editor.closed()
    }

    Timer {
        id: typing

        interval: 600
        onTriggered: editor.sendLines(false)
    }

    Timer {
        id: looking

        interval: 250
        onTriggered: editor.found = clientField.text.trim() === "" ? [] : JSON.parse(editor.sioul.invoiceClients(clientField.text) || "[]")
    }

    ScrollView {
        id: scroll

        anchors.fill: parent
        contentWidth: availableWidth

        ColumnLayout {
            width: scroll.availableWidth
            spacing: 8

            // Back, the title, and the draft's own acts.
            RowLayout {
                Layout.fillWidth: true
                spacing: 6

                Button {
                    objectName: "invoiceEditorBack"
                    flat: true
                    text: editor.compact ? "" : editor.sioul.text("invoices-keep-draft")
                    icon.name: "go-previous"
                    icon.color: editor.theme.text
                    Accessible.name: editor.sioul.text("invoices-keep-draft")
                    onClicked: {
                        if (typing.running)
                            editor.sendLines(false)
                        editor.closed()
                    }
                }
                Label {
                    Layout.fillWidth: true
                    text: editor.view ? editor.view.title : ""
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    font.pixelSize: 19
                    color: editor.theme.text
                }
            }
            Label {
                visible: editor.view !== null && (editor.view.copy_of !== "" || editor.view.corrects !== "" || editor.view.project !== "")
                Layout.fillWidth: true
                text: editor.view ? [editor.view.corrects, editor.view.project, editor.view.copy_of !== "" ? editor.sioul.textWith("invoices-draft-copy", "id", editor.view.copy_of) : ""].filter(t => t !== "").join("  ·  ") : ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: editor.theme.muted
            }
            Label {
                visible: editor.problem !== ""
                Layout.fillWidth: true
                text: editor.problem
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: editor.theme.warm
            }
            Button {
                visible: editor.elsewhere
                text: editor.sioul.text("invoice-take")
                onClicked: {
                    editor.problem = editor.sioul.takeInvoices()
                    editor.elsewhere = false
                }
            }

            // The client.
            Label {
                Layout.topMargin: 4
                text: editor.sioul.text("invoices-client")
                font.weight: Font.DemiBold
                color: editor.theme.text
            }
            Label {
                objectName: "invoiceClientName"
                visible: editor.view !== null && editor.view.client.name !== ""
                Layout.fillWidth: true
                text: editor.view ? editor.view.client.name : ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: editor.theme.text
            }
            Label {
                visible: editor.view !== null && editor.view.client.name !== ""
                Layout.fillWidth: true
                text: editor.view ? [editor.view.client.kind_words, editor.view.client.country_name !== "" ? editor.view.client.country_name : editor.sioul.text("invoices-country-unsaid"), editor.view.client.email].filter(t => t !== "").join("  ·  ") : ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: editor.theme.muted
            }
            Flow {
                Layout.fillWidth: true
                spacing: 6

                TextField {
                    id: clientField

                    objectName: "invoiceClientField"
                    width: Math.min(320, scroll.availableWidth)
                    visible: editor.view !== null && editor.view.kind !== "credit-note"
                    placeholderText: editor.view && editor.view.client.name !== "" ? editor.sioul.text("invoices-client-other") : editor.sioul.text("invoices-client-choose")
                    onTextEdited: looking.restart()
                    onAccepted: {
                        if (editor.found.length > 0)
                            editor.chooseClient(editor.found[0].ref)
                        else if (clientField.text.trim() !== "")
                            editor.chooseClient(clientField.text.trim())
                    }
                }
                Button {
                    visible: editor.view !== null && editor.view.client.name !== "" && editor.view.kind !== "credit-note"
                    flat: true
                    text: editor.sioul.text("invoices-client-traits")
                    onClicked: {
                        editor.tracing = !editor.tracing
                        kindChoice.currentIndex = Math.max(0, editor.view.choices.kinds.findIndex(k => k.id === editor.view.client.kind))
                        countryField.text = editor.view.client.country
                    }
                }
            }
            // Who it may be, as typed: a profile, a contact, or the name itself.
            Repeater {
                model: editor.found

                delegate: ItemDelegate {
                    id: choice

                    required property var modelData

                    objectName: "invoiceClientChoice"
                    Layout.fillWidth: true
                    padding: 4
                    text: editor.theme.plain([choice.modelData.name, choice.modelData.detail].filter(t => t !== "").join("  ·  "))
                    onClicked: editor.chooseClient(choice.modelData.ref)
                }
            }
            ItemDelegate {
                visible: clientField.text.trim() !== "" && !editor.found.some(f => f.name === clientField.text.trim())
                Layout.fillWidth: true
                padding: 4
                text: editor.theme.plain(editor.sioul.textWith("invoices-client-as-typed", "name", clientField.text.trim()))
                onClicked: editor.chooseClient(clientField.text.trim())
            }
            // Its kind and its country, as its profile says them.
            Flow {
                visible: editor.tracing
                Layout.fillWidth: true
                spacing: 6

                PlainComboBox {
                    id: kindChoice

                    model: editor.view ? editor.view.choices.kinds.map(k => editor.theme.plain(k.label)) : []
                }
                TextField {
                    id: countryField

                    width: 160
                    placeholderText: editor.sioul.text("invoices-country")
                }
                Button {
                    text: editor.sioul.text("ui-save")
                    onClicked: editor.saveTraits(editor.view.choices.kinds[kindChoice.currentIndex].id, countryField.text)
                }
            }

            // The days: the issue's, the service's.
            Label {
                Layout.topMargin: 6
                Layout.fillWidth: true
                text: editor.view ? editor.sioul.textWith("invoices-issued-when", "day", editor.view.issue_day) : ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: editor.theme.text
            }
            Flow {
                visible: editor.view !== null && editor.view.kind !== "deposit"
                Layout.fillWidth: true
                spacing: 6

                Label {
                    height: serviceFrom.height
                    verticalAlignment: Text.AlignVCenter
                    text: editor.sioul.text("invoices-service")
                    color: editor.theme.text
                }
                DateField {
                    id: serviceFrom

                    theme: editor.theme
                    sioul: editor.sioul
                    width: 170
                    height: 40
                    date: editor.view ? editor.view.period_from : ""
                    onEdited: editor.send({ period_from: serviceFrom.date, period_to: serviceTo.date < serviceFrom.date ? serviceFrom.date : serviceTo.date })
                }
                Label {
                    height: serviceFrom.height
                    verticalAlignment: Text.AlignVCenter
                    text: "–"
                    color: editor.theme.muted
                }
                DateField {
                    id: serviceTo

                    theme: editor.theme
                    sioul: editor.sioul
                    width: 170
                    height: 40
                    date: editor.view ? editor.view.period_to : ""
                    onEdited: editor.send({ period_from: serviceFrom.date, period_to: serviceTo.date })
                }
            }
            Label {
                visible: editor.view !== null && editor.view.period_default && editor.view.kind !== "deposit"
                Layout.fillWidth: true
                text: editor.sioul.text("invoices-service-default")
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 12
                color: editor.theme.muted
            }

            // The lines: time folded under one row, the others each in full.
            Label {
                Layout.topMargin: 6
                text: editor.sioul.text("invoices-lines")
                font.weight: Font.DemiBold
                color: editor.theme.text
            }
            Button {
                objectName: "invoiceTimeFold"
                visible: editor.timeLines > 0
                flat: true
                text: editor.theme.plain((editor.timeOpen ? "▾  " : "▸  ") + editor.sioul.textArgs("invoices-time-lines", JSON.stringify({ n: editor.timeLines })))
                onClicked: editor.timeOpen = !editor.timeOpen
            }
            Repeater {
                model: editor.shownLines

                delegate: ColumnLayout {
                    id: lineItem

                    required property int index
                    readonly property var line: editor.lines[lineItem.index] || ({})
                    readonly property var shown: editor.view && editor.view.lines[lineItem.index] ? editor.view.lines[lineItem.index] : ({})
                    property bool more: false

                    objectName: "invoiceLine"
                    visible: !lineItem.line.time || editor.timeOpen
                    Layout.fillWidth: true
                    Layout.leftMargin: lineItem.line.time ? 12 : 0
                    spacing: 4

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 6

                        TextField {
                            objectName: "invoiceLineDescription"
                            Layout.fillWidth: true
                            text: lineItem.line.description || ""
                            placeholderText: editor.sioul.text("invoices-line-what")
                            onTextEdited: editor.setLine(lineItem.index, "description", text)
                        }
                        Label {
                            text: lineItem.shown.amount || ""
                            textFormat: Text.PlainText
                            font.features: { "tnum": 1 }
                            color: editor.theme.text
                        }
                        ToolButton {
                            text: "×"
                            Accessible.name: editor.sioul.text("invoices-line-remove")
                            onClicked: editor.removeLine(lineItem.index)
                        }
                    }
                    Flow {
                        Layout.fillWidth: true
                        spacing: 6

                        // Time: its hours, as noted; its price an hour.
                        Label {
                            visible: lineItem.line.time === true
                            height: price.height
                            verticalAlignment: Text.AlignVCenter
                            text: lineItem.line.time_words || ""
                            textFormat: Text.PlainText
                            color: editor.theme.muted
                        }
                        TextField {
                            visible: lineItem.line.time !== true
                            width: 70
                            text: String(lineItem.line.quantity === undefined ? "" : lineItem.line.quantity)
                            horizontalAlignment: Text.AlignRight
                            inputMethodHints: Qt.ImhFormattedNumbersOnly
                            Accessible.name: editor.sioul.text("invoices-line-quantity")
                            onTextEdited: editor.setLine(lineItem.index, "quantity", text)
                        }
                        PlainComboBox {
                            visible: lineItem.line.time !== true
                            model: editor.view ? editor.view.choices.units.map(u => editor.theme.plain(u.label)) : []
                            currentIndex: editor.view ? Math.max(0, editor.view.choices.units.findIndex(u => u.code === lineItem.line.unit)) : 0
                            onActivated: index => editor.chooseLine(lineItem.index, { unit: editor.view.choices.units[index].code })
                        }
                        TextField {
                            id: price

                            width: 100
                            text: String(lineItem.line.price === undefined ? "" : lineItem.line.price)
                            horizontalAlignment: Text.AlignRight
                            inputMethodHints: Qt.ImhFormattedNumbersOnly
                            placeholderText: editor.sioul.text(lineItem.line.time === true ? "invoices-line-hourly" : "invoices-line-price")
                            Accessible.name: placeholderText
                            onTextEdited: editor.setLine(lineItem.index, "price", text)
                        }
                        Button {
                            flat: true
                            text: (lineItem.more ? "▾  " : "▸  ") + editor.sioul.text("invoices-line-more")
                            onClicked: lineItem.more = !lineItem.more
                        }
                    }
                    // Under More: its discount, its words, what it sells, its activity, its own tax.
                    ColumnLayout {
                        visible: lineItem.more
                        Layout.fillWidth: true
                        spacing: 4

                        TextField {
                            Layout.fillWidth: true
                            text: lineItem.line.details || ""
                            placeholderText: editor.sioul.text("invoices-line-details")
                            onTextEdited: editor.setLine(lineItem.index, "details", text)
                        }
                        Flow {
                            Layout.fillWidth: true
                            spacing: 6

                            TextField {
                                width: 110
                                text: lineItem.line.discount ? String(lineItem.line.discount) : ""
                                horizontalAlignment: Text.AlignRight
                                inputMethodHints: Qt.ImhFormattedNumbersOnly
                                placeholderText: editor.sioul.text("invoices-line-discount")
                                Accessible.name: placeholderText
                                onTextEdited: editor.setLine(lineItem.index, "discount", text)
                            }
                            PlainComboBox {
                                model: [editor.sioul.text("invoices-nature-service"), editor.sioul.text("invoices-nature-goods")]
                                currentIndex: lineItem.line.nature === "goods" ? 1 : 0
                                onActivated: index => editor.chooseLine(lineItem.index, { nature: index === 1 ? "goods" : "service" })
                            }
                            PlainComboBox {
                                visible: editor.view !== null && editor.view.choices.activities.length > 1
                                model: editor.view ? [editor.sioul.text("invoices-activity-usual")].concat(editor.view.choices.activities.map(a => editor.theme.plain(a.label))) : []
                                currentIndex: editor.view ? editor.view.choices.activities.findIndex(a => a.id === lineItem.line.activity) + 1 : 0
                                onActivated: index => editor.chooseLine(lineItem.index, { activity: index === 0 ? "" : editor.view.choices.activities[index - 1].id })
                            }
                        }
                        Label {
                            Layout.fillWidth: true
                            text: editor.sioul.textWith("invoices-line-tax", "tax", lineItem.shown.tax_words || "")
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            font.pixelSize: 12
                            color: editor.theme.muted
                        }
                        CheckBox {
                            text: editor.sioul.text("invoices-line-own-tax")
                            checked: lineItem.line.own_tax === true
                            onToggled: editor.chooseLine(lineItem.index, { own_tax: checked })
                        }
                        Flow {
                            visible: lineItem.line.own_tax === true
                            Layout.fillWidth: true
                            spacing: 6

                            PlainComboBox {
                                model: editor.view ? editor.view.choices.categories.map(c => editor.theme.plain(c.label)) : []
                                currentIndex: editor.view ? Math.max(0, editor.view.choices.categories.findIndex(c => c.code === lineItem.line.category)) : 0
                                onActivated: index => editor.chooseLine(lineItem.index, { category: editor.view.choices.categories[index].code })
                            }
                            PlainComboBox {
                                visible: lineItem.line.category === "S" && editor.view !== null && editor.view.choices.rates.length > 0
                                model: editor.view ? editor.view.choices.rates.map(r => editor.theme.plain(r.label)) : []
                                currentIndex: editor.view ? Math.max(0, editor.view.choices.rates.findIndex(r => r.rate === Number(lineItem.line.rate))) : 0
                                onActivated: index => editor.chooseLine(lineItem.index, { rate: editor.view.choices.rates[index].rate })
                            }
                        }
                        TextField {
                            visible: lineItem.line.own_tax === true && lineItem.line.category !== "S"
                            Layout.fillWidth: true
                            text: lineItem.line.exemption || ""
                            placeholderText: editor.sioul.text("invoices-line-exemption")
                            onTextEdited: editor.setLine(lineItem.index, "exemption", text)
                        }
                    }
                }
            }
            Button {
                objectName: "invoiceAddLine"
                visible: editor.view !== null && editor.view.kind !== "credit-note"
                flat: true
                text: "+  " + editor.sioul.text("invoices-line-add")
                onClicked: editor.addLine()
            }

            // A final invoice: the deposits it deducts.
            Label {
                visible: editor.view !== null && editor.view.deposits.length > 0
                Layout.topMargin: 6
                text: editor.sioul.text("invoices-deposits")
                font.weight: Font.DemiBold
                color: editor.theme.text
            }
            Repeater {
                model: editor.view ? editor.view.deposits : []

                delegate: CheckBox {
                    id: deposit

                    required property var modelData

                    objectName: "invoiceDeposit"
                    text: editor.theme.plain([deposit.modelData.number, deposit.modelData.date, deposit.modelData.amount].join("  ·  "))
                    checked: deposit.modelData.cited
                    onToggled: {
                        const chosen = editor.view.deposits.filter(d => d.number === deposit.modelData.number ? deposit.checked : d.cited).map(d => d.number)
                        editor.send({ deposits: chosen })
                    }
                }
            }

            // Terms, notes, language and currency: folded.
            Button {
                Layout.topMargin: 4
                flat: true
                text: (editor.moreOpen ? "▾  " : "▸  ") + editor.sioul.text("invoices-terms-notes")
                onClicked: editor.moreOpen = !editor.moreOpen
            }
            ColumnLayout {
                visible: editor.moreOpen
                Layout.fillWidth: true
                spacing: 6

                Flow {
                    Layout.fillWidth: true
                    spacing: 6

                    Label {
                        height: termsField.height
                        verticalAlignment: Text.AlignVCenter
                        text: editor.sioul.text("invoices-terms-days")
                        color: editor.theme.text
                    }
                    TextField {
                        id: termsField

                        width: 70
                        text: editor.view ? String(editor.view.terms_days) : ""
                        inputMethodHints: Qt.ImhDigitsOnly
                        horizontalAlignment: Text.AlignRight
                        onEditingFinished: {
                            const days = parseInt(termsField.text, 10)
                            if (!isNaN(days) && editor.view && days !== editor.view.terms_days)
                                editor.send({ terms_days: days })
                        }
                    }
                    CheckBox {
                        text: editor.sioul.text("invoices-end-of-month")
                        checked: editor.view !== null && editor.view.end_of_month
                        onToggled: editor.send({ end_of_month: checked })
                    }
                }
                Label {
                    Layout.fillWidth: true
                    text: editor.view && editor.view.due !== "" ? editor.sioul.textWith("invoices-due-on", "day", editor.view.due) : ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 12
                    color: editor.theme.muted
                }
                TextField {
                    Layout.fillWidth: true
                    text: editor.view ? editor.view.order : ""
                    placeholderText: editor.sioul.text("invoices-order")
                    onEditingFinished: {
                        if (editor.view && text !== editor.view.order)
                            editor.send({ order: text })
                    }
                }
                Flow {
                    Layout.fillWidth: true
                    spacing: 6

                    PlainComboBox {
                        model: editor.view ? editor.view.choices.languages.map(l => editor.theme.plain(l.label)) : []
                        currentIndex: editor.view ? Math.max(0, editor.view.choices.languages.findIndex(l => l.id === editor.view.language)) : 0
                        onActivated: index => editor.send({ language: editor.view.choices.languages[index].id })
                    }
                    TextField {
                        width: 80
                        visible: editor.view !== null && editor.view.kind !== "credit-note"
                        text: editor.view ? editor.view.currency : ""
                        placeholderText: "EUR"
                        Accessible.name: editor.sioul.text("invoices-currency")
                        onEditingFinished: {
                            if (editor.view && text.trim() !== "" && text.trim().toUpperCase() !== editor.view.currency)
                                editor.send({ currency: text.trim() })
                        }
                    }
                }
                TextArea {
                    objectName: "invoiceNotes"
                    Layout.fillWidth: true
                    text: editor.view ? editor.view.notes.join("\n") : ""
                    placeholderText: editor.sioul.text("invoices-notes")
                    wrapMode: TextEdit.Wrap
                    onEditingFinished: editor.send({ notes: text.split("\n") })
                }
                Repeater {
                    model: editor.view ? editor.view.legal : []

                    delegate: Label {
                        required property string modelData

                        Layout.fillWidth: true
                        text: modelData
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        font.pixelSize: 12
                        color: editor.theme.muted
                    }
                }
            }

            // The totals.
            Repeater {
                model: editor.view ? editor.view.totals : []

                delegate: RowLayout {
                    id: total

                    required property var modelData

                    Layout.fillWidth: true
                    spacing: 8

                    Label {
                        Layout.fillWidth: true
                        text: total.modelData.label
                        textFormat: Text.PlainText
                        elide: Text.ElideRight
                        font.weight: total.modelData.last === true ? Font.DemiBold : Font.Normal
                        color: editor.theme.text
                    }
                    Label {
                        text: total.modelData.amount
                        textFormat: Text.PlainText
                        font.features: { "tnum": 1 }
                        font.weight: total.modelData.last === true ? Font.DemiBold : Font.Normal
                        color: editor.theme.text
                    }
                }
            }

            // What it lacks: required items wait before Issue; the others are said.
            Repeater {
                model: editor.view ? editor.view.required : []

                delegate: Label {
                    required property string modelData

                    objectName: "invoiceRequired"
                    Layout.fillWidth: true
                    text: "•  " + modelData
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: editor.theme.warm
                }
            }
            Repeater {
                model: editor.view ? editor.view.said : []

                delegate: Label {
                    required property string modelData

                    objectName: "invoiceSaid"
                    Layout.fillWidth: true
                    text: "•  " + modelData
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 13
                    color: editor.theme.muted
                }
            }

            // Issue, Discard, a deposit invoice of it.
            Flow {
                Layout.fillWidth: true
                Layout.topMargin: 6
                spacing: 6

                Button {
                    objectName: "invoiceIssue"
                    text: editor.sioul.text("invoices-issue")
                    icon.name: "document-send"
                    icon.color: editor.theme.accentText
                    highlighted: true
                    enabled: editor.view !== null && editor.view.required.length === 0 && editor.lines.length > 0
                    onClicked: editor.issueNow()
                }
                Button {
                    objectName: "invoiceDepositOf"
                    visible: editor.view !== null && editor.view.kind === "invoice"
                    text: editor.sioul.text("invoices-deposit-of")
                    onClicked: editor.depositAsked(editor.draftId)
                }
                Button {
                    objectName: "invoiceDiscard"
                    flat: true
                    text: editor.sioul.text("invoices-discard")
                    onClicked: editor.discard()
                }
            }

            // The preview, as it would print.
            Button {
                objectName: "invoicePreviewFold"
                flat: true
                text: (editor.previewOpen ? "▾  " : "▸  ") + editor.sioul.text("invoices-preview")
                onClicked: editor.previewOpen = !editor.previewOpen
            }
            Rectangle {
                visible: editor.previewOpen
                Layout.fillWidth: true
                Layout.bottomMargin: 16
                implicitHeight: previewText.implicitHeight + 32
                color: "white"
                border.color: editor.theme.line
                radius: editor.theme.radius

                Text {
                    id: previewText

                    objectName: "invoicePreview"
                    anchors.fill: parent
                    anchors.margins: 16
                    // rich on purpose: the invoice's page in Qt's HTML, made by the core (`invoice::html`), every word in it escaped there.
                    text: editor.previewOpen && editor.view ? editor.view.preview : ""
                    textFormat: Text.RichText
                    wrapMode: Text.Wrap
                    color: "#222222"
                }
            }
        }
    }
}
