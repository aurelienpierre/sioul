// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A draft in its editor (docs/projects.md, "The editor"; the invoicing
// design §9), saved as it is typed (`invoice::edit`): each save sends only
// what was changed, lines by their ids, against the draft's revision, so
// that what changed elsewhere meanwhile is taken in and said, never written
// over; what was typed and not sent yet is written before the draft is left
// or Sioul quits (the review of phase 3, F2, F11). A draft a newer Sioul
// wrote opens to be read only, saying why (F13). Its client, chosen
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
    // The draft as read, sent with each change: one made elsewhere meanwhile is taken in and said.
    property string revision: ""
    // The lines as typed (each by its id), and the draft's other fields typed: notes, order, terms, currency.
    property var lines: []
    property var fields: ({})
    // What was typed and not sent yet: each line's fields by its id, the draft's fields.
    property var pendingLines: ({})
    property var pendingFields: ({})
    readonly property bool readOnly: editor.view !== null && editor.view.read_only === true
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
        // The draft left: what was typed in it written, one made and never written in taken out.
        if (editor.draftId !== "" && editor.draftId !== id)
            editor.leave()
        editor.draftId = id
        editor.pendingLines = ({})
        editor.pendingFields = ({})
        editor.elsewhere = false
        editor.problem = ""
        editor.tracing = false
        editor.previewOpen = false
        editor.moreOpen = false
        editor.found = []
        if (editor.take(editor.sioul.invoiceDraft(id)))
            editor.rebuild()
    }

    // The core's answer taken: the draft, or what went wrong; what it found
    // to say besides (a change made elsewhere taken in) in the status line.
    function take(answer) {
        const made = JSON.parse(answer || "{}")
        if (made.error) {
            editor.problem = made.error
            return false
        }
        editor.view = made
        editor.revision = made.revision || ""
        editor.problem = ""
        if (made.said_now)
            editor.sioul.status = made.said_now
        return true
    }

    // The draft's own fields as it holds them, as typed in the window.
    function viewFields() {
        const v = editor.view
        return v ? { notes: v.notes.join("\n"), order: v.order, terms_days: String(v.terms_days), currency: v.currency } : ({})
    }

    // The lines as the draft holds them, the page made again with them.
    function rebuild() {
        editor.lines = (editor.view ? editor.view.lines : []).map(l => Object.assign({}, l))
        editor.fields = editor.viewFields()
        editor.shownLines = 0
        editor.shownLines = editor.lines.length
    }

    // The draft read back after a change: what was just typed stays as
    // typed, the rest as the draft says now (a change made elsewhere taken
    // in); the page made again only when the lines themselves came or went.
    function merge(sentLines, sentFields) {
        const ids = editor.view.lines.map(l => l.id)
        const same = ids.length === editor.lines.length && ids.every((id, i) => editor.lines[i].id === id)
        if (!same) {
            editor.rebuild()
            return
        }
        editor.lines = editor.view.lines.map((server, i) => {
            const kept = {}
            for (const field in (sentLines[server.id] || {}))
                kept[field] = editor.lines[i][field]
            return Object.assign({}, server, kept)
        })
        const fresh = editor.viewFields()
        for (const field in sentFields)
            fresh[field] = editor.fields[field]
        editor.fields = fresh
    }

    // What was typed and not sent yet, sent with `change` (a choice, a line
    // added or taken out, the client): only what changed, lines by their
    // ids, the draft's revision as read. Returns whether it was kept.
    function send(change) {
        typing.stop()
        if (editor.readOnly || editor.draftId === "")
            return false
        const sentLines = editor.pendingLines
        const sentFields = editor.pendingFields
        editor.pendingLines = ({})
        editor.pendingFields = ({})
        const lines = Object.keys(sentLines).map(id => Object.assign({ id: id }, sentLines[id])).concat(change.lines || [])
        const full = Object.assign({ revision: editor.revision }, sentFields, change, { lines: lines })
        if (!editor.take(editor.sioul.changeInvoiceDraft(editor.draftId, JSON.stringify(full))))
            return false
        editor.merge(sentLines, sentFields)
        return true
    }

    // What was typed and not sent yet, written now (Issue, Back, another
    // draft, Quit). Returns whether the draft holds it.
    function flush() {
        typing.stop()
        if (Object.keys(editor.pendingLines).length === 0 && Object.keys(editor.pendingFields).length === 0)
            return true
        return editor.send({})
    }

    // The invoices changed: the draft open, when it changed elsewhere
    // (another device, the command line), read again at once while nothing
    // typed waits; else taken in by the next save, which sends its revision.
    // Its own saves change nothing here: the revision is the one it read.
    function changedElsewhere() {
        if (editor.draftId === "" || typing.running || Object.keys(editor.pendingLines).length > 0 || Object.keys(editor.pendingFields).length > 0)
            return
        if (editor.sioul.invoiceDraftRevision(editor.draftId) === editor.revision)
            return
        if (editor.take(editor.sioul.invoiceDraft(editor.draftId)))
            editor.merge({}, {})
    }

    // The draft left (Back, another draft, Quit): what was typed written; one
    // New made and never written in taken out, without a word (F17).
    function leave() {
        editor.flush()
        if (editor.draftId !== "")
            editor.sioul.leaveInvoiceDraft(editor.draftId)
    }

    // An amount typed, read by the window's language (F8): {ok, value, words},
    // its words ("= €1,500.00") said beside the field as it is typed.
    function reading(text) {
        return JSON.parse(editor.sioul.readAmount(String(text), editor.view ? editor.view.currency : "") || "{}")
    }

    // A number of the draft as typed in the window's language: "1500", "12,5".
    function typed(value) {
        if (value === undefined || value === null || value === "")
            return ""
        if (typeof value === "string")
            return value
        const n = Number(value)
        const places = Number.isInteger(n) ? 0 : (Number.isInteger(Math.round(n * 1000) / 10) ? 2 : 3)
        return n.toLocaleString(editor.window.sioulLocale || Qt.locale(), "f", places)
    }

    // A line's fields changed in a copy of the list, the list given again (a
    // list held by a property is changed by giving it again, never in place).
    function changeLine(index, change) {
        editor.lines = editor.lines.map((l, i) => i === index ? Object.assign({}, l, change) : l)
    }

    // A line's change waiting to be sent, by its id.
    function pend(index, change) {
        const id = editor.lines[index].id
        const pending = Object.assign({}, editor.pendingLines)
        pending[id] = Object.assign({}, pending[id] || {}, change)
        editor.pendingLines = pending
    }

    // One field of a line typed: kept as typed, sent a moment after the
    // typing stops; a number only once it reads as one.
    function setLine(index, field, value) {
        const change = {}
        change[field] = value
        editor.changeLine(index, change)
        if (["quantity", "price", "discount", "rate"].indexOf(field) >= 0) {
            const read = String(value).trim() === "" ? { ok: field === "discount", value: 0 } : editor.reading(value)
            if (!read.ok)
                return
            change[field] = read.value
        }
        editor.pend(index, change)
        typing.restart()
    }

    // One field of a line chosen: sent at once, with what was typed.
    function chooseLine(index, change) {
        editor.changeLine(index, change)
        editor.pend(index, change)
        editor.send({})
    }

    // One of the draft's own fields typed (notes, order, terms, currency):
    // kept as typed, sent a moment after the typing stops (F11).
    function setField(field, value) {
        const typedNow = {}
        typedNow[field] = value
        editor.fields = Object.assign({}, editor.fields, typedNow)
        let sent = value
        if (field === "terms_days") {
            sent = parseInt(value, 10)
            if (isNaN(sent))
                return
        } else if (field === "notes") {
            sent = value.split("\n")
        } else if (field === "currency") {
            sent = value.trim()
            if (sent.length !== 3)
                return
        }
        const pending = Object.assign({}, editor.pendingFields)
        pending[field] = sent
        editor.pendingFields = pending
        typing.restart()
    }

    function addLine() {
        editor.send({ lines: [{}] })
    }

    function removeLine(index) {
        const id = editor.lines[index].id
        const pending = Object.assign({}, editor.pendingLines)
        delete pending[id]
        editor.pendingLines = pending
        editor.send({ removed: [id] })
    }

    // The client chosen: its profile, its contact, or a name as typed.
    function chooseClient(reference) {
        editor.found = []
        clientField.text = ""
        editor.send({ client: reference })
    }

    // Its kind and its country, written in its profile.
    function saveTraits(kind, country) {
        if (editor.take(editor.sioul.setInvoiceClientTraits(editor.draftId, kind, country)))
            editor.tracing = false
    }

    // Issue: numbered, kept, its file copied where invoices go. Refused, the
    // reason is said beside Issue and in the status line (F14).
    function issueNow() {
        // What was typed a moment ago, written first.
        if (!editor.flush())
            return
        const made = JSON.parse(editor.sioul.issueInvoiceDraft(editor.draftId) || "{}")
        editor.elsewhere = made.elsewhere === true
        if (made.error) {
            editor.problem = made.error
            editor.sioul.status = made.error
            return
        }
        editor.draftId = ""
        editor.issued(made)
    }

    // The preview unfolded and in sight, at the page's end (the window's pictures).
    function toPreview() {
        editor.previewOpen = true
        Qt.callLater(() => {
            const flick = scroll.contentItem as Flickable
            if (flick)
                flick.contentY = Math.max(0, flick.contentHeight - flick.height)
        })
    }

    function discard() {
        typing.stop()
        editor.pendingLines = ({})
        editor.pendingFields = ({})
        const said = editor.sioul.discardInvoiceDraft(editor.draftId)
        if (said !== "") {
            editor.problem = said
            editor.sioul.status = said
            return
        }
        editor.draftId = ""
        editor.closed()
    }

    Timer {
        id: typing

        interval: 600
        onTriggered: editor.flush()
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
                    // What was typed is written as the list comes back (`leave`).
                    onClicked: editor.closed()
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
            // A draft that could not be read; once read, what went wrong is said beside Issue.
            Label {
                visible: editor.problem !== "" && editor.view === null
                Layout.fillWidth: true
                text: editor.problem
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: editor.theme.warm
            }
            // A newer Sioul's draft: read only, and why.
            Label {
                objectName: "invoiceReadOnly"
                visible: editor.readOnly
                Layout.fillWidth: true
                text: editor.view ? editor.view.read_only_words : ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: editor.theme.warm
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
                enabled: !editor.readOnly
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
                enabled: !editor.readOnly
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
                    // What the price and the discount typed read as, beside them (F8): {ok, words}.
                    property var priceRead: ({ ok: true, words: "" })
                    property var discountRead: ({ ok: true, words: "" })
                    // The unit, the category and the rate as their lists have them; one not there shown as itself (F17).
                    readonly property var units: editor.view ? editor.view.choices.units : []
                    readonly property int unitAt: lineItem.units.findIndex(u => u.code === lineItem.line.unit)
                    readonly property var categories: editor.view ? editor.view.choices.categories : []
                    readonly property int categoryAt: lineItem.categories.findIndex(c => c.code === lineItem.line.category)
                    readonly property var rates: editor.view ? editor.view.choices.rates : []
                    readonly property int rateAt: lineItem.rates.findIndex(r => r.rate === Number(lineItem.shown.rate))

                    objectName: "invoiceLine"
                    enabled: !editor.readOnly
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
                            objectName: "invoiceLineQuantity"
                            visible: lineItem.line.time !== true
                            width: 70
                            text: editor.typed(lineItem.line.quantity)
                            horizontalAlignment: Text.AlignRight
                            inputMethodHints: Qt.ImhFormattedNumbersOnly
                            Accessible.name: editor.sioul.text("invoices-line-quantity")
                            onTextEdited: editor.setLine(lineItem.index, "quantity", text)
                        }
                        PlainComboBox {
                            objectName: "invoiceLineUnit"
                            visible: lineItem.line.time !== true
                            model: lineItem.units.map(u => editor.theme.plain(u.label)).concat(lineItem.unitAt < 0 && lineItem.line.unit ? [editor.theme.plain(lineItem.shown.unit_words || lineItem.line.unit)] : [])
                            currentIndex: lineItem.unitAt >= 0 ? lineItem.unitAt : lineItem.units.length
                            onActivated: index => {
                                if (index < lineItem.units.length)
                                    editor.chooseLine(lineItem.index, { unit: lineItem.units[index].code })
                            }
                        }
                        TextField {
                            id: price

                            objectName: "invoiceLinePrice"
                            width: 100
                            text: editor.typed(lineItem.line.price)
                            horizontalAlignment: Text.AlignRight
                            inputMethodHints: Qt.ImhFormattedNumbersOnly
                            placeholderText: editor.sioul.text(lineItem.line.time === true ? "invoices-line-hourly" : "invoices-line-price")
                            Accessible.name: placeholderText
                            onTextEdited: {
                                lineItem.priceRead = editor.reading(text)
                                editor.setLine(lineItem.index, "price", text)
                            }
                        }
                        Label {
                            objectName: "invoiceLinePriceRead"
                            visible: lineItem.priceRead.words !== ""
                            height: price.height
                            verticalAlignment: Text.AlignVCenter
                            text: lineItem.priceRead.words
                            textFormat: Text.PlainText
                            font.pixelSize: 12
                            color: lineItem.priceRead.ok ? editor.theme.muted : editor.theme.warm
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
                                id: discount

                                width: 110
                                text: lineItem.line.discount ? editor.typed(lineItem.line.discount) : ""
                                horizontalAlignment: Text.AlignRight
                                inputMethodHints: Qt.ImhFormattedNumbersOnly
                                placeholderText: editor.sioul.text("invoices-line-discount")
                                Accessible.name: placeholderText
                                onTextEdited: {
                                    lineItem.discountRead = text.trim() === "" ? { ok: true, words: "" } : editor.reading(text)
                                    editor.setLine(lineItem.index, "discount", text)
                                }
                            }
                            Label {
                                visible: lineItem.discountRead.words !== ""
                                height: discount.height
                                verticalAlignment: Text.AlignVCenter
                                text: lineItem.discountRead.words
                                textFormat: Text.PlainText
                                font.pixelSize: 12
                                color: lineItem.discountRead.ok ? editor.theme.muted : editor.theme.warm
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
                                objectName: "invoiceLineCategory"
                                model: lineItem.categories.map(c => editor.theme.plain(c.label)).concat(lineItem.categoryAt < 0 && lineItem.line.category ? [editor.theme.plain(lineItem.line.category)] : [])
                                currentIndex: lineItem.categoryAt >= 0 ? lineItem.categoryAt : lineItem.categories.length
                                onActivated: index => {
                                    if (index < lineItem.categories.length)
                                        editor.chooseLine(lineItem.index, { category: lineItem.categories[index].code })
                                }
                            }
                            // VAT at a rate: the preset's rates, the one the line has shown as it is (F3).
                            PlainComboBox {
                                objectName: "invoiceLineRate"
                                visible: lineItem.line.category === "S" && lineItem.rates.length > 0
                                model: lineItem.rates.map(r => editor.theme.plain(r.label)).concat(lineItem.rateAt < 0 ? [editor.theme.plain(lineItem.shown.tax_words || "")] : [])
                                currentIndex: lineItem.rateAt >= 0 ? lineItem.rateAt : lineItem.rates.length
                                onActivated: index => {
                                    if (index < lineItem.rates.length)
                                        editor.chooseLine(lineItem.index, { rate: lineItem.rates[index].rate })
                                }
                            }
                            // Elsewhere has no rates of its own: the rate typed, in percent.
                            TextField {
                                objectName: "invoiceLineRateTyped"
                                visible: lineItem.line.category === "S" && lineItem.rates.length === 0
                                width: 80
                                text: editor.typed(lineItem.line.rate)
                                horizontalAlignment: Text.AlignRight
                                inputMethodHints: Qt.ImhFormattedNumbersOnly
                                placeholderText: editor.sioul.text("invoices-line-rate")
                                Accessible.name: placeholderText
                                onTextEdited: editor.setLine(lineItem.index, "rate", text)
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
                enabled: !editor.readOnly
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
                    enabled: !editor.readOnly
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
                enabled: !editor.readOnly
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
                    // Saved as it is typed, as every field of the draft (F11).
                    TextField {
                        id: termsField

                        width: 70
                        text: editor.fields.terms_days || ""
                        inputMethodHints: Qt.ImhDigitsOnly
                        horizontalAlignment: Text.AlignRight
                        onTextEdited: editor.setField("terms_days", text)
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
                    objectName: "invoiceOrder"
                    Layout.fillWidth: true
                    text: editor.fields.order || ""
                    placeholderText: editor.sioul.text("invoices-order")
                    onTextEdited: editor.setField("order", text)
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
                        text: editor.fields.currency || ""
                        placeholderText: "EUR"
                        Accessible.name: editor.sioul.text("invoices-currency")
                        onTextEdited: editor.setField("currency", text)
                    }
                }
                TextArea {
                    objectName: "invoiceNotes"
                    Layout.fillWidth: true
                    text: editor.fields.notes || ""
                    placeholderText: editor.sioul.text("invoices-notes")
                    wrapMode: TextEdit.Wrap
                    onTextChanged: {
                        if (activeFocus && text !== (editor.fields.notes || ""))
                            editor.setField("notes", text)
                    }
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
                        Layout.preferredWidth: 80
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
                    enabled: !editor.readOnly && editor.view !== null && editor.view.required.length === 0 && editor.lines.length > 0
                    onClicked: editor.issueNow()
                }
                Button {
                    objectName: "invoiceDepositOf"
                    visible: editor.view !== null && editor.view.kind === "invoice"
                    enabled: !editor.readOnly
                    text: editor.sioul.text("invoices-deposit-of")
                    onClicked: {
                        editor.flush()
                        editor.depositAsked(editor.draftId)
                    }
                }
                Button {
                    objectName: "invoiceDiscard"
                    enabled: !editor.readOnly
                    flat: true
                    text: editor.sioul.text("invoices-discard")
                    onClicked: editor.discard()
                }
            }
            // What stopped Issue or Discard, beside them, and the way out when invoices are numbered elsewhere (F14).
            Label {
                objectName: "invoiceIssueProblem"
                visible: editor.problem !== "" && editor.view !== null
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
