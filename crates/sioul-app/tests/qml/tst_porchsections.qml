// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The Porch's sections and rows (PorchSection.qml, NoticeRow.qml, in
// PorchPage.qml), on a stand-in for Sioul and its window: a section folds
// and unfolds by its title, its folding kept on this device and read again;
// a folded-by-default one opens by hand; a section with nothing new is not
// there; a row's Dismiss and Mark read hand its notices to Rust's words at
// once, the row gone at once, ten seconds to undo, Undo taking the word back
// and the row back; a row dismissed on another device (the view read again)
// is gone. What the words are and how they travel is tested in sioul-core
// (notices.rs). Fiction only. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 1100
    height: 900

    Theme {
        id: testTheme
    }

    // Made up: no real address, no real message.
    function message(n, subject, state) {
        return { key: "/stand-in/mail/alice/cur/17770000" + n + ".U1-" + n + ".sioul:2,", uri: "mid:m" + n + "@example.org", account: "alice", sender: "Ana " + n, address: "ana@example.org",
                 trust: "", trust_level: "verified", subject: subject, date: "", preview: "", reasons: [], attachments: [], screener: false, summary: "", checks: "",
                 notice: { id: "mail:alice:000000000000000" + n, kind: "mail", who: "Ana " + n, what: subject, when: "Wed 4 Nov 09:1" + n, readable: true, state: { mark: state || "new", at: 0, device: "" } } }
    }
    function porchWith(people, filed) {
        return JSON.stringify({
            open: true, closed: null, summary: "", status: "", all: false, right_now: [],
            lanes: [{ key: "people", title: "From people you know", folded: false, counted: true, about: "From people you know.", rules: ["Anyone in your address books comes here."], settings: "", items: people },
                    { key: "filed", title: "Filed", folded: true, counted: true, about: "Newsletters.", rules: [], settings: "", items: filed }]
        })
    }

    QtObject {
        id: mock

        property string porch: root.porchWith([root.message(1, "The plan"), root.message(2, "Lunch")], [root.message(3, "A newsletter")])
        property string status: ""
        property string overlaps: "[]"
        property string missedView: "[]"
        // What the window asked of the notices, in order: [verb, args].
        property var asked: []
        property var flags: ({})

        signal shareWithheldBack(string problem)
        signal sitesChanged()
        signal lettersChanged()
        signal messageRead(string key, string reading, string invitation)

        function text(key) {
            return ({ "notice-dismiss": "Dismiss", "notice-read": "Mark read", "ui-undo": "Undo" })[key] || key
        }
        function textWith(key, name, value) {
            return key + ":" + value
        }
        function textArgs(key, args) {
            return key
        }
        function settings() {
            return "[]"
        }
        function viewFlag(name) {
            return mock.flags[name] === true
        }
        function setViewFlag(name, value) {
            const next = Object.assign({}, mock.flags)
            next[name] = value
            mock.flags = next
        }
        function uriOf(kind, key) {
            return ""
        }
        function shareWithheld() {
            return "[]"
        }
        function refreshHealth() {
        }
        function readMessage(key) {
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
        // `notices::call`: the word said answers its time, to take it back.
        function notices(verb, json) {
            const args = JSON.parse(json)
            if (verb === "view")
                return JSON.stringify({ sites: [], money: [], letters: [] })
            mock.asked = mock.asked.concat([[verb, args]])
            if (verb === "undo")
                return JSON.stringify({ said: "", at: 0 })
            return JSON.stringify({ said: verb === "read" ? "Marked read on each of your devices." : "Dismissed on each of your devices.", at: 1793000000000 + mock.asked.length, mark: verb === "read" ? "read" : "dismissed" })
        }
    }

    QtObject {
        id: fakeWindow

        property bool compact: false
        property int page: 0
        property var moment: ({ sleep: false })

        function openMail(ref) {
        }
        function openThing(item) {
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
        name: "PorchSections"
        when: windowShown

        function all(item, test, into) {
            const found = into || []
            if (!item)
                return found
            if (test(item) && found.indexOf(item) < 0)
                found.push(item)
            const kids = item.children || []
            for (let i = 0; i < kids.length; ++i)
                all(kids[i], test, found)
            if (item.contentItem && item.contentItem !== item)
                all(item.contentItem, test, found)
            return found
        }
        function section(key) {
            return all(porch, i => i.key === key && typeof i.toggle === "function" && typeof i.unfold === "function")[0] || null
        }
        // The rows shown, by who.
        function rows(under) {
            return all(under, i => i.visible && i.notice !== undefined && i.notice !== null && typeof i.dismissPressed === "function").map(r => r.notice.who)
        }
        function named(under, name) {
            return all(under, i => i.visible && i.objectName === name)
        }

        function init() {
            mock.asked = []
            mock.flags = {}
            mock.porch = root.porchWith([root.message(1, "The plan"), root.message(2, "Lunch")], [root.message(3, "A newsletter")])
            porch.gone = {}
            porch.lastSaid = null
            porch.porchText = mock.porch
            const people = section("people")
            if (people !== null)
                people.turned = false
            const filed = section("filed")
            if (filed !== null)
                filed.turned = false
            wait(50)
        }

        // A section folds and unfolds by its title, and this device keeps it.
        function test_a_section_folds_and_keeps_it() {
            const people = section("people")
            verify(people !== null && people.visible)
            compare(rows(people), ["Ana 1", "Ana 2"])
            mouseClick(named(people, "sectionHeader")[0])
            verify(people.folded, "folded by its title")
            compare(rows(people), [], "its rows hidden")
            verify(mock.viewFlag("porch-folded.people"), "kept on this device")
            // Read again (the Porch made anew): still folded.
            people.turned = mock.viewFlag(people.flag)
            verify(people.folded)
            mouseClick(named(people, "sectionHeader")[0])
            verify(!people.folded)
            compare(rows(people), ["Ana 1", "Ana 2"])
        }

        // Filed starts folded; opened by hand, it stays opened.
        function test_a_folded_section_opens_by_hand() {
            const filed = section("filed")
            verify(filed.folded)
            compare(rows(filed), [])
            mouseClick(named(filed, "sectionHeader")[0])
            verify(!filed.folded)
            verify(mock.viewFlag("porch-unfolded.filed"))
            compare(rows(filed), ["Ana 3"])
        }

        // Dismiss: the notice handed to Rust, the row gone at once; Undo takes the word back, the row back.
        function test_dismiss_then_undo() {
            const people = section("people")
            mouseClick(named(people, "dismissButton")[0])
            compare(mock.asked.length, 1)
            compare(mock.asked[0][0], "dismiss")
            compare(JSON.stringify(mock.asked[0][1]), JSON.stringify({ kind: "mail", ids: ["mail:alice:0000000000000001"], targets: [root.message(1, "The plan").key] }))
            wait(50)
            compare(rows(people), ["Ana 2"], "gone at once")
            const undo = named(porch, "noticeUndoButton")
            compare(undo.length, 1, "ten seconds to undo")
            mouseClick(undo[0])
            compare(mock.asked[1][0], "undo")
            compare([mock.asked[1][1].mark, mock.asked[1][1].at, mock.asked[1][1].ids[0]], ["dismissed", 1793000000001, "mail:alice:0000000000000001"])
            wait(50)
            compare(rows(people), ["Ana 1", "Ana 2"], "back")
            compare(named(porch, "noticeUndoButton").length, 0, "Undo gone with it")
        }

        // Mark read: its message read too (its file handed as the item to read), the row gone; Undo unreads.
        function test_mark_read_then_undo() {
            const people = section("people")
            mouseClick(named(people, "readButton")[1])
            compare(mock.asked[0][0], "read")
            compare(mock.asked[0][1].targets, [root.message(2, "Lunch").key])
            wait(50)
            compare(rows(people), ["Ana 1"])
            mouseClick(named(porch, "noticeUndoButton")[0])
            compare(mock.asked[1][1].mark, "read")
            wait(50)
            compare(rows(people), ["Ana 1", "Ana 2"])
        }

        // The Undo line goes by itself after its ten seconds.
        function test_undo_goes_after_ten_seconds() {
            mouseClick(named(section("people"), "dismissButton")[0])
            wait(50)
            compare(named(porch, "noticeUndoButton").length, 1)
            tryCompare(porch, "lastSaid", null, 11000)
            compare(named(porch, "noticeUndoButton").length, 0)
            compare(rows(section("people")), ["Ana 2"], "still dismissed")
        }

        // Dismissed or read on another device: gone once the Porch is read again; a section with nothing new is not there.
        function test_a_word_said_elsewhere_and_an_empty_section() {
            mock.porch = root.porchWith([root.message(1, "The plan", "dismissed"), root.message(2, "Lunch", "read")], [root.message(3, "A newsletter")])
            porch.porchText = mock.porch
            wait(50)
            const people = section("people")
            verify(people === null || !people.visible, "no section with nothing new, no line saying so")
            verify(section("filed").visible)
        }

        // The "?": its help once, in a tooltip; pressed, in its popup.
        function test_the_help_is_said_once() {
            const help = named(section("people"), "sectionHelp")
            compare(help.length, 1)
            verify(help[0].Accessible.description.indexOf("From people you know.") === 0, "what it holds, then how mail lands there")
            mouseClick(help[0])
            wait(100)
            verify(porch.helpItem() !== null && porch.helpItem().opened, "pressed: its popup")
            porch.closeHelp()
        }
    }
}
