// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The focus session: a small window that stays on top while you work in
// another one (a form, a letter). A soft disc drains as the time chosen
// passes, in a neutral colour; there is no ticking and no sound. Two minutes
// before the end, the disc and a line say it is time to find a stopping point.
// At the end, nothing rings: "Keep going" or "Stop here, it counts". Stopping
// offers one line, "next time, start by…", kept with the task (docs/tasks.md:
// time made visible, transitions as steps, a breadcrumb to come back to).

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

SioulWindow {
    id: focusWindow

    required property var sioul
    // The session, as the core keeps it (`focusSession`), parsed.
    property var session: null
    property real now: Date.now() / 1000
    property bool stopping: false
    readonly property bool openEnded: focusWindow.session !== null && focusWindow.session.planned === 0
    readonly property real elapsed: focusWindow.session === null ? 0 : Math.max(0, (focusWindow.session.paused_at !== null ? focusWindow.session.paused_at : focusWindow.now) - focusWindow.session.start - focusWindow.session.paused)
    readonly property real remaining: focusWindow.session === null ? 0 : focusWindow.session.planned * 60 - focusWindow.elapsed
    readonly property bool paused: focusWindow.session !== null && focusWindow.session.paused_at !== null
    readonly property bool closing: !focusWindow.openEnded && focusWindow.remaining > 0 && focusWindow.remaining <= 120
    readonly property bool over: !focusWindow.openEnded && focusWindow.remaining <= 0
    // A slow fill, then a drain: the share of the time chosen still left.
    readonly property real left: focusWindow.openEnded || focusWindow.session === null ? 1 : Math.max(0, Math.min(1, focusWindow.remaining / (focusWindow.session.planned * 60)))

    // Stopped here, and the day closed: the main window says where everything went.
    signal closeDay()

    function minutes(seconds) {
        return Math.max(0, Math.ceil(seconds / 60))
    }

    function line() {
        if (focusWindow.paused)
            return focusWindow.sioul.text("focus-paused")
        if (focusWindow.openEnded)
            return focusWindow.sioul.textWith("focus-so-far", "minutes", String(Math.floor(focusWindow.elapsed / 60)))
        if (focusWindow.over)
            return focusWindow.sioul.text("focus-over")
        if (focusWindow.closing)
            return focusWindow.sioul.text("focus-two-left")
        return focusWindow.sioul.textWith("focus-left", "minutes", String(focusWindow.minutes(focusWindow.remaining)))
    }

    function stop(done) {
        const said = focusWindow.sioul.focusStop(done, breadcrumb.text)
        breadcrumb.clear()
        focusWindow.stopping = false
        return said
    }

    title: focusWindow.session ? focusWindow.session.title : focusWindow.sioul.text("focus-title")
    width: 320
    height: column.implicitHeight + 32
    minimumWidth: 280
    flags: Qt.Window | Qt.WindowStaysOnTopHint
    visible: focusWindow.session !== null

    // The application is quitting: the session stays on disk, the window goes.
    property bool quitting: false
    // The pause to move: minutes of focus between two (Health page), 0 when off;
    // the focus counted at the last offer; offered now; taken now.
    property int moveEvery: 0
    property real movedAt: 0
    property bool moving: false
    property bool breaking: false
    property real sessionStart: 0

    // A new session counts from zero, with the pause as the Health page sets it now.
    onSessionChanged: {
        if (focusWindow.session === null) {
            focusWindow.moving = false
            focusWindow.breaking = false
            focusWindow.sessionStart = 0
            return
        }
        if (focusWindow.session.start !== focusWindow.sessionStart) {
            focusWindow.sessionStart = focusWindow.session.start
            focusWindow.movedAt = 0
            focusWindow.moving = false
            focusWindow.breaking = false
            focusWindow.moveEvery = focusWindow.sioul.movementMinutes()
        }
    }

    // Time to move: asked, never imposed. The session goes on until you pause
    // it: something urgent may come first, and a pause missed must not stop
    // the time counted. Unanswered, it stays asked; "Not now" asks again later.
    function checkMove() {
        if (focusWindow.moveEvery <= 0 || focusWindow.paused || focusWindow.moving || focusWindow.session === null)
            return
        if (focusWindow.elapsed - focusWindow.movedAt >= focusWindow.moveEvery * 60) {
            focusWindow.moving = true
            focusWindow.movedAt = focusWindow.elapsed
            focusWindow.raise()
        }
    }

    // Paused for it: a line on where you stopped, for when you are back.
    function takeBreak() {
        focusWindow.breaking = true
        if (!focusWindow.paused)
            focusWindow.sioul.focusPause()
        stoppedLine.text = focusWindow.session ? focusWindow.session.stopped : ""
        stoppedLine.forceActiveFocus()
    }

    function notNow() {
        focusWindow.moving = false
    }

    function backFromMoving() {
        if (focusWindow.breaking && stoppedLine.text.trim() !== "")
            focusWindow.sioul.setStopped(stoppedLine.text)
        focusWindow.moving = false
        focusWindow.breaking = false
        if (focusWindow.paused)
            focusWindow.sioul.focusPause()
    }

    // Closing the window is stopping: the line to come back to first.
    onClosing: close => {
        if (focusWindow.quitting)
            return
        close.accepted = false
        focusWindow.stopping = true
    }
    onVisibleChanged: {
        if (!focusWindow.visible || focusWindow.screen === null)
            return
        // Out of the way: the bottom right corner of its screen. The desktop's
        // available size spans every screen (a second one to the right would put
        // the window off any screen): kept within this one, above a panel.
        const screen = focusWindow.screen
        focusWindow.x = Math.min(screen.virtualX + screen.width, screen.desktopAvailableWidth) - focusWindow.width - 40
        focusWindow.y = Math.min(screen.virtualY + screen.height, screen.desktopAvailableHeight) - focusWindow.height - 60
    }

    Timer {
        running: focusWindow.visible && !focusWindow.paused
        interval: 1000
        repeat: true
        onTriggered: {
            focusWindow.now = Date.now() / 1000
            focusWindow.checkMove()
        }
    }

    // What the window shows, for the window's images.
    property alias face: face

    Rectangle {
        id: face

        anchors.fill: parent
        color: focusWindow.theme.background

        ColumnLayout {
            id: column

            anchors.fill: parent
            anchors.margins: 16
            spacing: 10

            Label {
                Layout.fillWidth: true
                text: focusWindow.session ? focusWindow.session.title : ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                maximumLineCount: 3
                elide: Text.ElideRight
                font.pixelSize: 16
                color: focusWindow.theme.text
            }
            // A pause to move and stretch, offered: the session goes on until you take it.
            ColumnLayout {
                visible: focusWindow.moving
                Layout.fillWidth: true
                spacing: 8

                Label {
                    Layout.fillWidth: true
                    text: focusWindow.sioul.text(focusWindow.breaking ? "focus-move" : "focus-move-ask")
                    wrapMode: Text.Wrap
                    font.pixelSize: 15
                    color: focusWindow.theme.text
                }
                RowLayout {
                    visible: !focusWindow.breaking
                    Layout.fillWidth: true
                    spacing: 8

                    Button {
                        Layout.fillWidth: true
                        text: focusWindow.sioul.text("focus-move-now")
                        highlighted: true
                        onClicked: focusWindow.takeBreak()
                    }
                    Button {
                        Layout.fillWidth: true
                        flat: true
                        text: focusWindow.sioul.text("focus-move-not-now")
                        onClicked: focusWindow.notNow()
                    }
                }
                // Where you stopped: one line, shown again when you are back.
                TextField {
                    id: stoppedLine

                    visible: focusWindow.breaking
                    Layout.fillWidth: true
                    placeholderText: focusWindow.sioul.text("stopped-hint")
                    onAccepted: focusWindow.backFromMoving()
                }
                Button {
                    visible: focusWindow.breaking
                    Layout.fillWidth: true
                    text: focusWindow.sioul.text("focus-move-back")
                    highlighted: true
                    onClicked: focusWindow.backFromMoving()
                }
            }
            Label {
                visible: focusWindow.session !== null && focusWindow.session.stopped !== ""
                Layout.fillWidth: true
                text: focusWindow.session ? focusWindow.sioul.textWith("task-stopped", "text", focusWindow.session.stopped) : ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 12
                color: focusWindow.theme.muted
            }

            // The time left, as a disc that drains; open-ended, a dot that breathes.
            Item {
                Layout.alignment: Qt.AlignHCenter
                Layout.preferredWidth: 120
                Layout.preferredHeight: 120

                Canvas {
                    id: disc

                    anchors.fill: parent
                    visible: !focusWindow.openEnded
                    property real share: focusWindow.left
                    property color fill: focusWindow.closing || focusWindow.over ? focusWindow.theme.warm : focusWindow.theme.accent
                    property color track: focusWindow.theme.line

                    onShareChanged: requestPaint()
                    onFillChanged: requestPaint()
                    onPaint: {
                        const ctx = getContext("2d")
                        const r = Math.min(width, height) / 2
                        ctx.reset()
                        ctx.fillStyle = disc.track
                        ctx.beginPath()
                        ctx.arc(r, r, r, 0, 2 * Math.PI)
                        ctx.fill()
                        if (disc.share > 0) {
                            ctx.fillStyle = disc.fill
                            ctx.beginPath()
                            ctx.moveTo(r, r)
                            ctx.arc(r, r, r, -Math.PI / 2, -Math.PI / 2 + 2 * Math.PI * disc.share)
                            ctx.closePath()
                            ctx.fill()
                        }
                    }
                }
                Rectangle {
                    visible: focusWindow.openEnded
                    anchors.centerIn: parent
                    width: 60
                    height: 60
                    radius: 30
                    color: focusWindow.theme.accent
                    opacity: 0.6

                    SequentialAnimation on scale {
                        running: focusWindow.openEnded && !focusWindow.paused && focusWindow.visible
                        loops: Animation.Infinite
                        NumberAnimation {
                            from: 0.85
                            to: 1.1
                            duration: 4000
                            easing.type: Easing.InOutSine
                        }
                        NumberAnimation {
                            from: 1.1
                            to: 0.85
                            duration: 4000
                            easing.type: Easing.InOutSine
                        }
                    }
                }
            }

            Label {
                Layout.fillWidth: true
                horizontalAlignment: Text.AlignHCenter
                text: focusWindow.line()
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: focusWindow.closing || focusWindow.over ? focusWindow.theme.text : focusWindow.theme.muted
            }

            // At the end: go on, or stop, and it counts.
            RowLayout {
                visible: focusWindow.over && !focusWindow.stopping
                Layout.fillWidth: true
                spacing: 6

                Button {
                    Layout.fillWidth: true
                    text: focusWindow.sioul.text("focus-keep-going")
                    onClicked: focusWindow.sioul.focusExtend(5)
                }
                Button {
                    Layout.fillWidth: true
                    text: focusWindow.sioul.text("focus-stop-counts")
                    highlighted: true
                    onClicked: focusWindow.stopping = true
                }
            }
            RowLayout {
                visible: !focusWindow.over && !focusWindow.stopping
                Layout.fillWidth: true
                spacing: 6

                Button {
                    implicitWidth: implicitContentWidth + leftPadding + rightPadding
                    text: focusWindow.paused ? focusWindow.sioul.text("focus-resume") : focusWindow.sioul.text("focus-pause")
                    icon.name: focusWindow.paused ? "media-playback-start" : "media-playback-pause"
                    icon.color: focusWindow.theme.text
                    onClicked: focusWindow.sioul.focusPause()
                }
                Button {
                    implicitWidth: implicitContentWidth + leftPadding + rightPadding
                    visible: !focusWindow.openEnded
                    text: "+5"
                    Accessible.name: focusWindow.sioul.text("focus-plus-five")
                    ToolTip.visible: hovered
                    ToolTip.text: focusWindow.sioul.text("focus-plus-five")
                    onClicked: focusWindow.sioul.focusExtend(5)
                }
                Item {
                    Layout.fillWidth: true
                }
                Button {
                    implicitWidth: implicitContentWidth + leftPadding + rightPadding
                    text: focusWindow.sioul.text("focus-stop")
                    flat: true
                    onClicked: focusWindow.stopping = true
                }
            }

            // Stopping: one line to come back to, then done or not.
            ColumnLayout {
                visible: focusWindow.stopping
                Layout.fillWidth: true
                spacing: 6

                TextField {
                    id: breadcrumb

                    Layout.fillWidth: true
                    placeholderText: focusWindow.sioul.text("focus-breadcrumb")
                    onAccepted: focusWindow.stop(false)
                }
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 6

                    Button {
                        Layout.fillWidth: true
                        text: focusWindow.sioul.text("focus-done")
                        icon.name: "task-complete"
                        icon.color: focusWindow.theme.text
                        onClicked: focusWindow.stop(true)
                    }
                    Button {
                        Layout.fillWidth: true
                        text: focusWindow.sioul.text("focus-keep")
                        highlighted: true
                        onClicked: focusWindow.stop(false)
                    }
                }
                // The line above becomes the first step when work comes back.
                Button {
                    Layout.fillWidth: true
                    flat: true
                    text: focusWindow.sioul.text("done-button")
                    icon.name: "weather-clear-night"
                    icon.color: focusWindow.theme.text
                    onClicked: {
                        focusWindow.stop(false)
                        focusWindow.closeDay()
                    }
                }
            }
        }
    }
}
