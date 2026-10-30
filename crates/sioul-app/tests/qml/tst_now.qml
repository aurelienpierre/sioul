// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The window's clock (Clock.qml) and the line at now in the day views
// (WeekPlanning.qml, TaskTimeline.qml), on a
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
        id: planningComponent

        WeekPlanning {
            sioul: mock
            theme: testTheme
            locale: Qt.locale("en_GB")
            days: []
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

        // A trip (Settings ▸ Agenda ▸ Time zone, the window's `zoneOffset`):
        // today and the hour are the zone shown's, whatever zone JavaScript
        // is in (Paris here, qml-test.sh's TZ), the minute the same instant;
        // back home, this device's day again at the next read.
        function test_clock_on_another_zone() {
            // 05:00 in Paris on 7 October 2026 (summer time, UTC+2): 03:00 UTC, 23:00 the day before in New York (UTC−4).
            root.fake = root.ms(2026, 10, 7, 5, 0, 30)
            const clock = createTemporaryObject(clockComponent, root, { offset: ms => -4 * 3600 * 1000 })
            compare(clock.today, "2026-10-06", "05:00 in Paris is 23:00 the day before in New York")
            compare(clock.wall(root.fake).getHours(), 23, "New York's hour, not one moved from Paris's")
            compare(clock.now, root.sec(2026, 10, 7, 5, 0), "the same instant")
            clock.offset = null
            clock.read()
            compare(clock.today, "2026-10-07", "back home: this device's day")
        }

        // The agenda's day and week (WeekPlanning): the line follows the clock, nothing laid again for it; the
        // block under way follows too; laid again at each five minutes' turn only.
        function test_week() {
            // The agenda's days (version 3): the call, a `timeline::Placed` row.
            const day = (date, d) => ({ title: date, date: date, today: false, past: false, start: root.sec(2026, 10, d, 0, 0), hours_from: -1, hours_to: -1, head: [], timed: [], planned: true })
            const start = root.sec(2026, 10, 6, 9, 0)
            const end = root.sec(2026, 10, 6, 10, 0)
            const event = { item: { id: "event:e1", kind: "event", time: { type: "span", start: start, at: start, until: end, end: end }, source: { type: "calendar", key: "e1", uid: "e1", start: start, recurring: false, calendar: "" }, can: { edit: "event", drag: "event", ends: true }, title: "Call" }, when: "09:00 – 10:00", from_minute: 540, to_minute: 600, detail: { end: end } }
            const days = [day("2026-10-05", 5), Object.assign(day("2026-10-06", 6), { timed: [event] }), day("2026-10-07", 7)]
            const view = createTemporaryObject(planningComponent, root, { width: 700, height: 700, days: days, frame: { from: 420, to: 1320 }, today: "2026-10-06", now: root.sec(2026, 10, 6, 10, 0) })
            waitForRendering(view)
            const line = root.find(view, "nowLine")
            verify(line !== null && line.visible)
            compare(view.todayIndex, 1)
            compare(line.x, view.gutter + view.dayWidth)
            compare(line.y, view.yOf(600))
            const card = root.all(view, c => c.itemId === "event:e1")[0] || null
            verify(card !== null)
            const bold = () => root.all(view, c => c.font !== undefined && c.font.weight === Font.Bold && c.text !== undefined).map(c => c.text)
            compare(bold().length, 1)
            const todayTitle = bold()[0]
            // Three quarters of an hour later: lower, the same events.
            view.now = root.sec(2026, 10, 6, 10, 45)
            compare(line.y, view.yOf(645))
            verify(root.all(view, c => c.itemId === "event:e1")[0] === card, "the week is not laid again for the line")
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

        // Tasks ▸ Gantt (TaskTimeline): one line on today; today's ground and title move at midnight.
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
