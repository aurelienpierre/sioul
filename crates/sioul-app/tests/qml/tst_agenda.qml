// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The agenda's page on a stand-in for Sioul and its window (the unified
// agenda, part 5): What comes (today in full and its doses as the Porch lists
// them, unnamed, nothing to press; other days what differs; the plan's line
// opening its day), the Month (dates, then appointments; a phone's marks and
// list), and a phone's Day head (DayHead.qml, as the page uses it). Fiction
// only. Run by tools/qml-test.sh.

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
        function textArgs(key, args) { return key }
        function agendaView() { return "{}" }
        function setAgendaView(view) { return "" }
        function showDays(from, count, upcoming) { mock.asked.push(count) }
        function refreshHealth() { mock.healthAsked++ }
        function relayToday() { mock.relaid++ }
        function setOverlapAside(key) {}
        function setTaskAt(uid, at) { return "" }
    }

    QtObject {
        id: shell

        property string today: "2099-01-05"
        property real now: root.monday + 10 * 3600
        property real backAt: 0
        property var cardRef: null
        property var opened: []
        property var menuAt: null
        property var cardHost: QtObject {
            function eventMenu(row) {}
            function needMenu(row) {}
        }

        function openCard(ref) {
            shell.opened.push(ref)
        }

        function openTask(uid) {}
    }

    Component {
        id: pageComponent

        AgendaPage {
            sioul: mock
            theme: testTheme
            window: shell
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

        name: "Agenda"
        when: windowShown

        function init() {
            mock.asked = []
            shell.opened = []
            mock.doseMarks = ""
            mock.agenda = ""
            shell.now = root.monday + 10 * 3600
            shell.backAt = 0
            mock.healthAsked = 0
            mock.relaid = 0
        }

        // What comes showing today asks for the doses' reading as the Day does: when
        // shown, at each minute's turn and on coming back; and lays today again at
        // each five minutes' turn. Showing last week, it asks nothing (review of
        // 21436bf, M2 and later 9).
        function test_what_comes_asks_for_the_doses_each_minute() {
            const page = makePage()
            page.mode = "agenda"
            const today = day(0)
            today.comes = { rows: [], doses: [{ minute: 1200, when: "20:00", title: "Dose", keys: ["a@2"] }] }
            mock.agenda = JSON.stringify({ version: 3, from: "2099-01-05", sentence: "", days: [today, day(1, { comes: { rows: [] } })] })
            wait(20)
            mock.healthAsked = 0
            mock.relaid = 0
            page.visible = false
            page.visible = true
            compare(mock.healthAsked, 1, "shown: asked")
            shell.now += 60
            compare(mock.healthAsked, 2, "a minute later: asked again")
            shell.backAt = shell.now
            compare(mock.healthAsked, 3, "back on the screen: asked at once")
            shell.now = (Math.floor(shell.now / 300) + 1) * 300 + 1
            compare(mock.relaid, 1, "at the five minutes' turn: today laid again")
            verify(page.todayShown, "today shown in What comes")
            // Last week: nothing of today shown, nothing asked.
            mock.agenda = JSON.stringify({ version: 3, from: "2098-12-29", sentence: "", days: [day(-7), day(-6)] })
            wait(20)
            verify(!page.todayShown)
            const asked = mock.healthAsked
            shell.now += 60
            compare(mock.healthAsked, asked, "nothing asked")
        }

        // What comes (§4.6): today's rows in order, tonight's night last; a dose
        // marked taken left out, one still to come plain, one come with its mark
        // and what is known; never a name, nothing to press (a tap opens its day
        // in Health); the plan's line opens the day; another day its own rows.
        function test_what_comes_lists_today_and_what_differs() {
            const page = makePage()
            page.mode = "agenda"
            const today = day(0)
            today.comes = {
                rows: [placed("Doctor", "event", today.start, 840, 870, { when: "14:00 – 14:30" }), placed("Lunch", "meal", today.start, 750, 780, { when: "12:30 – 13:00" }, { privacy: "health" }), placed("Night", "night", today.start, 1380, 1440, { when: "From 23:00" }, { privacy: "health" })],
                doses: [{ minute: 480, when: "08:00", title: "Dose", keys: ["a@1"] }, { minute: 720, when: "12:00", title: "Dose", keys: ["b@1"] }, { minute: 1200, when: "20:00", title: "Two doses", keys: ["a@2", "b@2"] }],
                plan: "The plan: Write, then Sort"
            }
            const thursday = day(3)
            thursday.comes = { rows: [placed("Dentist", "event", thursday.start, 600, 630, { when: "10:00 – 10:30" })], plan: "The plan: Call first" }
            const tuesday = day(1)
            tuesday.comes = { rows: [] }
            const state = (key, minute, given) => ({ key: key, day: "2099-01-05", minute: minute, states: [{ from: 0, state: given, words: given.state === "taken" ? "Taken at 08:02" : given.come ? "Due at 12:00" : "Due at 20:00" }] })
            mock.doseReadAt = shell.now - 30
            mock.doseMarks = JSON.stringify({ rows: [state("a@1", 480, { state: "taken" }), state("b@1", 720, { state: "open", come: true }), state("a@2", 1200, { state: "open", come: false }), state("b@2", 1200, { state: "open", come: false })], reading: "Reading", holds: 120, old: "Old" })
            mock.agenda = JSON.stringify({ version: 3, from: "2099-01-05", sentence: "", days: [today, tuesday, day(2, { comes: { rows: [] } }), thursday] })
            wait(50)
            const rows = root.all(page, c => c.timeWidth !== undefined && c.known !== undefined && c.visible)
            compare(rows.map(r => r.title).join(" | "), "Dose | Lunch | Doctor | Two doses | Night | Dentist", "today's in time order, the night last; the taken dose left out; days without anything left out")
            const come = rows.find(r => r.when === "12:00")
            compare([come.known, come.line].join(" "), "come Due at 12:00")
            const later = rows.find(r => r.title === "Two doses")
            compare(later.known, "", "still to come: plainly")
            // A dose row says Rust's words alone, never a key or a name (review of 21436bf, later 9).
            compare(rows.filter(r => r.kind === "dose").map(r => r.title).join(" | "), "Dose | Two doses")
            verify(rows.every(r => r.title.indexOf("@") < 0 && r.line.indexOf("@") < 0), "never a dose's key")
            mouseClick(come)
            compare(shell.opened.length, 1)
            compare([shell.opened[0].kind, shell.opened[0].date, shell.opened[0].srcKey].join(" "), "dose 2099-01-05 b@1")
            const plans = root.all(page, c => c.objectName === "comesPlan" && c.visible)
            compare(plans.map(p => p.text).join(" | "), "The plan: Write, then Sort | The plan: Call first")
            mouseClick(plans[1])
            compare(page.mode, "day", "its day opens")
        }

        // Show leaves out of What comes and the Month what it leaves out of the Day
        // and the Week, on this device: doses, meals and rest, a calendar by its
        // place, the plan's line with the steps; never a date asked (review of
        // 21436bf, later 2). Its button shows there too.
        function test_show_holds_in_what_comes_and_the_month() {
            const page = makePage()
            page.mode = "agenda"
            page.view = { doses: false, needs: false, steps: false, calendars: [{ id: "local/c", name: "c" }] }
            const today = day(0)
            const other = placed("Choir", "event", today.start, 1080, 1140, { when: "18:00 – 19:00" }, { source: { type: "calendar", key: "choir.ics", uid: "choir", start: today.start + 1080 * 60, recurring: false, calendar: "Leisure", collection: "local/leisure" } })
            today.comes = {
                rows: [placed("Doctor", "event", today.start, 840, 870, { when: "14:00 – 14:30" }), placed("Lunch", "meal", today.start, 750, 780, { when: "12:30 – 13:00" }, { privacy: "health" }), other, dated("CAF", today.start, "plain")],
                doses: [{ minute: 1200, when: "20:00", title: "Dose", keys: ["a@2"] }],
                plan: "The plan: Write, then Sort"
            }
            mock.agenda = JSON.stringify({ version: 3, from: "2099-01-05", sentence: "", days: [today] })
            wait(50)
            const rows = root.all(page, c => c.timeWidth !== undefined && c.known !== undefined && c.visible)
            compare(rows.map(r => r.title).join(" | "), "CAF | Choir", "the hidden calendar, the meal and the dose left out; the date kept")
            compare(root.all(page, c => c.objectName === "comesPlan" && c.visible).length, 0, "the plan's line with the steps")
            verify(root.find(page, "agendaShow").visible, "Show offered in What comes")
            page.mode = "month"
            const monday = day(0)
            monday.lines = [dated("CAF", monday.start, "plain"), placed("Doctor", "event", monday.start, 840, 870), other]
            mock.agenda = JSON.stringify({ version: 3, from: "2099-01-05", sentence: "", days: [monday] })
            wait(50)
            compare(root.all(page, c => c.objectName === "monthLine").map(l => l.modelData.item.title).join(" | "), "CAF | Choir")
        }

        // The Month on a computer (§4.5): a cell's dates first, then its
        // appointments, three lines, then "…"; a tap on a line opens its card;
        // a day off shaded.
        function test_month_on_a_computer() {
            const page = makePage()
            page.mode = "month"
            const monday = day(0)
            monday.lines = [dated("CAF", monday.start, "plain"), placed("Fair", "event", monday.start, 0, 1440, { all_day: true }), placed("Doctor", "event", monday.start, 840, 870), placed("Yoga", "event", monday.start, 1080, 1140)]
            const tuesday = day(1, { off: true })
            mock.agenda = JSON.stringify({ version: 3, from: "2099-01-05", sentence: "", days: [monday, tuesday].concat([2, 3, 4, 5, 6].map(n => day(n))) })
            wait(50)
            const cells = root.all(page, c => c.objectName === "monthCell")
            compare(cells.length, 7)
            const lines = root.all(cells[0], c => c.objectName === "monthLine")
            compare(lines.length, 3, "three lines, then …")
            verify(root.all(cells[0], c => c.text === "…" && c.visible).length === 1)
            compare(lines[0].modelData.item.title, "CAF", "its date first")
            verify(String(cells[1].color) !== String(cells[2].color), "a day off shaded")
            mouseClick(lines[1])
            compare(shell.opened.length, 1)
            compare([shell.opened[0].kind, shell.opened[0].srcKey].join(" "), "event Fair.ics")
        }

        // The Month on a phone: two marks a cell, a diamond for a date and a dot
        // for appointments; under the grid, the month's dates and appointments
        // from today on, day by day; a cell's tap brings its day to the list's top.
        function test_month_on_a_phone() {
            const page = makePage(412)
            page.mode = "month"
            const monday = day(0)
            monday.lines = [dated("CAF", monday.start, "plain"), placed("Doctor", "event", monday.start, 840, 870, { when: "14:00 – 14:30" })]
            const friday = day(4)
            friday.lines = [placed("Dentist", "event", friday.start, 600, 630, { when: "10:00 – 10:30" })]
            mock.agenda = JSON.stringify({ version: 3, from: "2099-01-05", sentence: "", days: [monday, day(1), day(2), day(3), friday, day(5), day(6)] })
            wait(50)
            const cells = root.all(page, c => c.objectName === "monthCell")
            verify(root.all(cells[0], c => c.objectName === "monthDiamond" && c.visible).length === 1 && root.all(cells[0], c => c.objectName === "monthDot" && c.visible).length === 1)
            compare(root.all(cells[4], c => c.objectName === "monthDiamond" && c.visible).length, 0)
            compare(root.all(page, c => c.objectName === "monthLine").length, 0, "no words in a phone's cell")
            const listed = root.all(page, c => c.timeWidth !== undefined && c.known !== undefined && c.visible)
            compare(listed.map(r => r.title).join(" | "), "CAF | Doctor | Dentist")
            mouseClick(listed[2])
            compare([shell.opened[0].kind, shell.opened[0].srcKey].join(" "), "event Dentist.ics")
            // A cell's tap brings its day to the top of the list (review of 21436bf, later 9).
            const busy = [0, 1, 2, 3, 4, 5, 6].map(n => {
                const d = day(n)
                d.lines = [placed("Morning " + n, "event", d.start, 540, 570, { when: "09:00 – 09:30" }), placed("Noon " + n, "event", d.start, 720, 750, { when: "12:00 – 12:30" }), placed("Evening " + n, "event", d.start, 1080, 1110, { when: "18:00 – 18:30" })]
                return d
            })
            mock.agenda = JSON.stringify({ version: 3, from: "2099-01-05", sentence: "", days: busy })
            wait(50)
            const list = root.all(page, c => c.positionViewAtIndex !== undefined && c.visible)[0]
            verify(list.contentHeight > list.height, "a list longer than its room")
            compare(list.contentY, 0)
            const busyCells = root.all(page, c => c.objectName === "monthCell")
            mouseClick(busyCells[2])
            wait(50)
            verify(list.contentY > 0, "scrolled to its day")
            compare(list.itemAtIndex(2).y, list.contentY, "Wednesday at the top")
        }

        // A phone's Day head as the page uses it (review of e5bd27b, later 7):
        // the day's chips, the date first with its diamond and words, within
        // its share of the line; a tap opens the date's card. The phone's Day
        // page's objects, counted.
        function test_a_phone_s_day_head_on_the_page() {
            // Laid out once: never a line rearranged by what it holds (found here, 29 October 2026).
            failOnWarning(/recursive rearrange/)
            const page = makePage(412)
            page.mode = "day"
            const monday = day(0, { head: [dated("CAF", root.monday, "plain"), placed("Fair", "event", root.monday, 0, 1440, { all_day: true, when: "All day" })] })
            mock.agenda = JSON.stringify({ version: 3, from: "2099-01-05", sentence: "", head: [{ from: shell.now - 60, until: shell.now + 3600, text: "Work until 17:00.", said: "Work until 17:00." }], days: [monday] })
            wait(100)
            const diamond = root.find(page, "agendaDayDiamond")
            const words = root.find(page, "agendaDayChips")
            verify(diamond !== null && diamond.visible, "its diamond")
            compare(words.text, "CAF · Date asked · today  agenda-chips-more")
            verify(words.parent.width <= page.width * 0.4 + 1, "within its share: " + words.parent.width)
            mouseClick(words)
            compare(shell.opened.length, 1)
            compare([shell.opened[0].kind, shell.opened[0].datedKind, shell.opened[0].owner].join(" "), "dated task CAF")
            console.warn("sioul-objects: a phone's agenda page, its Day " + root.count(page))
        }
    }
}
