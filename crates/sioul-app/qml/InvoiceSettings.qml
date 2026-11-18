// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Settings ▸ Invoices (docs/projects.md, "Settings ▸ Invoices"; the
// invoicing design §3, §9): the country's preset first, then what it asks
// (your legal form, asked once of a table from before November 2026; your
// name; the identifiers it requires, each printed or not, a SIRET being
// given among them); the rest folded under "More": the income regime, the
// VAT scheme from a day on, your trade name, address, country and email,
// bank accounts and the payment QR code, terms and the late-payment rate,
// the invoices' language and currency, the numbering, the legal lines on
// and off, insurance, a company's capital. Then the books (activities,
// period, versement libératoire, the day you started, the VAT summary's
// period, the option for debits, rates of your own such as ACRE's) and the
// e-invoicing platform you use. Each value is saved as it is left.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: block

    required property var sioul
    required property var theme
    // Its sentence, said once at its head (the setting's help).
    property string about: ""
    // Settings ▸ Invoices as the core gives it (`invoices::settings_view`).
    property var view: null
    property bool more: false
    property bool books: false
    property string problem: ""
    // The lists as typed, sent whole when a row is left.
    property var ids: []
    property var tax: []
    property var banks: []
    property var lines: []
    property var contributions: []

    function reload() {
        block.view = JSON.parse(block.sioul.invoiceSettings() || "null")
        if (!block.view)
            return
        block.ids = block.view.ids.map(i => Object.assign({}, i))
        block.tax = block.view.tax.map(t => Object.assign({}, t))
        block.banks = block.view.banks.map(b => ({ holder: b.holder, iban: b.iban, bic: b.bic }))
        block.lines = block.view.lines.map(l => Object.assign({}, l))
        block.contributions = block.view.contributions.map(c => Object.assign({}, c))
    }

    // One value saved (its key without "invoice."), then read again.
    function save(key, value) {
        block.problem = block.sioul.setInvoiceSetting(key, JSON.stringify(value))
        block.reload()
    }

    // A text saved when it changed.
    function saveText(key, text) {
        if (block.view && String(block.view[key] === null || block.view[key] === undefined ? "" : block.view[key]) !== text.trim())
            block.save(key, text.trim())
    }

    function number(text) {
        const plain = String(text).replace(/[\s  %]/g, "").replace(",", ".")
        return plain === "" ? NaN : Number(plain)
    }

    // A legal line's row as Settings keeps it: changed or off; a preset's line as it is, not written.
    function lineRows() {
        return block.lines.filter(l => l.off || l.own.trim() !== "").map(l => ({ id: l.id, text: l.own, subject: l.preset ? "" : l.subject, off: l.off }))
    }

    Component.onCompleted: block.reload()

    spacing: 6

    Label {
        visible: block.about !== ""
        Layout.fillWidth: true
        text: block.about
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.pixelSize: 13
        color: block.theme.muted
    }
    Label {
        visible: block.problem !== ""
        Layout.fillWidth: true
        text: block.problem
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: block.theme.warm
    }
    Repeater {
        model: block.view ? block.view.unread : []

        delegate: Label {
            required property string modelData

            Layout.fillWidth: true
            text: modelData
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: block.theme.warm
        }
    }

    // An older table's legal form, asked here once, first.
    Rectangle {
        objectName: "invoiceFormAsked"
        visible: block.view !== null && block.view.form_asked
        Layout.fillWidth: true
        implicitHeight: asked.implicitHeight + 20
        radius: block.theme.radius
        color: block.theme.surface
        border.color: block.theme.accent

        ColumnLayout {
            id: asked

            anchors.fill: parent
            anchors.margins: 10
            spacing: 6

            Label {
                Layout.fillWidth: true
                text: block.sioul.text("invoices-settings-form-asked")
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: block.theme.text
            }
            Flow {
                Layout.fillWidth: true
                spacing: 6

                Repeater {
                    model: block.view ? block.view.forms : []

                    delegate: Button {
                        required property var modelData

                        text: block.theme.plain(modelData.name)
                        onClicked: block.save("legal_form", modelData.id)
                    }
                }
            }
        }
    }

    // The country first.
    Label {
        text: block.sioul.text("invoices-settings-preset")
        color: block.theme.text
    }
    PlainComboBox {
        objectName: "invoicePreset"
        Layout.fillWidth: true
        model: block.view ? block.view.presets.map(p => block.theme.plain(p.supported ? p.name : block.sioul.textWith("invoices-settings-unsupported", "country", p.name))) : []
        currentIndex: block.view ? Math.max(0, block.view.presets.findIndex(p => p.id === block.view.preset)) : 0
        onActivated: index => block.save("preset", block.view.presets[index].id)
    }
    Label {
        visible: block.view !== null && (block.view.why_not !== "" || block.view.limits !== "")
        Layout.fillWidth: true
        text: block.view ? [block.view.why_not, block.view.limits].filter(t => t !== "").join("\n") : ""
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.pixelSize: 13
        color: block.view && block.view.why_not !== "" ? block.theme.warm : block.theme.muted
    }

    // What it asks: your legal form, your name, the identifiers it requires.
    Label {
        visible: block.view !== null && block.view.forms.length > 0
        text: block.sioul.text("set-invoice-legal-form")
        color: block.theme.text
    }
    PlainComboBox {
        objectName: "invoiceLegalForm"
        visible: block.view !== null && block.view.forms.length > 0
        Layout.fillWidth: true
        model: block.view ? [block.sioul.text("set-invoice-legal-form-unsaid")].concat(block.view.forms.map(f => block.theme.plain(f.name))) : []
        currentIndex: block.view ? block.view.forms.findIndex(f => f.id === block.view.form) + 1 : 0
        onActivated: index => block.save("legal_form", index === 0 ? "" : block.view.forms[index - 1].id)
    }
    Label {
        text: block.sioul.text("set-invoice-name")
        color: block.theme.text
    }
    TextField {
        objectName: "invoiceName"
        Layout.fillWidth: true
        text: block.view ? block.view.name : ""
        onEditingFinished: block.saveText("name", text)
    }
    Label {
        Layout.fillWidth: true
        text: block.sioul.text("set-invoice-name-help")
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.pixelSize: 12
        color: block.theme.muted
    }
    Label {
        visible: block.view !== null && block.view.asks.length > 0
        Layout.fillWidth: true
        text: block.view ? block.sioul.text("invoices-settings-asks") + "\n" + block.view.asks.map(a => "•  " + a).join("\n") : ""
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.pixelSize: 13
        color: block.theme.muted
    }
    Label {
        text: block.sioul.text("invoices-settings-ids")
        color: block.theme.text
    }
    Repeater {
        model: block.ids.length

        delegate: Flow {
            id: idRow

            required property int index

            Layout.fillWidth: true
            spacing: 6

            PlainComboBox {
                model: block.view ? block.view.id_kinds.map(k => block.theme.plain(k.name)).concat([block.sioul.text("invoices-id-other")]) : []
                currentIndex: block.view ? (block.view.id_kinds.findIndex(k => k.kind === block.ids[idRow.index].kind) + 1 || block.view.id_kinds.length + 1) - 1 : 0
                onActivated: index => {
                    block.ids[idRow.index].kind = index < block.view.id_kinds.length ? block.view.id_kinds[index].kind : "other"
                    block.save("id", block.ids)
                }
            }
            TextField {
                objectName: "invoiceIdValue"
                width: Math.min(260, block.width)
                text: block.ids[idRow.index].value
                onEditingFinished: {
                    if (block.ids[idRow.index].value !== text.trim()) {
                        block.ids[idRow.index].value = text.trim()
                        block.save("id", block.ids)
                    }
                }
            }
            CheckBox {
                text: block.sioul.text("invoices-settings-printed")
                checked: block.ids[idRow.index].printed !== false
                onToggled: {
                    block.ids[idRow.index].printed = checked
                    block.save("id", block.ids)
                }
            }
            ToolButton {
                text: "×"
                Accessible.name: block.sioul.text("invoices-settings-remove")
                onClicked: block.save("id", block.ids.filter((_, i) => i !== idRow.index))
            }
        }
    }
    Button {
        objectName: "invoiceIdAdd"
        flat: true
        text: "+  " + block.sioul.text("invoices-settings-id-add")
        onClicked: block.ids = block.ids.concat([{ kind: block.view && block.view.id_kinds.length > 0 ? block.view.id_kinds[0].kind : "other", value: "", printed: true }])
    }
    Label {
        visible: block.view !== null && block.view.older_siret !== ""
        Layout.fillWidth: true
        text: block.view ? block.sioul.textWith("invoices-settings-older-siret", "siret", block.view.older_siret) : ""
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.pixelSize: 12
        color: block.theme.muted
    }
    Label {
        Layout.fillWidth: true
        text: block.view ? block.sioul.textWith("invoices-settings-scheme-now", "scheme", (block.view.schemes.find(s => s.id === block.view.scheme_now) || { name: block.view.scheme_now }).name) : ""
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: block.theme.text
    }

    // The rest, folded.
    BareButton {
        objectName: "invoiceSettingsMore"
        flat: true
        text: (block.more ? "▾  " : "▸  ") + block.sioul.text("invoices-settings-more")
        onClicked: block.more = !block.more
    }
    ColumnLayout {
        visible: block.more
        Layout.fillWidth: true
        Layout.leftMargin: 8
        spacing: 6

        Label {
            visible: block.view !== null && block.view.regimes.length > 0
            text: block.sioul.text("invoices-settings-regime")
            color: block.theme.text
        }
        PlainComboBox {
            visible: block.view !== null && block.view.regimes.length > 0
            Layout.fillWidth: true
            model: block.view ? [block.sioul.text("invoices-settings-unsaid")].concat(block.view.regimes.map(r => block.theme.plain(r.name))) : []
            currentIndex: block.view ? block.view.regimes.findIndex(r => r.id === block.view.regime) + 1 : 0
            onActivated: index => block.save("regime", index === 0 ? "" : block.view.regimes[index - 1].id)
        }
        // The VAT scheme, from a day on.
        Label {
            text: block.sioul.text("invoices-settings-schemes")
            color: block.theme.text
        }
        Repeater {
            model: block.tax.length

            delegate: ColumnLayout {
                id: taxRow

                required property int index

                Layout.fillWidth: true
                spacing: 4

                Flow {
                    Layout.fillWidth: true
                    spacing: 6

                    // Its first day, picked or typed as a day (the review of phase 3,
                    // F19); the first row without one holds always.
                    Label {
                        visible: taxRow.index === 0 && block.tax[taxRow.index].from === ""
                        height: 40
                        verticalAlignment: Text.AlignVCenter
                        text: block.sioul.text("invoices-settings-from-always")
                        color: block.theme.text
                    }
                    DateField {
                        id: taxDay

                        objectName: "invoiceSchemeDay"
                        visible: taxRow.index > 0 || block.tax[taxRow.index].from !== ""
                        width: 170
                        height: 40
                        theme: block.theme
                        sioul: block.sioul
                        date: block.tax[taxRow.index].from
                        onEdited: {
                            if (/^\d{4}-\d{2}-\d{2}$/.test(taxDay.date) && block.tax[taxRow.index].from !== taxDay.date) {
                                block.tax[taxRow.index].from = taxDay.date
                                block.save("tax", block.tax)
                            }
                        }
                    }
                    PlainComboBox {
                        model: block.view ? block.view.schemes.map(s => block.theme.plain(s.name)) : []
                        currentIndex: block.view ? Math.max(0, block.view.schemes.findIndex(s => s.id === block.tax[taxRow.index].scheme)) : 0
                        onActivated: index => {
                            block.tax[taxRow.index].scheme = block.view.schemes[index].id
                            block.save("tax", block.tax)
                        }
                    }
                    ToolButton {
                        text: "×"
                        Accessible.name: block.sioul.text("invoices-settings-remove")
                        onClicked: block.save("tax", block.tax.filter((_, i) => i !== taxRow.index))
                    }
                }
                TextField {
                    visible: block.tax[taxRow.index].scheme === "franchise"
                    Layout.fillWidth: true
                    text: block.tax[taxRow.index].mention
                    placeholderText: block.view ? block.theme.plain(block.view.mention_default) : ""
                    onEditingFinished: {
                        if (block.tax[taxRow.index].mention !== text.trim()) {
                            block.tax[taxRow.index].mention = text.trim()
                            block.save("tax", block.tax)
                        }
                    }
                }
            }
        }
        Button {
            flat: true
            text: "+  " + block.sioul.text("invoices-settings-scheme-add")
            onClicked: block.tax = block.tax.concat([{ from: block.tax.length === 0 ? "" : Qt.formatDate(new Date(), "yyyy-MM-dd"), scheme: block.view && block.view.schemes.length > 0 ? block.view.schemes[0].id : "none", mention: "" }])
        }
        // Who you are, where.
        Repeater {
            model: [["trade_name", "invoices-settings-trade-name"], ["address", "set-invoice-address"], ["postcode", "invoices-settings-postcode"], ["city", "invoices-settings-city"], ["country", "invoices-country"], ["email", "invoices-settings-email"]]

            delegate: ColumnLayout {
                id: field

                required property var modelData

                Layout.fillWidth: true
                spacing: 2

                Label {
                    text: block.sioul.text(field.modelData[1])
                    color: block.theme.text
                }
                TextField {
                    Layout.fillWidth: true
                    visible: field.modelData[0] !== "address"
                    text: block.view ? String(block.view[field.modelData[0]] || "") : ""
                    placeholderText: field.modelData[0] === "country" && block.view ? block.theme.plain(block.view.country_default) : ""
                    onEditingFinished: block.saveText(field.modelData[0], text)
                }
                TextArea {
                    Layout.fillWidth: true
                    visible: field.modelData[0] === "address"
                    text: block.view ? block.view.address : ""
                    wrapMode: TextEdit.Wrap
                    onEditingFinished: block.saveText("address", text)
                }
            }
        }
        // Bank accounts, and the payment QR code on euro invoices.
        Label {
            text: block.sioul.text("invoices-settings-banks")
            color: block.theme.text
        }
        Repeater {
            model: block.banks.length

            delegate: Flow {
                id: bankRow

                required property int index

                Layout.fillWidth: true
                spacing: 6

                Repeater {
                    model: [["holder", "invoices-settings-holder", 180], ["iban", "IBAN", 260], ["bic", "BIC", 110]]

                    delegate: TextField {
                        id: bankField

                        required property var modelData

                        width: Math.min(bankField.modelData[2], block.width)
                        text: block.banks[bankRow.index][bankField.modelData[0]]
                        placeholderText: block.theme.plain(bankField.modelData[1].indexOf("-") > 0 ? block.sioul.text(bankField.modelData[1]) : bankField.modelData[1])
                        onEditingFinished: {
                            if (block.banks[bankRow.index][bankField.modelData[0]] !== text.trim()) {
                                block.banks[bankRow.index][bankField.modelData[0]] = text.trim()
                                block.save("bank", block.banks)
                            }
                        }
                    }
                }
                ToolButton {
                    text: "×"
                    Accessible.name: block.sioul.text("invoices-settings-remove")
                    onClicked: block.save("bank", block.banks.filter((_, i) => i !== bankRow.index))
                }
                Label {
                    visible: block.view !== null && block.view.banks[bankRow.index] !== undefined && block.view.banks[bankRow.index].valid === false
                    text: block.sioul.text("invoices-settings-iban-wrong")
                    color: block.theme.warm
                }
            }
        }
        Button {
            flat: true
            text: "+  " + block.sioul.text("invoices-settings-bank-add")
            onClicked: block.banks = block.banks.concat([{ holder: "", iban: "", bic: "" }])
        }
        Switch {
            text: block.sioul.text("invoices-settings-qr")
            checked: block.view !== null && block.view.qr
            onToggled: block.save("qr", checked)
        }
        Label {
            text: block.sioul.text("set-invoice-payment")
            color: block.theme.text
        }
        TextArea {
            Layout.fillWidth: true
            text: block.view ? block.view.payment : ""
            wrapMode: TextEdit.Wrap
            onEditingFinished: block.saveText("payment", text)
        }
        // Terms and the late-payment rate.
        Flow {
            Layout.fillWidth: true
            spacing: 6

            Label {
                height: termsField.height
                verticalAlignment: Text.AlignVCenter
                text: block.sioul.text("invoices-terms-days")
                color: block.theme.text
            }
            TextField {
                id: termsField

                width: 70
                inputMethodHints: Qt.ImhDigitsOnly
                text: block.view && block.view.terms_days !== null ? String(block.view.terms_days) : ""
                placeholderText: block.view ? block.theme.plain(String(block.view.terms_default)) : ""
                onEditingFinished: {
                    const days = parseInt(text, 10)
                    block.save("terms_days", isNaN(days) ? "" : days)
                }
            }
            CheckBox {
                text: block.sioul.text("invoices-end-of-month")
                checked: block.view !== null && block.view.end_of_month
                onToggled: block.save("end_of_month", checked)
            }
        }
        Label {
            text: block.sioul.text("invoices-settings-penalty")
            color: block.theme.text
        }
        TextField {
            Layout.fillWidth: true
            inputMethodHints: Qt.ImhFormattedNumbersOnly
            text: block.view && block.view.penalty_rate !== null ? String(block.view.penalty_rate) : ""
            placeholderText: block.view ? block.theme.plain(block.view.penalty_default) : ""
            onEditingFinished: {
                const rate = block.number(text)
                block.save("penalty_rate", isNaN(rate) ? "" : rate)
            }
        }
        // Language and currency of invoices, by default.
        Flow {
            Layout.fillWidth: true
            spacing: 6

            PlainComboBox {
                model: [block.sioul.text("invoices-settings-language-window"), block.sioul.text("invoices-language-en"), block.sioul.text("invoices-language-fr")]
                currentIndex: block.view ? Math.max(0, ["", "en", "fr"].indexOf(block.view.language)) : 0
                onActivated: index => block.save("language", ["", "en", "fr"][index])
            }
            TextField {
                width: 90
                text: block.view ? block.view.currency : ""
                placeholderText: block.view ? block.theme.plain(block.view.currency_default) : "EUR"
                Accessible.name: block.sioul.text("invoices-currency")
                onEditingFinished: block.saveText("currency", text.toUpperCase())
            }
        }
        // The numbering.
        Label {
            text: block.sioul.text("invoices-settings-pattern")
            color: block.theme.text
        }
        Flow {
            Layout.fillWidth: true
            spacing: 6

            TextField {
                objectName: "invoicePattern"
                width: 180
                text: block.view ? block.view.pattern : ""
                placeholderText: block.view ? block.theme.plain(block.view.pattern_default) : ""
                onEditingFinished: block.saveText("pattern", text)
            }
            PlainComboBox {
                model: [block.sioul.text("invoices-settings-reset-yearly"), block.sioul.text("invoices-settings-reset-never")]
                currentIndex: block.view && block.view.reset === "never" ? 1 : 0
                onActivated: index => block.save("reset", index === 1 ? "never" : "yearly")
            }
            TextField {
                width: 180
                text: block.view ? block.view.credit_pattern : ""
                placeholderText: block.sioul.text("invoices-settings-credit-pattern")
                Accessible.name: placeholderText
                onEditingFinished: block.saveText("credit_pattern", text)
            }
        }
        Label {
            Layout.fillWidth: true
            text: block.view ? (block.view.pattern_problem !== "" ? block.view.pattern_problem : block.sioul.textWith("invoices-settings-next", "number", block.view.next)) : ""
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            font.pixelSize: 12
            color: block.view && block.view.pattern_problem !== "" ? block.theme.warm : block.theme.muted
        }
        // The legal lines: each of the preset's on or off, in its words or yours; your own after them.
        Label {
            text: block.sioul.text("invoices-settings-lines")
            color: block.theme.text
        }
        Repeater {
            model: block.lines.length

            delegate: ColumnLayout {
                id: lineRow

                required property int index
                readonly property var line: block.lines[lineRow.index]

                Layout.fillWidth: true
                spacing: 2

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 4

                    Switch {
                        Layout.alignment: Qt.AlignTop
                        checked: !lineRow.line.off
                        Accessible.name: lineRow.line.preset ? lineRow.line.text : block.sioul.text("invoices-settings-own-line")
                        onToggled: {
                            block.lines[lineRow.index].off = !checked
                            block.save("line", block.lineRows())
                        }
                    }
                    Label {
                        Layout.fillWidth: true
                        text: lineRow.line.preset ? lineRow.line.text : block.sioul.text("invoices-settings-own-line")
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        font.pixelSize: 13
                        color: lineRow.line.off ? block.theme.muted : block.theme.text
                    }
                }
                TextField {
                    visible: !lineRow.line.off
                    Layout.fillWidth: true
                    Layout.leftMargin: 24
                    text: lineRow.line.own
                    placeholderText: block.sioul.text(lineRow.line.preset ? "invoices-settings-line-own-words" : "invoices-settings-line-text")
                    onEditingFinished: {
                        if (block.lines[lineRow.index].own !== text.trim()) {
                            block.lines[lineRow.index].own = text.trim()
                            block.save("line", block.lineRows())
                        }
                    }
                }
            }
        }
        Button {
            flat: true
            text: "+  " + block.sioul.text("invoices-settings-line-add")
            onClicked: block.lines = block.lines.concat([{ id: "", text: "", own: "", subject: "", off: false, preset: false }])
        }
        Label {
            text: block.sioul.text("invoices-settings-insurance")
            color: block.theme.text
        }
        TextField {
            Layout.fillWidth: true
            text: block.view ? block.view.insurance : ""
            onEditingFinished: block.saveText("insurance", text)
        }
        Label {
            visible: block.view !== null && block.view.company
            text: block.sioul.text("invoices-settings-capital")
            color: block.theme.text
        }
        TextField {
            visible: block.view !== null && block.view.company
            Layout.fillWidth: true
            text: block.view ? block.view.capital : ""
            placeholderText: "1 000 €"
            onEditingFinished: block.saveText("capital", text)
        }
    }

    // The books: what is declared, how often, and the rates of your own.
    BareButton {
        objectName: "invoiceSettingsBooks"
        visible: block.view !== null && block.view.declares
        flat: true
        text: (block.books ? "▾  " : "▸  ") + block.sioul.text("invoices-settings-books")
        onClicked: block.books = !block.books
    }
    ColumnLayout {
        visible: block.books && block.view !== null && block.view.declares
        Layout.fillWidth: true
        Layout.leftMargin: 8
        spacing: 6

        Label {
            text: block.sioul.text("invoices-settings-activities")
            color: block.theme.text
        }
        Repeater {
            model: block.view ? block.view.activity_choices : []

            delegate: CheckBox {
                id: activity

                required property var modelData

                text: block.theme.plain(activity.modelData.name)
                checked: block.view !== null && block.view.activities.indexOf(activity.modelData.id) >= 0
                onToggled: block.save("activity", activity.checked ? block.view.activities.concat([activity.modelData.id]) : block.view.activities.filter(a => a !== activity.modelData.id))
            }
        }
        Flow {
            Layout.fillWidth: true
            spacing: 6

            Label {
                height: everyChoice.height
                verticalAlignment: Text.AlignVCenter
                text: block.sioul.text("invoices-settings-declare")
                color: block.theme.text
            }
            PlainComboBox {
                id: everyChoice

                model: block.view ? [block.sioul.text("invoices-settings-unsaid")].concat(block.view.periods.map(p => block.theme.plain(p.name))) : []
                currentIndex: block.view ? block.view.periods.findIndex(p => p.id === block.view.declare) + 1 : 0
                onActivated: index => block.save("declare", index === 0 ? "" : block.view.periods[index - 1].id)
            }
        }
        // The trade the training contribution is counted for; unsaid, as the activity tells.
        Flow {
            Layout.fillWidth: true
            spacing: 6

            Label {
                height: tradeChoice.height
                verticalAlignment: Text.AlignVCenter
                text: block.sioul.text("invoices-settings-trade")
                color: block.theme.text
            }
            PlainComboBox {
                id: tradeChoice

                readonly property var trades: ["", "commerce", "craft", "liberal"]

                objectName: "invoiceTrade"
                model: tradeChoice.trades.map(t => block.sioul.text(t === "" ? "invoices-settings-trade-unsaid" : "invoices-settings-trade-" + t))
                currentIndex: block.view ? Math.max(0, tradeChoice.trades.indexOf(block.view.trade || "")) : 0
                onActivated: index => block.save("trade", tradeChoice.trades[index])
            }
        }
        Switch {
            text: block.sioul.text("invoices-settings-vl")
            checked: block.view !== null && block.view.vl
            onToggled: block.save("vl", checked)
        }
        Label {
            text: block.sioul.text("invoices-settings-started")
            color: block.theme.text
        }
        TextField {
            Layout.fillWidth: true
            text: block.view ? block.view.started : ""
            placeholderText: "2026-03-01"
            onEditingFinished: block.saveText("started", text)
        }
        Flow {
            Layout.fillWidth: true
            spacing: 6

            Label {
                height: vatEvery.height
                verticalAlignment: Text.AlignVCenter
                text: block.sioul.text("invoices-settings-vat-period")
                color: block.theme.text
            }
            PlainComboBox {
                id: vatEvery

                model: [block.sioul.text("invoices-every-month"), block.sioul.text("invoices-every-quarter")]
                currentIndex: block.view && block.view.vat_period === "quarter" ? 1 : 0
                onActivated: index => block.save("vat_period", index === 1 ? "quarter" : "")
            }
        }
        CheckBox {
            text: block.sioul.text("invoices-settings-debits")
            checked: block.view !== null && block.view.debits
            onToggled: block.save("debits", checked)
        }
        // Rates of your own (ACRE's, a rate URSSAF gave you), each from a day until a day.
        Label {
            text: block.sioul.text("invoices-settings-rates")
            color: block.theme.text
        }
        Repeater {
            model: block.contributions.length

            delegate: Flow {
                id: rateRow

                required property int index
                readonly property var kinds: ["social", "vl", "cfp"]

                Layout.fillWidth: true
                spacing: 6

                PlainComboBox {
                    model: rateRow.kinds.map(k => block.sioul.text("invoices-settings-rate-" + k))
                    currentIndex: Math.max(0, rateRow.kinds.indexOf(block.contributions[rateRow.index].kind))
                    onActivated: index => {
                        block.contributions[rateRow.index].kind = rateRow.kinds[index]
                        block.save("contribution", block.contributions)
                    }
                }
                TextField {
                    width: 120
                    text: block.contributions[rateRow.index].activity
                    placeholderText: block.sioul.text("invoices-settings-rate-activity")
                    onEditingFinished: {
                        block.contributions[rateRow.index].activity = text.trim()
                        block.save("contribution", block.contributions)
                    }
                }
                TextField {
                    width: 70
                    inputMethodHints: Qt.ImhFormattedNumbersOnly
                    text: String(block.contributions[rateRow.index].rate)
                    Accessible.name: block.sioul.text("invoices-settings-rate-percent")
                    onEditingFinished: {
                        const rate = block.number(text)
                        block.contributions[rateRow.index].rate = isNaN(rate) ? 0 : rate
                        block.save("contribution", block.contributions)
                    }
                }
                TextField {
                    width: 120
                    text: block.contributions[rateRow.index].from
                    placeholderText: block.sioul.text("invoices-settings-from-always")
                    onEditingFinished: {
                        block.contributions[rateRow.index].from = text.trim()
                        block.save("contribution", block.contributions)
                    }
                }
                TextField {
                    width: 120
                    text: block.contributions[rateRow.index].until
                    placeholderText: block.sioul.text("invoices-settings-until-always")
                    onEditingFinished: {
                        block.contributions[rateRow.index].until = text.trim()
                        block.save("contribution", block.contributions)
                    }
                }
                ToolButton {
                    text: "×"
                    Accessible.name: block.sioul.text("invoices-settings-remove")
                    onClicked: block.save("contribution", block.contributions.filter((_, i) => i !== rateRow.index))
                }
            }
        }
        Button {
            flat: true
            text: "+  " + block.sioul.text("invoices-settings-rate-add")
            onClicked: block.contributions = block.contributions.concat([{ kind: "social", activity: block.view && block.view.activities.length > 0 ? block.view.activities[0] : "", rate: 0, from: "", until: "" }])
        }
    }

    // The e-invoicing platform you use; until Sioul sends through it, the guide says how to upload.
    Label {
        Layout.topMargin: 6
        text: block.sioul.text("invoices-settings-platform")
        color: block.theme.text
    }
    TextField {
        objectName: "invoicePlatform"
        Layout.fillWidth: true
        text: block.view ? block.view.platform : ""
        placeholderText: block.sioul.text("invoices-settings-platform-hint")
        onEditingFinished: block.saveText("platform", text)
    }
    Label {
        Layout.fillWidth: true
        text: block.sioul.text("invoices-settings-platform-help")
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.pixelSize: 12
        color: block.theme.muted
    }
}
