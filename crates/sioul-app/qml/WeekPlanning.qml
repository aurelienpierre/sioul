// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The week as a planning: seven days side by side, the hours down, each event
// where and as long as it happens, side by side with what overlaps it. Whole
// days sit in a band on top. Only the hours given to work, your admin or free
// time are shown, and those of any event outside them: not the night. They
// fill the height, scrolled only when that would make an hour too thin. A
// quiet line marks now. A double click on an empty hour makes an event there.
//
// An event is moved by hand (docs/client.md, "Calendars"): dragged, it keeps
// its length, to another time, and in the week to another day; by its
// bottom edge, its end changes. Where it would go is drawn, with its times;
// let go, it is written and sent as its form writes it, and "Undo" waits in
// the status line. A repeating one asks first: only this time, or every
// time. One in a calendar that can only be read stays where it is; whole
// days stay in their band. With a mouse, press and drag; with a finger, a
// long press picks it up (a plain swipe scrolls). One set of handlers for
// the whole planning (TimeDrag.qml): the events carry none.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: planning

    required property var sioul
    required property var theme
    required property var days
    required property var locale
    // The event open on the right, to show it chosen.
    property var opened: null
    readonly property int gutter: 52
    // The hours shown, in minutes from midnight, whole hours: those given to
    // something (docs/areas.md), else the usual day from its start to 22:00;
    // widened to every event of the days shown.
    readonly property var range: {
        let from = 24 * 60
        let to = 0
        for (const day of planning.days) {
            if (day.hours_from >= 0 && day.hours_to > day.hours_from) {
                from = Math.min(from, day.hours_from)
                to = Math.max(to, day.hours_to)
            }
        }
        if (from >= to) {
            from = planning.sioul.dayStart() * 60
            to = 22 * 60
        }
        for (const day of planning.days) {
            for (const event of day.events) {
                if (!event.all_day) {
                    from = Math.min(from, event.from_minute)
                    to = Math.max(to, event.to_minute)
                }
            }
        }
        from = Math.max(0, Math.floor(from / 60) * 60)
        to = Math.min(24 * 60, Math.max(from + 60, Math.ceil(to / 60) * 60))
        return { from: from, to: to }
    }
    readonly property int hoursShown: (planning.range.to - planning.range.from) / 60
    // As tall as the window allows; never thinner than a line of text.
    readonly property real hourHeight: Math.max(30, hours.height / Math.max(1, planning.hoursShown))
    readonly property real dayWidth: Math.max(40, (planning.width - planning.gutter - 12) / Math.max(1, planning.days.length))

    // Where a minute of the day sits.
    function yOf(minute) {
        return (minute - planning.range.from) / 60 * planning.hourHeight
    }

    // The minute at `y` in the hours' content (not clamped).
    function minuteAt(y) {
        return planning.range.from + y / planning.hourHeight * 60
    }

    signal open(var event)
    signal menu(var event)
    signal newAt(string day, int hour)

    function chosen(event) {
        return planning.opened !== null && planning.opened.key === event.key && planning.opened.start === event.start
    }

    // When the hours do not all fit: on today, an hour before now; else the first.
    function scrollToStart() {
        const minutes = planning.days.some(d => d.today) ? Math.max(planning.range.from, planning.minutesNow() - 60) : planning.range.from
        hours.contentY = Math.max(0, Math.min(planning.yOf(minutes) - 4, hours.contentHeight - hours.height))
    }

    onVisibleChanged: {
        if (!planning.visible)
            return
        planning.now = planning.minutesNow()
        Qt.callLater(planning.scrollToStart)
    }
    // The days read again: what was drawn where an event went is the event itself now.
    onDaysChanged: {
        planning.landed = null
        if (planning.visible)
            Qt.callLater(planning.scrollToStart)
    }

    function minutesNow() {
        const now = new Date()
        return now.getHours() * 60 + now.getMinutes()
    }

    // Now, in minutes from midnight, for its line: moved on each minute while shown.
    property int now: planning.minutesNow()

    // An event's block, as its column draws it: {x, y, width, height} in the hours' content.
    function blockOf(event, column) {
        const slot = (planning.dayWidth - 4) / Math.max(1, event.columns)
        return { x: planning.gutter + column * planning.dayWidth + 2 + event.column * slot, y: planning.yOf(event.from_minute), width: slot - 2, height: Math.max(18, (event.to_minute - event.from_minute) / 60 * planning.hourHeight - 1) }
    }

    // Its end in this day's column, not cut at midnight: its bottom edge is its end.
    function endsHere(event, day) {
        return Math.floor((event.end - day.start) / 60) === event.to_minute
    }

    function movable(event) {
        return !event.read_only && !event.all_day
    }

    function columnAt(x) {
        return Math.floor((x - planning.gutter) / planning.dayWidth)
    }

    // What lies at `p` (the hours' content): {event, column, part: "body" or
    // "bottom" (its bottom edge's grab zone, 8 px at least)}; else null.
    function eventAt(p) {
        const column = planning.columnAt(p.x)
        if (column < 0 || column >= planning.days.length || p.x < planning.gutter)
            return null
        const day = planning.days[column]
        let found = null
        for (const event of day.events) {
            if (event.all_day)
                continue
            const box = planning.blockOf(event, column)
            const canBottom = planning.movable(event) && planning.endsHere(event, day)
            if (p.x < box.x || p.x > box.x + box.width || p.y < box.y || p.y > box.y + box.height + (canBottom ? 4 : 0))
                continue
            const inside = Math.max(4, Math.min(8, box.height / 3))
            found = { event: event, column: column, part: canBottom && p.y >= box.y + box.height - inside ? "bottom" : "body" }
        }
        return found
    }

    // An event taken by the hand: {event, column, part, pressMinute}; where the pointer is now.
    property var grab: null
    property point pointer: Qt.point(0, 0)
    // Let go, or waiting for "only this time / every time": drawn where it goes until the days come back.
    property var landed: null

    // Where the event taken would go: {start, end} in Unix seconds, and the column it shows in.
    function target(grab, p) {
        const e = grab.event
        const day = planning.days[grab.column]
        const shift = planning.minuteAt(p.y) - grab.pressMinute
        const range = planning.range
        if (grab.part === "bottom") {
            const to = Math.max(e.from_minute + 5, Math.min(range.to, Math.round((e.to_minute + shift) / 5) * 5))
            return { start: e.start, end: day.start + to * 60, column: grab.column }
        }
        const length = e.end - e.start
        const column = planning.days.length > 1 ? Math.max(0, Math.min(planning.days.length - 1, planning.columnAt(p.x))) : grab.column
        const there = planning.days[column]
        // By whole five minutes; on its new day, within the hours shown.
        const lengthMinutes = length / 60
        const least = there.start + range.from * 60
        const most = there.start + (lengthMinutes <= range.to - range.from ? range.to - lengthMinutes : range.to - 15) * 60
        const start = Math.max(least, Math.min(most, Math.round((e.start + (there.start - day.start) + shift * 60) / 300) * 300))
        return { start: start, end: start + length, column: column }
    }

    function clock(seconds) {
        return Qt.formatTime(new Date(seconds * 1000), "HH:mm")
    }

    // "09:00–10:30"; on another day, its day first: "Tue 6, 09:00–10:30".
    function said(grab, to) {
        const times = planning.clock(to.start) + "–" + planning.clock(to.end)
        if (to.column === grab.column)
            return times
        return new Date(to.start * 1000).toLocaleDateString(planning.locale, "ddd d") + ", " + times
    }

    function take(thing, p) {
        planning.grab = { event: thing.event, column: thing.column, part: thing.part, pressMinute: planning.minuteAt(p.y) }
        planning.pointer = p
        planning.landed = null
    }

    function drop(p) {
        const grab = planning.grab
        planning.grab = null
        if (!grab)
            return
        const to = planning.target(grab, p)
        // Let go where it was: nothing changes, nothing to undo.
        if (to.start === grab.event.start && to.end === grab.event.end)
            return
        planning.landed = { event: grab.event, column: to.column, start: to.start, end: to.end, text: planning.said(grab, to) }
        if (grab.event.recurring) {
            which.now().ask()
            return
        }
        planning.save(false)
    }

    // The event let go written: that time alone, or every time.
    function save(onlyThis) {
        const landed = planning.landed
        if (!landed)
            return
        landedLate.restart()
        const problem = planning.sioul.moveEvent(landed.event.key, landed.event.start, landed.start, landed.end, onlyThis)
        if (problem !== "") {
            planning.landed = null
            planning.sioul.status = problem
        }
    }

    Timer {
        id: landedLate

        interval: 4000
        onTriggered: planning.landed = null
    }

    Timer {
        interval: 60000
        running: planning.visible && !planning.sioul.away
        repeat: true
        triggeredOnStart: true
        onTriggered: planning.now = planning.minutesNow()
    }

    spacing: 4

    // The days.
    RowLayout {
        Layout.fillWidth: true
        spacing: 0

        Item {
            Layout.preferredWidth: planning.gutter
        }
        Repeater {
            model: planning.days

            delegate: Label {
                id: dayTitle

                required property var modelData

                Layout.preferredWidth: planning.dayWidth
                horizontalAlignment: Text.AlignHCenter
                // Narrow days (a phone's week): the weekday above its number.
                text: planning.dayWidth < 64 ? new Date(dayTitle.modelData.date + "T12:00:00").toLocaleDateString(planning.locale, "ddd") + "\n" + Number(dayTitle.modelData.date.slice(8, 10)) : new Date(dayTitle.modelData.date + "T12:00:00").toLocaleDateString(planning.locale, "ddd d")
                textFormat: Text.PlainText
                font.weight: dayTitle.modelData.today ? Font.Bold : Font.DemiBold
                color: dayTitle.modelData.today ? planning.theme.accent : planning.theme.text
            }
        }
    }

    // Whole days, on top.
    RowLayout {
        visible: planning.days.some(d => d.events.some(e => e.all_day))
        Layout.fillWidth: true
        spacing: 0

        Item {
            Layout.preferredWidth: planning.gutter
        }
        Repeater {
            model: planning.days

            delegate: ColumnLayout {
                id: band

                required property var modelData

                Layout.preferredWidth: planning.dayWidth
                Layout.alignment: Qt.AlignTop
                spacing: 2

                Repeater {
                    model: band.modelData.events.filter(e => e.all_day)

                    delegate: Rectangle {
                        id: chip

                        required property var modelData

                        Layout.fillWidth: true
                        Layout.rightMargin: 3
                        implicitHeight: chipLabel.implicitHeight + 6
                        radius: 4
                        color: planning.chosen(chip.modelData) ? planning.theme.line : planning.theme.surface
                        border.color: chip.modelData.color ? chip.modelData.color : planning.theme.accent

                        Label {
                            id: chipLabel

                            anchors.fill: parent
                            anchors.margins: 3
                            text: chip.modelData.summary
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            font.pixelSize: 12
                            color: planning.theme.text
                        }
                        TapHandler {
                            onTapped: planning.open(chip.modelData)
                        }
                        TapHandler {
                            acceptedButtons: Qt.RightButton
                            // A touch has no buttons: on a touch screen, the long press below.
                            acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
                            onTapped: planning.menu(chip.modelData)
                        }
                        // On a touch screen, the menu at a long press.
                        TapHandler {
                            id: chipHold

                            acceptedDevices: PointerDevice.TouchScreen
                            onLongPressed: {
                                chip.Window.window.menuAt = chipHold.point.scenePosition
                                planning.menu(chip.modelData)
                            }
                        }
                    }
                }
            }
        }
    }

    // The hours.
    Flickable {
        id: hours

        Layout.fillWidth: true
        Layout.fillHeight: true
        clip: true
        contentWidth: width
        contentHeight: planning.hoursShown * planning.hourHeight + 1
        boundsBehavior: Flickable.StopAtBounds
        // A mouse drags events, not the view: the wheel scrolls it.
        acceptedButtons: Qt.NoButton
        ScrollBar.vertical: ScrollBar {}
        Component.onCompleted: planning.scrollToStart()

        Item {
            id: canvas

            width: hours.width
            height: hours.contentHeight

            Repeater {
                model: planning.hoursShown

                delegate: Item {
                    id: hour

                    required property int index
                    readonly property int clock: planning.range.from / 60 + hour.index

                    y: hour.index * planning.hourHeight
                    width: parent.width
                    height: planning.hourHeight

                    Label {
                        width: planning.gutter - 8
                        horizontalAlignment: Text.AlignRight
                        text: (hour.clock < 10 ? "0" : "") + hour.clock + ":00"
                        textFormat: Text.PlainText
                        font.pixelSize: 11
                        color: planning.theme.muted
                    }
                    Rectangle {
                        x: planning.gutter
                        width: parent.width - planning.gutter
                        height: 1
                        color: planning.theme.line
                    }
                }
            }

            Repeater {
                model: planning.days

                delegate: Item {
                    id: column

                    required property var modelData
                    required property int index

                    x: planning.gutter + column.index * planning.dayWidth
                    width: planning.dayWidth
                    height: parent.height

                    Rectangle {
                        anchors.fill: parent
                        color: column.modelData.today ? planning.theme.surface : "transparent"
                        opacity: 0.6
                        border.color: planning.theme.line
                        border.width: 0
                    }
                    Rectangle {
                        width: 1
                        height: parent.height
                        color: planning.theme.line
                    }

                    Repeater {
                        model: column.modelData.events.filter(e => !e.all_day)

                        delegate: Rectangle {
                            id: block

                            required property var modelData
                            readonly property real slot: (planning.dayWidth - 4) / Math.max(1, block.modelData.columns)
                            // Taken by the hand: outlined; moved, or let go: faint where it was.
                            readonly property bool taken: planning.grab !== null && planning.grab.event.key === block.modelData.key && planning.grab.event.start === block.modelData.start
                            readonly property bool away: (block.taken && drag.dragging) || (planning.landed !== null && planning.landed.event.key === block.modelData.key && planning.landed.event.start === block.modelData.start)

                            x: 2 + block.modelData.column * block.slot
                            y: planning.yOf(block.modelData.from_minute)
                            width: block.slot - 2
                            height: Math.max(18, (block.modelData.to_minute - block.modelData.from_minute) / 60 * planning.hourHeight - 1)
                            radius: 4
                            color: planning.chosen(block.modelData) ? planning.theme.line : planning.theme.background
                            border.color: block.taken ? planning.theme.accent : block.modelData.color ? block.modelData.color : planning.theme.accent
                            border.width: planning.chosen(block.modelData) || block.taken ? 2 : 1
                            opacity: block.away ? 0.3 : block.modelData.tentative ? 0.7 : 1
                            clip: true
                            Accessible.role: Accessible.Button
                            Accessible.name: block.modelData.summary

                            Rectangle {
                                width: 3
                                height: parent.height
                                radius: 2
                                color: block.modelData.color ? block.modelData.color : planning.theme.accent
                            }
                            ColumnLayout {
                                anchors.fill: parent
                                anchors.leftMargin: 7
                                anchors.rightMargin: 3
                                anchors.topMargin: 2
                                spacing: 0

                                Label {
                                    Layout.fillWidth: true
                                    text: block.modelData.summary
                                    textFormat: Text.PlainText
                                    elide: Text.ElideRight
                                    wrapMode: block.height > 40 ? Text.Wrap : Text.NoWrap
                                    maximumLineCount: 2
                                    font.pixelSize: 12
                                    font.strikeout: block.modelData.cancelled
                                    color: planning.theme.text
                                }
                                Label {
                                    visible: block.height > 34
                                    Layout.fillWidth: true
                                    text: block.modelData.when + (block.modelData.location ? "  ·  " + block.modelData.location : "")
                                    textFormat: Text.PlainText
                                    elide: Text.ElideRight
                                    font.pixelSize: 11
                                    color: planning.theme.muted
                                }
                            }
                        }
                    }

                    // Now: a quiet line in today's column.
                    Rectangle {
                        visible: column.modelData.today && planning.now >= planning.range.from && planning.now <= planning.range.to
                        y: planning.yOf(planning.now)
                        width: parent.width
                        height: 2
                        color: planning.theme.accent
                    }
                }
            }

            // Where the event taken goes, and a tag with its times above it, out
            // of the finger's way (under it at the top of the view).
            Rectangle {
                id: ghost

                readonly property var going: planning.grab !== null && drag.dragging ? planning.target(planning.grab, planning.pointer) : null
                readonly property var shownAt: ghost.going ? { event: planning.grab.event, column: ghost.going.column, start: ghost.going.start, end: ghost.going.end, text: planning.said(planning.grab, ghost.going) } : planning.landed
                readonly property var day: ghost.shownAt ? planning.days[ghost.shownAt.column] : null
                readonly property real from: ghost.day ? Math.max(planning.range.from, (ghost.shownAt.start - ghost.day.start) / 60) : 0
                readonly property real until: ghost.day ? Math.min(planning.range.to, (ghost.shownAt.end - ghost.day.start) / 60) : 0

                visible: ghost.day !== undefined && ghost.day !== null && ghost.until > ghost.from
                x: ghost.day ? planning.gutter + ghost.shownAt.column * planning.dayWidth + 2 : 0
                y: planning.yOf(ghost.from)
                width: planning.dayWidth - 4
                height: Math.max(18, (ghost.until - ghost.from) / 60 * planning.hourHeight - 1)
                radius: 4
                color: Qt.rgba(planning.theme.accent.r, planning.theme.accent.g, planning.theme.accent.b, 0.16)
                border.color: planning.theme.accent
                border.width: 2
                z: 2
            }
            Rectangle {
                id: tag

                readonly property bool above: ghost.y - tag.height - 4 >= hours.contentY + 2

                visible: ghost.visible
                x: Math.max(planning.gutter, Math.min(canvas.width - tag.width - 2, ghost.x))
                y: tag.above ? ghost.y - tag.height - 4 : ghost.y + ghost.height + 4
                width: tagText.implicitWidth + 12
                height: tagText.implicitHeight + 8
                radius: 4
                color: planning.theme.surface
                border.color: planning.theme.accent
                z: 3

                Label {
                    id: tagText

                    x: 6
                    y: 4
                    text: ghost.shownAt ? ghost.shownAt.text : ""
                    textFormat: Text.PlainText
                    font.features: { "tnum": 1 }
                    font.pixelSize: 13
                    font.weight: Font.DemiBold
                    color: planning.theme.text
                }
            }

            TimeDrag {
                id: drag

                anchors.fill: parent
                z: 4
                flick: hours
                onHovered: position => {
                    const thing = position.x < 0 ? null : planning.eventAt(position)
                    drag.cursorShape = !thing ? Qt.ArrowCursor : !planning.movable(thing.event) ? Qt.PointingHandCursor : thing.part === "bottom" ? Qt.SizeVerCursor : Qt.OpenHandCursor
                }
                onTapped: position => {
                    const thing = planning.eventAt(position)
                    if (thing)
                        planning.open(thing.event)
                }
                onDoubleTapped: position => {
                    const column = planning.columnAt(position.x)
                    if (column >= 0 && column < planning.days.length && !planning.eventAt(position))
                        planning.newAt(planning.days[column].date, Math.floor(planning.minuteAt(position.y) / 60))
                }
                onMenuAsked: (position, scenePosition) => {
                    planning.grab = null
                    const thing = planning.eventAt(position)
                    if (!thing)
                        return
                    planning.Window.window.menuAt = scenePosition
                    planning.menu(thing.event)
                }
                onHeld: (position, scenePosition) => {
                    const thing = planning.eventAt(position)
                    if (!thing)
                        return
                    if (planning.movable(thing.event)) {
                        planning.take(thing, position)
                        drag.pickUp(true)
                    } else {
                        planning.Window.window.menuAt = scenePosition
                        planning.menu(thing.event)
                    }
                }
                onStarted: position => {
                    // A finger took it when held; a mouse takes what it pressed on.
                    if (drag.holding)
                        return
                    const thing = planning.eventAt(position)
                    if (thing && planning.movable(thing.event))
                        planning.take(thing, position)
                    else
                        planning.grab = null
                }
                onMoved: position => planning.pointer = position
                onReleased: position => planning.drop(position)
                onCanceled: planning.grab = null
            }
        }
    }

    // A repeating event let go: this time alone, or every time. Made the first time.
    Later {
        id: which

        sourceComponent: Component {
            Dialog {
                id: whichForm

                // Answered: the window closes on its way; closed without: nothing moves.
                property bool answered: false

                function ask() {
                    whichForm.answered = false
                    whichForm.open()
                }

                parent: Overlay.overlay
                anchors.centerIn: parent
                modal: true
                width: Math.min(440, (parent ? parent.width : 440) - 2 * planning.theme.gap)
                title: planning.landed ? planning.theme.plain(planning.landed.event.summary) : ""
                onClosed: {
                    if (!whichForm.answered)
                        planning.landed = null
                }

                ColumnLayout {
                    width: parent.width
                    spacing: 8

                    Label {
                        Layout.fillWidth: true
                        text: planning.landed ? planning.sioul.textWith("agenda-move-which", "time", planning.landed.text) : ""
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: planning.theme.text
                    }
                    Button {
                        Layout.fillWidth: true
                        text: planning.sioul.text("agenda-delete-this")
                        onClicked: {
                            whichForm.answered = true
                            whichForm.close()
                            planning.save(true)
                        }
                    }
                    Button {
                        Layout.fillWidth: true
                        text: planning.sioul.text("agenda-delete-all")
                        onClicked: {
                            whichForm.answered = true
                            whichForm.close()
                            planning.save(false)
                        }
                    }
                    Button {
                        Layout.alignment: Qt.AlignRight
                        text: planning.sioul.text("ui-cancel")
                        onClicked: whichForm.close()
                    }
                }
            }
        }
    }
}
