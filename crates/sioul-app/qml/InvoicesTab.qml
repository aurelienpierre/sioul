// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Money ▸ Invoices (docs/projects.md, "Invoices in the window"; the
// invoicing design §9): every draft, then every invoice by its day, the
// newest first, each with where it stands in words (a draft, not paid yet,
// partly paid, paid, credited, to pay back: nothing "overdue", nothing red),
// filtered by client, year and status; New; For the declaration, the
// receipts book and the declarations beside them. A draft opens in its
// editor (InvoiceEditor.qml); an issued invoice in its pane, with Back:
// what it holds, its payments, what changed outside Sioul, and its acts
// (Print again, Paid…, Credit note…, Write the mail, a deposit's final
// invoice), its menu the same on a long press, a right click or "⋯".

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import com.aurelienpierre.sioul

Item {
    id: tab

    required property var sioul
    required property var theme
    required property var window
    // What the filters ask: {client, year, status}, "" for any.
    property var asked: ({ client: "", year: "", status: "" })
    // The list's answer: {rows, clients, years, statuses, said, declares}.
    property var shown: ({ rows: [], clients: [], years: [], statuses: [], said: [], declares: false })
    // An issued invoice in its pane (`invoiceShown`); null for none.
    property var invoice: null
    // A draft in the editor, by its id; "" for none.
    property string draftId: ""
    property string problem: ""
    readonly property bool compact: tab.window.compact === true
    readonly property var editor: editorLoader.item

    // "For the declaration": the page shows the declaration view, on `anchor` ("" today's).
    signal declarationAsked(string anchor)

    function reload() {
        tab.read()
        if (tab.invoice !== null)
            tab.openInvoice(tab.invoice.number)
    }

    function read() {
        const answer = JSON.parse(tab.sioul.invoices(JSON.stringify(tab.asked)) || "{}")
        tab.shown = Object.assign({ rows: [], clients: [], years: [], statuses: [], said: [], declares: false }, answer)
    }

    // A filter changed: {its field: its value}.
    function ask(change) {
        tab.asked = Object.assign({}, tab.asked, change)
        tab.read()
    }

    function openRow(row) {
        if (row.draft !== "")
            tab.openDraft(row.draft)
        else
            tab.openInvoice(row.number)
    }

    // A draft in the editor: read by it at once when it is made already,
    // else when it is made (the Loader's onLoaded), never twice.
    function openDraft(id) {
        tab.invoice = null
        tab.problem = ""
        const made = tab.editor !== null
        tab.draftId = id
        if (made)
            (tab.editor as InvoiceEditor).load(id)
    }

    // An issued invoice in its pane.
    function openInvoice(number) {
        const answer = JSON.parse(tab.sioul.invoiceShown(number) || "{}")
        if (answer.error) {
            tab.problem = answer.error
            tab.invoice = null
            return
        }
        tab.draftId = ""
        tab.invoice = answer
    }

    // Back to the list: what was typed in the draft written first, a draft
    // made and never written in taken out (the editor's `leave`).
    function back() {
        // A save that failed keeps the editor open once (its `leave`, R5).
        if (tab.draftId !== "" && tab.editor !== null && !(tab.editor as InvoiceEditor).leave())
            return
        tab.draftId = ""
        tab.invoice = null
        tab.read()
    }

    // What was typed in the editor, written now (Quit, Sioul put away).
    function flush() {
        if (tab.draftId !== "" && tab.editor !== null)
            (tab.editor as InvoiceEditor).flush()
    }

    // New: a blank draft, its client chosen in the editor.
    function startNew() {
        const made = JSON.parse(tab.sioul.newInvoiceDraft("{}") || "{}")
        if (made.error) {
            tab.problem = made.error
            return
        }
        tab.read()
        tab.openDraft(made.draft)
    }

    // A draft made elsewhere (a credit note, a deposit, a final invoice): opened, the list read again.
    function made(answer) {
        const made = typeof answer === "string" ? JSON.parse(answer || "{}") : answer
        if (made.error) {
            tab.problem = made.error
            return
        }
        tab.read()
        tab.openDraft(made.draft)
        // What it found to say: the time Final invoice… added to the draft.
        if (made.said)
            tab.sioul.status = made.said
    }

    // An issued invoice's file: the one kept, copied where invoices go; an
    // invoice from before November 2026, which kept none, printed from its page.
    function printed(made) {
        const failed = made.html ? pdf.write(made.html, made.pdf) : ""
        if (failed !== "") {
            tab.problem = tab.sioul.textWith("invoice-not-written", "path", failed)
            return
        }
        const said = []
        if (made.pdf)
            said.push(tab.sioul.textWith("invoice-written", "path", made.pdf))
        if (made.said)
            said.push(made.said)
        tab.sioul.status = said.join(" ")
        // Shown in your PDF reader, except while the window takes its own pictures.
        if (made.pdf && tab.sioul.grabFolder() === "")
            Qt.openUrlExternally(tab.theme.fileUrl(made.pdf))
    }

    function printAgain(number) {
        const again = JSON.parse(tab.sioul.invoiceAgain(number) || "{}")
        if (again.error)
            tab.problem = again.error
        else
            tab.printed(again)
    }

    // Issued from the editor: its file shown, its pane opened.
    function issued(made) {
        tab.printed(made)
        tab.read()
        tab.openInvoice(made.number)
    }

    function writeMail(number) {
        const answer = JSON.parse(tab.sioul.writeInvoiceMail(number) || "{}")
        if (answer.error) {
            tab.problem = answer.error
            return
        }
        tab.window.openDraft(answer.draft)
    }

    // Paid… (Paid back… on a credit note), Credit note…, a deposit invoice: their forms.
    function askPaid(invoice) {
        paidForm.now().ask(invoice)
    }

    function askCredit(invoice) {
        creditForm.now().ask(invoice)
    }

    function askDeposit(of) {
        depositForm.now().ask(of)
    }

    // A row's menu: at the finger on a phone.
    function menuOf(row, item, x, y) {
        if (item && x !== undefined)
            item.Window.window.menuAt = item.mapToItem(null, x, y)
        rowMenu.now().show(row)
    }

    // Every form and menu closed (the window's pictures).
    function closeForms() {
        paidForm.close()
        creditForm.close()
        depositForm.close()
        rowMenu.close()
    }

    Component.onCompleted: tab.read()
    // Back in view: the list read again, and the draft open in the editor
    // when it changed elsewhere meanwhile (the second review of phase 3, R6).
    onVisibleChanged: {
        if (!tab.visible)
            return
        tab.reload()
        if (tab.draftId !== "" && tab.editor !== null)
            (tab.editor as InvoiceEditor).changedElsewhere()
    }

    Connections {
        target: tab.sioul

        // An invoice, a draft or a client profile written here or on another
        // device: the list read again; the draft in the editor read again
        // while nothing typed waits, else at its next save (its revision).
        function onInvoicesChanged() {
            if (!tab.visible)
                return
            if (tab.draftId === "")
                tab.reload()
            else if (tab.editor !== null)
                (tab.editor as InvoiceEditor).changedElsewhere()
        }

        function onMoneyChanged() {
            if (tab.visible && tab.draftId === "")
                tab.reload()
        }
    }

    PdfWriter {
        id: pdf
    }

    // The list, its filters, New and the declaration.
    ColumnLayout {
        objectName: "invoicesList"
        visible: tab.draftId === "" && tab.invoice === null
        anchors.fill: parent
        spacing: 6

        Flow {
            Layout.fillWidth: true
            spacing: 6

            Button {
                objectName: "invoicesNew"
                text: tab.sioul.text("invoices-new")
                icon.name: "list-add"
                icon.color: tab.theme.text
                onClicked: tab.startNew()
            }
            Button {
                objectName: "invoicesDeclaration"
                visible: tab.shown.declares
                flat: true
                text: tab.sioul.text("receipts-for-declaration")
                icon.name: "taxes-finances"
                icon.color: tab.theme.text
                onClicked: tab.declarationAsked("")
            }
            PlainComboBox {
                objectName: "invoicesClientFilter"
                implicitContentWidthPolicy: ComboBox.WidestText
                visible: tab.shown.clients.length > 1
                model: [tab.sioul.text("invoices-any-client")].concat(tab.shown.clients.map(c => tab.theme.plain(c)))
                currentIndex: tab.asked.client === "" ? 0 : tab.shown.clients.indexOf(tab.asked.client) + 1
                onActivated: index => tab.ask({ client: index === 0 ? "" : tab.shown.clients[index - 1] })
            }
            PlainComboBox {
                objectName: "invoicesYearFilter"
                implicitContentWidthPolicy: ComboBox.WidestText
                visible: tab.shown.years.length > 1
                model: [tab.sioul.text("invoices-any-year")].concat(tab.shown.years.map(y => tab.theme.plain(y)))
                currentIndex: tab.asked.year === "" ? 0 : tab.shown.years.indexOf(tab.asked.year) + 1
                onActivated: index => tab.ask({ year: index === 0 ? "" : tab.shown.years[index - 1] })
            }
            PlainComboBox {
                objectName: "invoicesStatusFilter"
                // "Tous les états" whole (F17).
                implicitContentWidthPolicy: ComboBox.WidestText
                visible: tab.shown.statuses.length > 1 || tab.asked.status !== ""
                model: [tab.sioul.text("invoices-any-status")].concat(tab.shown.statuses.map(s => tab.theme.plain(s.label)))
                currentIndex: tab.asked.status === "" ? 0 : tab.shown.statuses.findIndex(s => s.id === tab.asked.status) + 1
                onActivated: index => tab.ask({ status: index === 0 ? "" : tab.shown.statuses[index - 1].id })
            }
        }
        Label {
            visible: tab.problem !== ""
            Layout.fillWidth: true
            text: tab.problem
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: tab.theme.warm
        }
        // What the files say besides: a number with no record here, one given twice.
        Repeater {
            model: tab.shown.said

            delegate: Label {
                required property string modelData

                Layout.fillWidth: true
                text: modelData
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: tab.theme.muted
            }
        }
        Label {
            visible: tab.shown.rows.length === 0
            Layout.fillWidth: true
            text: tab.sioul.text("invoices-none")
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: tab.theme.muted
        }

        // An invoice made, sent, paid or deleted: the list keeps its place (ListPlace.qml); other
        // filters, from its top.
        ListPlace {
            id: listPlace

            view: list
            source: tab.shown.rows
            scope: tab.asked
        }
        ListView {
            id: list

            objectName: "invoicesRows"
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            model: listPlace.rows
            reuseItems: true
            ScrollBar.vertical: ScrollBar {}

            delegate: ItemDelegate {
                id: line

                required property var modelData

                objectName: "invoiceRow"
                width: list.width - 12
                padding: 4
                leftPadding: 0
                Accessible.name: [line.modelData.title, line.modelData.client, line.modelData.total, line.modelData.status_words].join(", ")
                onClicked: tab.openRow(line.modelData)
                onPressAndHold: tab.menuOf(line.modelData, line, line.pressX, line.pressY)

                TapHandler {
                    acceptedButtons: Qt.RightButton
                    acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
                    onTapped: tab.menuOf(line.modelData)
                }

                contentItem: ColumnLayout {
                    spacing: 0

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 8

                        Label {
                            Layout.preferredWidth: 80
                            Layout.fillWidth: true
                            text: line.modelData.client !== "" ? line.modelData.client : line.modelData.title
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            color: tab.theme.text
                        }
                        Label {
                            text: line.modelData.total
                            textFormat: Text.PlainText
                            horizontalAlignment: Text.AlignRight
                            font.features: { "tnum": 1 }
                            color: tab.theme.text
                        }
                        // Its menu, on a phone too (a long press opens it as well).
                        ToolButton {
                            icon.name: "overflow-menu"
                            icon.color: tab.theme.text
                            Accessible.name: tab.sioul.text("invoices-menu")
                            onClicked: tab.menuOf(line.modelData)
                        }
                    }
                    Label {
                        Layout.fillWidth: true
                        text: [line.modelData.number !== "" ? line.modelData.number : line.modelData.title, line.modelData.date, line.modelData.status_words, line.modelData.left, line.modelData.sent, line.modelData.outside ? tab.sioul.text("invoices-changed-outside") : ""].filter(t => t !== "").join("  ·  ")
                        textFormat: Text.PlainText
                        elide: Text.ElideRight
                        font.pixelSize: 12
                        color: line.modelData.status === "draft" ? tab.theme.accent : tab.theme.muted
                    }
                }
            }
        }
    }

    // An issued invoice: what it holds, where it stands, its acts.
    ScrollView {
        id: pane

        objectName: "invoicePane"
        visible: tab.invoice !== null
        anchors.fill: parent
        contentWidth: availableWidth

        ColumnLayout {
            width: pane.availableWidth
            spacing: 8

            RowLayout {
                Layout.fillWidth: true
                spacing: 6

                Button {
                    objectName: "invoicePaneBack"
                    flat: true
                    text: tab.compact ? "" : tab.sioul.text("ui-back")
                    icon.name: "go-previous"
                    icon.color: tab.theme.text
                    Accessible.name: tab.sioul.text("ui-back")
                    onClicked: tab.back()
                }
                Label {
                    Layout.fillWidth: true
                    text: tab.invoice ? tab.invoice.title : ""
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    font.pixelSize: 19
                    color: tab.theme.text
                }
                ToolButton {
                    icon.name: "overflow-menu"
                    icon.color: tab.theme.text
                    Accessible.name: tab.sioul.text("invoices-menu")
                    onClicked: tab.menuOf({ number: tab.invoice.number, draft: "", kind: tab.invoice.kind, title: tab.invoice.title, status: tab.invoice.status })
                }
            }
            Label {
                Layout.fillWidth: true
                text: tab.invoice ? [tab.invoice.client.split("\n")[0], tab.invoice.project, tab.invoice.date].filter(t => t !== "").join("  ·  ") : ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: tab.theme.muted
            }
            Label {
                objectName: "invoicePaneStatus"
                Layout.fillWidth: true
                text: tab.invoice ? [tab.invoice.total, tab.invoice.status_words, tab.invoice.left_words, tab.invoice.sent].filter(t => t !== "").join("  ·  ") : ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 17
                color: tab.theme.text
            }
            Label {
                visible: tab.invoice !== null && (tab.invoice.due !== "" && tab.invoice.acts.paid || tab.invoice.corrects !== "")
                Layout.fillWidth: true
                text: tab.invoice ? [tab.invoice.corrects, tab.invoice.acts.paid && tab.invoice.due !== "" ? tab.sioul.textWith("invoices-due-on", "day", tab.invoice.due) : ""].filter(t => t !== "").join("  ·  ") : ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
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
            // Changed outside Sioul since it was issued: said calmly, once.
            Repeater {
                model: tab.invoice ? tab.invoice.outside : []

                delegate: Label {
                    required property string modelData

                    objectName: "invoiceOutside"
                    Layout.fillWidth: true
                    text: modelData
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: tab.theme.warm
                }
            }
            // Its acts.
            Flow {
                Layout.fillWidth: true
                spacing: 6

                Button {
                    objectName: "invoicePaid"
                    visible: tab.invoice !== null && tab.invoice.acts.paid
                    text: tab.sioul.text("invoices-paid-ask")
                    highlighted: true
                    onClicked: tab.askPaid(tab.invoice)
                }
                Button {
                    objectName: "invoicePaidBack"
                    visible: tab.invoice !== null && tab.invoice.acts.paid_back
                    text: tab.sioul.text("invoices-paid-back-ask")
                    highlighted: true
                    onClicked: tab.askPaid(tab.invoice)
                }
                Button {
                    objectName: "invoiceMail"
                    visible: tab.invoice !== null && tab.invoice.acts.mail
                    text: tab.sioul.text("invoices-write-mail")
                    icon.name: "mail-message-new"
                    icon.color: tab.theme.text
                    onClicked: tab.writeMail(tab.invoice.number)
                }
                Button {
                    objectName: "invoicePrint"
                    text: tab.sioul.text("invoice-print")
                    onClicked: tab.printAgain(tab.invoice.number)
                }
                Button {
                    objectName: "invoiceCredit"
                    visible: tab.invoice !== null && tab.invoice.acts.credit
                    flat: true
                    text: tab.sioul.text("invoices-credit-ask")
                    onClicked: tab.askCredit(tab.invoice)
                }
                Button {
                    objectName: "invoiceFinal"
                    visible: tab.invoice !== null && tab.invoice.acts.final
                    flat: true
                    text: tab.sioul.text("invoices-final-ask")
                    onClicked: tab.made(tab.sioul.finalInvoice(tab.invoice.number))
                }
            }
            // Its payments, each on its day.
            Label {
                visible: tab.invoice !== null && tab.invoice.payments.length > 0
                Layout.topMargin: 4
                text: tab.sioul.text(tab.invoice && tab.invoice.kind === "credit-note" ? "invoices-paid-back-list" : "invoices-payments")
                font.weight: Font.DemiBold
                color: tab.theme.text
            }
            Repeater {
                model: tab.invoice ? tab.invoice.payments : []

                delegate: Label {
                    required property var modelData

                    objectName: "invoicePayment"
                    Layout.fillWidth: true
                    text: modelData.words
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: tab.theme.text
                }
            }
            Label {
                visible: tab.invoice !== null && (tab.invoice.credit_notes.length > 0 || tab.invoice.deducted_by.length > 0)
                Layout.fillWidth: true
                text: tab.invoice ? [tab.invoice.credit_notes.length > 0 ? tab.sioul.textWith("invoices-credited-by", "numbers", tab.invoice.credit_notes.join(", ")) : "", tab.invoice.deducted_by.length > 0 ? tab.sioul.textWith("invoices-deducted-by", "numbers", tab.invoice.deducted_by.join(", ")) : ""].filter(t => t !== "").join("  ·  ") : ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: tab.theme.muted
            }
            // What it bills.
            Label {
                Layout.topMargin: 4
                text: tab.sioul.text("invoices-lines")
                font.weight: Font.DemiBold
                color: tab.theme.text
            }
            Repeater {
                model: tab.invoice ? tab.invoice.lines : []

                delegate: RowLayout {
                    id: billed

                    required property var modelData

                    Layout.fillWidth: true
                    spacing: 8

                    Label {
                        Layout.preferredWidth: 80
                        Layout.fillWidth: true
                        text: [billed.modelData.description, billed.modelData.time].filter(t => t !== "").join("  ·  ")
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: tab.theme.text
                    }
                    Label {
                        text: billed.modelData.amount
                        textFormat: Text.PlainText
                        font.features: { "tnum": 1 }
                        color: tab.theme.text
                    }
                }
            }
        }
    }

    // A draft in its editor: made the first time one opens.
    Loader {
        id: editorLoader

        visible: tab.draftId !== ""
        anchors.fill: parent
        active: tab.draftId !== "" || editorLoader.item !== null

        sourceComponent: Component {
            InvoiceEditor {
                sioul: tab.sioul
                theme: tab.theme
                window: tab.window
                onClosed: tab.back()
                onIssued: made => tab.issued(made)
                onDepositAsked: id => tab.askDeposit(id)
            }
        }
        onLoaded: {
            if (tab.draftId !== "")
                (editorLoader.item as InvoiceEditor).load(tab.draftId)
        }
    }

    // A row's menu: Open, Paid…, Write the mail, Print again, Credit note…, Discard.
    Later {
        id: rowMenu

        sourceComponent: Component {
            SioulMenu {
                id: menu

                property var row: null
                readonly property bool draft: menu.row !== null && menu.row.draft !== ""

                function show(row) {
                    menu.row = row
                    menu.popup()
                }

                MenuItem {
                    objectName: "invoiceMenuOpen"
                    text: menu.draft ? tab.theme.plain(menu.row.title) : tab.sioul.text("ui-open")
                    onTriggered: tab.openRow(menu.row)
                }
                MenuItem {
                    objectName: "invoiceMenuPaid"
                    visible: !menu.draft && menu.row !== null && (menu.row.status === "unpaid" || menu.row.status === "partly-paid" || menu.row.status === "to-pay-back")
                    height: visible ? implicitHeight : 0
                    text: tab.sioul.text(menu.row && menu.row.kind === "credit-note" ? "invoices-paid-back-ask" : "invoices-paid-ask")
                    onTriggered: {
                        const shown = JSON.parse(tab.sioul.invoiceShown(menu.row.number) || "{}")
                        if (!shown.error)
                            tab.askPaid(shown)
                    }
                }
                MenuItem {
                    visible: !menu.draft
                    height: visible ? implicitHeight : 0
                    text: tab.sioul.text("invoices-write-mail")
                    onTriggered: tab.writeMail(menu.row.number)
                }
                MenuItem {
                    visible: !menu.draft
                    height: visible ? implicitHeight : 0
                    text: tab.sioul.text("invoice-print")
                    onTriggered: tab.printAgain(menu.row.number)
                }
                MenuItem {
                    visible: !menu.draft && menu.row !== null && menu.row.kind !== "credit-note" && menu.row.status !== "credited"
                    height: visible ? implicitHeight : 0
                    text: tab.sioul.text("invoices-credit-ask")
                    onTriggered: {
                        const shown = JSON.parse(tab.sioul.invoiceShown(menu.row.number) || "{}")
                        if (!shown.error)
                            tab.askCredit(shown)
                    }
                }
                MenuItem {
                    objectName: "invoiceMenuDiscard"
                    // Never a draft a newer Sioul wrote: read only here (R13).
                    visible: menu.draft && menu.row.read_only !== true
                    height: visible ? implicitHeight : 0
                    text: tab.sioul.text("invoices-discard")
                    onTriggered: {
                        tab.problem = tab.sioul.discardInvoiceDraft(menu.row.draft)
                        tab.read()
                    }
                }
            }
        }
    }

    Later {
        id: paidForm

        sourceComponent: Component {
            InvoicePaidDialog {
                sioul: tab.sioul
                theme: tab.theme
                onPaid: tab.reload()
            }
        }
    }

    Later {
        id: creditForm

        sourceComponent: Component {
            InvoiceCreditDialog {
                sioul: tab.sioul
                theme: tab.theme
                onMade: id => {
                    tab.read()
                    tab.openDraft(id)
                }
            }
        }
    }

    Later {
        id: depositForm

        sourceComponent: Component {
            InvoiceDepositDialog {
                sioul: tab.sioul
                theme: tab.theme
                onMade: id => {
                    tab.read()
                    tab.openDraft(id)
                }
            }
        }
    }
}
