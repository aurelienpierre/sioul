// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Meals, naps, the night and the doses down the hours: one column for a day,
// seven for its week (docs/health.md, "The page"). Each column has three
// lanes: the day's events and planned steps, faded, for context only; the
// times kept free, getting ready, winding down and coming back lighter than
// the time itself; the doses, a dot each (hollow when due, filled when
// taken, warm when another device may know: never "not taken"). The hours
// are the same every day, so that nothing moves from one day to the next. A
// quiet line marks now. A tap on a block chooses it (its row in the list
// lights up, and the other way round) and offers what can change it.
//
// A block is moved by hand, on its own day (docs/health.md, "Moved by hand"):
// dragged whole, it keeps its length; by its bottom edge, a meal or a nap
// changes its length; the night's top edge is bedtime, its bottom edge waking
// (in the next morning's column). What it would become is drawn where it
// would go, with its times; let go, it is that day's change, as "Move to…"
// makes it, and "Undo" waits in the status line. A day past, or a block
// over, stays as it was. With a mouse, press and drag; with a finger, a long
// press picks it up (a plain swipe scrolls). A timeline too thin to take with
// a finger (a phone's) asks to be opened readable at the long press
// (`readableAsked`), and the block under the finger, if there is no doubt
// which, comes with it. One set of handlers for the whole timeline
// (TimeDrag.qml): the blocks carry none.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: timeline

    required property var sioul
    required property var theme
    // The page's days (`health::DayView`): one, or the seven of a week.
    property var days: []
    // The hours shown, in minutes from midnight: the same for each day.
    property int fromMinute: 7 * 60
    property int toMinute: 22 * 60
    // A block lit by the pointer or chosen, as "date|key": its row lights up too.
    property string lit: ""
    property string chosen: ""
    // The days' titles above their columns (the week).
    property bool titles: false
    // Opened readable, a block a finger can take: the page says (a phone's long press).
    property bool readable: false
    readonly property int readableHour: 56
    readonly property int gutter: 46
    readonly property int hours: Math.max(1, Math.round((timeline.toMinute - timeline.fromMinute) / 60))
    // As tall as the room allows, the whole day in view; never thinner than a line of small text.
    readonly property real hourHeight: timeline.readable ? timeline.readableHour : Math.max(14, (flick.height - timeline.topRoom - 6) / timeline.hours)
    readonly property real columnWidth: Math.max(30, (timeline.width - timeline.gutter - 4) / Math.max(1, timeline.days.length))
    // A wide column: names in the blocks, the doses' names beside their dots.
    readonly property bool wide: timeline.columnWidth >= 160
    // Now on the clock (Unix seconds) and today ("2026-10-06"): the window's
    // clock (main.qml), moved at each minute's turn and when the app comes
    // back. Only the line follows it, and today's marks when the day turns.
    property real now: Date.now() / 1000
    property string today: {
        const d = new Date()
        const pad = n => n < 10 ? "0" + n : String(n)
        return d.getFullYear() + "-" + pad(d.getMonth() + 1) + "-" + pad(d.getDate())
    }
    // Now in minutes from midnight, for its line.
    readonly property int minuteNow: {
        const d = new Date(timeline.now * 1000)
        return d.getHours() * 60 + d.getMinutes()
    }
    // Today's column among the days shown; -1 when today is not shown.
    readonly property int todayIndex: timeline.days.findIndex(d => d.date === timeline.today)
    // Too thin to take a block with a finger: a long press asks for the readable scale.
    readonly property bool thin: !timeline.readable && timeline.hourHeight < 36
    // An edge's grab zone, in pixels: at least 8, more for a finger.
    readonly property real edgeZone: timeline.readable ? 14 : 8

    // The pointer over a block, or off it ("").
    signal hover(string id)
    // A block or a dose tapped (`segment` as the view gives it), where.
    signal tapped(var segment, point at)
    // A block's menu asked for: right click, or a long press let go where it was.
    signal menu(var segment, point at)
    // A day's title tapped (the week): that day, shown.
    signal openDay(string date)
    // A long press on a timeline too thin to drag in: open it readable, at that minute.
    signal readableAsked
    // The readable scale closed ("Done").
    signal readableDone

    // Room above the first hour, for its label.
    readonly property int topRoom: 8

    function yOf(minute) {
        const clamped = Math.min(Math.max(minute, timeline.fromMinute), timeline.toMinute)
        return timeline.topRoom + (clamped - timeline.fromMinute) / 60 * timeline.hourHeight
    }

    // The minute at `y` in the timeline's content (not clamped).
    function minuteAt(y) {
        return timeline.fromMinute + (y - timeline.topRoom) / timeline.hourHeight * 60
    }

    // Each kind its colour, calm and apart (the theme's chart colours, no red).
    function kindColor(kind) {
        const colors = timeline.theme.chartColors
        return kind === "meal" ? colors[1] : kind === "nap" ? colors[2] : kind === "sleep" ? colors[4] : timeline.theme.muted
    }

    function tint(color, alpha) {
        return Qt.rgba(color.r, color.g, color.b, alpha)
    }

    // "12:45" for minutes from a column's midnight (the day before's, or the next's, on the clock).
    function clock(minute) {
        const m = ((Math.round(minute) % 1440) + 1440) % 1440
        const pad = n => n < 10 ? "0" + n : String(n)
        return pad(Math.floor(m / 60)) + ":" + pad(m % 60)
    }

    // A column's lanes, as its delegate lays them: context, kept times, doses.
    function lanes(index) {
        const width = timeline.columnWidth
        const contextWidth = Math.round(width * (timeline.days.length > 1 ? 0.22 : 0.3))
        const dosesWidth = width < 60 ? 0 : Math.round(width * (timeline.wide ? 0.22 : 0.16))
        const keptX = contextWidth + 3
        const keptWidth = width - keptX - dosesWidth - 4
        return { x: timeline.gutter + index * width, contextWidth: contextWidth, dosesWidth: dosesWidth, keptX: keptX, keptWidth: keptWidth, doseX: dosesWidth > 0 ? keptX + keptWidth + 4 : keptX + keptWidth - 10 }
    }

    function shown(segment) {
        return segment.to_minute > timeline.fromMinute && segment.from_minute < timeline.toMinute
    }

    // A block's top and bottom in the content, as drawn.
    function topOf(segment) {
        return timeline.yOf(segment.from_minute) + 1
    }

    function bottomOf(segment) {
        return timeline.topOf(segment) + Math.max(4, timeline.yOf(segment.to_minute) - timeline.yOf(segment.from_minute) - 2)
    }

    // What a hand can change: a meal, a nap or a night not over, on a day not past.
    function editable(segment) {
        return segment.kind !== "dose" && !segment.past
    }

    // Its ends this column holds: a meal's or a nap's bottom (its length); the
    // night's top (bedtime) where it starts, its bottom (waking) where it ends.
    function hasTop(segment) {
        return segment.kind === "sleep" && segment.start_minute >= 0 && segment.start_minute === segment.from_minute
    }

    function hasBottom(segment) {
        return segment.end_minute <= 1440 && segment.end_minute === segment.to_minute
    }

    // What lies at `p` (the content's coordinates): {segment, part, column},
    // part "body", "top", "bottom" (an edge's grab zone) or "dose"; else null.
    function thingAt(p) {
        const index = Math.floor((p.x - timeline.gutter) / timeline.columnWidth)
        if (index < 0 || index >= timeline.days.length || p.x < timeline.gutter)
            return null
        const day = timeline.days[index]
        const lane = timeline.lanes(index)
        const x = p.x - lane.x
        for (const segment of day.segments) {
            if (segment.kind !== "dose" || segment.from_minute < timeline.fromMinute || segment.from_minute > timeline.toMinute)
                continue
            if (x >= lane.doseX - 3 && x <= lane.doseX + Math.max(14, lane.dosesWidth) + 3 && Math.abs(p.y - timeline.yOf(segment.from_minute)) <= 9)
                return { segment: segment, part: "dose", column: index }
        }
        if (x < lane.keptX - 2 || x > lane.keptX + lane.keptWidth + 2)
            return null
        let found = null
        for (const segment of day.segments) {
            if (segment.kind === "dose" || !timeline.shown(segment))
                continue
            const top = timeline.topOf(segment), bottom = timeline.bottomOf(segment)
            const canTop = timeline.editable(segment) && timeline.hasTop(segment)
            const canBottom = timeline.editable(segment) && timeline.hasBottom(segment)
            const zone = timeline.edgeZone
            if (p.y < top - (canTop ? zone / 2 : 0) || p.y > bottom + (canBottom ? zone / 2 : 0))
                continue
            // Inside, an edge takes a third of a short block at most (4 px at least: 8 with the outside); the rest is the block.
            const inside = Math.max(4, Math.min(zone, (bottom - top) / 3))
            const part = canBottom && p.y >= bottom - inside ? "bottom" : canTop && p.y <= top + inside ? "top" : "body"
            // Two touching: the one the point is inside, the later drawn on top.
            if (found === null || (p.y >= top && p.y <= bottom))
                found = { segment: segment, part: part, column: index }
        }
        return found
    }

    // Under a finger on a thin timeline: the block it is on, else the only one
    // within 20 px; when two lie as close (a meal right after the night's end),
    // none: the readable scale opens, and the next long press chooses.
    function pickNear(p) {
        const index = Math.floor((p.x - timeline.gutter) / timeline.columnWidth)
        if (index < 0 || index >= timeline.days.length || p.x < timeline.gutter)
            return null
        const near = []
        for (const segment of timeline.days[index].segments) {
            if (segment.kind === "dose" || !timeline.shown(segment))
                continue
            const top = timeline.topOf(segment), bottom = timeline.bottomOf(segment)
            if (p.y >= top && p.y <= bottom)
                return { segment: segment, part: "body", column: index }
            if (Math.min(Math.abs(p.y - top), Math.abs(p.y - bottom)) <= 20)
                near.push(segment)
        }
        return near.length === 1 ? { segment: near[0], part: "body", column: index } : null
    }

    // A block taken by the hand: {segment, part, column, pressMinute}, and how far it went, in minutes.
    property var grab: null
    property real shift: 0
    // Let go: what it became stays drawn until the page has it (or a moment passes).
    property var landed: null

    function take(thing, p) {
        timeline.grab = { segment: thing.segment, part: thing.part, column: thing.column, pressMinute: timeline.minuteAt(p.y) }
        timeline.shift = 0
        timeline.landed = null
    }

    // Where the block taken would go, `shift` minutes on, by whole five minutes,
    // within the hours shown and the day's rules: {start, end} in its column's
    // minutes, not cut at midnight.
    function span(grab, shift) {
        const s = grab.segment
        const snap = m => Math.round(m / 5) * 5
        const clamp = (v, lo, hi) => Math.max(lo, Math.min(hi, v))
        const lo = timeline.fromMinute, hi = timeline.toMinute
        if (shift === 0)
            return { start: s.start_minute, end: s.end_minute }
        // A night stays on its evening (`needs`): winding down from noon on its own
        // day, bedtime before noon the next morning; in this column's minutes.
        const evening = s.date === timeline.days[grab.column].date ? 0 : -1440
        const lead = s.at_minute - s.start_minute
        if (grab.part === "bottom") {
            // Five minutes at least, and a nap's time to come back; waking after bedtime, within a day.
            const least = s.kind === "sleep" ? s.at_minute + 5 : s.at_minute + 5 + (s.end_minute - s.until_minute)
            const most = s.kind === "sleep" ? Math.min(hi, s.at_minute + 1435) : hi
            return { start: s.start_minute, end: clamp(snap(s.end_minute + shift), Math.max(lo, least), most) }
        }
        if (grab.part === "top") {
            // Bedtime, its winding down before it as long as it was.
            const start = clamp(snap(s.start_minute + shift), Math.max(lo, evening + 720), Math.min(hi - 15, evening + 2159 - lead, s.end_minute - lead - 5))
            return { start: start, end: s.end_minute }
        }
        const length = s.end_minute - s.start_minute
        // A night: some of it stays in view; a meal or a nap: all of it, on its day.
        const start = s.kind === "sleep" ? clamp(snap(s.start_minute + shift), Math.max(lo + 15 - length, evening + 720), Math.min(hi - 15, evening + 2159 - lead)) : clamp(snap(s.start_minute + shift), lo, hi - length)
        return { start: start, end: start + length }
    }

    // What the ghost says under its times: a night's bedtime, when a meal is eaten.
    function detailOf(segment, to) {
        const at = segment.at_minute + (to.start - segment.start_minute)
        if (segment.kind === "sleep")
            return timeline.sioul.textWith("need-night-detail", "bed", timeline.clock(at))
        if (segment.kind === "meal" && segment.at_minute > segment.start_minute)
            return timeline.sioul.textWith("need-meal-detail", "at", timeline.clock(at))
        return ""
    }

    // What the day is asked (`health::drag_need`), for the block taken gone to `to`.
    function edit(grab, to) {
        const s = grab.segment
        const asked = { date: s.date, key: s.key }
        if (grab.part === "body")
            return Object.assign(asked, { action: "move", from: timeline.clock(to.start) })
        if (s.kind === "sleep") {
            const bed = s.at_minute + (to.start - s.start_minute)
            return Object.assign(asked, { action: "times", from: timeline.clock(to.start), bed: timeline.clock(bed), to: timeline.clock(to.end) })
        }
        return Object.assign(asked, { action: "times", from: timeline.clock(to.start), to: timeline.clock(to.end) })
    }

    function drop() {
        const grab = timeline.grab
        timeline.grab = null
        if (!grab)
            return
        const to = timeline.span(grab, timeline.shift)
        // Let go where it was: nothing changes, nothing to undo.
        if (to.start === grab.segment.start_minute && to.end === grab.segment.end_minute)
            return
        timeline.landed = { segment: grab.segment, column: grab.column, start: to.start, end: to.end }
        landedLate.restart()
        const problem = timeline.sioul.dragNeed(JSON.stringify(timeline.edit(grab, to)))
        if (problem !== "") {
            timeline.landed = null
            timeline.sioul.status = problem
        }
    }

    // The hour now in view when the day does not fit (a short window).
    function showNow() {
        if (flick.contentHeight <= flick.height || timeline.readable)
            return
        const minute = timeline.todayIndex >= 0 ? Math.max(timeline.fromMinute, timeline.minuteNow - 60) : timeline.fromMinute
        flick.contentY = Math.max(0, Math.min(flick.contentHeight - flick.height, timeline.yOf(minute)))
    }

    // Opened readable at a long press: the minute under the finger stays under it.
    property var anchor: null

    function keepAnchor() {
        if (!timeline.anchor)
            return
        const y = flick.mapFromItem(null, timeline.anchor.x, timeline.anchor.y).y
        flick.contentY = Math.max(0, Math.min(flick.contentHeight + flick.bottomMargin - flick.height, timeline.yOf(timeline.anchor.minute) - y))
    }

    spacing: 4
    onVisibleChanged: if (visible) Qt.callLater(timeline.showNow)
    onReadableChanged: {
        if (timeline.readable) {
            timeline.keepAnchor()
            Qt.callLater(timeline.keepAnchor)
        } else {
            timeline.anchor = null
            Qt.callLater(timeline.showNow)
        }
    }
    // The page has the change: what was drawn where it went is the block itself now.
    onDaysChanged: timeline.landed = null

    Timer {
        id: landedLate

        interval: 3000
        onTriggered: timeline.landed = null
    }

    // The week: each day's title, a tap away from that day.
    Row {
        visible: timeline.titles
        Layout.fillWidth: true
        leftPadding: timeline.gutter

        Repeater {
            model: timeline.titles ? timeline.days : []

            delegate: Label {
                id: title

                required property var modelData

                width: timeline.columnWidth
                horizontalAlignment: Text.AlignHCenter
                // Narrow days (a phone's week): the weekday above its number.
                text: timeline.columnWidth < 64 ? title.modelData.weekday + "\n" + title.modelData.number : title.modelData.weekday + " " + title.modelData.number
                textFormat: Text.PlainText
                font.weight: title.modelData.date === timeline.today ? Font.Bold : Font.DemiBold
                color: title.modelData.date === timeline.today ? timeline.theme.accent : title.modelData.date < timeline.today ? timeline.theme.muted : timeline.theme.text
                Accessible.role: Accessible.Button
                Accessible.name: title.modelData.title

                TapHandler {
                    cursorShape: Qt.PointingHandCursor
                    onTapped: timeline.openDay(title.modelData.date)
                }
            }
        }
    }

    Item {
        Layout.fillWidth: true
        Layout.fillHeight: true

        Flickable {
            id: flick

            anchors.fill: parent
            clip: true
            contentWidth: width
            contentHeight: timeline.topRoom + timeline.hours * timeline.hourHeight + 6
            // Readable: the last hours scroll up above the bar.
            bottomMargin: readableBar.active ? readableBar.height : 0
            boundsBehavior: Flickable.StopAtBounds
            // A mouse drags blocks, not the view: the wheel scrolls it.
            acceptedButtons: Qt.NoButton
            ScrollBar.vertical: ScrollBar {}
            onHeightChanged: Qt.callLater(timeline.showNow)

            Item {
                id: canvas

                width: flick.width
                height: flick.contentHeight

                // The hours, faint.
                Repeater {
                    model: timeline.hours + 1

                    delegate: Item {
                        id: hour

                        required property int index
                        readonly property int clock: timeline.fromMinute / 60 + hour.index

                        y: timeline.topRoom + hour.index * timeline.hourHeight
                        width: parent.width
                        height: 1

                        Label {
                            // Every hour while they are tall enough, else every other one.
                            visible: timeline.hourHeight >= 22 || hour.clock % 2 === 0
                            y: -height / 2
                            width: timeline.gutter - 8
                            horizontalAlignment: Text.AlignRight
                            text: (hour.clock % 24 < 10 ? "0" : "") + hour.clock % 24 + ":00"
                            textFormat: Text.PlainText
                            font.pixelSize: 11
                            color: timeline.theme.muted
                        }
                        Rectangle {
                            x: timeline.gutter
                            width: parent.width - timeline.gutter
                            height: 1
                            color: timeline.theme.line
                        }
                    }
                }

                Repeater {
                    model: timeline.days

                    delegate: Item {
                        id: column

                        required property var modelData
                        required property int index
                        // The lanes: context, kept times, doses; a phone's week has no room for names.
                        readonly property var lane: timeline.lanes(column.index)

                        x: column.lane.x
                        width: timeline.columnWidth
                        height: parent.height

                        // Today: a faint surface; between days, a line.
                        Rectangle {
                            anchors.fill: parent
                            visible: column.index === timeline.todayIndex && timeline.days.length > 1
                            color: timeline.theme.surface
                        }
                        Rectangle {
                            visible: column.index > 0
                            width: 1
                            height: parent.height
                            color: timeline.theme.line
                        }

                        // The day's events and planned steps: faded, for context only.
                        Repeater {
                            model: column.modelData.context

                            delegate: Rectangle {
                                id: around

                                required property var modelData

                                visible: around.modelData.to_minute > timeline.fromMinute && around.modelData.from_minute < timeline.toMinute
                                x: 2
                                y: timeline.yOf(around.modelData.from_minute) + 1
                                width: column.lane.contextWidth - 2
                                height: Math.max(3, timeline.yOf(around.modelData.to_minute) - timeline.yOf(around.modelData.from_minute) - 2)
                                radius: 3
                                color: timeline.tint(timeline.theme.muted, around.modelData.kind === "event" ? 0.14 : 0.08)
                                border.color: timeline.tint(timeline.theme.muted, 0.25)
                                clip: true

                                Label {
                                    visible: parent.height >= 15 && parent.width >= 48
                                    anchors.fill: parent
                                    anchors.leftMargin: 4
                                    anchors.rightMargin: 2
                                    text: around.modelData.title
                                    textFormat: Text.PlainText
                                    elide: Text.ElideRight
                                    font.pixelSize: 11
                                    color: timeline.theme.muted
                                }
                            }
                        }

                        // Meals, naps, the night: the time kept free, lighter before and after it proper.
                        Repeater {
                            model: column.modelData.segments.filter(s => s.kind !== "dose")

                            delegate: Rectangle {
                                id: block

                                required property var modelData
                                readonly property string ident: block.modelData.date + "|" + block.modelData.key
                                readonly property bool lit: timeline.lit === block.ident || timeline.chosen === block.ident
                                readonly property color hue: timeline.kindColor(block.modelData.kind)
                                // Quiet that day: kept all the same, said lighter.
                                readonly property real strength: block.modelData.quiet ? 0.5 : 1
                                // Taken by the hand: outlined; moved, or let go and not yet back from the page: shown where it goes.
                                readonly property bool taken: timeline.grab !== null && timeline.grab.segment.date === block.modelData.date && timeline.grab.segment.key === block.modelData.key
                                readonly property bool away: (block.taken && timeline.shift !== 0) || (timeline.landed !== null && timeline.landed.segment.date === block.modelData.date && timeline.landed.segment.key === block.modelData.key)
                                // Its edges a hand can take: shown as grips when readable, or under the pointer.
                                readonly property bool grips: (timeline.readable || block.lit) && timeline.editable(block.modelData)

                                visible: timeline.shown(block.modelData)
                                x: column.lane.keptX
                                y: timeline.topOf(block.modelData)
                                width: column.lane.keptWidth
                                height: timeline.bottomOf(block.modelData) - block.y
                                radius: 3
                                color: timeline.tint(block.hue, 0.1 * block.strength)
                                border.color: block.lit || block.taken ? timeline.theme.accent : timeline.tint(block.hue, 0.45 * block.strength)
                                border.width: block.lit || block.taken ? 2 : 1
                                opacity: block.away ? 0.3 : block.modelData.past ? 0.55 : 1
                                clip: true
                                Accessible.role: Accessible.Button
                                Accessible.name: block.modelData.name

                                // The time itself: eating, the nap, the night.
                                Rectangle {
                                    readonly property real inset: timeline.yOf(Math.max(block.modelData.at_minute, block.modelData.from_minute)) - timeline.yOf(block.modelData.from_minute)

                                    x: 1
                                    y: inset
                                    width: parent.width - 2
                                    height: Math.max(0, Math.min(parent.height - inset, timeline.yOf(Math.min(block.modelData.until_minute, block.modelData.to_minute)) - timeline.yOf(Math.max(block.modelData.at_minute, block.modelData.from_minute))))
                                    color: timeline.tint(block.hue, 0.28 * block.strength)
                                }
                                Rectangle {
                                    width: 3
                                    height: parent.height
                                    radius: 1
                                    color: timeline.tint(block.hue, 0.9 * block.strength)
                                }
                                Label {
                                    visible: block.height >= 14 && block.width >= 34
                                    x: 6
                                    width: block.width - 8
                                    height: Math.min(block.height, implicitHeight)
                                    text: block.modelData.name
                                    textFormat: Text.PlainText
                                    elide: Text.ElideRight
                                    font.pixelSize: timeline.wide ? 12 : 11
                                    color: timeline.theme.text
                                }
                                // Grips: where an end can be taken.
                                Rectangle {
                                    visible: block.grips && timeline.hasTop(block.modelData) && block.height >= 16
                                    anchors.horizontalCenter: parent.horizontalCenter
                                    y: 2
                                    width: Math.min(28, parent.width / 3)
                                    height: 3
                                    radius: 1.5
                                    color: timeline.tint(block.hue, 0.9)
                                }
                                Rectangle {
                                    visible: block.grips && timeline.hasBottom(block.modelData) && block.height >= 16
                                    anchors.horizontalCenter: parent.horizontalCenter
                                    y: parent.height - 5
                                    width: Math.min(28, parent.width / 3)
                                    height: 3
                                    radius: 1.5
                                    color: timeline.tint(block.hue, 0.9)
                                }
                            }
                        }

                        // The doses: a dot each, its name beside it when there is room.
                        Repeater {
                            model: column.modelData.segments.filter(s => s.kind === "dose")

                            delegate: Item {
                                id: dose

                                required property var modelData
                                readonly property string ident: dose.modelData.date + "|" + dose.modelData.key
                                readonly property bool lit: timeline.lit === dose.ident || timeline.chosen === dose.ident

                                visible: dose.modelData.from_minute >= timeline.fromMinute && dose.modelData.from_minute <= timeline.toMinute
                                x: column.lane.doseX
                                y: timeline.yOf(dose.modelData.from_minute) - 7
                                width: Math.max(14, column.lane.dosesWidth)
                                height: 14
                                opacity: dose.modelData.past && dose.modelData.state === "" ? 0.55 : 1
                                Accessible.role: Accessible.Button
                                Accessible.name: dose.modelData.name

                                Rectangle {
                                    anchors.verticalCenter: parent.verticalCenter
                                    width: dose.lit ? 12 : 10
                                    height: width
                                    radius: width / 2
                                    color: dose.modelData.state === "taken" ? timeline.theme.accent : timeline.theme.background
                                    border.width: dose.modelData.state === "check" || dose.lit ? 2 : 1.5
                                    border.color: dose.modelData.state === "check" ? timeline.theme.warm : dose.modelData.state === "" ? timeline.theme.muted : timeline.theme.accent
                                }
                                Label {
                                    visible: timeline.wide && column.lane.dosesWidth > 40
                                    x: 15
                                    width: parent.width - 15
                                    anchors.verticalCenter: parent.verticalCenter
                                    text: dose.modelData.name
                                    textFormat: Text.PlainText
                                    elide: Text.ElideRight
                                    font.pixelSize: 11
                                    color: timeline.theme.muted
                                }
                            }
                        }
                    }
                }

                // Now: a quiet line on today, one for the timeline; the clock
                // moves it alone, nothing else is laid again.
                Rectangle {
                    objectName: "nowLine"
                    visible: timeline.todayIndex >= 0 && timeline.minuteNow >= timeline.fromMinute && timeline.minuteNow <= timeline.toMinute
                    x: timeline.gutter + Math.max(0, timeline.todayIndex) * timeline.columnWidth
                    y: timeline.yOf(timeline.minuteNow) - 1
                    width: timeline.columnWidth
                    height: 2
                    color: timeline.theme.accent
                }

                // Where the block taken goes, and a tag with its times ("12:45–13:15"),
                // above it, out of the finger's way (under it at the top of the view).
                // The night says its whole span, its morning in the next column.
                Rectangle {
                    id: ghost

                    readonly property var shownAt: timeline.grab !== null && drag.dragging ? { segment: timeline.grab.segment, column: timeline.grab.column, to: timeline.span(timeline.grab, timeline.shift) } : timeline.landed !== null ? { segment: timeline.landed.segment, column: timeline.landed.column, to: { start: timeline.landed.start, end: timeline.landed.end } } : null
                    readonly property var lane: ghost.shownAt ? timeline.lanes(ghost.shownAt.column) : null
                    readonly property color hue: ghost.shownAt ? timeline.kindColor(ghost.shownAt.segment.kind) : "transparent"

                    visible: ghost.shownAt !== null
                    x: ghost.lane ? ghost.lane.x + ghost.lane.keptX - 2 : 0
                    y: ghost.shownAt ? timeline.yOf(Math.max(0, ghost.shownAt.to.start)) : 0
                    width: ghost.lane ? ghost.lane.keptWidth + 4 : 0
                    height: ghost.shownAt ? Math.max(6, timeline.yOf(Math.min(1440, ghost.shownAt.to.end)) - ghost.y) : 0
                    radius: 4
                    color: timeline.tint(ghost.hue, 0.22)
                    border.color: timeline.theme.accent
                    border.width: 2
                    z: 2
                }
                Rectangle {
                    id: tag

                    readonly property bool above: ghost.y - tag.height - 4 >= flick.contentY + 2

                    visible: ghost.visible
                    x: Math.max(timeline.gutter, Math.min(canvas.width - tag.width - 2, ghost.x))
                    y: tag.above ? ghost.y - tag.height - 4 : ghost.y + ghost.height + 4
                    width: Math.max(ghost.width, tagText.implicitWidth + 12)
                    height: tagText.implicitHeight + 8
                    radius: 4
                    color: timeline.theme.surface
                    border.color: timeline.theme.accent
                    z: 3

                    Column {
                        id: tagText

                        x: 6
                        y: 4
                        spacing: 0

                        Label {
                            text: ghost.shownAt ? timeline.clock(ghost.shownAt.to.start) + "–" + timeline.clock(ghost.shownAt.to.end) : ""
                            textFormat: Text.PlainText
                            font.features: { "tnum": 1 }
                            font.pixelSize: 13
                            font.weight: Font.DemiBold
                            color: timeline.theme.text
                        }
                        Label {
                            readonly property string said: ghost.shownAt ? timeline.detailOf(ghost.shownAt.segment, ghost.shownAt.to) : ""

                            visible: said !== ""
                            text: said
                            textFormat: Text.PlainText
                            font.pixelSize: 11
                            color: timeline.theme.muted
                        }
                    }
                }

                // A dose's name, under the pointer.
                Item {
                    id: tip

                    property var dose: null

                    z: 3
                    width: 14
                    height: 14
                    ToolTip.visible: tip.dose !== null
                    ToolTip.text: tip.dose ? tip.dose.name : ""
                    ToolTip.delay: 400
                }

                TimeDrag {
                    id: drag

                    anchors.fill: parent
                    z: 4
                    flick: flick
                    onHovered: position => {
                        const thing = position.x < 0 ? null : timeline.thingAt(position)
                        timeline.hover(thing ? thing.segment.date + "|" + thing.segment.key : "")
                        drag.cursorShape = !thing ? Qt.ArrowCursor : thing.part === "top" || thing.part === "bottom" ? Qt.SizeVerCursor : thing.part === "body" && timeline.editable(thing.segment) ? Qt.OpenHandCursor : Qt.PointingHandCursor
                        if (thing && thing.part === "dose") {
                            const lane = timeline.lanes(thing.column)
                            tip.x = lane.x + lane.doseX
                            tip.y = timeline.yOf(thing.segment.from_minute) - 7
                        }
                        tip.dose = thing && thing.part === "dose" ? thing.segment : null
                    }
                    onTapped: (position, scenePosition) => {
                        const thing = timeline.thingAt(position)
                        if (thing)
                            timeline.tapped(thing.segment, scenePosition)
                    }
                    onMenuAsked: (position, scenePosition) => {
                        timeline.grab = null
                        const thing = timeline.thingAt(position)
                        if (thing && thing.part !== "dose")
                            timeline.menu(thing.segment, scenePosition)
                    }
                    onHeld: (position, scenePosition) => {
                        // Too thin for a finger: opened readable, the minute under the finger kept there.
                        if (timeline.thin) {
                            const near = timeline.pickNear(position)
                            timeline.anchor = { minute: timeline.minuteAt(position.y), x: scenePosition.x, y: scenePosition.y }
                            if (near && timeline.editable(near.segment)) {
                                timeline.take(near, position)
                                drag.pickUp(false)
                            }
                            timeline.readableAsked()
                            return
                        }
                        const thing = timeline.thingAt(position)
                        if (!thing || thing.part === "dose")
                            return
                        if (timeline.editable(thing.segment)) {
                            timeline.take(thing, position)
                            drag.pickUp(true)
                        } else {
                            timeline.menu(thing.segment, scenePosition)
                        }
                    }
                    onStarted: position => {
                        // A finger took it when held; a mouse takes what it pressed on.
                        if (drag.holding)
                            return
                        const thing = timeline.thingAt(position)
                        if (thing && thing.part !== "dose" && timeline.editable(thing.segment))
                            timeline.take(thing, position)
                        else
                            timeline.grab = null
                    }
                    onMoved: position => {
                        if (timeline.grab !== null)
                            timeline.shift = timeline.minuteAt(position.y) - timeline.grab.pressMinute
                    }
                    onReleased: timeline.drop()
                    onCanceled: timeline.grab = null
                }
            }
        }

        // Readable: how it works, and back to the whole day at a glance, in a bar
        // under the hours (nothing of them hidden). Made only then.
        Loader {
            id: readableBar

            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            active: timeline.readable
            z: 5

            sourceComponent: Rectangle {
                implicitHeight: barRow.implicitHeight + 12
                color: timeline.theme.background

                Rectangle {
                    width: parent.width
                    height: 1
                    color: timeline.theme.line
                }
                RowLayout {
                    id: barRow

                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    anchors.leftMargin: 8
                    anchors.rightMargin: 4
                    spacing: 8

                    Label {
                        Layout.fillWidth: true
                        text: timeline.sioul.text("health-readable-help")
                        wrapMode: Text.Wrap
                        font.pixelSize: 12
                        color: timeline.theme.muted
                    }
                    Button {
                        highlighted: true
                        text: timeline.sioul.text("health-readable-done")
                        onClicked: timeline.readableDone()
                    }
                }
            }
        }
    }
}
