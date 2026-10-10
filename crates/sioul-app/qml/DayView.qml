// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Today, seen: your hours by what they are for, the events at their times,
// the steps the plan gives today in hours of their kind, a pause between
// them; a line where now is. The step under way is marked, the next one
// outlined. A layout to look at, never a schedule to keep: a step that runs
// over only moves the next ones (dayview.rs). Each card is as tall as its
// text with air around it: the hours stretch for that, and the day scrolls,
// opened at the current hour. Nothing is drawn over anything else: two
// events at once sit side by side. Time for you is kept quietly, twice a
// day, and the free time kept for steps running long is shown as such; why
// today holds what it holds is said above, in words (docs/capacity.md).
//
// A step is moved by hand to another time today (docs/tasks.md, "The day,
// seen", "Pinned to a time"): dragged, it is pinned there, its time block (an
// event in your calendar) made or moved, its own dates as you set them, and
// the day is laid again around it; "Let the plan place it" (its menu) takes
// its block away and gives it back to the plan. A meal or a nap moves as on
// the Health page, that day's change; an event, when the day knows it can be
// written, as in the agenda. Where it would go is drawn, with its times; let
// go, it is changed, and "Undo" waits in the status line. With a mouse,
// press and drag; with a finger, a long press picks it up (a plain swipe
// scrolls). One set of handlers for the whole day (TimeDrag.qml).

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: dayView

    required property var sioul
    required property var theme
    // The page's `day`: {from, to, now, hours: [{start, end, kind}], blocks: [{start, end, kind, title, key, energy, location, column, columns, part, pinned, note}], all_day, more, said}.
    // Kinds: "event", "task", "meal", "nap", "margin", "done", "gain" (time for you), "slack" (kept free).
    property var day: null
    // Now on the clock (Unix seconds): the window's clock (main.qml), moved at
    // each minute's turn and when the app comes back. It moves the line and
    // marks the block under way; nothing is laid again for it.
    property real now: Date.now() / 1000
    readonly property var blocks: dayView.day ? dayView.day.blocks : []
    readonly property var hours: dayView.day && dayView.day.hours ? dayView.day.hours : []
    // The block under way, and the next one; margins around them are neither.
    readonly property int current: dayView.blocks.findIndex(b => b.kind !== "margin" && b.kind !== "done" && b.kind !== "slack" && b.start <= dayView.now && dayView.now < b.end)
    readonly property int next: dayView.blocks.findIndex(b => b.kind !== "margin" && b.kind !== "done" && b.kind !== "slack" && b.start > dayView.now)

    // Laid again from now at each five minutes' turn while shown, when shown
    // again, and as soon as the app comes back (the clock jumps): steps are
    // laid from the next five minutes, and what another device marked done or
    // noted comes in through the sharing.
    signal relay
    // When it was last laid again (Unix seconds, the clock's).
    property real relaid: 0

    function relayNow() {
        dayView.relaid = dayView.now
        dayView.relay()
    }

    // Made with what the page had: laid again at the next five minutes' turn.
    Component.onCompleted: dayView.relaid = dayView.now

    onNowChanged: {
        if (!dayView.visible)
            return
        dayView.showNow(false)
        if (Math.floor(dayView.now / 300) !== Math.floor(dayView.relaid / 300))
            dayView.relayNow()
    }

    // Air inside a card, above and below its text.
    readonly property int padding: 8
    // A card's least height: one line of its text, and its air.
    readonly property real cardHeight: Math.max(titleMetrics.height, timeMetrics.height) + 2 * dayView.padding
    // The whole day in the height when it fits; else as much as the tightest
    // card needs to hold its line before the next one starts (a quarter of an
    // hour at least: a step is never shorter), and the day scrolls.
    readonly property real perMinute: {
        if (!dayView.day)
            return 1.5
        const fit = (flick.height - 20) / Math.max(60, (dayView.day.to - dayView.day.from) / 60)
        let needed = 1.2
        for (const block of dayView.blocks)
            needed = Math.max(needed, (dayView.cardHeight + 2) / dayView.roomOf(block))
        return Math.max(fit, needed)
    }
    // The kinds of hours today has, for the legend.
    readonly property var kinds: {
        const seen = []
        for (const stretch of dayView.hours)
            for (const kind of stretch.kind.split("+"))
                if (seen.indexOf(kind) < 0)
                    seen.push(kind)
        return ["work", "admin", "leisure"].filter(k => seen.indexOf(k) >= 0)
    }
    // The hour the view was last opened at.
    property real anchoredHour: -1

    signal openTask(string uid)
    signal openEvent(string key)

    function at(seconds) {
        return (seconds - dayView.day.from) / 60 * dayView.perMinute
    }

    function time(seconds) {
        return Qt.formatTime(new Date(seconds * 1000), "HH:mm")
    }

    function kindColor(kind) {
        return dayView.theme.chartColors[Math.max(0, ["work", "admin", "leisure"].indexOf(kind))]
    }

    // Minutes from a block's start to the next one's (side by side ones aside),
    // or to the day's end: the room its card has.
    function roomOf(block) {
        const later = dayView.blocks.filter(b => b.start > block.start).map(b => b.start)
        const until = later.length > 0 ? Math.min(...later) : Math.max(block.end, dayView.day.to)
        return Math.max(15, (until - block.start) / 60)
    }

    // A block's height: as long as it lasts, a little air before the next;
    // a short one as tall as its text, within the room before the next.
    function heightOf(index) {
        const block = dayView.blocks[index]
        return Math.max(dayView.cardHeight, (block.end - block.start) / 60 * dayView.perMinute - 2)
    }

    // "1/2": a long step cut by a break, its part among all.
    function partOf(block) {
        if (!block.part)
            return ""
        return block.part + "/" + dayView.blocks.filter(b => b.key === block.key && b.part > 0).length
    }

    // The time at `y` in the day's content, Unix seconds.
    function secondsAt(y) {
        return dayView.day.from + y / dayView.perMinute * 60
    }

    // A block's card, as drawn: {x, y, width, height} in the day's content.
    function cardOf(index) {
        const block = dayView.blocks[index]
        const slot = (flick.width - 80) / Math.max(1, block.columns)
        return { x: 72 + block.column * slot, y: dayView.at(block.start) + 1, width: slot - (block.columns > 1 ? 4 : 0), height: dayView.heightOf(index) }
    }

    // What a block moves with: itself; for a margin, its step's card (the same
    // part) or its event (the one it is around); null for a margin of nothing.
    function ownerOf(block) {
        if (block.kind !== "margin")
            return block
        return dayView.blocks.find(b => b.kind === "task" && b.key === block.key && b.part === block.part) || dayView.blocks.find(b => b.kind === "event" && b.key === block.key && (b.start === block.end || b.end === block.start)) || null
    }

    // The step a block belongs to: its card, or null.
    function stepOf(block) {
        const owner = dayView.ownerOf(block)
        return owner !== null && owner.kind === "task" ? owner : null
    }

    // Midnight tonight, Unix seconds: a time given stays on today.
    function midnight() {
        const d = new Date(dayView.now * 1000)
        d.setHours(24, 0, 0, 0)
        return d.getTime() / 1000
    }

    // What a hand can move: a step (by its card or its margins); a meal or a nap
    // not over; an event in a calendar that can be written, wholly today (by
    // its card or its margins).
    function movable(block) {
        const owner = dayView.ownerOf(block)
        if (owner === null)
            return false
        if (owner.kind === "task")
            return true
        if (owner.kind === "meal" || owner.kind === "nap")
            return owner.end > dayView.now
        return owner.kind === "event" && owner.read_only === false && owner.start > dayView.day.from && owner.end < dayView.midnight()
    }

    // What lies at `p` (the day's content): {block, index, part: "body", or
    // "bottom" for a meal's, a nap's or an event's end (8 px at least)}; else null.
    function blockAt(p) {
        let found = null
        for (let i = 0; i < dayView.blocks.length; i++) {
            const block = dayView.blocks[i]
            const card = dayView.cardOf(i)
            const canBottom = (block.kind === "meal" || block.kind === "nap" || block.kind === "event") && dayView.movable(block)
            if (p.x < card.x || p.x > card.x + card.width || p.y < card.y || p.y > card.y + card.height + (canBottom ? 4 : 0))
                continue
            const inside = Math.max(4, Math.min(8, card.height / 3))
            found = { block: block, index: i, part: canBottom && p.y >= card.y + card.height - inside ? "bottom" : "body" }
        }
        return found
    }

    // A block taken by the hand: {block, part, pressSeconds}; where the pointer is now.
    property var grab: null
    property point pointer: Qt.point(0, 0)
    // Let go: drawn where it goes until the day comes back laid again.
    property var landed: null

    // Where the block taken would go, by whole five minutes, today: {start, end}, Unix seconds.
    function target(grab, p) {
        const block = grab.block
        const shift = dayView.secondsAt(p.y) - grab.pressSeconds
        const snap = s => Math.round(s / 300) * 300
        const last = Math.min(dayView.day.to, dayView.midnight())
        if (grab.part === "bottom")
            return { start: block.start, end: Math.max(block.start + 300, Math.min(last, snap(block.end + shift))) }
        // A step: its own card, given a time not before now (one past is laid from now); an event with its margins.
        const moved = dayView.ownerOf(block) || block
        const length = moved.end - moved.start
        const first = dayView.stepOf(block) !== null ? Math.max(dayView.day.from, Math.ceil(dayView.now / 300) * 300) : dayView.day.from
        const start = Math.max(first, Math.min(last - length, snap(moved.start + shift)))
        return { start: start, end: start + length }
    }

    function take(thing, p) {
        dayView.grab = { block: thing.block, part: thing.part, pressSeconds: dayView.secondsAt(p.y) }
        dayView.pointer = p
        dayView.landed = null
    }

    // "2026-10-06T14:30", "14:30": a time given, a time on the clock.
    function local(seconds) {
        const d = new Date(seconds * 1000)
        const pad = n => n < 10 ? "0" + n : String(n)
        return d.getFullYear() + "-" + pad(d.getMonth() + 1) + "-" + pad(d.getDate()) + "T" + pad(d.getHours()) + ":" + pad(d.getMinutes())
    }

    function drop(p) {
        const grab = dayView.grab
        dayView.grab = null
        if (!grab)
            return
        const to = dayView.target(grab, p)
        const step = dayView.stepOf(grab.block)
        const was = dayView.ownerOf(grab.block) || grab.block
        // Let go where it was: nothing changes, nothing to undo.
        if (to.start === was.start && to.end === was.end)
            return
        dayView.landed = { block: was, start: to.start, end: to.end }
        landedLate.restart()
        let problem = ""
        if (step !== null) {
            // Pinned there, as long as the card dragged: its time block made or moved (docs/tasks.md, "Pinned to a time").
            problem = dayView.sioul.pinTask(step.key, dayView.local(to.start), Math.round((to.end - to.start) / 60))
        } else if (was.kind === "event") {
            if (was.recurring) {
                which.now().ask()
                return
            }
            problem = dayView.sioul.moveEvent(was.key, was.start, to.start, to.end, false)
        } else {
            const asked = grab.part === "bottom" ? { key: grab.block.key, action: "times", from: dayView.time(to.start), to: dayView.time(to.end) } : { key: grab.block.key, action: "move", from: dayView.time(to.start) }
            problem = dayView.sioul.dragNeed(JSON.stringify(asked))
        }
        if (problem !== "") {
            dayView.landed = null
            dayView.sioul.status = problem
        }
    }

    // A repeating event let go: this time alone, or every time.
    function saveEvent(onlyThis) {
        const landed = dayView.landed
        if (!landed)
            return
        landedLate.restart()
        const problem = dayView.sioul.moveEvent(landed.block.key, landed.block.start, landed.start, landed.end, onlyThis)
        if (problem !== "") {
            dayView.landed = null
            dayView.sioul.status = problem
        }
    }

    // The current hour at the top, the hours before it a scroll away; again
    // when the hour turns, never under your hand otherwise.
    function showNow(always) {
        if (!dayView.day || flick.height <= 0 || drag.dragging)
            return
        const now = Math.min(Math.max(dayView.now, dayView.day.from), dayView.day.to)
        const hour = dayView.day.from + Math.floor((now - dayView.day.from) / 3600) * 3600
        if (!always && hour === dayView.anchoredHour)
            return
        dayView.anchoredHour = hour
        flick.contentY = Math.max(0, Math.min(flick.contentHeight - flick.height, dayView.at(hour)))
    }

    spacing: 6
    onDayChanged: {
        // Laid again: what was drawn where a block went is the block itself now.
        dayView.landed = null
        Qt.callLater(dayView.showNow, false)
    }

    Timer {
        id: landedLate

        interval: 4000
        onTriggered: dayView.landed = null
    }
    onVisibleChanged: {
        if (!visible)
            return
        dayView.relayNow()
        Qt.callLater(dayView.showNow, true)
    }

    FontMetrics {
        id: titleMetrics
    }
    FontMetrics {
        id: timeMetrics

        font.pixelSize: 12
    }

    Repeater {
        model: dayView.day ? dayView.day.all_day : []

        delegate: Label {
            required property string modelData

            Layout.fillWidth: true
            text: dayView.sioul.textWith("day-all-day", "what", modelData)
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: dayView.theme.muted
        }
    }
    // Why today holds what it holds, when something changed it: a line or two, in words.
    Repeater {
        model: dayView.day && dayView.day.said ? dayView.day.said : []

        delegate: Label {
            required property string modelData

            Layout.fillWidth: true
            text: modelData
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            font.pixelSize: 13
            color: dayView.theme.muted
        }
    }
    Label {
        visible: dayView.day !== null && dayView.blocks.length === 0
        Layout.fillWidth: true
        text: dayView.sioul.text("day-empty")
        wrapMode: Text.Wrap
        color: dayView.theme.muted
    }
    // What today's hours are for: a mark each, as the band beside the hours.
    Flow {
        visible: dayView.kinds.length > 1
        Layout.fillWidth: true
        spacing: 14

        Repeater {
            model: dayView.kinds

            delegate: Row {
                id: legend

                required property string modelData

                spacing: 6

                Rectangle {
                    anchors.verticalCenter: parent.verticalCenter
                    width: 4
                    height: 14
                    radius: 2
                    color: dayView.kindColor(legend.modelData)
                }
                Label {
                    text: dayView.sioul.text("area-" + legend.modelData)
                    textFormat: Text.PlainText
                    font.pixelSize: 12
                    color: dayView.theme.muted
                }
            }
        }
    }

    Flickable {
        id: flick

        Layout.fillWidth: true
        Layout.fillHeight: true
        contentWidth: width
        contentHeight: dayView.day ? dayView.at(dayView.day.to) + 20 : 0
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        // A mouse drags steps, not the view: the wheel scrolls it.
        acceptedButtons: Qt.NoButton
        ScrollBar.vertical: ScrollBar {}
        onHeightChanged: Qt.callLater(dayView.showNow, true)

        // Your hours, by what they are for: a band beside the hours, a faint wash behind.
        Repeater {
            model: dayView.hours

            delegate: Item {
                id: stretch

                required property var modelData
                readonly property var parts: stretch.modelData.kind.split("+")

                x: 60
                y: dayView.at(stretch.modelData.start)
                width: flick.width - 60
                height: Math.max(1, dayView.at(stretch.modelData.end) - dayView.at(stretch.modelData.start))

                Rectangle {
                    anchors.fill: parent
                    color: dayView.kindColor(stretch.parts[0])
                    opacity: 0.06
                }
                // Overlapping hours: one band per kind, side by side.
                Repeater {
                    model: stretch.parts

                    delegate: Rectangle {
                        required property string modelData
                        required property int index

                        x: index * 5
                        width: 4
                        height: stretch.height
                        color: dayView.kindColor(modelData)
                        opacity: 0.7
                    }
                }
            }
        }

        // The hours, faint.
        Repeater {
            model: dayView.day ? Math.round((dayView.day.to - dayView.day.from) / 3600) + 1 : 0

            delegate: Item {
                id: hour

                required property int index
                readonly property real at: dayView.day.from + hour.index * 3600

                x: 0
                y: dayView.at(hour.at)
                width: flick.width
                height: 1

                Label {
                    // Centred on its line; the first hour's under it, else
                    // the top of the view cut it in half.
                    y: hour.index === 0 ? 0 : -height / 2
                    width: 52
                    text: dayView.time(hour.at)
                    textFormat: Text.PlainText
                    font.pixelSize: 12
                    horizontalAlignment: Text.AlignRight
                    color: dayView.theme.muted
                }
                Rectangle {
                    x: 60
                    width: parent.width - 60
                    height: 1
                    color: dayView.theme.line
                }
            }
        }

        // The events and the steps.
        Repeater {
            model: dayView.blocks

            delegate: Rectangle {
                id: block

                required property var modelData
                required property int index
                readonly property bool isEvent: block.modelData.kind === "event"
                // A meal or a nap kept free (docs/health.md): shown, never a step to open.
                readonly property bool isNeed: block.modelData.kind === "meal" || block.modelData.kind === "nap"
                // Getting there and back, getting ready, around an event or a step: kept free, lighter.
                readonly property bool isMargin: block.modelData.kind === "margin"
                // Done today: kept in view, ticked.
                readonly property bool isDone: block.modelData.kind === "done"
                // Time for you: kept free, quiet, yours to fill or leave empty.
                readonly property bool isGain: block.modelData.kind === "gain"
                // Free time kept for steps running long: nothing to do in it.
                readonly property bool isSlack: block.modelData.kind === "slack"
                // A step given its time by hand: it stays there.
                readonly property bool isPinned: block.modelData.pinned === true
                readonly property bool now: block.index === dayView.current
                readonly property bool comingNext: block.index === dayView.next
                readonly property string part: dayView.partOf(block.modelData)

                readonly property real slot: (flick.width - 80) / Math.max(1, block.modelData.columns)
                // Taken by the hand (a step with its margins): outlined; moved, or let go: faint where it was.
                readonly property var moving: dayView.grab !== null ? (dayView.ownerOf(dayView.grab.block) || dayView.grab.block) : dayView.landed !== null ? dayView.landed.block : null
                readonly property bool taken: block.moving !== null && block.moving.key === block.modelData.key && (block.modelData.kind === block.moving.kind || block.modelData.kind === "margin") && (block.moving.kind !== "task" || block.modelData.part === block.moving.part)
                readonly property bool away: block.taken && ((dayView.grab !== null && drag.dragging) || dayView.landed !== null)

                x: 72 + block.modelData.column * block.slot
                y: dayView.at(block.modelData.start) + 1
                width: block.slot - (block.modelData.columns > 1 ? 4 : 0)
                height: dayView.heightOf(block.index)
                clip: true
                radius: 4
                color: block.isMargin || block.isSlack || block.isGain ? "transparent" : block.now ? dayView.theme.hover : block.isEvent || block.isNeed ? dayView.theme.surface : dayView.theme.button
                border.color: block.taken || (!block.isMargin && !block.isSlack && (block.now || block.comingNext)) ? dayView.theme.accent : dayView.theme.line
                border.width: block.isSlack ? 0 : block.now || block.taken ? 2 : 1
                opacity: block.away ? 0.3 : 1

                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 10
                    anchors.rightMargin: 10
                    anchors.topMargin: dayView.padding
                    anchors.bottomMargin: dayView.padding
                    spacing: 10

                    // Pinned to a time (its block in your calendar, docs/tasks.md): a small pin, quietly.
                    Icon {
                        visible: block.isPinned && !block.isMargin
                        Layout.alignment: Qt.AlignTop
                        Layout.topMargin: Math.max(0, (titleMetrics.height - 12) / 2)
                        Layout.rightMargin: -6
                        iconName: "pin"
                        size: 12
                        color: dayView.theme.accent
                    }
                    Label {
                        Layout.alignment: Qt.AlignTop
                        text: dayView.time(block.modelData.start) + "–" + dayView.time(block.modelData.end)
                        textFormat: Text.PlainText
                        font.pixelSize: 12
                        font.weight: block.isPinned ? Font.DemiBold : Font.Normal
                        topPadding: Math.max(0, (titleMetrics.height - timeMetrics.height) / 2)
                        color: block.isPinned ? dayView.theme.accent : dayView.theme.muted
                    }
                    Label {
                        Layout.fillWidth: true
                        Layout.alignment: Qt.AlignTop
                        text: {
                            if (block.isMargin)
                                return dayView.sioul.textWith("day-margin", "what", block.modelData.title)
                            if (block.isDone)
                                return "✓  " + block.modelData.title
                            const title = block.part === "" ? block.modelData.title : block.modelData.title + " (" + block.part + ")"
                            const beside = block.isGain ? (block.modelData.note || "") : block.modelData.location
                            return beside === "" ? title : title + " · " + beside
                        }
                        textFormat: Text.PlainText
                        elide: Text.ElideRight
                        maximumLineCount: Math.max(1, Math.floor((block.height - 2 * dayView.padding) / titleMetrics.height))
                        wrapMode: Text.Wrap
                        font.weight: block.now && !block.isMargin ? Font.DemiBold : Font.Normal
                        font.italic: block.isSlack
                        color: block.isMargin || block.isDone || block.isSlack || block.isGain ? dayView.theme.muted : dayView.theme.text
                    }
                    Label {
                        visible: block.modelData.energy !== ""
                        Layout.alignment: Qt.AlignTop
                        text: dayView.sioul.text("task-energy-" + block.modelData.energy)
                        textFormat: Text.PlainText
                        font.pixelSize: 12
                        topPadding: Math.max(0, (titleMetrics.height - timeMetrics.height) / 2)
                        color: dayView.theme.muted
                    }
                }
            }
        }

        // Where the block taken goes, and a tag with its times above it, out of
        // the finger's way (under it at the top of the view).
        Rectangle {
            id: ghost

            readonly property var going: dayView.grab !== null && drag.dragging ? dayView.target(dayView.grab, dayView.pointer) : null
            readonly property var shownAt: ghost.going ? ghost.going : dayView.landed

            visible: ghost.shownAt !== null && dayView.day !== null
            x: 72
            y: ghost.shownAt && dayView.day ? dayView.at(ghost.shownAt.start) + 1 : 0
            width: flick.width - 80
            height: ghost.shownAt ? Math.max(dayView.cardHeight, (ghost.shownAt.end - ghost.shownAt.start) / 60 * dayView.perMinute - 2) : 0
            radius: 4
            color: Qt.rgba(dayView.theme.accent.r, dayView.theme.accent.g, dayView.theme.accent.b, 0.14)
            border.color: dayView.theme.accent
            border.width: 2
            z: 2
        }
        Rectangle {
            id: tag

            readonly property bool above: ghost.y - tag.height - 4 >= flick.contentY + 2

            visible: ghost.visible
            x: 72
            y: tag.above ? ghost.y - tag.height - 4 : ghost.y + ghost.height + 4
            width: tagText.implicitWidth + 12
            height: tagText.implicitHeight + 8
            radius: 4
            color: dayView.theme.surface
            border.color: dayView.theme.accent
            z: 3

            Label {
                id: tagText

                x: 6
                y: 4
                text: ghost.shownAt ? dayView.time(ghost.shownAt.start) + "–" + dayView.time(ghost.shownAt.end) : ""
                textFormat: Text.PlainText
                font.features: { "tnum": 1 }
                font.pixelSize: 13
                font.weight: Font.DemiBold
                color: dayView.theme.text
            }
        }

        // One set of handlers for the whole day: a click opens a step or an
        // event (time for you, free time and margins open nothing); a step, a
        // meal, a nap or an event that can be written moves by hand.
        TimeDrag {
            id: drag

            width: flick.width
            height: Math.max(flick.height, flick.contentHeight)
            z: 4
            flick: flick
            onHovered: position => {
                const thing = position.x < 0 || !dayView.day ? null : dayView.blockAt(position)
                const opens = thing !== null && ["meal", "nap", "margin", "gain", "slack"].indexOf(thing.block.kind) < 0
                drag.cursorShape = !thing ? Qt.ArrowCursor : thing.part === "bottom" ? Qt.SizeVerCursor : dayView.movable(thing.block) ? Qt.OpenHandCursor : opens ? Qt.PointingHandCursor : Qt.ArrowCursor
            }
            onTapped: position => {
                const thing = dayView.day ? dayView.blockAt(position) : null
                if (!thing || ["meal", "nap", "margin", "gain", "slack"].indexOf(thing.block.kind) >= 0)
                    return
                if (thing.block.kind === "event")
                    dayView.openEvent(thing.block.key)
                else
                    dayView.openTask(thing.block.key)
            }
            onMenuAsked: (position, scenePosition) => {
                dayView.grab = null
                const thing = dayView.day ? dayView.blockAt(position) : null
                const step = thing ? dayView.stepOf(thing.block) : null
                if (!step)
                    return
                dayView.Window.window.menuAt = scenePosition
                stepMenu.now().show(step)
            }
            onHeld: (position, scenePosition) => {
                const thing = dayView.day ? dayView.blockAt(position) : null
                if (!thing || !dayView.movable(thing.block))
                    return
                dayView.take(thing, position)
                // Let go where it was, a step's menu.
                drag.pickUp(dayView.stepOf(thing.block) !== null)
            }
            onStarted: position => {
                // A finger took it when held; a mouse takes what it pressed on.
                if (drag.holding)
                    return
                const thing = dayView.day ? dayView.blockAt(position) : null
                if (thing && dayView.movable(thing.block))
                    dayView.take(thing, position)
                else
                    dayView.grab = null
            }
            onMoved: position => dayView.pointer = position
            onReleased: position => dayView.drop(position)
            onCanceled: dayView.grab = null
        }

        // Now: the clock moves it alone, nothing else is laid again.
        Rectangle {
            objectName: "nowLine"
            visible: dayView.day !== null && dayView.now >= dayView.day.from && dayView.now <= dayView.day.to
            x: 56
            y: dayView.day ? dayView.at(dayView.now) - 1 : 0
            width: flick.width - 56
            height: 2
            color: dayView.theme.accent

            Label {
                anchors.right: parent.right
                anchors.bottom: parent.top
                text: dayView.sioul.text("day-now")
                font.pixelSize: 11
                color: dayView.theme.accent
            }
        }
    }
    // A step's menu: open it; given its time by hand, back to the plan.
    Later {
        id: stepMenu

        sourceComponent: Component {
            SioulMenu {
                id: menu

                property var step: null

                function show(step) {
                    menu.step = step
                    menu.popup()
                }

                MenuItem {
                    text: dayView.sioul.text("ui-open")
                    onTriggered: dayView.openTask(menu.step.key)
                }
                MenuItem {
                    visible: menu.step !== null && menu.step.pinned === true
                    height: visible ? implicitHeight : 0
                    text: dayView.sioul.text("day-let-plan")
                    onTriggered: {
                        const problem = dayView.sioul.setTaskAt(menu.step.key, "")
                        if (problem !== "")
                            dayView.sioul.status = problem
                    }
                }
            }
        }
    }

    // A repeating event let go: this time alone, or every time.
    Later {
        id: which

        sourceComponent: Component {
            Dialog {
                id: whichForm

                // Answered: it closes on its way; closed without: nothing moves.
                property bool answered: false

                function ask() {
                    whichForm.answered = false
                    whichForm.open()
                }

                parent: Overlay.overlay
                anchors.centerIn: parent
                modal: true
                width: Math.min(440, (parent ? parent.width : 440) - 2 * dayView.theme.gap)
                title: dayView.landed ? dayView.theme.plain(dayView.landed.block.title) : ""
                onClosed: {
                    if (!whichForm.answered)
                        dayView.landed = null
                }

                ColumnLayout {
                    width: parent.width
                    spacing: 8

                    Label {
                        Layout.fillWidth: true
                        text: dayView.landed ? dayView.sioul.textWith("agenda-move-which", "time", dayView.time(dayView.landed.start) + "–" + dayView.time(dayView.landed.end)) : ""
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: dayView.theme.text
                    }
                    Button {
                        Layout.fillWidth: true
                        text: dayView.sioul.text("agenda-delete-this")
                        onClicked: {
                            whichForm.answered = true
                            whichForm.close()
                            dayView.saveEvent(true)
                        }
                    }
                    Button {
                        Layout.fillWidth: true
                        text: dayView.sioul.text("agenda-delete-all")
                        onClicked: {
                            whichForm.answered = true
                            whichForm.close()
                            dayView.saveEvent(false)
                        }
                    }
                    Button {
                        Layout.alignment: Qt.AlignRight
                        text: dayView.sioul.text("ui-cancel")
                        onClicked: whichForm.close()
                    }
                }
            }
        }
    }

    Label {
        visible: dayView.day !== null && dayView.day.more > 0
        Layout.fillWidth: true
        text: dayView.day ? dayView.sioul.textArgs("day-more", JSON.stringify({ count: dayView.day.more })) : ""
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.pixelSize: 13
        color: dayView.theme.muted
    }
}
