// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Settings ▸ Invoices (InvoiceSettings.qml; docs/projects.md, "Settings ▸
// Invoices"), on a stand-in for Sioul: the country's preset first; an older
// table's legal form asked once, first, and saved by a tap; what the preset
// asks before an invoice is issued; a value saved when it is left, a list
// written whole (an identifier added, printed or not); the rest under More,
// the books under their own fold, the e-invoicing platform last; at a
// phone's 360 points nothing passes the edge. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 800
    height: 900

    Theme {
        id: testTheme
    }

    // Made up: a French sole trader of example.org, from before November 2026 (no legal form said).
    function view(formAsked) {
        return {
            preset: "fr", chosen: false, presets: [{ id: "fr", name: "France", supported: true, why_not: "" }, { id: "elsewhere", name: "Elsewhere", supported: true, why_not: "" }, { id: "es", name: "Spain", supported: false, why_not: "Certified software is asked." }],
            limits: "", why_not: "", asks: ["Your SIREN or SIRET."], form: formAsked ? "" : "ei", form_asked: formAsked,
            forms: [{ id: "ei", name: "Entrepreneur individuel", company: false }, { id: "sasu", name: "SASU", company: true }], company: false,
            regime: "micro", regimes: [{ id: "micro", name: "Micro" }, { id: "reel", name: "Réel" }],
            schemes: [{ id: "franchise", name: "Small-business exemption (franchise)" }, { id: "liable", name: "Liable for VAT" }], tax: mock.taxRows, scheme_now: "franchise", mention_default: "TVA non applicable, art. 293 B du CGI",
            ids: [{ kind: "siret", value: "732 829 320 00017", printed: true }], id_kinds: [{ kind: "siren", name: "SIREN" }, { kind: "siret", name: "SIRET" }, { kind: "vat", name: "VAT number" }], older_siret: "",
            name: "Camille Exemple", trade_name: "", address: "2 place de l’Exemple", postcode: "69001", city: "Lyon", country: "", country_default: "FR", email: "",
            banks: [{ holder: "Camille Exemple", iban: "FR76 3000 6000 0112 3456 7890 189", bic: "", valid: true }], qr: true, payment: "",
            terms_days: null, terms_default: 30, end_of_month: false, penalty_rate: null, penalty_default: "12.15 % (ECB rate + 10 points)",
            language: "", currency: "", currency_default: "EUR", pattern: "", pattern_default: "{YYYY}-{N:3}", pattern_problem: "", reset: "yearly", credit_pattern: "", next: "2026-015",
            lines: [{ id: "penalties", text: "Late payment: { $rate }.", own: "", subject: "PMT", off: false, preset: true }], insurance: "", capital: "",
            declares: true, activities: ["services-bic"], activity_choices: [{ id: "services-bic", name: "Services (BIC)" }, { id: "bnc", name: "Liberal profession (BNC)" }], declare: "", periods: [{ id: "quarter", name: "Each quarter" }, { id: "month", name: "Each month" }],
            vl: false, started: "", vat_period: "", debits: false, contributions: [], platform: "", unread: []
        }
    }

    QtObject {
        id: mock

        property bool asked: true
        property var saved: []
        property var taxRows: []

        function text(key) {
            return key
        }
        function textWith(key, name, value) {
            return key + ":" + value
        }
        function textArgs(key, args) {
            return key
        }
        function invoiceSettings() {
            return JSON.stringify(root.view(mock.asked))
        }
        function setInvoiceSetting(key, value) {
            mock.saved = mock.saved.concat([{ key: key, value: JSON.parse(value) }])
            if (key === "legal_form")
                mock.asked = false
            return ""
        }
    }

    Component {
        id: blockComponent

        InvoiceSettings {
            sioul: mock
            theme: testTheme
            about: "Who sends your invoices."
        }
    }

    TestCase {
        name: "InvoiceSettings"
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

        function cleanup() {
            mock.asked = true
            mock.saved = []
            mock.taxRows = []
        }

        // A VAT scheme's first day is a day, picked or typed in a date field;
        // the first row without one holds always (the review of phase 3, F19).
        function test_a_scheme_s_day_is_a_day() {
            mock.taxRows = [{ from: "", scheme: "franchise", mention: "" }, { from: "2027-01-01", scheme: "liable", mention: "" }]
            mock.asked = false
            const block = createTemporaryObject(blockComponent, root, { width: 700 })
            tryVerify(() => named(block, "invoiceSchemeDay").length === 2, 3000)
            named(block, "invoiceSettingsMore")[0].clicked()
            const days = named(block, "invoiceSchemeDay").filter(d => d.visible)
            compare(days.length, 1, "always: no day")
            compare(days[0].date, "2027-01-01")
            days[0].date = "2027-02-01"
            days[0].edited()
            const last = mock.saved[mock.saved.length - 1]
            compare(last.key, "tax")
            compare(last.value[1].from, "2027-02-01")
        }

        // The preset first; an older table's legal form asked once, first, and saved by a tap.
        function test_the_preset_first_and_the_form_asked_once() {
            const block = createTemporaryObject(blockComponent, root, { width: 700 })
            tryVerify(() => block.view !== null, 2000)
            const asked = named(block, "invoiceFormAsked")[0]
            verify(asked.visible, "asked, first")
            const preset = named(block, "invoicePreset")[0]
            verify(asked.y < preset.y, "before the preset")
            compare(preset.currentIndex, 0, "France")
            const ei = all(asked, i => i.text === "Entrepreneur individuel" && i.clicked !== undefined)[0]
            ei.clicked()
            compare(JSON.stringify(mock.saved[0]), JSON.stringify({ key: "legal_form", value: "ei" }))
            verify(!named(block, "invoiceFormAsked")[0].visible, "asked once: answered, gone")
            compare(named(block, "invoiceLegalForm")[0].currentIndex, 1)
        }

        // A value saved when it is left; a list written whole.
        function test_values_are_saved_as_they_are_left() {
            const block = createTemporaryObject(blockComponent, root, { width: 700 })
            tryVerify(() => block.view !== null, 2000)
            const name = named(block, "invoiceName")[0]
            name.text = "Camille Exemple, design"
            name.editingFinished()
            compare(JSON.stringify(mock.saved[0]), JSON.stringify({ key: "name", value: "Camille Exemple, design" }))
            named(block, "invoiceIdAdd")[0].clicked()
            tryVerify(() => named(block, "invoiceIdValue").length === 2, 2000)
            const value = named(block, "invoiceIdValue").find(f => f.text === "")
            value.text = "FR40732829320"
            value.editingFinished()
            const written = mock.saved[mock.saved.length - 1]
            compare(written.key, "id")
            compare(written.value.length, 2, "the list whole")
            compare(written.value[1].value, "FR40732829320")
            const platform = named(block, "invoicePlatform")[0]
            platform.text = "Example PA"
            platform.editingFinished()
            compare(JSON.stringify(mock.saved[mock.saved.length - 1]), JSON.stringify({ key: "platform", value: "Example PA" }))
        }

        // The rest under More, the books under their own fold.
        function test_more_and_the_books_are_folded() {
            mock.asked = false
            const block = createTemporaryObject(blockComponent, root, { width: 700 })
            tryVerify(() => block.view !== null, 2000)
            const pattern = named(block, "invoicePattern")[0]
            verify(!pattern.visible, "under More")
            named(block, "invoiceSettingsMore")[0].clicked()
            verify(pattern.visible)
            pattern.text = "F{YY}-{N:4}"
            pattern.editingFinished()
            compare(JSON.stringify(mock.saved[0]), JSON.stringify({ key: "pattern", value: "F{YY}-{N:4}" }))
            verify(named(block, "invoiceSettingsBooks")[0].visible, "France's preset declares")
        }

        // At a phone's 360 points: nothing past the edge, More and the books unfolded.
        function test_a_phone_keeps_it_within_its_width() {
            mock.asked = false
            const block = createTemporaryObject(blockComponent, root, { width: 360 - 32 })
            tryVerify(() => block.view !== null, 2000)
            block.more = true
            block.books = true
            waitForRendering(block)
            for (const field of all(block, i => i.visible && i.width > 0 && (i.placeholderText !== undefined || i.currentIndex !== undefined)))
                verify(field.mapToItem(block, field.width, 0).x <= 360 - 32 + 0.5, (field.objectName || field.placeholderText || "a field") + " within the screen: " + field.mapToItem(block, field.width, 0).x)
        }
    }
}
