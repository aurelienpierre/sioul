// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Today, seen: your hours by what they are for, the events at their times,
// the steps the plan gives today in hours of their kind, a pause between
// them; a line where now is. The step under way is marked, the next one
// outlined. A layout to look at, never a schedule to keep: a step that runs
// over only moves the next ones (dayview.rs). Each card is as tall as its
// text with air around it: the hours stretch for that, and the day scrolls,
// opened at the current hour. Nothing is drawn over anything else: two
// events at once sit side by side.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: dayView

    required property var sioul
    required property var theme
    // The page's `day`: {from, to, now, hours: [{start, end, kind}], blocks: [{start, end, kind, title, key, energy, location, column, columns, part}], all_day, more}.
    property var day: null
    readonly property var blocks: dayView.day ? dayView.day.blocks : []
    readonly property var hours: dayView.day && dayView.day.hours ? dayView.day.hours : []
    // The block under way, and the next one; margins around them are neither.
    readonly property int current: dayView.blocks.findIndex(b => b.kind !== "margin" && b.kind !== "done" && b.start <= dayView.day.now && dayView.day.now < b.end)
    readonly property int next: dayView.blocks.findIndex(b => b.kind !== "margin" && b.kind !== "done" && b.start > (dayView.day ? dayView.day.now : 0))

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

    // The current hour at the top, the hours before it a scroll away; again
    // when the hour turns, never under your hand otherwise.
    function showNow(always) {
        if (!dayView.day || flick.height <= 0)
            return
        const now = Math.min(Math.max(dayView.day.now, dayView.day.from), dayView.day.to)
        const hour = dayView.day.from + Math.floor((now - dayView.day.from) / 3600) * 3600
        if (!always && hour === dayView.anchoredHour)
            return
        dayView.anchoredHour = hour
        flick.contentY = Math.max(0, Math.min(flick.contentHeight - flick.height, dayView.at(hour)))
    }

    spacing: 6
    onDayChanged: Qt.callLater(dayView.showNow, false)
    onVisibleChanged: if (visible) Qt.callLater(dayView.showNow, true)

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
                    y: -height / 2
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
                readonly property bool now: block.index === dayView.current
                readonly property bool comingNext: block.index === dayView.next
                readonly property string part: dayView.partOf(block.modelData)

                readonly property real slot: (flick.width - 80) / Math.max(1, block.modelData.columns)

                x: 72 + block.modelData.column * block.slot
                y: dayView.at(block.modelData.start) + 1
                width: block.slot - (block.modelData.columns > 1 ? 4 : 0)
                height: dayView.heightOf(block.index)
                clip: true
                radius: 4
                color: block.isMargin ? "transparent" : block.now ? dayView.theme.hover : block.isEvent || block.isNeed ? dayView.theme.surface : dayView.theme.button
                border.color: !block.isMargin && (block.now || block.comingNext) ? dayView.theme.accent : dayView.theme.line
                border.width: block.now ? 2 : 1

                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 10
                    anchors.rightMargin: 10
                    anchors.topMargin: dayView.padding
                    anchors.bottomMargin: dayView.padding
                    spacing: 10

                    Label {
                        Layout.alignment: Qt.AlignTop
                        text: dayView.time(block.modelData.start) + "–" + dayView.time(block.modelData.end)
                        textFormat: Text.PlainText
                        font.pixelSize: 12
                        topPadding: Math.max(0, (titleMetrics.height - timeMetrics.height) / 2)
                        color: dayView.theme.muted
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
                            return block.modelData.location === "" ? title : title + " · " + block.modelData.location
                        }
                        textFormat: Text.PlainText
                        elide: Text.ElideRight
                        maximumLineCount: Math.max(1, Math.floor((block.height - 2 * dayView.padding) / titleMetrics.height))
                        wrapMode: Text.Wrap
                        font.weight: block.now && !block.isMargin ? Font.DemiBold : Font.Normal
                        color: block.isMargin || block.isDone ? dayView.theme.muted : dayView.theme.text
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
                MouseArea {
                    enabled: !block.isNeed && !block.isMargin
                    anchors.fill: parent
                    cursorShape: Qt.PointingHandCursor
                    onClicked: block.isEvent ? dayView.openEvent(block.modelData.key) : dayView.openTask(block.modelData.key)
                }
            }
        }

        // Now.
        Rectangle {
            visible: dayView.day !== null && dayView.day.now >= dayView.day.from && dayView.day.now <= dayView.day.to
            x: 56
            y: dayView.day ? dayView.at(dayView.day.now) - 1 : 0
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
