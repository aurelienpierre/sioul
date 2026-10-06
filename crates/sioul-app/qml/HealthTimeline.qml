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
    readonly property int gutter: 46
    readonly property int hours: Math.max(1, Math.round((timeline.toMinute - timeline.fromMinute) / 60))
    // As tall as the room allows, the whole day in view; never thinner than a line of small text.
    readonly property real hourHeight: Math.max(14, (flick.height - timeline.topRoom - 6) / timeline.hours)
    readonly property real columnWidth: Math.max(30, (timeline.width - timeline.gutter - 4) / Math.max(1, timeline.days.length))
    // A wide column: names in the blocks, the doses' names beside their dots.
    readonly property bool wide: timeline.columnWidth >= 160
    // Now, in minutes from midnight, for its line.
    property int now: timeline.minutesNow()

    // The pointer over a block, or off it ("").
    signal hover(string id)
    // A block or a dose tapped (`segment` as the view gives it), where.
    signal tapped(var segment, point at)
    // A block's menu asked for: right click, or a long press on a touch screen.
    signal menu(var segment, point at)
    // A day's title tapped (the week): that day, shown.
    signal openDay(string date)

    function minutesNow() {
        const d = new Date()
        return d.getHours() * 60 + d.getMinutes()
    }

    // Room above the first hour, for its label.
    readonly property int topRoom: 8

    function yOf(minute) {
        const clamped = Math.min(Math.max(minute, timeline.fromMinute), timeline.toMinute)
        return timeline.topRoom + (clamped - timeline.fromMinute) / 60 * timeline.hourHeight
    }

    // Each kind its colour, calm and apart (the theme's chart colours, no red).
    function kindColor(kind) {
        const colors = timeline.theme.chartColors
        return kind === "meal" ? colors[1] : kind === "nap" ? colors[2] : kind === "sleep" ? colors[4] : timeline.theme.muted
    }

    function tint(color, alpha) {
        return Qt.rgba(color.r, color.g, color.b, alpha)
    }

    // The hour now in view when the day does not fit (a short window).
    function showNow() {
        if (flick.contentHeight <= flick.height)
            return
        const today = timeline.days.some(d => d.today)
        const minute = today ? Math.max(timeline.fromMinute, timeline.now - 60) : timeline.fromMinute
        flick.contentY = Math.max(0, Math.min(flick.contentHeight - flick.height, timeline.yOf(minute)))
    }

    spacing: 4
    onVisibleChanged: if (visible) Qt.callLater(timeline.showNow)

    Timer {
        interval: 60000
        running: timeline.visible && !timeline.sioul.away
        repeat: true
        onTriggered: timeline.now = timeline.minutesNow()
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
                font.weight: title.modelData.today ? Font.Bold : Font.DemiBold
                color: title.modelData.today ? timeline.theme.accent : title.modelData.past ? timeline.theme.muted : timeline.theme.text
                Accessible.role: Accessible.Button
                Accessible.name: title.modelData.title

                TapHandler {
                    cursorShape: Qt.PointingHandCursor
                    onTapped: timeline.openDay(title.modelData.date)
                }
            }
        }
    }

    Flickable {
        id: flick

        Layout.fillWidth: true
        Layout.fillHeight: true
        clip: true
        contentWidth: width
        contentHeight: timeline.topRoom + timeline.hours * timeline.hourHeight + 6
        boundsBehavior: Flickable.StopAtBounds
        ScrollBar.vertical: ScrollBar {}
        onHeightChanged: Qt.callLater(timeline.showNow)

        Item {
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
                    readonly property real contextWidth: Math.round(width * (timeline.days.length > 1 ? 0.22 : 0.3))
                    readonly property real dosesWidth: width < 60 ? 0 : Math.round(width * (timeline.wide ? 0.22 : 0.16))
                    readonly property real keptX: column.contextWidth + 3
                    readonly property real keptWidth: width - column.keptX - column.dosesWidth - 4

                    x: timeline.gutter + column.index * timeline.columnWidth
                    width: timeline.columnWidth
                    height: parent.height

                    // Today: a faint surface; between days, a line.
                    Rectangle {
                        anchors.fill: parent
                        visible: column.modelData.today && timeline.days.length > 1
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
                            width: column.contextWidth - 2
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

                            visible: block.modelData.to_minute > timeline.fromMinute && block.modelData.from_minute < timeline.toMinute
                            x: column.keptX
                            y: timeline.yOf(block.modelData.from_minute) + 1
                            width: column.keptWidth
                            height: Math.max(4, timeline.yOf(block.modelData.to_minute) - timeline.yOf(block.modelData.from_minute) - 2)
                            radius: 3
                            color: timeline.tint(block.hue, 0.1 * block.strength)
                            border.color: block.lit ? timeline.theme.accent : timeline.tint(block.hue, 0.45 * block.strength)
                            border.width: block.lit ? 2 : 1
                            opacity: block.modelData.past ? 0.55 : 1
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

                            HoverHandler {
                                acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
                                onHoveredChanged: timeline.hover(hovered ? block.ident : "")
                            }
                            TapHandler {
                                cursorShape: Qt.PointingHandCursor
                                onTapped: eventPoint => timeline.tapped(block.modelData, eventPoint.scenePosition)
                            }
                            TapHandler {
                                acceptedButtons: Qt.RightButton
                                // A touch has no buttons: on a touch screen, the long press below.
                                acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
                                onTapped: eventPoint => timeline.menu(block.modelData, eventPoint.scenePosition)
                            }
                            TapHandler {
                                id: blockHold

                                acceptedDevices: PointerDevice.TouchScreen
                                onLongPressed: timeline.menu(block.modelData, blockHold.point.scenePosition)
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
                            x: column.dosesWidth > 0 ? column.keptX + column.keptWidth + 4 : column.keptX + column.keptWidth - 10
                            y: timeline.yOf(dose.modelData.from_minute) - 7
                            width: Math.max(14, column.dosesWidth)
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
                                visible: timeline.wide && column.dosesWidth > 40
                                x: 15
                                width: parent.width - 15
                                anchors.verticalCenter: parent.verticalCenter
                                text: dose.modelData.name
                                textFormat: Text.PlainText
                                elide: Text.ElideRight
                                font.pixelSize: 11
                                color: timeline.theme.muted
                            }
                            HoverHandler {
                                id: doseHover

                                acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
                                onHoveredChanged: timeline.hover(hovered ? dose.ident : "")
                            }
                            ToolTip.visible: doseHover.hovered
                            ToolTip.text: dose.modelData.name
                            ToolTip.delay: 400
                            TapHandler {
                                onTapped: eventPoint => timeline.tapped(dose.modelData, eventPoint.scenePosition)
                            }
                        }
                    }

                    // Now: a quiet line on today.
                    Rectangle {
                        visible: column.modelData.today && timeline.now >= timeline.fromMinute && timeline.now <= timeline.toMinute
                        y: timeline.yOf(timeline.now) - 1
                        width: parent.width
                        height: 2
                        color: timeline.theme.accent
                    }
                }
            }
        }
    }
}
