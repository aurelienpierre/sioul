// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The three timelines' drags, by mouse and by touch (HealthTimeline.qml,
// WeekPlanning.qml, DayView.qml), on a stand-in for Sioul that records what
// each drop asks: a meal or the night moved or stretched, an event moved or
// its end dragged, a repeating one asking first, one that can only be read
// staying, a step pinned to a time. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 1000
    height: 800

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        property bool away: false
        property string status: ""
        property var calls: []
        function text(key) { return key }
        function textWith(key, name, value) { return key + "(" + value + ")" }
        function textArgs(key, args) { return key }
        function dayStart() { return 7 }
        function dragNeed(json) { mock.calls.push({ what: "dragNeed", edit: JSON.parse(json) }); return "" }
        function moveEvent(key, start, newStart, newEnd, onlyThis) { mock.calls.push({ what: "moveEvent", key: key, start: start, newStart: newStart, newEnd: newEnd, onlyThis: onlyThis }); return "" }
        function pinTask(uid, at, minutes) { mock.calls.push({ what: "pinTask", uid: uid, at: at, minutes: minutes }); return "" }
    }

    function iso(d) {
        const pad = n => n < 10 ? "0" + n : String(n)
        return d.getFullYear() + "-" + pad(d.getMonth() + 1) + "-" + pad(d.getDate())
    }

    // A Health day, far ahead (never past): breakfast, lunch, a nap, dinner, the night; and an old one, past.
    function healthDay(date, past) {
        const seg = (key, kind, start, end, at, until) => ({ date: date, key: key, kind: kind, name: key, from_minute: Math.max(0, start), to_minute: Math.min(1440, end), start_minute: start, end_minute: end, at_minute: at, until_minute: until, quiet: false, past: past, state: "" })
        return { date: date, title: date, weekday: "D", number: 1, today: false, past: past, items: [], context: [], lines: [], segments: [
            seg("meal:0", "meal", 470, 500, 480, 500),
            seg("meal:1", "meal", 730, 780, 750, 780),
            seg("nap:0", "nap", 840, 875, 840, 860),
            seg("meal:2", "meal", 1140, 1200, 1170, 1200),
            seg("sleep", "sleep", 1320, 1860, 1380, 1860)
        ] }
    }

    Component {
        id: healthLine

        HealthTimeline {
            sioul: mock
            theme: testTheme
            fromMinute: 360
            toMinute: 1440
        }
    }

    Component {
        id: planningComponent

        WeekPlanning {
            sioul: mock
            theme: testTheme
            locale: Qt.locale("en_GB")
        }
    }

    Component {
        id: dayComponent

        DayView {
            sioul: mock
            theme: testTheme
        }
    }

    function flickIn(item) {
        for (let i = 0; i < item.children.length; i++) {
            const child = item.children[i]
            if (child.flicking !== undefined && child.contentY !== undefined)
                return child
            const found = root.flickIn(child)
            if (found)
                return found
        }
        return null
    }

    TestCase {
        id: test

        name: "TimelineDrags"
        when: windowShown

        function init() {
            mock.calls = []
            mock.status = ""
        }

        // Where minute `m` of `line`'s column `column` is, in `line`'s coordinates (lane of the kept times).
        function healthPoint(line, column, minute, dy) {
            const flick = root.flickIn(line)
            const lane = line.lanes(column)
            const content = Qt.point(lane.x + lane.keptX + lane.keptWidth / 2, line.yOf(minute) + (dy || 0))
            return flick.contentItem.mapToItem(line, content.x, content.y)
        }

        function drag(item, from, to) {
            mousePress(item, from.x, from.y)
            const steps = 8
            for (let i = 1; i <= steps; i++)
                mouseMove(item, from.x + (to.x - from.x) * i / steps, from.y + (to.y - from.y) * i / steps)
            mouseRelease(item, to.x, to.y)
        }

        function test_health_move_and_ends() {
            const line = createTemporaryObject(healthLine, root, { width: 600, height: 760, days: [root.healthDay("2099-01-07", false)] })
            waitForRendering(line)
            // Lunch (kept 12:10–13:00) taken by its middle and dragged 35 minutes down.
            const from = healthPoint(line, 0, 755)
            const to = healthPoint(line, 0, 790)
            drag(line, from, to)
            compare(mock.calls.length, 1, JSON.stringify(mock.calls))
            compare(mock.calls[0].edit.action, "move")
            compare(mock.calls[0].edit.key, "meal:1")
            compare(mock.calls[0].edit.date, "2099-01-07")
            compare(mock.calls[0].edit.from, "12:45")
            // Its bottom edge, 13:00 to 14:00: its length.
            mock.calls = []
            drag(line, healthPoint(line, 0, 780, -3), healthPoint(line, 0, 840, -3))
            compare(mock.calls.length, 1, JSON.stringify(mock.calls))
            compare(mock.calls[0].edit.action, "times")
            compare(mock.calls[0].edit.from, "12:10")
            compare(mock.calls[0].edit.to, "14:00")
            // The night's top edge, half an hour down: bedtime 23:30, its winding down with it, waking as it was.
            mock.calls = []
            drag(line, healthPoint(line, 0, 1320, 4), healthPoint(line, 0, 1350, 4))
            compare(mock.calls.length, 1, JSON.stringify(mock.calls))
            compare(mock.calls[0].edit.key, "sleep")
            compare(mock.calls[0].edit.action, "times")
            compare([mock.calls[0].edit.from, mock.calls[0].edit.bed, mock.calls[0].edit.to].join(" "), "22:30 23:30 07:00")
            // Let go where it was: nothing changes.
            mock.calls = []
            drag(line, healthPoint(line, 0, 1160), healthPoint(line, 0, 1161))
            compare(mock.calls.length, 0, JSON.stringify(mock.calls))
            // A tap chooses, a right click asks for the menu: neither changes anything.
            let tapped = null, menu = null
            line.tapped.connect(segment => tapped = segment.key)
            line.menu.connect(segment => menu = segment.key)
            const lunch = healthPoint(line, 0, 755)
            mouseClick(line, lunch.x, lunch.y)
            mouseClick(line, lunch.x, lunch.y, Qt.RightButton)
            compare(tapped, "meal:1")
            compare(menu, "meal:1")
            compare(mock.calls.length, 0)
        }

        function test_health_week_stays_on_its_day() {
            const days = [root.healthDay("2099-01-05", false), root.healthDay("2099-01-06", false), root.healthDay("2099-01-07", false)]
            const line = createTemporaryObject(healthLine, root, { width: 900, height: 760, days: days, titles: true })
            waitForRendering(line)
            // Tuesday's dinner dragged towards Wednesday's column: an hour later, still Tuesday's.
            const from = healthPoint(line, 1, 1170)
            const to = healthPoint(line, 2, 1230)
            drag(line, from, to)
            compare(mock.calls.length, 1, JSON.stringify(mock.calls))
            compare(mock.calls[0].edit.date, "2099-01-06")
            compare(mock.calls[0].edit.from, "20:00")
        }

        function test_health_past_stays() {
            const line = createTemporaryObject(healthLine, root, { width: 600, height: 760, days: [root.healthDay("2001-01-07", true)] })
            waitForRendering(line)
            drag(line, healthPoint(line, 0, 755), healthPoint(line, 0, 800))
            compare(mock.calls.length, 0, JSON.stringify(mock.calls))
        }

        function test_health_touch_swipe_scrolls_and_long_press_picks_up() {
            // A readable timeline taller than its view: a swipe scrolls, nothing moves.
            const line = createTemporaryObject(healthLine, root, { width: 412, height: 400, readable: true, days: [root.healthDay("2099-01-07", false)] })
            waitForRendering(line)
            const flick = root.flickIn(line)
            flick.contentY = line.yOf(700) - 40
            const before = flick.contentY
            const lunch = healthPoint(line, 0, 755)
            let touch = touchEvent(line)
            touch.press(0, line, lunch.x, lunch.y).commit()
            for (let i = 1; i <= 6; i++)
                touchEvent(line).move(0, line, lunch.x, lunch.y - 20 * i).commit()
            touchEvent(line).release(0, line, lunch.x, lunch.y - 120).commit()
            wait(50)
            verify(flick.contentY > before + 20, "scrolled: " + before + " → " + flick.contentY)
            compare(mock.calls.length, 0, JSON.stringify(mock.calls))
            // Held, it is picked up; moved half an hour, let go: lunch at 12:40.
            flick.contentY = line.yOf(700) - 40
            const held = healthPoint(line, 0, 755)
            touchEvent(line).press(0, line, held.x, held.y).commit()
            wait(700)
            const scroll = flick.contentY
            for (let i = 1; i <= 6; i++)
                touchEvent(line).move(0, line, held.x, held.y + (line.yOf(785) - line.yOf(755)) * i / 6).commit()
            touchEvent(line).release(0, line, held.x, held.y + line.yOf(785) - line.yOf(755)).commit()
            wait(50)
            compare(flick.contentY, scroll, "the view held still")
            compare(mock.calls.length, 1, JSON.stringify(mock.calls))
            compare(mock.calls[0].edit.action, "move")
            compare(mock.calls[0].edit.from, "12:40")
            // Held and let go in place: its menu, nothing changed.
            mock.calls = []
            let menu = null
            line.menu.connect(segment => menu = segment.key)
            flick.contentY = line.yOf(1100) - 40
            const again = healthPoint(line, 0, 1180)
            touchEvent(line).press(0, line, again.x, again.y).commit()
            wait(700)
            touchEvent(line).release(0, line, again.x, again.y).commit()
            wait(50)
            compare(menu, "meal:2")
            compare(mock.calls.length, 0)
        }

        function test_health_thin_long_press_asks_readable() {
            // A phone's thin day (the whole day in 250 px): a long press opens it readable.
            const line = createTemporaryObject(healthLine, root, { width: 412, height: 250, days: [root.healthDay("2099-01-07", false)] })
            waitForRendering(line)
            verify(line.thin)
            let asked = 0
            line.readableAsked.connect(() => asked++)
            const lunch = healthPoint(line, 0, 755)
            touchEvent(line).press(0, line, lunch.x, lunch.y).commit()
            wait(700)
            compare(asked, 1)
            // The block under the finger came with it.
            verify(line.grab !== null && line.grab.segment.key === "meal:1", JSON.stringify(line.grab))
            touchEvent(line).release(0, line, lunch.x, lunch.y).commit()
            wait(50)
            // Between two close blocks (the nap's end, 14:35, and a point 10 px below): none taken.
            line.readable = false
            wait(50)
            const between = healthPoint(line, 0, 875, 6)
            const near = line.pickNear(Qt.point(line.lanes(0).x + line.lanes(0).keptX + 5, line.yOf(875) + 6))
            verify(near === null || near.segment.key === "nap:0")
            compare(mock.calls.length, 0)
        }

        function event(key, day, from, to, extra) {
            const start = day.start + from * 60
            return Object.assign({ key: key, uid: key, summary: key, location: "", notes: "", calendar: "c", color: "", start: start, end: day.start + to * 60, from_minute: from, to_minute: to, all_day: false, recurring: false, read_only: false, cancelled: false, tentative: false, when: "", column: 0, columns: 1, organizer: "", attendees: [] }, extra || {})
        }

        function week() {
            const days = []
            for (let n = 0; n < 3; n++) {
                const d = new Date(2099, 0, 5 + n)
                days.push({ date: root.iso(d), today: false, start: d.getTime() / 1000, events: [], hours_from: 9 * 60, hours_to: 18 * 60, title: "" })
            }
            days[0].events = [event("a.ics", days[0], 600, 660), event("r.ics", days[0], 840, 900, { recurring: true }), event("ro.ics", days[0], 720, 780, { read_only: true })]
            return days
        }

        function planningPoint(planning, column, minute, dy) {
            const flick = root.flickIn(planning)
            const content = Qt.point(planning.gutter + column * planning.dayWidth + planning.dayWidth / 2, planning.yOf(minute) + (dy || 0))
            return flick.contentItem.mapToItem(planning, content.x, content.y)
        }

        function test_agenda_move_end_and_repeating() {
            const days = week()
            const planning = createTemporaryObject(planningComponent, root, { width: 900, height: 760, days: days })
            waitForRendering(planning)
            // 10:00–11:00 to the next day at 11:30, an hour still.
            drag(planning, planningPoint(planning, 0, 630), planningPoint(planning, 1, 720))
            compare(mock.calls.length, 1, JSON.stringify(mock.calls))
            compare(mock.calls[0].what, "moveEvent")
            compare(mock.calls[0].onlyThis, false)
            compare(mock.calls[0].newStart, days[1].start + 690 * 60)
            compare(mock.calls[0].newEnd - mock.calls[0].newStart, 3600)
            // Its end, 11:00 to 11:45.
            mock.calls = []
            drag(planning, planningPoint(planning, 0, 660, -3), planningPoint(planning, 0, 705, -3))
            compare(mock.calls.length, 1, JSON.stringify(mock.calls))
            compare(mock.calls[0].newStart, days[0].start + 600 * 60)
            compare(mock.calls[0].newEnd, days[0].start + 705 * 60)
            // A calendar that can only be read: it stays.
            mock.calls = []
            drag(planning, planningPoint(planning, 0, 750), planningPoint(planning, 0, 800))
            compare(mock.calls.length, 0, JSON.stringify(mock.calls))
            // A repeating one asks first; "Only this time" moves that time alone.
            drag(planning, planningPoint(planning, 0, 870), planningPoint(planning, 0, 900))
            compare(mock.calls.length, 0, "asked before")
            verify(planning.landed !== null)
            const dialog = findChild(planning, "") // the question lives in its Loader
            let question = null
            for (let i = 0; i < planning.children.length; i++)
                if (planning.children[i].item && planning.children[i].item.ask)
                    question = planning.children[i].item
            verify(question !== null && question.opened, "the question is open")
            question.answered = true
            question.close()
            planning.save(true)
            compare(mock.calls.length, 1, JSON.stringify(mock.calls))
            compare(mock.calls[0].onlyThis, true)
            compare(mock.calls[0].newStart, days[0].start + 870 * 60)
        }

        function test_day_step_given_a_time() {
            const today = new Date()
            today.setHours(0, 0, 0, 0)
            const midnight = today.getTime() / 1000
            const at = m => midnight + m * 60
            const block = (start, end, kind, title, key, more) => Object.assign({ start: at(start), end: at(end), kind: kind, title: title, key: key, energy: "", location: "", column: 0, columns: 1, part: 0, pinned: false, note: "", read_only: false, recurring: false }, more || {})
            const day = { from: at(7 * 60), to: at(21 * 60), now: at(8 * 60), hours: [{ start: at(9 * 60), end: at(17 * 60), kind: "work" }], all_day: [], more: 0, said: [], blocks: [
                block(540, 570, "task", "Letter", "letter-uid"),
                block(600, 660, "event", "Meeting", "m.ics"),
                block(660, 680, "margin", "Meeting", "m.ics"),
                block(750, 780, "meal", "Lunch", "meal:1"),
                block(840, 870, "event", "Yoga", "y.ics", { recurring: true }),
                block(960, 1020, "event", "Shared", "s.ics", { read_only: true })
            ] }
            const view = createTemporaryObject(dayComponent, root, { width: 700, height: 760, day: day })
            waitForRendering(view)
            view.now = at(8 * 60)
            const flick = root.flickIn(view)
            flick.contentY = 0
            const point = (seconds, dy) => flick.contentItem.mapToItem(view, 300, view.at(seconds) + 1 + (dy || 0))
            // The letter, 09:00, dragged to 10:30: pinned there today, as long as its card.
            drag(view, point(at(550)), point(at(640)))
            compare(mock.calls.length, 1, JSON.stringify(mock.calls))
            compare(mock.calls[0].what, "pinTask")
            compare(mock.calls[0].uid, "letter-uid")
            compare(mock.calls[0].at, root.iso(today) + "T10:30")
            compare(mock.calls[0].minutes, 30)
            // Lunch, 12:30, an hour later: today's change, as on the Health page.
            mock.calls = []
            drag(view, point(at(760)), point(at(820)))
            compare(mock.calls.length, 1, JSON.stringify(mock.calls))
            compare(mock.calls[0].what, "dragNeed")
            compare(mock.calls[0].edit.key, "meal:1")
            compare(mock.calls[0].edit.from, "13:30")
            // The meeting, by its margin card: half an hour later, its length kept.
            mock.calls = []
            drag(view, point(at(668)), point(at(698)))
            compare(mock.calls.length, 1, JSON.stringify(mock.calls))
            compare(mock.calls[0].what, "moveEvent")
            compare([mock.calls[0].key, mock.calls[0].start, mock.calls[0].newStart, mock.calls[0].newEnd, mock.calls[0].onlyThis].join(" "), ["m.ics", at(600), at(630), at(690), false].join(" "))
            // A calendar that can only be read: it stays, and opens on a click.
            mock.calls = []
            let opened = ""
            view.openEvent.connect(key => opened = key)
            drag(view, point(at(980)), point(at(1040)))
            compare(mock.calls.length, 0, JSON.stringify(mock.calls))
            mouseClick(view, point(at(980)).x, point(at(980)).y)
            compare(opened, "s.ics")
            // A repeating one asks first.
            drag(view, point(at(850)), point(at(880)))
            compare(mock.calls.length, 0, "asked before")
            verify(view.landed !== null)
            view.saveEvent(true)
            compare(mock.calls.length, 1, JSON.stringify(mock.calls))
            compare([mock.calls[0].key, mock.calls[0].newStart, mock.calls[0].onlyThis].join(" "), ["y.ics", at(870), true].join(" "))
        }
    }
}
