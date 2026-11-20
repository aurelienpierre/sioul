// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The Agenda's What comes keeps its place (AgendaPage.qml, ListPlace.qml;
// docs/qt-quick.md, "A list keeps its place"), on a stand-in for Sioul that
// hands the whole agenda again after each change, as Rust does: scrolled to
// its middle, an appointment deleted from its row's menu, the top one too,
// one added above the view (another device, or New): the row at the top
// stays at its pixel, or the next takes its place. ◂ (another stretch) opens
// it on today again. Fiction only. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 1000
    height: 860

    // 5 January 2099, 10:00, in the tests' time zone.
    readonly property real monday: new Date(2099, 0, 5).getTime() / 1000

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        property string agenda: ""
        property string doseMarks: ""
        property real doseReadAt: 0
        property string overlaps: "[]"
        property string status: ""
        property bool replanDue: false
        property var asked: []
        // What the page asks of the Health job and of today's layout (review of 21436bf, M2).
        property int healthAsked: 0
        property int relaid: 0

        function text(key) { return key }
        function textWith(key, name, value) { return key + ":" + value }
        // Its words with their arguments, long as real ones, when a test asks.
        property bool longWords: false
        function textArgs(key, args) { return mock.longWords ? key + ": " + args : key }
        function agendaView() { return "{}" }
        function setAgendaView(view) { return "" }
        function showDays(from, count, upcoming) { mock.asked.push(count) }
        function refreshHealth() { mock.healthAsked++ }
        function relayToday() { mock.relaid++ }
        // What the last "Don't mention it again" gave.
        property string asideGiven: ""
        function setOverlapAside(key) { mock.asideGiven = key }
        function setTaskAt(uid, at) { return "" }
        // The agenda filter's word: what the page asked, last.
        property var turned: null
        function turnAgendas(ids, on) { mock.turned = { ids: JSON.parse(ids), on: on }; return "" }
        function agendasAsHours() { mock.turned = "hours"; return "" }
        // The Day's and the Week's steps by area.
        property var stepAreas: []
        function tickStepAreas(areas) { mock.stepAreas = mock.stepAreas.concat([areas]); return "" }
        function stepAreasEverything() { mock.stepAreas = mock.stepAreas.concat(["everything"]); return "" }
        function stepAreasAsHours() { mock.stepAreas = mock.stepAreas.concat(["hours"]); return "" }
        // Children's events asked about (part 3): none here.
        function holderQuestions() { return "[]" }
    }

    QtObject {
        id: shell

        property string today: "2099-01-05"
        // The places at the window's left, New at their top (main.qml); false folded into ☰.
        property bool railShown: true
        property real now: root.monday + 10 * 3600
        property real backAt: 0
        property var cardRef: null
        property var opened: []
        property var menuAt: null
        // The window's event menu, its Delete chosen: Rust deletes it and hands the agenda again.
        property var cardHost: QtObject {
            function eventMenu(row) {
                root.gone = root.gone.concat([row.title])
                root.tell()
            }
            function needMenu(row) {}
        }

        function openCard(ref) {
            shell.opened.push(ref)
        }

        function openTask(uid) {}
    }

    // The appointments deleted, and those added, by title.
    property var gone: []
    property var added: []

    // Thirty days, each with three appointments; the agenda's whole answer made again.
    function tell() {
        const days = []
        for (let n = 0; n < 30; ++n) {
            const d = root.day(n)
            const rows = [["Morning", 540], ["Noon", 720], ["Evening", 1080]].map(p => root.placed(p[0] + " " + n, "event", d.start, p[1], p[1] + 30, { when: "at " + p[1] }))
                .concat(root.added.filter(a => a.day === n).map(a => root.placed(a.title, "event", d.start, 600, 630, { when: "10:00" })))
            d.comes = { rows: rows.filter(r => root.gone.indexOf(r.item.title) < 0).sort((a, b) => a.from_minute - b.from_minute) }
            days.push(d)
        }
        mock.agenda = JSON.stringify({ version: 3, from: "2099-01-05", count: 31, comes: true, sentence: "", days: days })
    }

    Component {
        id: pageComponent

        AgendaPage {
            sioul: mock
            theme: testTheme
            window: shell
        }
    }

    // Tasks ▸ The day's own declaration (TasksPage.qml): its window through a
    // binding, its way given as a literal, which Qt sets first.
    QtObject {
        id: tasksPage

        property var window: shell
    }

    Component {
        id: embeddedComponent

        AgendaPage {
            sioul: mock
            theme: testTheme
            window: tasksPage.window
            modes: ["day"]
            mode: "day"
            embedded: true
        }
    }

    function find(item, name) {
        if (!item)
            return null
        if (item.objectName === name)
            return item
        for (let i = 0; i < item.children.length; i++) {
            const found = root.find(item.children[i], name)
            if (found)
                return found
        }
        return null
    }

    function all(item, test, out) {
        out = out || []
        for (let i = 0; i < item.children.length; i++) {
            const child = item.children[i]
            if (test(child))
                out.push(child)
            root.all(child, test, out)
        }
        return out
    }

    function count(item) {
        let n = 1
        for (let i = 0; i < item.children.length; i++)
            n += root.count(item.children[i])
        return n
    }

    // A day of the agenda's JSON (version 3), `offset` days after Monday.
    function day(offset, more) {
        const start = root.monday + offset * 86400
        const d = new Date(start * 1000)
        const pad = n => n < 10 ? "0" + n : String(n)
        const date = d.getFullYear() + "-" + pad(d.getMonth() + 1) + "-" + pad(d.getDate())
        return Object.assign({ date: date, title: offset === 0 ? "Today" : date, today: offset === 0, past: false, start: start, hours_from: -1, hours_to: -1, head: [], timed: [], planned: true }, more || {})
    }

    function placed(id, kind, start, from, to, more, itemMore) {
        const t = m => start + m * 60
        const item = Object.assign({ id: id, kind: kind, time: { type: "span", start: t(from), at: t(from), until: t(to), end: t(to) }, source: { type: "calendar", key: id + ".ics", uid: id, start: t(from), recurring: false, calendar: "c", collection: "local/c" }, can: { edit: "event", drag: "event", ends: true }, title: id }, itemMore || {})
        return Object.assign({ item: item, when: "", from_minute: from, to_minute: to }, more || {})
    }

    function dated(id, start, tone) {
        return placed(id, "dated", start, 0, 1440, { all_day: true, when: "All day" }, { time: { type: "day", date: "2099-01-05", until: "2099-01-05" }, source: { type: "dated", kind: "task", key: id }, can: { edit: "task", drag: "none" }, tone: tone, owner: id, line: "Date asked · today" })
    }

    function makePage(width) {
        const page = test.createTemporaryObject(pageComponent, root, { width: width || 1000, height: 860 })
        test.waitForRendering(page)
        return page
    }

    TestCase {
        id: test

        name: "PlaceAgenda"
        when: windowShown

        property var page: null
        property var list: null

        function frame() {
            root.Window.window.update()
            waitForRendering(root)
        }

        // The appointments shown, with their tops in the list's view, top first.
        function rows() {
            return root.all(list.contentItem, c => c.timeWidth !== undefined && c.known !== undefined && c.visible)
                .map(r => ({ title: r.title, y: r.mapToItem(list, 0, 0).y, height: r.height, row: r }))
                .filter(r => r.y + r.height > 0 && r.y < list.height)
                .sort((a, b) => a.y - b.y)
        }

        function toMiddle() {
            list.contentY = (list.contentHeight - list.height) / 2
            frame()
            list.contentY += rows()[0].y + 7
            frame()
            return rows()[0]
        }

        function init() {
            root.gone = []
            root.added = []
            page = createTemporaryObject(pageComponent, root, { width: 1000, height: 860 })
            page.mode = "agenda"
            wait(20)
            root.tell()
            wait(50)
            frame()
            list = root.all(page, c => c.positionViewAtIndex !== undefined && c.visible && c.count === 30)[0]
            verify(list !== undefined, "What comes, its thirty days")
        }

        function test_an_appointment_deleted_from_its_menu_keeps_the_place() {
            const top = toMiddle()
            verify(top.title.indexOf(" 0") < 0, "scrolled into the list")
            rows()[4].row.menu()
            wait(0)
            frame()
            compare(rows()[0].title, top.title, "the same appointment at the top")
            compare(rows()[0].y, top.y, "at the same pixel")
            // The top one deleted: the next one takes its place.
            const next = rows()[1]
            rows()[0].row.menu()
            wait(0)
            frame()
            compare(rows()[0].title, next.title, "the next one at the top")
            compare(rows()[0].y, top.y, "in its place")
        }

        function test_one_added_above_keeps_the_place() {
            const top = toMiddle()
            root.added = [{ day: 1, title: "Added on another device" }]
            root.tell()
            wait(0)
            frame()
            compare(rows()[0].title, top.title)
            compare(rows()[0].y, top.y)
        }

        // ◂: another stretch asked, opened on today (its first day) again.
        function test_another_stretch_opens_on_today() {
            toMiddle()
            page.move(-1)
            wait(20)
            const from = page.asked.from
            root.tell()
            const answer = JSON.parse(mock.agenda)
            answer.from = from
            mock.agenda = JSON.stringify(answer)
            wait(50)
            frame()
            compare(rows()[0].title, "Morning 0", "today at the top")
        }
    }
}
