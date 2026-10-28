// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The three timelines' drags, by mouse and by touch (HealthTimeline.qml,
// WeekPlanning.qml, DayView.qml), on a stand-in for Sioul that records what
// each drop asks: a meal or the night moved or stretched, an event moved or
// its end dragged, a repeating one asking first, one that can only be read
// or a meeting with others staying, a step pinned to a time; each kind by its
// own writer (Paths.qml). On the agenda's timeline too: a publish changing
// one thing in place, the doses' marks under a fake clock. Run by
// tools/qml-test.sh.

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
        id: pathsComponent

        Paths {
            sioul: mock
        }
    }

    Component {
        id: dayComponent

        DayView {
            sioul: mock
            theme: testTheme
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

        // The agenda's days as its job gives them (version 3): each thing a
        // `timeline::Placed` row, its item saying what may move it.
        function v3day(date) {
            const d = new Date(date + "T00:00:00")
            return { date: date, title: date, today: false, past: false, start: d.getTime() / 1000, hours_from: -1, hours_to: -1, head: [], timed: [], edges: [], doses: [], planned: true }
        }

        function span(day, from, to, lead, tail) {
            const t = m => day.start + m * 60
            return { type: "span", start: t(lead === undefined ? from : lead), at: t(from), until: t(to), end: t(tail === undefined ? to : tail) }
        }

        function placed(item, day, from, to, more) {
            return Object.assign({ item: item, when: "", from_minute: Math.max(0, from), to_minute: Math.min(1440, to) }, more || {})
        }

        function event(key, day, from, to, extra) {
            extra = extra || {}
            const fixed = extra.read_only === true || extra.with_others === true
            const time = span(day, from, to)
            return placed({ id: "event:" + key, kind: "event", time: time, source: { type: "calendar", key: key, uid: key, start: time.at, recurring: extra.recurring === true, calendar: "c" }, can: { edit: fixed ? "none" : "event", drag: fixed ? "none" : "event", ends: !fixed }, title: key }, day, from, to, { detail: Object.assign({ end: time.until }, extra) })
        }

        function step(uid, day, from, to, held) {
            return placed({ id: "step:" + uid + "@" + day.date, kind: "step", time: span(day, from, to), certainty: held ? "held" : "planned", source: { type: "plan", uid: uid, date: day.date, part: 0 }, can: { edit: "task", drag: "pin" }, owner: uid, title: uid }, day, from, to)
        }

        function meal(key, day, lead, from, to, tail) {
            return placed({ id: "need:" + day.date + "/" + key, kind: "meal", time: span(day, from, to, lead, tail), certainty: "usual", source: { type: "needs", date: day.date, key: key }, can: { edit: "need", drag: "need", ends: true }, privacy: "health", title: "Lunch" }, day, from, to)
        }

        // The night from `evening` to `morning`: its winding down and bedtime on the evening, its waking on the morning.
        function night(evening, morning, wind, bed, wake) {
            const time = { type: "span", start: evening.start + wind * 60, at: evening.start + bed * 60, until: morning.start + wake * 60, end: morning.start + wake * 60 }
            const item = { id: "need:" + evening.date + "/sleep", kind: "night", time: time, certainty: "usual", source: { type: "needs", date: evening.date, key: "sleep" }, can: { edit: "need", drag: "need", ends: true }, privacy: "health", title: "Night" }
            evening.edges.push(placed(item, evening, bed, 1440))
            morning.edges.push(placed(item, morning, 0, wake))
        }

        function dose(key, day, minute, edge) {
            return placed({ id: "dose:" + key, kind: "dose", time: { type: "at", at: day.start + minute * 60 }, certainty: { dose: { state: "reading" } }, source: { type: "dose", key: key }, can: { edit: "none", drag: "none" }, privacy: "health", title: "Dose" }, day, minute, minute, { edge: edge || "" })
        }

        // Three days far ahead: appointments (one repeating, one only read, a
        // meeting with others), a step of the plan, lunch, the night between.
        function week() {
            const days = [v3day("2099-01-05"), v3day("2099-01-06"), v3day("2099-01-07")]
            days[0].timed = [event("a.ics", days[0], 600, 660), event("ro.ics", days[0], 720, 780, { read_only: true }), event("r.ics", days[0], 840, 900, { recurring: true }), event("m.ics", days[0], 960, 1020, { with_others: true })]
            days[1].timed = [step("letter", days[1], 540, 570, false), meal("meal:1", days[1], 730, 750, 780, 780)]
            night(days[0], days[1], 1350, 1380, 420)
            return days
        }

        function planningPoint(planning, column, minute, dy) {
            const flick = root.flickIn(planning)
            const content = Qt.point(planning.gutter + column * planning.dayWidth + planning.dayWidth / 2, planning.yOf(minute) + (dy || 0))
            return flick.contentItem.mapToItem(planning, content.x, content.y)
        }

        function makeWeek(more) {
            const planning = createTemporaryObject(planningComponent, root, Object.assign({ width: 900, height: 760, days: week(), frame: { from: 360, to: 1440 } }, more || {}))
            waitForRendering(planning)
            return planning
        }

        function test_agenda_move_end_and_repeating() {
            const planning = makeWeek()
            const days = planning.days
            // 10:00–11:00 to the next day at 11:30, an hour still.
            drag(planning, planningPoint(planning, 0, 630), planningPoint(planning, 1, 720))
            compare(mock.calls.length, 1, JSON.stringify(mock.calls))
            compare(mock.calls[0].what, "moveEvent")
            compare(mock.calls[0].key, "a.ics")
            compare(mock.calls[0].onlyThis, false)
            compare(mock.calls[0].newStart, days[1].start + 690 * 60)
            compare(mock.calls[0].newEnd - mock.calls[0].newStart, 3600)
            // Its end, 11:00 to 11:45.
            mock.calls = []
            drag(planning, planningPoint(planning, 0, 660, -3), planningPoint(planning, 0, 705, -3))
            compare(mock.calls.length, 1, JSON.stringify(mock.calls))
            compare(mock.calls[0].newStart, days[0].start + 600 * 60)
            compare(mock.calls[0].newEnd, days[0].start + 705 * 60)
            // A calendar that can only be read, a meeting with others: they stay.
            mock.calls = []
            drag(planning, planningPoint(planning, 0, 750), planningPoint(planning, 0, 800))
            drag(planning, planningPoint(planning, 0, 990), planningPoint(planning, 0, 1050))
            compare(mock.calls.length, 0, JSON.stringify(mock.calls))
            // ... and open on a click, as they are.
            let opened = ""
            planning.open.connect(row => opened = row.srcKey)
            mouseClick(planning, planningPoint(planning, 0, 750).x, planningPoint(planning, 0, 750).y)
            compare(opened, "ro.ics")
            // A repeating one asks first; "Only this time" moves that time alone.
            drag(planning, planningPoint(planning, 0, 870), planningPoint(planning, 0, 900))
            compare(mock.calls.length, 0, "asked before")
            verify(planning.landed !== null)
            let question = null
            for (let i = 0; i < planning.children.length; i++)
                if (planning.children[i].item && planning.children[i].item.ask)
                    question = planning.children[i].item
            verify(question !== null && question.opened, "the question is open")
            question.answer(true)
            compare(mock.calls.length, 1, JSON.stringify(mock.calls))
            compare(mock.calls[0].key, "r.ics")
            compare(mock.calls[0].onlyThis, true)
            compare(mock.calls[0].newStart, days[0].start + 870 * 60)
            // Its end dragged asks too; "Every time" changes every time's end.
            mock.calls = []
            planning.days = week()
            wait(20)
            drag(planning, planningPoint(planning, 0, 900, -3), planningPoint(planning, 0, 930, -3))
            compare(mock.calls.length, 0, "asked before")
            verify(question.opened, "the question is open again")
            question.answer(false)
            compare(mock.calls.length, 1, JSON.stringify(mock.calls))
            compare([mock.calls[0].key, mock.calls[0].newStart, mock.calls[0].newEnd, mock.calls[0].onlyThis].join(" "), ["r.ics", days[0].start + 840 * 60, days[0].start + 930 * 60, false].join(" "))
        }

        // A question waiting for its answer keeps what was let go, whatever the
        // days published meanwhile: the answer moves the event let go, as it was.
        function test_agenda_publish_while_asked() {
            const planning = makeWeek()
            const days = planning.days
            drag(planning, planningPoint(planning, 0, 870), planningPoint(planning, 0, 900))
            compare(mock.calls.length, 0, "asked before")
            // A publish meanwhile, the repeating event gone from it (deleted elsewhere, say).
            const again = week()
            again[0].timed = again[0].timed.filter(p => p.item.id !== "event:r.ics")
            planning.days = again
            wait(20)
            let question = null
            for (let i = 0; i < planning.children.length; i++)
                if (planning.children[i].item && planning.children[i].item.ask)
                    question = planning.children[i].item
            verify(question !== null && question.opened)
            question.answer(true)
            compare(mock.calls.length, 1, JSON.stringify(mock.calls))
            compare([mock.calls[0].key, mock.calls[0].start, mock.calls[0].newStart, mock.calls[0].onlyThis].join(" "), ["r.ics", days[0].start + 840 * 60, days[0].start + 870 * 60, true].join(" "))
        }

        // The plan's step, lunch and the night, each by its own writer.
        function test_agenda_step_meal_and_night() {
            const planning = makeWeek()
            // The step, 09:00, an hour and a half later: pinned there, as long as it is.
            drag(planning, planningPoint(planning, 1, 550), planningPoint(planning, 1, 640))
            compare(mock.calls.length, 1, JSON.stringify(mock.calls))
            compare(mock.calls[0].what, "pinTask")
            compare(mock.calls[0].uid, "letter")
            compare(mock.calls[0].at, "2099-01-06T10:30")
            compare(mock.calls[0].minutes, 30)
            // Lunch by its middle, 35 minutes down: moved from its getting ready, that day.
            mock.calls = []
            drag(planning, planningPoint(planning, 1, 755), planningPoint(planning, 1, 790))
            compare(mock.calls.length, 1, JSON.stringify(mock.calls))
            compare(mock.calls[0].what, "dragNeed")
            compare([mock.calls[0].edit.date, mock.calls[0].edit.key, mock.calls[0].edit.action, mock.calls[0].edit.from].join(" "), "2099-01-06 meal:1 move 12:45")
            // Its end, 13:00 to 14:00: its times.
            mock.calls = []
            drag(planning, planningPoint(planning, 1, 780, -3), planningPoint(planning, 1, 840, -3))
            compare(mock.calls.length, 1, JSON.stringify(mock.calls))
            compare([mock.calls[0].edit.action, mock.calls[0].edit.from, mock.calls[0].edit.to].join(" "), "times 12:10 14:00")
            // A meal never goes to another day: dragged to the next column, it stays on its own.
            mock.calls = []
            drag(planning, planningPoint(planning, 1, 755), planningPoint(planning, 2, 785))
            compare(mock.calls.length, 1, JSON.stringify(mock.calls))
            compare(mock.calls[0].edit.date, "2099-01-06")
            // The night's top, on its evening: bedtime half an hour later, its winding down with it, waking as it was.
            mock.calls = []
            drag(planning, planningPoint(planning, 0, 1350, 3), planningPoint(planning, 0, 1380, 3))
            compare(mock.calls.length, 1, JSON.stringify(mock.calls))
            compare([mock.calls[0].edit.date, mock.calls[0].edit.key, mock.calls[0].edit.action].join(" "), "2099-01-05 sleep times")
            compare([mock.calls[0].edit.from, mock.calls[0].edit.bed, mock.calls[0].edit.to].join(" "), "23:00 23:30 07:00")
            // Its bottom, on the morning: waking half an hour later, the evening as it was.
            mock.calls = []
            drag(planning, planningPoint(planning, 1, 420, -3), planningPoint(planning, 1, 450, -3))
            compare(mock.calls.length, 1, JSON.stringify(mock.calls))
            compare([mock.calls[0].edit.date, mock.calls[0].edit.from, mock.calls[0].edit.bed, mock.calls[0].edit.to].join(" "), "2099-01-05 22:30 23:00 07:30")
        }

        // A publish that changes one thing changes that card in place: no
        // other card is made again, and the view stays where it was.
        function test_agenda_publish_changes_in_place() {
            const planning = makeWeek()
            const repeater = root.find(planning, "planningThings")
            verify(repeater !== null)
            const before = {}
            for (let i = 0; i < repeater.count; i++)
                before[repeater.itemAt(i).key] = repeater.itemAt(i)
            const flick = root.flickIn(planning)
            flick.contentY = planning.yOf(800)
            const scrolled = flick.contentY
            const again = week()
            again[0].timed[0].item.title = "a.ics, moved to the big room"
            planning.days = again
            wait(50)
            compare(repeater.count, Object.keys(before).length)
            for (let i = 0; i < repeater.count; i++) {
                const card = repeater.itemAt(i)
                verify(before[card.key] === card, "made again: " + card.key)
            }
            compare(before["event:a.ics|2099-01-05"].title, "a.ics, moved to the big room")
            compare(flick.contentY, scrolled, "nothing scrolled")
            // Other days shown: opened on their first hour.
            const later = week().map(d => Object.assign(d, { date: d.date.replace("2099-01", "2099-02") }))
            planning.days = later
            wait(50)
            compare(flick.contentY, 0)
        }

        // Each kind's drop sends its own writer's call, and nothing else (Paths.qml).
        function test_paths_call_each_kind_s_writer() {
            const paths = createTemporaryObject(pathsComponent, root)
            const day = v3day("2099-01-06")
            const rowOf = (p, more) => Object.assign({ kind: p.item.kind, drag: p.item.can.drag, owner: p.item.owner || "", srcKey: p.item.source.key || "", srcStart: p.item.source.start || 0, srcDate: p.item.source.date || "", start: p.item.time.start, at: p.item.time.at, until: p.item.time.until, end: p.item.time.end }, more || {})
            const to = { start: day.start + 600 * 60, end: day.start + 660 * 60 }
            compare(paths.drop(rowOf(event("a.ics", day, 540, 600)), "body", to, false), "")
            compare(paths.drop(rowOf(step("letter", day, 540, 600, true)), "body", to, false), "")
            compare(paths.drop(rowOf(meal("meal:1", day, 730, 750, 780, 780)), "body", to, false), "")
            compare(paths.drop(rowOf(event("ro.ics", day, 540, 600, { read_only: true })), "body", to, false), "")
            compare(paths.drop(rowOf(dose("m@1", day, 480)), "body", to, false), "")
            compare(mock.calls.map(c => c.what).join(" "), "moveEvent pinTask dragNeed")
            compare([mock.calls[0].key, mock.calls[0].newStart, mock.calls[0].newEnd].join(" "), ["a.ics", to.start, to.end].join(" "))
            compare([mock.calls[1].uid, mock.calls[1].at, mock.calls[1].minutes].join(" "), "letter 2099-01-06T10:00 60")
            compare([mock.calls[2].edit.date, mock.calls[2].edit.key, mock.calls[2].edit.action, mock.calls[2].edit.from].join(" "), "2099-01-06 meal:1 move 10:00")
        }

        // Today's doses as the Health job read them, under a fake clock: taken
        // stays taken; open turns "come" at its time and "check" when the
        // reading grows old; being read since Sioul came back; another day's,
        // a tick; several at one minute, one mark in the state asking the most
        // care; a 05:00 dose inside the night at the top edge, in each state.
        function test_agenda_dose_marks() {
            const days = week()
            const t = m => days[1].start + m * 60
            days[1].today = true
            days[1].doses = [dose("m@5", days[1], 300, "top"), dose("m@12", days[1], 720), dose("n@12", days[1], 720)]
            days[2].doses = [dose("m@8", days[2], 480)]
            const open = (from, come) => ({ from: from, state: { state: "open", come: come, late: false }, words: come ? "due" : "later" })
            const marks = { date: "2099-01-06", holds: 120, reading: "reading", old: "old", rows: [
                { key: "m@5", day: "2099-01-06", minute: 300, at: t(300), states: [open(0, true)] },
                { key: "m@12", day: "2099-01-06", minute: 720, at: t(720), states: [open(0, false), open(t(720), true)] },
                { key: "n@12", day: "2099-01-06", minute: 720, at: t(720), states: [{ from: 0, state: { state: "taken", at: t(700) }, words: "taken" }] }
            ] }
            const planning = makeWeek({ days: days, today: "2099-01-06", now: t(600), marks: null, readAt: 0 })
            const state = (key, date) => planning.doseState(key, date || "2099-01-06", planning.now).state
            // Nothing read yet: being read, never "not taken".
            compare(state("m@12"), "reading")
            planning.marks = marks
            planning.readAt = t(600) - 10
            compare(state("m@12"), "not-come")
            compare(state("n@12"), "taken")
            compare(state("m@5"), "come")
            compare(state("m@8", "2099-01-07"), "ahead")
            // Its time come, read a moment ago.
            planning.now = t(725)
            planning.readAt = t(724)
            compare(state("m@12"), "come")
            // The reading grown old: check.
            planning.now = t(724) + 121
            compare(state("m@12"), "check")
            compare(state("m@5"), "check")
            // Back on the screen after the reading: being read; taken stays taken.
            planning.now = t(726)
            planning.backAt = t(725)
            compare(state("m@12"), "reading")
            compare(state("n@12"), "taken")
            // A reading of another day is no reading, its "taken" included.
            planning.backAt = 0
            planning.marks = Object.assign({}, marks, { rows: marks.rows.map(r => Object.assign({}, r, { day: "2099-01-05" })) })
            compare(state("n@12"), "reading")
            planning.marks = marks
            // One mark at 12:00 for two doses, in the state asking the most care; the 05:00 one at the top edge.
            const drawn = root.all(planning, c => c.keys !== undefined && c.minute !== undefined && c.known !== undefined)
            const at12 = drawn.filter(c => c.minute === 720 && c.date === "2099-01-06")
            compare(at12.length, 1)
            compare(JSON.parse(at12[0].keys).sort().join(" "), "m@12 n@12")
            compare(at12[0].known.state, "come")
            const at5 = drawn.filter(c => c.minute === 300)
            compare(at5.length, 1)
            compare(at5[0].edge, "top")
            compare(at5[0].y, 0)
            for (const when of [{ now: t(600), readAt: t(599), backAt: 0, is: "come" }, { now: t(800), readAt: t(600), backAt: 0, is: "check" }, { now: t(800), readAt: t(790), backAt: t(795), is: "reading" }]) {
                planning.now = when.now
                planning.readAt = when.readAt
                planning.backAt = when.backAt
                compare(at5[0].known.state, when.is)
                compare(at5[0].y, 0)
            }
            const ahead = drawn.filter(c => c.date === "2099-01-07")
            compare(ahead.length, 1)
            compare(ahead[0].known.state, "ahead")
        }

        // Show leaves out what it is told, on this device: the plan's steps,
        // meals and the night, doses, a calendar.
        function test_agenda_show_leaves_out() {
            const days = week()
            days[1].doses = [dose("m@8", days[1], 480)]
            const planning = makeWeek({ days: days })
            const repeater = root.find(planning, "planningThings")
            const kinds = () => {
                const out = []
                for (let i = 0; i < repeater.count; i++)
                    out.push(repeater.itemAt(i).kind)
                return out.sort().join(" ")
            }
            const marks = () => root.all(planning, c => c.keys !== undefined && c.minute !== undefined && c.known !== undefined).length
            compare(kinds(), "event event event event meal night night step")
            compare(marks(), 1)
            planning.hide = { steps: true }
            compare(kinds(), "event event event event meal night night")
            planning.hide = { needs: true, doses: true }
            compare(kinds(), "event event event event step")
            compare(marks(), 0)
            planning.hide = { calendars: ["c"] }
            compare(kinds(), "meal night night step")
        }

        // On a phone the Week is a list of days, in time order: a day's title
        // opens it, a thing opens as on the timeline; today's doses on one line.
        function test_agenda_week_as_a_list() {
            const days = week().concat(["2099-01-08", "2099-01-09", "2099-01-10", "2099-01-11"].map(v3day))
            days[1].today = true
            days[1].doses = [dose("m@8", days[1], 480), dose("m@20", days[1], 1200)]
            days[2].plan = "First: letter"
            const planning = makeWeek({ width: 412, days: days, today: "2099-01-06" })
            verify(planning.listed)
            compare(root.find(planning, "planningThings").count, 0, "the hours are not made")
            const list = root.all(planning, c => c.positionViewAtIndex !== undefined && c.visible)[0]
            verify(list !== undefined)
            const keys = []
            for (let i = 0; i < list.count; i++)
                keys.push(list.model.get(i).key)
            compare(keys.filter(k => k.startsWith("day-title|")).length, 7)
            const tuesday = keys.indexOf("day-title|2099-01-06")
            compare(keys[tuesday + 1], "doses-line|2099-01-06")
            compare(keys.slice(tuesday + 2, tuesday + 5).join(" "), "need:2099-01-05/sleep|2099-01-06 step:letter@2099-01-06|2099-01-06 need:2099-01-06/meal:1|2099-01-06")
            verify(keys.indexOf("plan-line|2099-01-07") > keys.indexOf("day-title|2099-01-07"))
            verify(keys.indexOf("free|2099-01-07") > 0, "a free day says so")
            let day = "", opened = ""
            planning.openDay.connect(d => day = d)
            planning.open.connect(row => opened = row.kind + " " + row.owner)
            list.positionViewAtIndex(tuesday, ListView.Beginning)
            wait(50)
            const title = root.all(list, c => c.key === "day-title|2099-01-06")[0]
            mouseClick(title, 20, title.height / 2)
            compare(day, "2099-01-06")
            const step = root.all(list, c => c.key === "step:letter@2099-01-06|2099-01-06")[0]
            mouseClick(step, step.width / 2, step.height / 2)
            compare(opened, "step letter")
        }

        // A phone's Day: a swipe across goes to the next day or the one before; up and down scrolls.
        function test_agenda_phone_day_swipes() {
            const days = [week()[1]]
            const planning = makeWeek({ width: 412, height: 800, days: days })
            verify(planning.single && planning.narrow)
            let turned = 0
            planning.turn.connect(steps => turned += steps)
            const swipe = (dx, dy) => {
                const at = Qt.point(250, 400)
                touchEvent(planning).press(0, planning, at.x, at.y).commit()
                for (let i = 1; i <= 8; i++)
                    touchEvent(planning).move(0, planning, at.x + dx * i / 8, at.y + dy * i / 8).commit()
                touchEvent(planning).release(0, planning, at.x + dx, at.y + dy).commit()
                wait(50)
            }
            swipe(-200, 0)
            compare(turned, 1)
            swipe(200, 10)
            compare(turned, 0)
            swipe(10, -200)
            compare(turned, 0, "a scroll turns no day")
            compare(mock.calls.length, 0)
        }

        function test_day_step_given_a_time() {
            const today = new Date()
            today.setHours(0, 0, 0, 0)
            const midnight = today.getTime() / 1000
            // A local time of today by its minutes on the clock: on the day the clocks change (25 October), not midnight plus as many minutes.
            const at = m => new Date(today.getFullYear(), today.getMonth(), today.getDate(), 0, m).getTime() / 1000
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
