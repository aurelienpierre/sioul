// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The timeline (a Gantt chart): each open task on the days the plan gives
// it, from today on the left. Days without room for tasks are shaded; the
// date asked is a small diamond, never a red line; a task whose plan ends
// after its date has a warmer edge. A bigger task is a thin bar over its
// steps. Nothing here is a deadline in your face: it is the plan, on demand.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: timeline

    required property var sioul
    required property var theme
    required property var timeline
    property string opened: ""
    // Narrower days when that shows the whole plan, never under 20 pixels.
    readonly property int dayWidth: Math.max(20, Math.min(30, Math.floor((timeline.width - timeline.titleWidth - 12) / Math.max(1, timeline.days.length))))
    readonly property int rowHeight: 30
    // The titles' column: narrower on a phone, for the days.
    readonly property int titleWidth: Math.max(120, Math.min(240, Math.round(timeline.width * 0.4)))
    readonly property var days: timeline.timeline ? timeline.timeline.days : []
    readonly property var rows: timeline.timeline ? timeline.timeline.rows : []

    signal open(string uid)
    // Right click, or a long press on a touch screen: the task's menu (its details, Add ▾, Link to…).
    signal menu(var task)

    spacing: 6

    Label {
        Layout.fillWidth: true
        text: timeline.timeline ? timeline.timeline.note : ""
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: timeline.theme.muted
    }

    Flickable {
        id: view

        Layout.fillWidth: true
        Layout.fillHeight: true
        clip: true
        contentWidth: timeline.titleWidth + timeline.days.length * timeline.dayWidth
        contentHeight: 44 + timeline.rows.length * timeline.rowHeight
        boundsBehavior: Flickable.StopAtBounds
        ScrollBar.horizontal: ScrollBar {}
        ScrollBar.vertical: ScrollBar {}

        // Days: months, then numbers and weekdays; rest days shaded down the chart.
        Repeater {
            model: timeline.days

            delegate: Item {
                id: day

                required property var modelData
                required property int index

                x: timeline.titleWidth + day.index * timeline.dayWidth
                width: timeline.dayWidth
                height: view.contentHeight

                Rectangle {
                    anchors.fill: parent
                    color: day.modelData.rest ? timeline.theme.surface : "transparent"
                }
                Rectangle {
                    width: 1
                    height: parent.height
                    color: timeline.theme.line
                    opacity: 0.5
                }
                Label {
                    y: 14
                    width: parent.width
                    horizontalAlignment: Text.AlignHCenter
                    text: day.modelData.day
                    textFormat: Text.PlainText
                    font.pixelSize: 12
                    font.weight: day.index === 0 ? Font.Bold : Font.Normal
                    color: day.index === 0 ? timeline.theme.accent : timeline.theme.text
                }
                Label {
                    y: 28
                    width: parent.width
                    horizontalAlignment: Text.AlignHCenter
                    text: day.modelData.weekday.slice(0, 2)
                    textFormat: Text.PlainText
                    font.pixelSize: 10
                    color: timeline.theme.muted
                }
            }
        }

        // Months, above the days they begin, over the days' shading.
        Repeater {
            model: timeline.days

            delegate: Label {
                id: month

                required property var modelData
                required property int index

                visible: month.modelData.month !== ""
                x: timeline.titleWidth + month.index * timeline.dayWidth + 3
                text: month.modelData.month
                textFormat: Text.PlainText
                font.pixelSize: 11
                color: timeline.theme.muted
            }
        }

        // Today: a quiet line down the first day.
        Rectangle {
            x: timeline.titleWidth + timeline.dayWidth / 2
            y: 44
            width: 2
            height: view.contentHeight - 44
            color: timeline.theme.accent
            opacity: 0.5
        }

        Repeater {
            model: timeline.rows

            delegate: Item {
                id: bar

                required property var modelData
                required property int index

                y: 44 + bar.index * timeline.rowHeight
                width: view.contentWidth
                height: timeline.rowHeight

                Rectangle {
                    anchors.fill: parent
                    color: timeline.opened === bar.modelData.uid ? timeline.theme.surface : "transparent"
                }
                Label {
                    x: 6 + bar.modelData.depth * 14
                    width: timeline.titleWidth - x - 6
                    anchors.verticalCenter: parent.verticalCenter
                    text: bar.modelData.title
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    font.weight: bar.modelData.has_steps ? Font.DemiBold : Font.Normal
                    color: timeline.theme.text
                }
                Rectangle {
                    id: block

                    readonly property bool waiting: bar.modelData.column === "waiting"

                    x: timeline.titleWidth + bar.modelData.start * timeline.dayWidth + 2
                    width: Math.max(8, bar.modelData.length * timeline.dayWidth - 4)
                    height: bar.modelData.has_steps ? 6 : 18
                    anchors.verticalCenter: parent.verticalCenter
                    radius: bar.modelData.has_steps ? 2 : 5
                    color: bar.modelData.color ? bar.modelData.color : timeline.theme.accent
                    opacity: block.waiting ? 0.45 : 0.85
                    border.width: bar.modelData.tight ? 2 : 0
                    border.color: timeline.theme.warm
                }
                // The date asked.
                Rectangle {
                    visible: bar.modelData.due !== null
                    x: timeline.titleWidth + (bar.modelData.due === null ? 0 : bar.modelData.due) * timeline.dayWidth + timeline.dayWidth / 2 - 5
                    anchors.verticalCenter: parent.verticalCenter
                    width: 10
                    height: 10
                    rotation: 45
                    color: timeline.theme.warm
                }
                HoverHandler {
                    id: hover
                }
                ToolTip.visible: hover.hovered && bar.modelData.detail !== ""
                ToolTip.text: timeline.theme.plain(bar.modelData.detail)
                ToolTip.delay: 400
                TapHandler {
                    onTapped: timeline.open(bar.modelData.uid)
                    // On a touch screen, the menu at a long press; letting go then opens nothing.
                    onLongPressed: {
                        bar.Window.window.menuAt = bar.mapToItem(null, point.position.x, point.position.y)
                        timeline.menu({ uid: bar.modelData.uid, title: bar.modelData.title })
                    }
                }
                TapHandler {
                    acceptedButtons: Qt.RightButton
                    // A touch has no buttons: on a touch screen, the long press above.
                    acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
                    onTapped: timeline.menu({ uid: bar.modelData.uid, title: bar.modelData.title })
                }
            }
        }
    }
}
