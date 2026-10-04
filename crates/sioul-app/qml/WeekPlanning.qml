// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The week as a planning: seven days side by side, the hours down, each event
// where and as long as it happens, side by side with what overlaps it. Whole
// days sit in a band on top. Only the hours given to work, your admin or free
// time are shown, and those of any event outside them: not the night. They
// fill the height, scrolled only when that would make an hour too thin. A
// quiet line marks now. A double click on an empty hour makes an event there.

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
    onDaysChanged: if (planning.visible) Qt.callLater(planning.scrollToStart)

    function minutesNow() {
        const now = new Date()
        return now.getHours() * 60 + now.getMinutes()
    }

    // Now, in minutes from midnight, for its line: moved on each minute while shown.
    property int now: planning.minutesNow()

    Timer {
        interval: 60000
        running: planning.visible
        repeat: true
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
                text: new Date(dayTitle.modelData.date + "T12:00:00").toLocaleDateString(planning.locale, "ddd d")
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
                            onTapped: planning.menu(chip.modelData)
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
        ScrollBar.vertical: ScrollBar {}
        Component.onCompleted: planning.scrollToStart()

        Item {
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
                    // A double click on an empty hour: an event there.
                    TapHandler {
                        onDoubleTapped: point => planning.newAt(column.modelData.date, planning.range.from / 60 + Math.floor(point.position.y / planning.hourHeight))
                    }

                    Repeater {
                        model: column.modelData.events.filter(e => !e.all_day)

                        delegate: Rectangle {
                            id: block

                            required property var modelData
                            readonly property real slot: (planning.dayWidth - 4) / Math.max(1, block.modelData.columns)

                            x: 2 + block.modelData.column * block.slot
                            y: planning.yOf(block.modelData.from_minute)
                            width: block.slot - 2
                            height: Math.max(18, (block.modelData.to_minute - block.modelData.from_minute) / 60 * planning.hourHeight - 1)
                            radius: 4
                            color: planning.chosen(block.modelData) ? planning.theme.line : planning.theme.background
                            border.color: block.modelData.color ? block.modelData.color : planning.theme.accent
                            border.width: planning.chosen(block.modelData) ? 2 : 1
                            opacity: block.modelData.tentative ? 0.7 : 1
                            clip: true

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
                            TapHandler {
                                onTapped: planning.open(block.modelData)
                            }
                            TapHandler {
                                acceptedButtons: Qt.RightButton
                                onTapped: planning.menu(block.modelData)
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
        }
    }
}
