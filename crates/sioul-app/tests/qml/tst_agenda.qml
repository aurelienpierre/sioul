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
            mock.longWords = false
            mock.overlaps = "[]"
            mock.asideGiven = ""
            shell.railShown = true
        }

        // New event: none on the page beside the places' New; where they fold into ☰
        // (a phone, a narrow window), the page keeps its own (5 November 2026).
        function test_new_event_only_where_new_is_out_of_sight() {
            const wide = makePage()
            mock.agenda = JSON.stringify({ version: 3, from: "2099-01-05", sentence: "", can_add: true, days: [day(0)] })
            wait(50)
            const button = root.find(wide, "agendaNewEvent")
            verify(button !== null && !button.visible, "the places' New makes events")
            shell.railShown = false
            verify(button.visible, "the places folded: a new event from the page")
            const phone = makePage(412)
            wait(50)
            verify(root.find(phone, "agendaNewEvent").visible, "a phone's")
        }

        // Tasks ▸ The day as TasksPage.qml declares it: its way, a literal, is set before
        // the binding that gives its window, and the change of way read nothing before the
        // page was whole (10 November 2026: "Cannot read property 'today' of undefined"
        // in the phone demo). It asks for its one day once made, without an error.
        function test_the_tasks_day_made_as_tasks_declares_it() {
            failOnWarning(/TypeError|ReferenceError/)
            mock.asked = []
            const page = test.createTemporaryObject(embeddedComponent, root, { width: 1000, height: 860 })
            test.waitForRendering(page)
            compare(page.mode, "day")
            compare(mock.asked, [1], "its one day asked, once")
        }

        // Tasks ▸ The day is this page given the Day alone: no switch to the other ways, no
        // Agenda ⚙; it asks for its one day while in sight only, and never draws days asked
        // for another way or stretch, the window's agenda being one (8 November 2026).
        function test_the_tasks_day_is_this_page_given_the_day() {
            const page = test.createTemporaryObject(pageComponent, root, { width: 1000, height: 860, modes: ["day"], mode: "day", embedded: true })
            test.waitForRendering(page)
            verify(!root.find(page, "agendaModes").visible, "no switch to the other ways")
            verify(!root.all(page, o => o.view === "agenda" && o.visible).length, "no Agenda ⚙")
            compare(mock.asked[mock.asked.length - 1], 1, "its one day asked")
            // What comes' fifteen days, asked by the Agenda: not laid as the Day's columns.
            const from = page.asked.from
            mock.agenda = JSON.stringify({ version: 3, from: from, count: 15, sentence: "", days: [day(0), day(1), day(2)] })
            wait(20)
            compare(page.shown.days.length, 0, "another stretch's days never drawn")
            mock.agenda = JSON.stringify({ version: 3, from: from, count: 1, sentence: "", days: [day(0)] })
            wait(20)
            compare(page.shown.days.length, 1, "its own drawn")
            // Out of sight: its reading waits until it shows again.
            page.visible = false
            mock.asked = []
            page.load()
            compare(mock.asked.length, 0, "nothing asked out of sight")
            page.visible = true
            compare(mock.asked, [1], "asked once shown")
            // The Agenda's own page keeps its switch.
            const agenda = makePage()
            verify(root.find(agenda, "agendaModes").visible, "the Agenda's switch")
        }

        // What comes, all of it (8 November 2026): a month asked first, never two weeks; while
        // the list does not fill the page it loads on by itself, to a month past the next thing
        // beyond it (`next`), its days drawn meanwhile; nothing more once nothing comes next;
        // ◂ back to a month.
        function test_what_comes_loads_on_never_cut_at_two_weeks() {
            const page = makePage()
            wait(20)
            compare(mock.asked[mock.asked.length - 1], 31, "a month first")
            verify(page.asked.comes, "What comes asked as such")
            const from = page.asked.from
            const later = new Date(from + "T12:00:00")
            later.setDate(later.getDate() + 100)
            mock.asked = []
            mock.agenda = JSON.stringify({ version: 3, from: from, count: 31, comes: true, next: page.iso(later), sentence: "", days: [day(0)] })
            wait(50)
            compare(mock.asked, [132], "on to a month past the next thing")
            compare(page.shown.days.length, 1, "its days drawn while it loads on")
            mock.asked = []
            const far = day(100)
            far.comes = { rows: [placed("Far", "event", far.start, 600, 660, { when: "10:00 – 11:00" })] }
            mock.agenda = JSON.stringify({ version: 3, from: from, count: 132, comes: true, sentence: "", days: [day(0), far] })
            wait(50)
            compare(page.shown.days.length, 2)
            verify(root.all(page, o => o.title === "Far" && o.visible).length > 0, "the far appointment listed")
            compare(mock.asked, [], "nothing next: nothing more asked")
            page.move(-1)
            compare(mock.asked[mock.asked.length - 1], 31, "◂: a month again")
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

        // What comes with Show leaving the doses out: no reading asked of the Health job for
        // rows it would not show; the medicines unread, said there as in the Day (review of
        // 679f96a, later 9).
        function test_what_comes_asks_no_doses_while_show_leaves_them_out() {
            const page = makePage()
            page.mode = "agenda"
            const today = day(0)
            today.comes = { rows: [], doses: [{ minute: 1200, when: "20:00", title: "Dose", keys: ["a@2"] }] }
            mock.agenda = JSON.stringify({ version: 3, from: "2099-01-05", sentence: "", doses_note: "The medicines could not be read here.", days: [today, day(1, { comes: { rows: [] } })] })
            wait(20)
            const note = () => root.all(page, o => o.text === "The medicines could not be read here.")
            verify(note().some(o => o.visible), "the note in What comes")
            page.view = { doses: false, needs: true, steps: true, calendars: [] }
            wait(20)
            mock.healthAsked = 0
            page.visible = false
            page.visible = true
            shell.now += 60
            shell.backAt = shell.now
            compare(mock.healthAsked, 0, "nothing asked while Show leaves the doses out")
            verify(!note().some(o => o.visible), "nor the note")
            page.view = { doses: true, needs: true, steps: true, calendars: [] }
            wait(20)
            shell.now += 60
            compare(mock.healthAsked, 1, "asked again once shown")
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

        // What comes after midnight: the night going on (its evening the day before) at its place by
        // time, first, never after the evening (review of the branch `wake`, L1); this evening's last.
        function test_what_comes_puts_the_night_going_on_first() {
            const page = makePage()
            page.mode = "agenda"
            const today = day(0)
            const needs = (date) => ({ privacy: "health", source: { type: "needs", date: date, key: "sleep" } })
            today.comes = { rows: [placed("Lunch", "meal", today.start, 750, 780, { when: "12:30 – 13:00" }, { privacy: "health" }), placed("Last night", "night", today.start, 0, 450, { when: "Until 07:30" }, needs(day(-1).date)), placed("Night", "night", today.start, 1380, 1440, { when: "From 23:00" }, needs(today.date))], doses: [] }
            mock.agenda = JSON.stringify({ version: 3, from: today.date, sentence: "", days: [today] })
            wait(50)
            const rows = root.all(page, c => c.timeWidth !== undefined && c.known !== undefined && c.visible)
            compare(rows.map(r => r.title).join(" | "), "Last night | Lunch | Night")
        }

        // An event over several days and its line (`ongoing::Through`, the ongoing round), Monday to Wednesday.
        function trip(lane, more) {
            const line = (first, last) => ({ id: "Trip", uid: "trip", colour: "#3d6b8f", lane: lane || 0, first: first, last: last })
            const spans = [placed("Trip", "event", root.monday, 480, 1440, { when: "Mon 5 Jan 08:00 → Wed 7 Jan 19:00" }, { colour: "#3d6b8f" })]
            const monday = day(0, { comes: { rows: [] }, through: { lines: [line(true, false)], more: more || 0, rows: [{ id: "Trip", mark: "first", when: "08:00", line: "Mon 5 Jan 08:00 → Wed 7 Jan 19:00", minute: 480, lane: lane || 0 }] } })
            const tuesday = day(1, { comes: { rows: [] }, through: { lines: [line(false, false)], more: 0, rows: [] } })
            const wednesday = day(2, { through: { lines: [line(false, true)], more: 0, rows: [{ id: "Trip", mark: "end", when: "19:00", line: "until 19:00", minute: 1140, lane: lane || 0 }] } })
            wednesday.comes = { rows: [placed("Dentist", "event", wednesday.start, 600, 630, { when: "10:00 – 10:30" })] }
            return { days: [monday, tuesday, wednesday], spans: spans }
        }

        // What comes writes an event over several days once, on its first day, its
        // whole span under its title; its line runs down the list's left edge, in its
        // lane, from that row through the days shown to its end mark on its last day,
        // whose row says "until 19:00" with no mark of its own; a tap on the line opens
        // the event; beyond three lines, "+2" after the day's title.
        function test_what_comes_draws_an_event_over_several_days_once() {
            const page = makePage()
            page.mode = "agenda"
            const shown = trip(1, 2)
            mock.agenda = JSON.stringify({ version: 3, from: "2099-01-05", sentence: "", days: shown.days, spans: shown.spans })
            wait(50)
            const rows = root.all(page, c => c.timeWidth !== undefined && c.known !== undefined && c.visible)
            compare(rows.map(r => r.title + (r.ending ? " (end)" : "")).join(" | "), "Trip | Dentist | Trip (end)", "the trip once, its end mark's row on Wednesday; Tuesday, with nothing else, left out")
            compare([rows[0].when, rows[0].line].join(" "), "08:00 Mon 5 Jan 08:00 → Wed 7 Jan 19:00")
            compare([rows[2].when, rows[2].line].join(" "), "19:00 until 19:00")
            const lines = root.all(page, c => c.objectName === "spanLine" && c.visible)
            compare(lines.length, 2, "its line on each day row shown")
            verify(lines.every(l => l.x === 4 + 7 - 5), "in its lane, 1")
            const middle = item => item.mapToItem(page, 0, item.height / 2).y
            const top = l => l.mapToItem(page, 0, 0).y
            const bottom = l => l.mapToItem(page, 0, l.height).y
            fuzzyCompare(top(lines[0]), middle(rows[0]), 1, "from its row on its first day")
            fuzzyCompare(bottom(lines[1]), middle(rows[2]), 1, "to its end mark on its last")
            verify(bottom(lines[0]) >= top(lines[1]) - 1, "through the gap between the days")
            const ends = root.all(page, c => c.objectName === "spanEnd" && c.visible)
            compare(ends.length, 1, "one end mark")
            verify(rows.every(r => r.mapToItem(page, 0, 0).x >= page.lineRoom), "the rows beside the lines")
            const titles = root.all(page, c => c.text !== undefined && String(c.text).indexOf("agenda-lines-more") >= 0 && c.visible)
            compare(titles.map(t => t.text).join(" | "), "Today   agenda-lines-more", "+2 after Monday's title alone")
            mouseClick(lines[1], 6, 4)
            compare(shell.opened.length, 1)
            compare([shell.opened[0].kind, shell.opened[0].itemId].join(" "), "event Trip", "the line opens the event")
        }

        // A list starting after an event began: "Ongoing since …" at its first day's head,
        // its line from there (`ongoing::Mark::Since`).
        function test_what_comes_says_ongoing_since() {
            const page = makePage()
            page.mode = "agenda"
            const shown = trip(0, 0)
            const today = shown.days[1]
            today.through.lines[0].first = true
            today.through.rows = [{ id: "Trip", mark: "since", when: "", line: "Ongoing since Mon 5 Jan, until Wed 7 Jan", minute: -1, lane: 0 }]
            today.comes.rows = [placed("Call", "event", today.start, 540, 570, { when: "09:00 – 09:30" })]
            mock.agenda = JSON.stringify({ version: 3, from: "2099-01-06", sentence: "", days: [today, shown.days[2]], spans: shown.spans })
            wait(50)
            const rows = root.all(page, c => c.timeWidth !== undefined && c.known !== undefined && c.visible)
            compare(rows.map(r => [r.title, r.line].filter(t => t !== "").join(": ")).join(" | "), "Trip: Ongoing since Mon 5 Jan, until Wed 7 Jan | Call | Dentist | Trip: until 19:00", "at the head of its first day")
            const lines = root.all(page, c => c.objectName === "spanLine" && c.visible)
            fuzzyCompare(lines[0].mapToItem(page, 0, 0).y, rows[0].mapToItem(page, 0, rows[0].height / 2).y, 1, "its line from there")
        }

        // No list of conflicts on the agenda (several agendas, §5.3 of the research): whatever
        // the notices hold (the Porch's), the page lists none and counts none; a conflict of
        // the Day is a thin bracket joining its pair, both drawn, nothing else.
        function test_no_list_of_conflicts_and_a_bracket_in_the_day() {
            const page = makePage()
            mock.overlaps = JSON.stringify([{ key: "k1", day: "2099-01-05", today: true, first: { key: "a.ics", start: root.monday, title: "A", from: "10:00", to: "11:00" }, second: { key: "b.ics", start: root.monday, title: "B", from: "10:30", to: "11:30" }, gap: -30, needed: 0, at_once: true, source: "", notice: true, answers: ["keep"] }])
            mock.agenda = JSON.stringify({ version: 3, from: "2099-01-05", sentence: "", days: [day(0, { comes: { rows: [] } })] })
            wait(50)
            compare(root.all(page, c => c.text === "overlap-set-aside" && c.visible).length, 0, "no list above the agenda")
            compare(root.all(page, c => c.objectName === "overlapsMore").length, 0, "no count")
            page.mode = "day"
            const today = day(0)
            const a = placed("Dentist", "event", today.start, 600, 660, { when: "10:00 – 11:00", columns: 2 })
            const b = placed("Swim", "event", today.start, 630, 690, { when: "10:30 – 11:30", column: 1, columns: 2 }, { source: { type: "calendar", key: "swim.ics", uid: "swim", start: today.start + 630 * 60, recurring: false, calendar: "Léa", collection: "local/lea" } })
            today.timed = [a, b]
            today.conflicts = [{ a: a.item.id, b: b.item.id }]
            mock.agenda = JSON.stringify({ version: 3, from: "2099-01-05", sentence: "", frame: { from: 420, to: 1320 }, days: [today] })
            wait(80)
            const brackets = root.all(page, c => c.objectName === "conflictBracket" && c.visible)
            compare(brackets.length, 1, "one bracket, joining the pair")
            verify(brackets[0].across, "side by side: across the gap")
            // An agenda the filter leaves out is never joined: its pair is not drawn.
            mock.agenda = JSON.stringify({ version: 3, from: "2099-01-05", sentence: "", frame: { from: 420, to: 1320 }, hidden: ["local/lea"], days: [today] })
            wait(80)
            compare(root.all(page, c => c.objectName === "conflictBracket" && c.visible).length, 0, "left out: no bracket")
        }

        // A grid's bar of the trip (`ongoing::Bar`) on a week's row.
        function tripBar(row, from, to, more) {
            return Object.assign({ id: "Trip", uid: "trip", title: "Trip", colour: "#3d6b8f", row: row, from_column: from, to_column: to, lane: 0, from: "08:00", to: "19:00", cut_before: false, cut_after: false, line: "Trip, Mon 5 Jan 08:00 → Wed 7 Jan 19:00" }, more || {})
        }

        // The grids (the ongoing round): the Week draws an event of 24 hours or more as one
        // bar across its days at the top, its times at its two ends; "+2" under a day beyond
        // three; a tap opens it; an evening past midnight keeps its two hour blocks. The Day
        // says its head line. The Month: one bar a week's row, its title in each.
        function test_the_grids_draw_long_events_as_bars() {
            const page = makePage()
            page.mode = "week"
            const shown = trip(0, 0)
            const days = [0, 1, 2, 3, 4, 5, 6].map(n => day(n))
            days[3].timed = [placed("Concert", "event", days[3].start, 1320, 1440, { when: "From 22:00", spans: true })]
            days[4].timed = [placed("Concert", "event", days[4].start, 0, 120, { when: "Until 02:00", spans: true })]
            mock.agenda = JSON.stringify({ version: 3, from: "2099-01-05", sentence: "", days: days, spans: shown.spans, bars: [tripBar(0, 0, 2)], bars_more: [0, 0, 0, 0, 2, 0, 0] })
            wait(50)
            const planning = root.all(page, o => typeof o.thingAt === "function" && typeof o.rowOf === "function")[0]
            // Drawn as a grid once the page has its width (a list until then).
            tryVerify(() => !planning.listed && root.all(page, c => c.objectName === "weekBar" && c.visible).length === 1, 3000, "one bar")
            const bars = root.all(page, c => c.objectName === "weekBar" && c.visible)
            fuzzyCompare(bars[0].width, 3 * planning.dayWidth - 3, 1, "across its three days")
            fuzzyCompare(bars[0].mapToItem(planning, 0, 0).x, planning.gutter, 1, "from Monday")
            verify(root.all(bars[0], c => c.text === "19:00").length === 1 && root.all(bars[0], c => String(c.text).indexOf("08:00") === 0).length === 1, "its times at its two ends")
            const beyond = root.all(page, c => c.objectName === "weekBarsMore" && c.visible)
            compare(beyond.map(l => l.text).join(" "), "agenda-lines-more", "+2 under Friday")
            verify(root.all(page, c => c.text === "Concert" && c.visible).length >= 2, "the evening's two blocks in the hours")
            mouseClick(bars[0])
            compare([shell.opened[0].kind, shell.opened[0].itemId].join(" "), "event Trip", "a tap opens it")
            // "+2" opens its day, whose head says every long event (fix W3).
            mouseClick(beyond[0])
            compare([page.mode, page.iso(page.anchor)].join(" "), "day 2099-01-09")
            // The Day: its head line.
            page.mode = "day"
            mock.agenda = JSON.stringify({ version: 3, from: "2099-01-06", sentence: "", days: [day(1)], spans: shown.spans, bars: [tripBar(0, 0, 0, { cut_before: true, cut_after: true, line: "Ongoing: Trip, until Wed 7 Jan 19:00" })], bars_more: [0] })
            wait(50)
            compare(root.all(page, c => c.objectName === "dayBar" && c.visible).map(l => l.text).join(" | "), "Ongoing: Trip, until Wed 7 Jan 19:00")
            compare(root.all(page, c => c.objectName === "weekBar" && c.visible).length, 0, "no week band in the Day")
            // The Month: one bar a week's row, over its cells.
            page.mode = "month"
            const weeks = []
            for (let n = 0; n < 42; n++)
                weeks.push(day(n))
            mock.agenda = JSON.stringify({ version: 3, from: "2099-01-05", sentence: "", days: weeks, spans: shown.spans, bars: [tripBar(0, 5, 6, { cut_after: true }), tripBar(1, 0, 1, { cut_before: true })] })
            wait(50)
            const pieces = root.all(page, c => c.objectName === "monthBar" && c.visible)
            compare(pieces.length, 2, "one a week's row")
            verify(pieces.every(b => root.all(b, c => c.text === "Trip").length === 1), "its title in each")
            const cells = root.all(page, c => c.objectName === "monthCell")
            const left = item => item.mapToItem(page, 0, 0).x
            fuzzyCompare(left(pieces[0]), left(cells[5]) + 3, 1, "from Saturday")
            fuzzyCompare(left(pieces[1]), left(cells[7]) + 3, 1, "the next row from Monday")
            verify(pieces[1].mapToItem(page, 0, 0).y > cells[7].mapToItem(page, 0, 0).y, "within its row")
        }

        // A phone's Week and Month lists (the ongoing round): the trip once, its line through
        // the days, its end mark; an evening's copies (`spans`) never written again; a day with
        // only a line passing, no "Nothing planned".
        function test_a_phone_s_lists_write_an_event_over_several_days_once() {
            const page = makePage(412)
            page.mode = "week"
            const shown = trip(0, 0)
            const days = shown.days.concat([3, 4, 5, 6].map(n => day(n)))
            days[3].timed = [placed("Concert", "event", days[3].start, 1320, 1440, { when: "From 22:00", spans: true })]
            mock.agenda = JSON.stringify({ version: 3, from: "2099-01-05", sentence: "", days: days, spans: shown.spans })
            wait(50)
            const texts = () => root.all(page, c => typeof c.text === "string" && c.visible).map(c => c.text)
            compare(texts().filter(t => t.indexOf("Trip") === 0).join(" | "), "Trip\nMon 5 Jan 08:00 → Wed 7 Jan 19:00 | Trip\nuntil 19:00", "its row, then its end mark's")
            compare(texts().filter(t => t.indexOf("Concert") === 0).length, 0, "the evening's copy not written")
            compare(texts().filter(t => t === "agenda-free").length, 4, "Thursday to Sunday free, Tuesday not: the line passes")
            compare(root.all(page, c => c.objectName === "spanLine" && c.visible).length, 4, "its row, Tuesday's title, Wednesday's title and its end mark's row")
            compare(root.all(page, c => c.objectName === "spanEnd" && c.visible).length, 1)
            // The Month's list.
            page.mode = "month"
            const month = shown.days.concat([3, 4, 5, 6].map(n => day(n)))
            month[0].lines = []
            month[3].lines = [placed("Concert", "event", month[3].start, 1320, 1440, { when: "From 22:00", spans: true })]
            mock.agenda = JSON.stringify({ version: 3, from: "2099-01-05", sentence: "", days: month, spans: shown.spans })
            wait(50)
            const rows = root.all(page, c => c.timeWidth !== undefined && c.known !== undefined && c.visible)
            compare(rows.map(r => r.title + ": " + r.line).join(" | "), "Trip: Mon 5 Jan 08:00 → Wed 7 Jan 19:00 | Trip: until 19:00", "once, and its end mark")
            compare(root.all(page, c => c.objectName === "spanLine" && c.visible).length, 2, "on Monday and Wednesday, the days listed")
            const cells = root.all(page, c => c.objectName === "monthCell")
            verify(root.all(cells[1], c => c.objectName === "monthDot" && c.visible).length === 1, "Tuesday's dot: the trip passes")
        }

        // Show leaves out of What comes and the Month what it leaves out of the Day
        // and the Week, on this device: doses, meals and rest, the plan's line with
        // the steps; never a date asked (review of 21436bf, later 2). The agenda
        // filter leaves out an agenda by its place (the agenda's `hidden`). Its
        // button shows there too, and the filter's when there are agendas.
        function test_show_holds_in_what_comes_and_the_month() {
            const page = makePage()
            page.mode = "agenda"
            page.view = { doses: false, needs: false, steps: false }
            const today = day(0)
            const other = placed("Choir", "event", today.start, 1080, 1140, { when: "18:00 – 19:00" }, { source: { type: "calendar", key: "choir.ics", uid: "choir", start: today.start + 1080 * 60, recurring: false, calendar: "Leisure", collection: "local/leisure" } })
            today.comes = {
                rows: [placed("Doctor", "event", today.start, 840, 870, { when: "14:00 – 14:30" }), placed("Lunch", "meal", today.start, 750, 780, { when: "12:30 – 13:00" }, { privacy: "health" }), other, dated("CAF", today.start, "plain")],
                doses: [{ minute: 1200, when: "20:00", title: "Dose", keys: ["a@2"] }],
                plan: "The plan: Write, then Sort"
            }
            const filter = { chosen: false, groups: [{ key: "yours:", title: "agendas-group-yours", agendas: [{ id: "local/c", name: "c", on: false, usual: false }, { id: "local/leisure", name: "Leisure", on: true, usual: true }] }] }
            mock.agenda = JSON.stringify({ version: 3, from: "2099-01-05", sentence: "", hidden: ["local/c"], filter: filter, days: [today] })
            wait(50)
            const rows = root.all(page, c => c.timeWidth !== undefined && c.known !== undefined && c.visible)
            compare(rows.map(r => r.title).join(" | "), "CAF | Choir", "the agenda left out, the meal and the dose left out; the date kept")
            compare(root.all(page, c => c.objectName === "comesPlan" && c.visible).length, 0, "the plan's line with the steps")
            verify(root.find(page, "agendaShow").visible, "Show offered in What comes")
            verify(root.find(page, "agendaFilter").visible, "the agenda filter offered")
            page.mode = "month"
            const monday = day(0)
            monday.lines = [dated("CAF", monday.start, "plain"), placed("Doctor", "event", monday.start, 840, 870), other]
            mock.agenda = JSON.stringify({ version: 3, from: "2099-01-05", sentence: "", hidden: ["local/c"], filter: filter, days: [monday] })
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
        // Part 6: a leg is a quiet line in its appointment's lead in the Day (no card of its
        // own), its row left out of a phone's week list (said on the appointment's line
        // there); Show's choice for the legs reads the days again.
        // A press at the end of an appointment's box, on its Coming back line, in the Week: the
        // appointment's body, never its end's handle, which would stretch it (review of
        // 4d7bab4, later 14).
        function test_a_leg_s_line_never_takes_the_end_handle() {
            const page = makePage()
            page.mode = "week"
            const span = { type: "span", start: root.monday + 840 * 60, at: root.monday + 840 * 60, until: root.monday + 870 * 60, end: root.monday + 900 * 60 }
            const monday = day(0, { timed: [placed("doctor", "event", root.monday, 840, 870, { when: "14:00 – 14:30" }, { time: span }), placed("walk", "leg", root.monday, 870, 900, { when: "14:30 – 15:00" }, { owner: "doctor" })] })
            mock.agenda = JSON.stringify({ version: 3, from: "2099-01-05", sentence: "", days: [monday, day(1), day(2), day(3), day(4), day(5), day(6)] })
            wait(50)
            const planning = root.all(page, o => typeof o.thingAt === "function" && typeof o.rowOf === "function")[0]
            // Laid as a grid once the page has its width: a list (no box) until then.
            tryVerify(() => !planning.listed, 3000, "the Week as a grid")
            const box = planning.boxOf(planning.rowOf(monday.timed[1], monday, 0, ""))
            const thing = planning.thingAt({ x: box.x + box.width / 2, y: box.y + box.height - 2 })
            compare(thing.row.kind + " " + thing.part, "event body")
        }

        function test_a_leg_is_a_quiet_line() {
            const page = makePage()
            page.mode = "day"
            const monday = day(0, { timed: [placed("doctor", "event", root.monday, 840, 870, { when: "14:00 – 14:30" }, { line: "Leave at 13:10 · Bus 24", time: { type: "span", start: root.monday + 790 * 60, at: root.monday + 840 * 60, until: root.monday + 870 * 60, end: root.monday + 870 * 60 } }), placed("bus", "leg", root.monday, 790, 815, { when: "13:10 – 13:35" }, { owner: "doctor", certainty: "unconfirmed" })] })
            mock.agenda = JSON.stringify({ version: 3, from: "2099-01-05", sentence: "", days: [monday] })
            wait(50)
            const lines = root.all(page, o => o.objectName === "planning-leg" && o.visible)
            compare(lines.length, 1, "one quiet line")
            verify(lines[0].opacity < 0.5, "fainter while its timetable was not checked")
            // A press on the leg's line: the leg's in the Day; in the Week, its appointment's,
            // whose lead it lies in (review of ab7daa4, later 8).
            const pressedOn = () => {
                const planning = root.all(page, o => typeof o.thingAt === "function" && typeof o.rowOf === "function")[0]
                const box = planning.boxOf(planning.rowOf(monday.timed[1], monday, 0, ""))
                const thing = planning.thingAt({ x: box.x + box.width / 2, y: box.y + box.height / 2 })
                return thing ? thing.row.kind : ""
            }
            compare(pressedOn(), "leg", "the Day: the leg")
            page.mode = "week"
            mock.agenda = JSON.stringify({ version: 3, from: "2099-01-05", sentence: "", days: [monday, day(1), day(2), day(3), day(4), day(5), day(6)] })
            wait(50)
            tryVerify(() => !root.all(page, o => typeof o.thingAt === "function" && typeof o.rowOf === "function")[0].listed, 3000, "the Week as a grid")
            compare(pressedOn(), "event", "the Week: its appointment")
            // A phone's week: the leg in the appointment's line, no row of its own.
            const phone = makePage(412)
            phone.mode = "week"
            mock.agenda = JSON.stringify({ version: 3, from: "2099-01-05", sentence: "", days: [monday, day(1), day(2), day(3), day(4), day(5), day(6)] })
            wait(50)
            const rows = root.all(phone, o => typeof o.text === "string" && o.visible && o.text.indexOf("bus") === 0)
            compare(rows.length, 0, "no row for the leg")
            verify(root.all(phone, o => typeof o.text === "string" && o.visible && o.text.indexOf("Leave at 13:10 · Bus 24") >= 0).length > 0, "said on its appointment's line")
            // Show: each leg as an event, the days read again.
            const asked = mock.asked.length
            page.showPart("legs", true)
            page.readAgain()
            verify(mock.asked.length > asked, "the days read again")
        }

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
