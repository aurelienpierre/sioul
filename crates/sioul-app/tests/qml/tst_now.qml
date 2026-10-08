// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The window's clock (Clock.qml) and the line at now in the day views
// (DayView.qml, WeekPlanning.qml, HealthTimeline.qml, TaskTimeline.qml), on a
// fake clock: the minute and the day said only when they turn, a turn early
// or late set right, the line moving without the day laid again, today's
// column moving at midnight. Run by tools/qml-test.sh, in a fixed time zone
// (TZ=Europe/Paris).

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 1000
    height: 800

    // The fake wall clock, in milliseconds since 1970.
    property real fake: 0

    // Local time, as the views read it.
    function ms(y, mo, d, h, mi, s) {
        return new Date(y, mo - 1, d, h || 0, mi || 0, s || 0).getTime()
    }
    function sec(y, mo, d, h, mi) {
        return root.ms(y, mo, d, h, mi, 0) / 1000
    }

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        property bool away: false
        property string status: ""
        function text(key) { return key }
        function textWith(key, name, value) { return key + "(" + value + ")" }
        function textArgs(key, args) { return key }
        function dayStart() { return 7 }
    }

    Component {
        id: clockComponent

        Clock {
            source: () => root.fake
        }
    }
    Component {
        id: dayComponent

        DayView {
            sioul: mock
            theme: testTheme
        }
    }
    Component {
        id: planningComponent

        WeekPlanning {
            sioul: mock
            theme: testTheme
            locale: Qt.locale("en_GB")
            days: []
        }
    }
    Component {
        id: healthComponent

        HealthTimeline {
            sioul: mock
            theme: testTheme
            fromMinute: 360
            toMinute: 1380
        }
    }
    Component {
        id: taskTimelineComponent

        TaskTimeline {
            sioul: mock
            theme: testTheme
            timeline: null
        }
    }

    // The item named `name` under `item`, its visual children searched.
    function find(item, name) {
        for (let i = 0; i < item.children.length; i++) {
            const child = item.children[i]
            if (child.objectName === name)
                return child
            const found = root.find(child, name)
            if (found)
                return found
        }
        return null
    }
    // Every item under `item` that `test` takes.
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
    // A delegate drawing the thing whose key is `key`.
    function drawn(item, key) {
        return root.all(item, c => c.modelData !== undefined && c.modelData !== null && typeof c.modelData === "object" && c.modelData.key === key)[0] || null
    }

    TestCase {
        name: "Now"
        when: windowShown

        // One clock: the minute and the day, said only when they turn, today
        // first; a turn early or late set right; held while put away, read at
        // once when back; the backend's minute at each turn, put away or not.
        function test_clock() {
            root.fake = root.ms(2026, 10, 6, 23, 58, 30)
            const clock = createTemporaryObject(clockComponent, root)
            compare(clock.today, "2026-10-06")
            compare(clock.now, root.sec(2026, 10, 6, 23, 58))
            compare(clock.turn.interval, 30050, "to the next turn, a little after it")
            const said = []
            clock.nowChanged.connect(() => said.push("now"))
            clock.todayChanged.connect(() => said.push("today"))
            let turns = 0
            clock.turned.connect(() => turns++)
            // Within the same minute (a window come back): nothing said.
            root.fake = root.ms(2026, 10, 6, 23, 58, 50)
            clock.read()
            compare(said, [])
            // A turn come early (Qt's coarse timers): nothing turned, set again to the turn.
            root.fake = root.ms(2026, 10, 6, 23, 58, 58)
            clock.turn.triggered()
            compare(turns, 0)
            compare(said, [])
            compare(clock.turn.interval, 2050)
            // The minute turns: now, alone.
            root.fake = root.ms(2026, 10, 6, 23, 59, 0) + 60
            clock.turn.triggered()
            compare(turns, 1)
            compare(said, ["now"])
            compare(clock.now, root.sec(2026, 10, 6, 23, 59))
            // Midnight: today first, then now.
            root.fake = root.ms(2026, 10, 7, 0, 0, 0) + 50
            clock.turn.triggered()
            compare(said, ["now", "today", "now"])
            compare(clock.today, "2026-10-07")
            compare(turns, 2)
            // A computer asleep two hours: its late turn says the time read, not counted.
            root.fake = root.ms(2026, 10, 7, 2, 0, 41)
            clock.turn.triggered()
            compare(clock.now, root.sec(2026, 10, 7, 2, 0))
            compare(clock.turn.interval, 19050)
            // Put away on a phone: held, the backend's minute still turned; back: read at once.
            clock.paused = true
            root.fake = root.ms(2026, 10, 7, 7, 30, 5)
            clock.turn.triggered()
            compare(turns, 4)
            compare(clock.now, root.sec(2026, 10, 7, 2, 0))
            clock.paused = false
            compare(clock.now, root.sec(2026, 10, 7, 7, 30))
        }

        // Tasks ▸ Day: the line follows the clock, nothing laid again for it; the
        // block under way follows too; laid again at each five minutes' turn only.
        function test_day_view() {
            const midnight = root.sec(2026, 10, 6, 0, 0)
            const block = (start, end, kind, title, key) => ({ start: midnight + start * 60, end: midnight + end * 60, kind: kind, title: title, key: key, energy: "", location: "", column: 0, columns: 1, part: 0, pinned: false, note: "", read_only: false, recurring: false })
            const day = { from: midnight + 7 * 3600, to: midnight + 22 * 3600, now: midnight + 10 * 3600, hours: [], blocks: [block(540, 600, "task", "Write", "t1"), block(660, 720, "event", "Call", "e1")], all_day: [], more: 0, said: [] }
            const view = createTemporaryObject(dayComponent, root, { width: 600, height: 700, day: day, now: midnight + 10 * 3600 })
            waitForRendering(view)
            let relays = 0
            view.relay.connect(() => relays++)
            const line = root.find(view, "nowLine")
            verify(line !== null)
            verify(line.visible)
            compare(line.y, view.at(midnight + 600 * 60) - 1)
            const card = root.drawn(view, "t1")
            verify(card !== null)
            compare(view.current, -1, "between the step and the call")
            compare(view.next, 1)
            // Half an hour later: the line lower, the same cards (nothing made again).
            const before = line.y
            view.now = midnight + 630 * 60
            compare(line.y, view.at(midnight + 630 * 60) - 1)
            verify(line.y > before)
            verify(root.drawn(view, "t1") === card, "the day is not laid again for the line")
            compare(relays, 1, "a five minutes' turn passed: laid again once")
            // A minute within the same five minutes: the line alone.
            view.now = midnight + 631 * 60
            compare(relays, 1)
            compare(line.y, view.at(midnight + 631 * 60) - 1)
            // In the call: it is under way.
            view.now = midnight + 670 * 60
            compare(view.current, 1)
            // Past the day's end (after midnight, before the day is laid again): no line.
            view.now = midnight + 24 * 3600 + 60
            verify(!line.visible)
        }

        // Agenda ▸ Week: one line, in today's column; today's title and column
        // move at midnight, before the agenda is read again.
        function test_week() {
            const day = (date, d) => ({ title: date, date: date, today: false, start: root.sec(2026, 10, d, 0, 0), events: [], hours_from: 420, hours_to: 1320 })
            const event = { key: "e1", uid: "e1", start: root.sec(2026, 10, 6, 9, 0), end: root.sec(2026, 10, 6, 10, 0), when: "09:00 – 10:00", summary: "Call", location: "", notes: "", calendar: "", color: null, recurring: false, all_day: false, cancelled: false, tentative: false, read_only: false, from_minute: 540, to_minute: 600, column: 0, columns: 1 }
            const days = [day("2026-10-05", 5), Object.assign(day("2026-10-06", 6), { events: [event] }), day("2026-10-07", 7)]
            const view = createTemporaryObject(planningComponent, root, { width: 700, height: 700, days: days, today: "2026-10-06", now: root.sec(2026, 10, 6, 10, 0) })
            waitForRendering(view)
            const line = root.find(view, "nowLine")
            verify(line !== null && line.visible)
            compare(view.todayIndex, 1)
            compare(line.x, view.gutter + view.dayWidth)
            compare(line.y, view.yOf(600))
            const card = root.drawn(view, "e1")
            verify(card !== null)
            const bold = () => root.all(view, c => c.font !== undefined && c.font.weight === Font.Bold && c.text !== undefined).map(c => c.text)
            compare(bold().length, 1)
            const todayTitle = bold()[0]
            // Three quarters of an hour later: lower, the same events.
            view.now = root.sec(2026, 10, 6, 10, 45)
            compare(line.y, view.yOf(645))
            verify(root.drawn(view, "e1") === card, "the week is not laid again for the line")
            // Midnight: today's column is the next one; 00:00 is before the hours shown.
            view.today = "2026-10-07"
            view.now = root.sec(2026, 10, 7, 0, 0)
            compare(view.todayIndex, 2)
            verify(!line.visible)
            compare(bold().length, 1)
            verify(bold()[0] !== todayTitle, "today's title moved")
            view.now = root.sec(2026, 10, 7, 8, 0)
            verify(line.visible)
            compare(line.x, view.gutter + 2 * view.dayWidth)
            compare(line.y, view.yOf(480))
            // A week without today: no line, no title marked.
            view.today = "2026-10-12"
            compare(view.todayIndex, -1)
            verify(!line.visible)
            compare(bold().length, 0)
        }

        // Health, day and week: one line on today; today's ground and title move at midnight.
        function test_health() {
            const day = (date, d) => ({ date: date, title: date, weekday: "D", number: d, today: false, past: false, items: [], context: [], lines: [], segments: [] })
            const days = [day("2026-10-05", 5), day("2026-10-06", 6), day("2026-10-07", 7)]
            const week = createTemporaryObject(healthComponent, root, { width: 600, height: 700, titles: true, days: days, today: "2026-10-06", now: root.sec(2026, 10, 6, 12, 0) })
            waitForRendering(week)
            const line = root.find(week, "nowLine")
            verify(line !== null && line.visible)
            compare(week.todayIndex, 1)
            compare(line.x, week.gutter + week.columnWidth)
            compare(line.y, week.yOf(720) - 1)
            week.now = root.sec(2026, 10, 6, 12, 30)
            compare(line.y, week.yOf(750) - 1)
            const ground = () => root.all(week, c => c.color !== undefined && c.visible && c.anchors !== undefined && c.anchors.fill !== null && Qt.colorEqual(c.color, testTheme.surface))
            compare(ground().length, 1)
            const was = ground()[0]
            // Midnight: the next column is today, its ground with it; 00:00 is before the hours shown.
            week.today = "2026-10-07"
            week.now = root.sec(2026, 10, 7, 0, 1)
            compare(week.todayIndex, 2)
            verify(!line.visible)
            compare(ground().length, 1)
            verify(ground()[0] !== was, "today's ground moved")
            week.now = root.sec(2026, 10, 7, 7, 0)
            verify(line.visible)
            compare(line.x, week.gutter + 2 * week.columnWidth)
            // The day view: one column; another day than today shows no line.
            const one = createTemporaryObject(healthComponent, root, { y: 0, width: 300, height: 700, days: [day("2026-10-06", 6)], today: "2026-10-06", now: root.sec(2026, 10, 6, 9, 0) })
            waitForRendering(one)
            const oneLine = root.find(one, "nowLine")
            verify(oneLine.visible)
            compare(oneLine.y, one.yOf(540) - 1)
            one.today = "2026-10-07"
            verify(!oneLine.visible)
        }

        // Tasks ▸ Timeline: today's line and bold day follow the day, before the plan is made again.
        function test_task_timeline() {
            const head = (date, day) => ({ date: date, day: day, weekday: "Tuesday", month: "", rest: false })
            const view = createTemporaryObject(taskTimelineComponent, root, { width: 800, height: 400, timeline: { days: [head("2026-10-06", "6"), head("2026-10-07", "7"), head("2026-10-08", "8")], rows: [], note: "" }, today: "2026-10-06" })
            waitForRendering(view)
            compare(view.todayIndex, 0)
            view.today = "2026-10-07"
            compare(view.todayIndex, 1)
            view.today = "2026-10-20"
            compare(view.todayIndex, -1)
        }
    }
}
