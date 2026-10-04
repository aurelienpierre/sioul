// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A routine played: a small window on top, one step at a time, its time
// draining in a neutral colour, the next one said before it comes, the whole
// routine as a row of dots. When a step's time is up, the routine moves on by
// itself if it was asked to; else it waits, saying so, never ringing. Done,
// skip, five more minutes, pause, stop: none of them is counted.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

SioulWindow {
    id: player

    required property var sioul
    // {id, title, auto, step: [{title, minutes, open}]}; null when none plays.
    property var routine: null
    property int index: 0
    property real stepStart: 0
    property real extra: 0
    property real pausedAt: 0
    property real pausedFor: 0
    property real now: Date.now()
    readonly property var steps: player.routine ? player.routine.step : []
    readonly property var step: player.index < player.steps.length ? player.steps[player.index] : null
    readonly property bool finished: player.routine !== null && player.index >= player.steps.length
    readonly property bool paused: player.pausedAt > 0
    readonly property real elapsed: ((player.paused ? player.pausedAt : player.now) - player.stepStart - player.pausedFor) / 1000
    // At least a second: a step written "0 min" would share its time by zero.
    readonly property real length: player.step ? Math.max(1, player.step.minutes * 60 + player.extra) : 1
    readonly property real remaining: player.length - player.elapsed
    readonly property bool over: player.step !== null && player.remaining <= 0

    // What a step opens: "porch", "sioul:task/<UID>".
    signal openThing(string what)

    function play(routine) {
        player.routine = routine
        player.begin(0)
        player.show()
        player.raise()
    }

    function begin(index) {
        player.index = index
        player.stepStart = Date.now()
        player.now = player.stepStart
        player.extra = 0
        player.pausedAt = 0
        player.pausedFor = 0
    }

    function pause() {
        if (player.paused) {
            player.pausedFor += Date.now() - player.pausedAt
            player.pausedAt = 0
        } else {
            player.pausedAt = Date.now()
        }
    }

    function stop() {
        player.routine = null
        player.close()
    }

    title: player.routine ? player.routine.title : ""
    width: 340
    height: column.implicitHeight + 32
    minimumWidth: 300
    flags: Qt.Window | Qt.WindowStaysOnTopHint
    visible: false

    onVisibleChanged: {
        if (!player.visible || player.screen === null)
            return
        // Out of the way: the bottom right corner of its screen, kept within it
        // (the desktop's available size spans every screen), above a panel.
        const screen = player.screen
        player.x = Math.min(screen.virtualX + screen.width, screen.desktopAvailableWidth) - player.width - 40
        player.y = Math.min(screen.virtualY + screen.height, screen.desktopAvailableHeight) - player.height - 60
    }

    Timer {
        running: player.visible && player.step !== null && !player.paused
        interval: 1000
        repeat: true
        onTriggered: {
            player.now = Date.now()
            if (player.over && player.routine.auto)
                player.begin(player.index + 1)
        }
    }

    // What the window shows, for the window's images.
    property alias face: face

    Rectangle {
        id: face

        anchors.fill: parent
        color: player.theme.background

        ColumnLayout {
            id: column

            anchors.fill: parent
            anchors.margins: 16
            spacing: 10

            Label {
                Layout.fillWidth: true
                text: player.routine ? player.routine.title : ""
                textFormat: Text.PlainText
                elide: Text.ElideRight
                font.pixelSize: 13
                color: player.theme.muted
            }
            // The whole routine: done, now, to come.
            Row {
                spacing: 6

                Repeater {
                    model: player.steps.length

                    delegate: Rectangle {
                        required property int index

                        width: 10
                        height: 10
                        radius: 5
                        color: index < player.index ? player.theme.accent : "transparent"
                        border.color: index <= player.index ? player.theme.accent : player.theme.line
                        border.width: index === player.index ? 2 : 1
                    }
                }
            }
            Label {
                Layout.fillWidth: true
                text: player.finished ? player.sioul.text("routine-finished") : player.step ? player.step.title : ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 18
                color: player.theme.text
            }
            // The time left, draining; neutral, whatever is left.
            Rectangle {
                visible: !player.finished
                Layout.fillWidth: true
                Layout.preferredHeight: 6
                radius: 3
                color: player.theme.line

                Rectangle {
                    width: parent.width * Math.max(0, Math.min(1, player.remaining / player.length))
                    height: parent.height
                    radius: 3
                    color: player.theme.accent
                    opacity: 0.6
                }
            }
            Label {
                visible: !player.finished
                Layout.fillWidth: true
                text: player.paused ? player.sioul.text("focus-paused") : player.over ? player.sioul.text("routine-over") : player.sioul.textWith("routine-left", "minutes", String(Math.max(1, Math.ceil(player.remaining / 60))))
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: player.theme.muted
            }
            Label {
                visible: !player.finished
                Layout.fillWidth: true
                text: player.index + 1 < player.steps.length ? player.sioul.textWith("routine-next", "title", player.steps[player.index + 1].title) : player.sioul.text("routine-last")
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: player.theme.muted
            }
            RowLayout {
                visible: !player.finished
                Layout.fillWidth: true
                spacing: 6

                Button {
                    highlighted: true
                    text: player.sioul.text("routine-done")
                    onClicked: player.begin(player.index + 1)
                }
                Button {
                    visible: player.step !== null && player.step.open !== undefined && player.step.open !== ""
                    text: player.sioul.text("routine-open")
                    onClicked: player.openThing(player.step.open)
                }
                Button {
                    flat: true
                    text: player.sioul.text("routine-more")
                    onClicked: player.extra += 300
                }
            }
            RowLayout {
                Layout.fillWidth: true
                spacing: 6

                Button {
                    visible: !player.finished
                    flat: true
                    text: player.sioul.text("routine-skip")
                    onClicked: player.begin(player.index + 1)
                }
                Button {
                    visible: !player.finished
                    flat: true
                    text: player.sioul.text(player.paused ? "routine-resume" : "routine-pause")
                    onClicked: player.pause()
                }
                Item {
                    Layout.fillWidth: true
                }
                Button {
                    flat: !player.finished
                    highlighted: player.finished
                    text: player.sioul.text(player.finished ? "ui-close" : "routine-stop")
                    onClicked: player.stop()
                }
            }
        }
    }
}
