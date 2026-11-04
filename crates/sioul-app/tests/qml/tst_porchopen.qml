// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The Porch's ways to the window's one opener (the unified mailbox's part 2,
// PorchPage.qml), on a stand-in for Sioul and its window: a message tapped
// on the phone's home card opens in the Porch's own reader while the Porch
// shows it, found by its address (this version's card) or its file (an
// older card's), else through the window's opener by its address; a code's
// row opens its message through the opener by its address, its file a
// hint. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 1100
    height: 800

    Theme {
        id: testTheme
    }

    // Made up: no real address, no real message.
    readonly property string shownKey: "/stand-in/mail/alice/cur/1777000000.U1-1.sioul:2,S"
    readonly property string codeKey: "/stand-in/mail/alice/cur/1777000001.U1-2.sioul:2,S"
    readonly property string porchJson: JSON.stringify({
        open: true, closed: null, summary: "", status: "", all: false,
        right_now: [{ key: root.codeKey, address: "mid:code-1@example.org", title: "Code from Example Bank (verified)", code: "482913", validity: "valid about 10 minutes", warning: null,
                      notice: { id: "mail:alice:00000000000000c1", kind: "mail", who: "Code from Example Bank (verified)", what: "482913, valid about 10 minutes", when: "", readable: true, state: { mark: "new" } } }],
        lanes: [{ key: "people", title: "People", folded: false, counted: true, about: "", rules: [], settings: "", items: [
            { key: root.shownKey, uri: "mid:plan@example.org", account: "alice", sender: "Ana", address: "ana@example.org", trust: "", trust_level: "known", subject: "The plan", date: "", preview: "", reasons: [], attachments: [], screener: false,
              notice: { id: "mail:alice:00000000000000a1", kind: "mail", who: "Ana", what: "The plan", when: "", readable: true, state: { mark: "new" } } }
        ] }]
    })

    QtObject {
        id: mock

        property string porch: root.porchJson
        property string status: ""
        property string overlaps: "[]"
        property string missedView: "[]"
        property string reading: ""

        signal shareWithheldBack(string problem)
        signal sitesChanged()
        signal messageRead(string key, string reading, string invitation)

        function text(key) {
            return key
        }
        function textWith(key, name, value) {
            return key + ":" + value
        }
        function textArgs(key, args) {
            return key
        }
        function settings() {
            return "{}"
        }
        function siteNotices() {
            return "[]"
        }
        function viewFlag(name) {
            return false
        }
        function setViewFlag(name, value) {
        }
        function uriOf(key) {
            return ""
        }
        function shareWithheld() {
            return "[]"
        }
        function bank() {
            return "{}"
        }
        function notices(verb, json) {
            return verb === "view" ? JSON.stringify({ sites: [], money: [], letters: [] }) : "{}"
        }
        function callsView() {
            return JSON.stringify({ lines: [] })
        }
        function phoneMessages(verb, json) {
            return JSON.stringify({ lines: [], notices: [] })
        }
        function lettersFolder() {
            return ""
        }
        function refreshHealth() {
        }
        function openMessage(key) {
        }
        function readMessage(key) {
        }
    }

    // The window, as the Porch sees it: what reaches the opener, and what else opens.
    QtObject {
        id: fakeWindow

        property bool compact: false
        property int page: 0
        property var moment: ({ sleep: false })
        property var mailAsked: []
        property var opened: []

        function openMail(ref) {
            fakeWindow.mailAsked = fakeWindow.mailAsked.concat([ref])
        }
        function openThing(item) {
            fakeWindow.opened = fakeWindow.opened.concat([item])
        }
        function copy(text) {
        }
        function linkFrom(source) {
        }
        function openSite(site) {
        }
        function askDose(dose) {
        }
        function joinDevices() {
        }
        function showParameters() {
        }
    }

    PorchPage {
        id: porch

        anchors.fill: parent
        sioul: mock
        theme: testTheme
        window: fakeWindow
    }

    TestCase {
        name: "PorchOpen"
        when: windowShown

        function find(item, name) {
            if (!item)
                return null
            if (item.objectName === name)
                return item
            const kids = item.children || []
            for (let i = 0; i < kids.length; ++i) {
                const found = find(kids[i], name)
                if (found)
                    return found
            }
            return item.contentItem && item.contentItem !== item ? find(item.contentItem, name) : null
        }

        function all(item, test, into) {
            const found = into || []
            if (!item)
                return found
            if (test(item))
                found.push(item)
            const kids = item.children || []
            for (let i = 0; i < kids.length; ++i)
                all(kids[i], test, found)
            if (item.contentItem && item.contentItem !== item)
                all(item.contentItem, test, found)
            return found
        }

        function init() {
            fakeWindow.mailAsked = []
            fakeWindow.opened = []
            porch.openKey = ""
            porch.porchText = mock.porch
        }

        // This version's card, by the address of a message the Porch shows: its own reader.
        function test_the_home_card_s_message_shown_here_opens_here() {
            porch.openCardMessage("mid:plan@example.org", "")
            compare(porch.openKey, root.shownKey, "the Porch's reader, by its address")
            compare(fakeWindow.mailAsked.length, 0, "never the opener")
        }

        // An older card (4d81220's), by its file: its own reader too.
        function test_an_older_card_s_file_opens_here() {
            porch.openCardMessage("", root.shownKey)
            compare(porch.openKey, root.shownKey)
            compare(fakeWindow.mailAsked.length, 0)
        }

        // A message the Porch no longer shows (filed, moved, read on another device): the
        // window's one opener, by its address, never by a path alone.
        function test_a_message_not_shown_here_goes_to_the_opener() {
            porch.openCardMessage("mid:filed@example.org", "")
            compare(porch.openKey, "")
            compare(JSON.stringify(fakeWindow.mailAsked), JSON.stringify([{ address: "mid:filed@example.org", file: "" }]))
            compare(fakeWindow.opened.length, 0, "never openThing's path")
        }

        // Asked before the Porch is read: opened once it is.
        function test_a_card_asked_before_the_porch_is_read_waits_for_it() {
            porch.porchText = ""
            porch.openCardMessage("mid:plan@example.org", "")
            compare(fakeWindow.mailAsked.length, 0)
            porch.porchText = mock.porch
            compare(JSON.stringify(fakeWindow.mailAsked), "[]", "never the opener")
            compare(porch.openKey, root.shownKey)
        }

        // A code's row: its message through the opener, by its address, its file a hint.
        function test_a_code_s_row_opens_its_message_by_its_address() {
            // The same row reached by two ways (an item's children and its contentItem): once.
            const rows = all(porch, c => c.notice !== undefined && c.notice !== null && c.notice.id === "mail:alice:00000000000000c1" && typeof c.opened === "function").filter((c, i, found) => found.indexOf(c) === i)
            compare(rows.length, 1, "one code's row")
            rows[0].opened()
            compare(JSON.stringify(fakeWindow.mailAsked), JSON.stringify([{ address: "mid:code-1@example.org", file: root.codeKey }]))
        }
    }
}
