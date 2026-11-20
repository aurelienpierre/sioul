// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The Porch keeps its place (PorchPage.qml, ListPlace.qml; docs/qt-quick.md,
// "A list keeps its place"), on a stand-in for Sioul that hands the whole
// Porch again after each change, as Rust does: a lane of forty messages
// scrolled to its middle; a row dismissed with its own button, the top one
// too; one dismissed on another device above the view, one come above it:
// the row at the top stays at its pixel, or the next takes its place.
// Fiction only. Run by tools/qml-test.sh.

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
    // A row of From your phone from a phone's history (`calls::history_lines`): a missed call, or a call of yours that did not reach them.
    function call(kind, who) {
        return { ids: ["1793000000000-+33199001234-" + who], kind: kind, text: "", number: "01 99 00 12 34", dial: "+33199001234", known: true, hidden: false,
                 doubt: "", message: null, why: "", again: "", targets: [],
                 notice: { id: "call:phone-1:" + who, kind: "call", who: who, what: kind === "missed" ? "Missed call at 09:30" : "Your call at 14:10 was not answered",
                           when: "Wed 4 Nov 14:10", readable: false, state: { mark: "new", at: 0, device: "" } } }
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
        // The calls' rows `calls::view` gives.
        property var callLines: []
        // The agenda's conflicts as rows (`notices::agenda_rows`), and the answers given.
        property var agendaRows: []
        property var answered: []
        // What the window asked of the notices, in order: [verb, args].
        property var asked: []
        property var flags: ({})
        // This device's training on the Porch (`spam::card`), and the card put away: [turn_on].
        property string learn: ""
        property var learnAway: []

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
        function spamCard() {
            return mock.learn
        }
        function spamCardAway(again) {
            mock.learnAway = mock.learnAway.concat([again])
            mock.learn = ""
            return ""
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
        function answerConflict(key, answer) {
            mock.answered = mock.answered.concat([[key, answer]])
            return JSON.stringify({ problem: "", open: answer === "move" ? "/cal/home/a.ics" : "" })
        }
        function setOverlapAside(keys) {
            return ""
        }
        function callsView() {
            return JSON.stringify({ lines: mock.callLines, phone: false, links: { tel: true, sms: true } })
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
            if (verb === "agenda")
                return JSON.stringify(mock.agendaRows)
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
        // What the Porch opened, in order.
        property var openedThings: []

        function openThing(item) {
            fakeWindow.openedThings = fakeWindow.openedThings.concat([item])
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
        name: "PlacePorch"
        when: windowShown

        property var flick: null

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

        function frame() {
            root.Window.window.update()
            waitForRendering(root)
        }

        // The message rows shown, by who, with their tops in the Porch's view, top first.
        function rows() {
            return all(flick.contentItem, i => i.visible && i.notice !== undefined && i.notice !== null && typeof i.dismissPressed === "function")
                .map(r => ({ who: r.notice.who, y: r.mapToItem(flick, 0, 0).y, height: r.height, row: r }))
                .filter(r => r.y + r.height > 0 && r.y < flick.height)
                .sort((a, b) => a.y - b.y)
        }

        function toMiddle() {
            flick.contentY = (flick.contentHeight - flick.height) / 2
            frame()
            flick.contentY += rows()[0].y + 7
            frame()
            return rows()[0]
        }

        // Forty messages in From people you know; `without` read on another device, `more` come.
        function porch(without, more) {
            const people = []
            for (const n of (more || []))
                people.push(root.message(n, "Just come " + n))
            for (let n = 100; n < 140; ++n)
                if ((without || []).indexOf(n) < 0)
                    people.push(root.message(n, "Subject " + n))
            // Its change read as Rust's signal reads it (onPorchChanged).
            mock.porch = root.porchWith(people, [])
        }

        function init() {
            mock.asked = []
            mock.flags = {}
            porch.gone = {}
            porch.lastSaid = null
            this.porch([])
            wait(0)
            frame()
            let item = all(porch, i => i.visible && i.notice !== undefined && i.notice !== null && typeof i.dismissPressed === "function")[0]
            while (item && item.flicking === undefined)
                item = item.parent
            flick = item
            verify(flick !== null && flick.contentHeight > flick.height, "a Porch longer than its room")
        }

        function test_a_row_dismissed_with_its_button_keeps_the_place() {
            const top = toMiddle()
            mouseClick(all(rows()[3].row, i => i.visible && i.objectName === "dismissButton")[0])
            wait(0)
            frame()
            compare(mock.asked[mock.asked.length - 1][0], "dismiss")
            compare(rows()[0].who, top.who, "the same row at the top")
            compare(rows()[0].y, top.y, "at the same pixel")
            // The top one dismissed: the next takes its place.
            const next = rows()[1]
            mouseClick(all(rows()[0].row, i => i.visible && i.objectName === "dismissButton")[0])
            wait(0)
            frame()
            compare(rows()[0].who, next.who, "the next one at the top")
            compare(rows()[0].y, top.y, "in its place")
        }

        function test_read_elsewhere_and_come_above_keep_the_place() {
            const top = toMiddle()
            this.porch([103])
            wait(0)
            frame()
            compare(rows()[0].who, top.who, "one read on another device above")
            compare(rows()[0].y, top.y)
            this.porch([103], [1, 2])
            wait(0)
            frame()
            compare(rows()[0].who, top.who, "two come above")
            compare(rows()[0].y, top.y)
        }
    }
}
