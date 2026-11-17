// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Money ▸ Invoices (InvoicesTab.qml, InvoiceEditor.qml, the Paid…, Credit
// note… and Deposit invoice… dialogs, ClientProfileDialog.qml; docs/
// projects.md, "Invoices in the window"), on a stand-in for Sioul: every
// draft and invoice with where it stands in words, the filters asking the
// core; New opening the editor; the editor sending what is typed (a line's
// words after the typing stops, a line added with no place of its own), the
// lines from time folded, Issue waiting while something required is
// missing; an issued invoice's pane with Paid… (today, by transfer, what is
// left), Credit note… (all lines, or those ticked), Write the mail; a
// deposit invoice by a share; a client's profile read and saved; at a
// phone's 360 points nothing passes the edge. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 1000
    height: 800

    Theme {
        id: testTheme
    }

    // Made up: fictional clients, example.org.
    readonly property var rows: [
        { key: "draft:20261117-090000-abcdef", draft: "20261117-090000-abcdef", number: "", kind: "invoice", title: "Draft invoice", client: "Studio Lumen", project: "The new site", day: "2026-11-17", date: "Tue 17 Nov", year: "2026", total: "€1,320", status: "draft", status_words: "draft", left: "", sent: "", outside: false, copy_of: "" },
        { key: "2026-014", draft: "", number: "2026-014", kind: "invoice", title: "Invoice No. 2026-014", client: "Fernhill Library", project: "", day: "2026-10-30", date: "Fri 30 Oct", year: "2026", total: "€990", status: "unpaid", status_words: "not paid yet", left: "", sent: "sent on Mon 2 Nov", outside: false, copy_of: "" },
        { key: "2026-013", draft: "", number: "2026-013", kind: "invoice", title: "Invoice No. 2026-013", client: "Silkworks Studio", project: "", day: "2026-10-12", date: "Mon 12 Oct", year: "2026", total: "€600", status: "partly-paid", status_words: "partly paid", left: "€200 left to pay", sent: "", outside: true, copy_of: "" }
    ]
    readonly property var lines: [
        { from: 0, description: "Site map", details: "", quantity: 90, unit: "MIN", price: 55, discount: 0, nature: "service", activity: "", own_tax: false, category: "E", rate: 0, exemption: "", time: true, time_words: "1 h 30", amount: "€82.50", tax_words: "Exempt from VAT" },
        { from: 1, description: "Printing", details: "", quantity: 2, unit: "C62", price: 30, discount: 0, nature: "goods", activity: "", own_tax: false, category: "E", rate: 0, exemption: "", time: false, time_words: "", amount: "€60", tax_words: "Exempt from VAT" }
    ]
    function draftView(required) {
        return { id: "20261117-090000-abcdef", kind: "invoice", title: "Draft invoice", copy_of: "", project: "The new site", client: { ref: "sioul:contact/lumen", name: "Studio Lumen", first: "Studio Lumen", kind: "business", kind_words: "A business", country: "FR", country_name: "France", email: "billing@example.org", address: "", profile: "p1", contact: "lumen" }, issue_day: "17 November 2026", period_from: "2026-11-17", period_to: "2026-11-17", period_default: true, terms_days: 30, end_of_month: false, due: "17 December 2026", order: "", language: "en", currency: "EUR", lines: root.lines, notes: [], legal: ["VAT not applicable, art. 293 B of the French tax code"], corrects: "", deposits: [], totals: [{ label: "Total", amount: "€142.50" }, { label: "Amount due", amount: "€142.50", last: true }], required: required, said: ["This invoice’s PDF will carry no Factur-X data yet."], preview: "<p>Draft invoice</p>", choices: { units: [{ code: "HUR", label: "Hours" }, { code: "C62", label: "Units" }], categories: [{ code: "S", label: "VAT at a rate" }, { code: "E", label: "Exempt from VAT" }], rates: [], activities: [], languages: [{ id: "en", label: "English" }, { id: "fr", label: "French" }], kinds: [{ id: "business", label: "A business" }, { id: "consumer", label: "A person" }, { id: "public", label: "A public body" }] } }
    }
    readonly property var shownInvoice: ({ number: "2026-013", kind: "invoice", title: "Invoice No. 2026-013", client: "Silkworks Studio", project: "", date: "12 October 2026", due: "11 November 2026", total: "€600", currency: "EUR", status: "partly-paid", status_words: "partly paid", left: 200.5, left_words: "€200.50 left to pay", sent: "", corrects: "", credit_notes: [], deducted_by: [], payments: [{ id: "hand:2026-013/x", words: "€399.50 received on 20 October 2026, by transfer", bank: false }], lines: [{ description: "Illustrations", amount: "€500", time: "" }, { description: "Printing", amount: "€100", time: "" }], outside: ["This invoice’s file was changed outside Sioul since it was issued."], older: false, acts: { paid: true, paid_back: false, credit: true, final: false, mail: true } })

    QtObject {
        id: mock

        signal projectsChanged
        signal moneyChanged

        property string status: ""
        property var asked: []
        property var changes: []
        property var made: []
        property var paid: []
        property var credited: []
        property var issuedIds: []
        property var profiles: []
        property var required: []

        function text(key) {
            return key
        }
        function textWith(key, name, value) {
            return key + ":" + value
        }
        function textArgs(key, args) {
            return key
        }
        function invoices(asked) {
            mock.asked = mock.asked.concat([JSON.parse(asked)])
            return JSON.stringify({ rows: root.rows, clients: ["Fernhill Library", "Silkworks Studio", "Studio Lumen"], years: ["2026", "2025"], statuses: [{ id: "draft", label: "draft" }, { id: "unpaid", label: "not paid yet" }, { id: "partly-paid", label: "partly paid" }], said: ["2026-012 has no record on this device."], declares: true })
        }
        function invoiceDraft(id) {
            return JSON.stringify(root.draftView(mock.required))
        }
        function newInvoiceDraft(making) {
            mock.made = mock.made.concat([JSON.parse(making)])
            return JSON.stringify({ draft: "20261117-090000-abcdef" })
        }
        function changeInvoiceDraft(id, change) {
            mock.changes = mock.changes.concat([JSON.parse(change)])
            return JSON.stringify(root.draftView(mock.required))
        }
        function setInvoiceClientTraits(id, kind, country) {
            return JSON.stringify(root.draftView(mock.required))
        }
        function discardInvoiceDraft(id) {
            return ""
        }
        function issueInvoiceDraft(id) {
            mock.issuedIds = mock.issuedIds.concat([id])
            return JSON.stringify({ number: "2026-015", pdf: "/stand-in/Invoices/2026-015 Studio Lumen.pdf", said: "The client’s SIREN: asked from September 2027." })
        }
        function invoiceShown(number) {
            return JSON.stringify(root.shownInvoice)
        }
        function payInvoice(number, paying) {
            mock.paid = mock.paid.concat([{ number: number, paying: JSON.parse(paying) }])
            return ""
        }
        function creditInvoice(number, lines) {
            mock.credited = mock.credited.concat([{ number: number, lines: JSON.parse(lines) }])
            return JSON.stringify({ draft: "20261117-100000-credit" })
        }
        function finalInvoice(number) {
            return JSON.stringify({ draft: "20261117-110000-final" })
        }
        function writeInvoiceMail(number) {
            return JSON.stringify({ draft: "mail-1", to: "billing@example.org" })
        }
        function invoiceClients(query) {
            return JSON.stringify([{ ref: "sioul:contact/lumen", name: "Studio Lumen", detail: "A business" }])
        }
        function invoiceAgain(number) {
            return JSON.stringify({ pdf: "/stand-in/Invoices/2026-013 Silkworks Studio.pdf" })
        }
        function grabFolder() {
            return "/stand-in/pictures"
        }
        function takeInvoices() {
            return ""
        }
        function clientProfile(reference) {
            return JSON.stringify({ exists: true, id: "p1", contact: reference, kind: "public", kind_words: "A public body", name: "Fernhill Library", lines: "2 Rue de l’Exemple", postcode: "69006", city: "Lyon", country: "FR", country_name: "France", delivery: "", ids: [{ kind: "siret", value: "732 829 320 00017" }], language: "fr", currency: "", terms_days: 45, end_of_month: false, email: "billing@example.org", order: "", summary: "A public body · France", id_kinds: [{ kind: "siren", label: "SIREN" }, { kind: "siret", label: "SIRET" }, { kind: "vat", label: "VAT number" }, { kind: "other", label: "Another identifier" }] })
        }
        function saveClientProfile(form) {
            mock.profiles = mock.profiles.concat([JSON.parse(form)])
            return JSON.stringify({ id: "p1" })
        }
    }

    QtObject {
        id: fakeWindow

        property bool compact: false
        property var menuAt: null
        property var drafts: []

        function openThing(item) {
        }
        function openDraft(id) {
            fakeWindow.drafts = fakeWindow.drafts.concat([id])
        }
    }

    Component {
        id: tabComponent

        InvoicesTab {
            sioul: mock
            theme: testTheme
            window: fakeWindow
        }
    }

    Component {
        id: profileComponent

        ClientProfileDialog {
            sioul: mock
            theme: testTheme
        }
    }

    TestCase {
        name: "Invoices"
        when: windowShown

        function all(item, test, into) {
            const found = into || []
            if (!item)
                return found
            if (test(item))
                found.push(item)
            for (let i = 0; i < (item.children || []).length; ++i)
                all(item.children[i], test, found)
            if (item.contentItem && item.contentItem !== item && (item.children || []).indexOf(item.contentItem) < 0)
                all(item.contentItem, test, found)
            return found
        }

        function named(item, name) {
            return all(item, i => i.objectName === name)
        }

        function shown(item, name) {
            return named(item, name).filter(i => i.visible)
        }

        function texts(item) {
            return all(item, i => i.text !== undefined && typeof i.text === "string" && i.visible).map(i => i.text)
        }

        function overlay(tab, name) {
            return all(tab.Overlay.overlay, i => i.objectName === name)
        }

        function cleanup() {
            fakeWindow.compact = false
            fakeWindow.drafts = []
            mock.asked = []
            mock.changes = []
            mock.made = []
            mock.paid = []
            mock.credited = []
            mock.issuedIds = []
            mock.profiles = []
            mock.required = []
            mock.status = ""
        }

        function editorOf(tab) {
            tryVerify(() => tab.editor !== null && tab.editor.view !== null, 3000, "the editor read its draft")
            return tab.editor
        }

        // Every draft and invoice, each with where it stands in words, never
        // "overdue"; the filters ask the core; what the files say besides.
        function test_the_list_says_where_each_stands() {
            const tab = createTemporaryObject(tabComponent, root, { width: 900, height: 700 })
            tryVerify(() => named(tab, "invoiceRow").length === 3, 5000)
            const said = texts(tab).join("\n")
            verify(said.indexOf("not paid yet") >= 0 && said.indexOf("partly paid") >= 0 && said.indexOf("€200 left to pay") >= 0, said)
            verify(said.indexOf("invoices-changed-outside") >= 0, "a file changed outside Sioul said on its row")
            verify(said.indexOf("2026-012 has no record on this device.") >= 0, said)
            verify(said.toLowerCase().indexOf("overdue") < 0 && said.toLowerCase().indexOf("late") < 0)
            tab.ask({ status: "unpaid" })
            compare(mock.asked[mock.asked.length - 1].status, "unpaid")
            tab.ask({ client: "Fernhill Library", year: "2026" })
            compare(JSON.stringify(mock.asked[mock.asked.length - 1]), JSON.stringify({ client: "Fernhill Library", year: "2026", status: "unpaid" }))
            let asked = ""
            tab.declarationAsked.connect(anchor => asked = "asked:" + anchor)
            named(tab, "invoicesDeclaration")[0].clicked()
            compare(asked, "asked:", "For the declaration: the page shows it")
        }

        // New: a blank draft, opened in the editor; Back keeps it and gives the list.
        function test_new_opens_the_editor() {
            const tab = createTemporaryObject(tabComponent, root, { width: 900, height: 700 })
            tryVerify(() => named(tab, "invoiceRow").length === 3, 5000)
            named(tab, "invoicesNew")[0].clicked()
            compare(mock.made.length, 1)
            compare(tab.draftId, "20261117-090000-abcdef")
            const editor = editorOf(tab)
            compare(shown(editor, "invoiceClientName")[0].text, "Studio Lumen")
            named(editor, "invoiceEditorBack")[0].clicked()
            compare(tab.draftId, "")
            verify(named(tab, "invoicesList")[0].visible)
        }

        // The editor: the lines from time folded; a line's words sent once the
        // typing stops, every line with its place; a new line with none.
        function test_the_editor_sends_what_is_typed() {
            const tab = createTemporaryObject(tabComponent, root, { width: 900, height: 700 })
            tab.openDraft("20261117-090000-abcdef")
            const editor = editorOf(tab)
            tryVerify(() => named(editor, "invoiceLine").length === 2, 3000)
            compare(shown(editor, "invoiceLine").length, 1, "the line of time folded")
            named(editor, "invoiceTimeFold")[0].clicked()
            compare(shown(editor, "invoiceLine").length, 2)
            const field = named(editor, "invoiceLineDescription").find(f => f.text === "Printing")
            field.text = "Printing, two posters"
            field.textEdited()
            tryVerify(() => mock.changes.length === 1, 3000, "sent once the typing stops")
            const sent = mock.changes[0].lines
            compare(sent.length, 2)
            compare(sent[1].description, "Printing, two posters")
            compare(sent[0].from, 0, "the line of time keeps its place, so its time")
            editor.addLine()
            const added = mock.changes[mock.changes.length - 1].lines
            compare(added.length, 3)
            compare(added[2].from, null, "a new line has no place of its own")
            verify(texts(editor).indexOf("•  This invoice’s PDF will carry no Factur-X data yet.") >= 0, "the notes said before the issue")
        }

        // Issue waits while something required is missing; issued, its file
        // is said in the status line and its pane opens.
        function test_issue_waits_for_what_is_required() {
            mock.required = ["The client’s SIREN."]
            const tab = createTemporaryObject(tabComponent, root, { width: 900, height: 700 })
            tab.openDraft("20261117-090000-abcdef")
            let editor = editorOf(tab)
            verify(!named(editor, "invoiceIssue")[0].enabled, "required: Issue waits")
            compare(shown(editor, "invoiceRequired").length, 1)
            mock.required = []
            editor.load("20261117-090000-abcdef")
            tryVerify(() => named(editor, "invoiceIssue")[0].enabled, 2000)
            named(editor, "invoiceIssue")[0].clicked()
            compare(mock.issuedIds, ["20261117-090000-abcdef"])
            verify(mock.status.indexOf("invoice-written:/stand-in/Invoices/2026-015 Studio Lumen.pdf") >= 0, mock.status)
            verify(mock.status.indexOf("The client’s SIREN") >= 0, "the checks' notes said too")
            tryVerify(() => tab.invoice !== null && tab.draftId === "", 2000, "its pane opens")
        }

        // Paid…: today, by transfer, what is left; a part of it a partial payment.
        function test_paid_asks_the_day_and_the_means() {
            const tab = createTemporaryObject(tabComponent, root, { width: 900, height: 700 })
            tab.openInvoice("2026-013")
            tryVerify(() => named(tab, "invoicePane")[0].visible, 2000)
            verify(shown(tab, "invoiceOutside").length === 1, "changed outside Sioul, said")
            compare(shown(tab, "invoicePayment").length, 1)
            named(tab, "invoicePaid")[0].clicked()
            tryVerify(() => overlay(tab, "invoicePaidAmount").length === 1 && overlay(tab, "invoicePaidAmount")[0].visible, 3000)
            const amount = overlay(tab, "invoicePaidAmount")[0]
            compare(amount.text, "200.5", "what is left")
            amount.text = "100"
            overlay(tab, "invoicePaidSave")[0].clicked()
            compare(mock.paid.length, 1)
            const paying = mock.paid[0].paying
            compare(paying.means, "transfer")
            compare(paying.amount, 100)
            const now = new Date()
            const pad = n => n < 10 ? "0" + n : String(n)
            compare(paying.day, now.getFullYear() + "-" + pad(now.getMonth() + 1) + "-" + pad(now.getDate()))
        }

        // Credit note…: every line cancels it ([]), some correct it (their places).
        function test_a_credit_note_takes_all_lines_or_some() {
            const tab = createTemporaryObject(tabComponent, root, { width: 900, height: 700 })
            tab.openInvoice("2026-013")
            tab.askCredit(tab.invoice)
            tryVerify(() => overlay(tab, "invoiceCreditLine").length === 2, 3000)
            overlay(tab, "invoiceCreditSave")[0].clicked()
            compare(JSON.stringify(mock.credited[0]), JSON.stringify({ number: "2026-013", lines: [] }))
            tryVerify(() => tab.draftId === "20261117-100000-credit", 2000, "the credit note's draft in the editor")
            tab.back()
            tab.openInvoice("2026-013")
            tab.askCredit(tab.invoice)
            tryVerify(() => overlay(tab, "invoiceCreditLine").length === 2, 3000)
            const first = overlay(tab, "invoiceCreditLine")[0]
            first.checked = false
            first.toggled()
            overlay(tab, "invoiceCreditSave")[0].clicked()
            compare(JSON.stringify(mock.credited[1].lines), JSON.stringify([1]))
        }

        // A deposit invoice by a share of the draft; Write the mail opens its draft.
        function test_a_deposit_by_its_share_and_the_mail() {
            const tab = createTemporaryObject(tabComponent, root, { width: 900, height: 700 })
            tab.askDeposit("20261117-090000-abcdef")
            tryVerify(() => overlay(tab, "invoiceDepositShare").length === 1 && overlay(tab, "invoiceDepositShare")[0].visible, 3000)
            overlay(tab, "invoiceDepositSave")[0].clicked()
            compare(JSON.stringify(mock.made[0]), JSON.stringify({ kind: "deposit", of: "20261117-090000-abcdef", description: "", amount: 0, share: 30 }))
            tab.openInvoice("2026-013")
            named(tab, "invoiceMail")[0].clicked()
            compare(fakeWindow.drafts, ["mail-1"])
        }

        // A client's profile read into its form, saved whole.
        function test_a_client_profile_is_read_and_saved() {
            const form = createTemporaryObject(profileComponent, root)
            form.edit("sioul:contact/fernhill")
            tryVerify(() => form.opened, 2000)
            compare(overlay(form, "clientProfileName")[0].text, "Fernhill Library")
            compare(overlay(form, "clientProfileCountry")[0].text, "France")
            compare(overlay(form, "clientProfileKind")[0].currentIndex, 2, "a public body")
            overlay(form, "clientProfileEmail")[0].text = "accounts@example.org"
            form.save()
            compare(mock.profiles.length, 1)
            const saved = mock.profiles[0]
            compare(saved.kind, "public")
            compare(saved.email, "accounts@example.org")
            compare(saved.terms_days, 45)
            compare(saved.ids.length, 1)
            compare(saved.contact, "sioul:contact/fernhill")
        }

        // At a phone's 360 points: the rows and the editor's fields within the screen.
        function test_a_phone_keeps_everything_within_its_width() {
            fakeWindow.compact = true
            const tab = createTemporaryObject(tabComponent, root, { width: 360 - 16, height: 700 })
            tryVerify(() => named(tab, "invoiceRow").length === 3, 5000)
            for (const row of named(tab, "invoiceRow"))
                verify(row.mapToItem(tab, row.width, 0).x <= 360 - 16 + 0.5, "a row within the screen")
            tab.openDraft("20261117-090000-abcdef")
            const editor = editorOf(tab)
            waitForRendering(editor)
            for (const field of shown(editor, "invoiceLineDescription").concat(shown(editor, "invoiceClientField")))
                verify(field.mapToItem(tab, field.width, 0).x <= 360 - 16 + 0.5, field.objectName + " within the screen: " + field.mapToItem(tab, field.width, 0).x)
            const issue = named(editor, "invoiceIssue")[0]
            verify(issue.mapToItem(tab, issue.width, 0).x <= 360 - 16 + 0.5, "Issue within the screen")
        }
    }
}
