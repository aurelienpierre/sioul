// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The add form's search among the usual sites (SiteAddDialog.qml): a word
// typed without its accents finds a name, a region or a description that
// has them, in any case ("sante" finds "Santé publique"). Its regular
// expression keeps its escapes (̀-ͯ), never the invisible
// combining characters themselves (review of 19 November 2026, C1). On a
// stand-in for Sioul; run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 900
    height: 800

    Theme {
        id: testTheme
    }
    QtObject {
        id: mock

        property string status: ""
        function text(key) {
            return key
        }
        function sitePresets(country, region) {
            return JSON.stringify({ country: "", region: "", countries: [], types: ["mailbox"], categories: [], sites: [
                { name: "Santé publique", url: "https://health.example.org", host: "health.example.org", kind: "mailbox", categories: ["Santé"], area: "admin", announced_by: [], about: "Assurance", calls: false, kept: false },
                { name: "Impôts", url: "https://tax.example.org", host: "tax.example.org", kind: "mailbox", categories: ["État"], area: "admin", announced_by: [], about: "Déclaration", calls: false, kept: false }
            ] })
        }
        function addSite(edit) {
            return JSON.stringify({ id: "x" })
        }
    }
    SiteAddDialog {
        id: adding

        sioul: mock
        theme: testTheme
        room: root.width
        roomTall: root.height
    }
    TestCase {
        name: "SiteAddFold"
        when: windowShown

        function test_a_word_without_accents_finds_a_name_with_them() {
            adding.open()
            tryCompare(adding, "opened", true)
            compare(adding.shownPresets.length, 2)
            adding.findText = "sante"
            compare(adding.shownPresets.length, 1)
            compare(adding.shownPresets[0].name, "Santé publique")
            adding.findText = "impots declaration"
            compare(adding.shownPresets.length, 1)
            adding.findText = "ÉTAT"
            compare(adding.shownPresets.length, 1)
            adding.close()
        }
    }
}
