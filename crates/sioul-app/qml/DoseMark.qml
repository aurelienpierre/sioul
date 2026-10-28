// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A dose's mark on the agenda (the unified agenda, §8.2), by what is known of
// it: a muted ring before its time, a ring once its time has come, a warm
// ring to check, filled once taken, dotted while it is being read; a short
// tick on a day to come. Never a medicine's name, never "not taken": what the
// device cannot know is said as a doubt (WeekPlanning.qml's `doseState`).

pragma ComponentBehavior: Bound

import QtQuick

Item {
    id: mark

    required property var theme
    // "ahead", "reading", "not-come", "come", "check", "taken".
    property string known: "reading"

    implicitWidth: 12
    implicitHeight: 12

    // A day to come: a short muted tick, never a ring.
    Rectangle {
        visible: mark.known === "ahead"
        anchors.centerIn: parent
        width: mark.width - 4
        height: 2
        color: mark.theme.muted
    }
    Rectangle {
        visible: mark.known !== "ahead" && mark.known !== "reading"
        anchors.centerIn: parent
        width: 10
        height: 10
        radius: 5
        color: mark.known === "taken" ? mark.theme.accent : mark.theme.background
        border.width: mark.known === "check" ? 2 : 1.5
        border.color: mark.known === "check" ? mark.theme.warm : mark.known === "taken" ? mark.theme.accent : mark.known === "come" ? mark.theme.text : mark.theme.muted
    }
    // Being read: a ring of dots.
    Repeater {
        model: mark.known === "reading" ? 8 : 0

        delegate: Rectangle {
            id: dot

            required property int index

            x: mark.width / 2 - 1 + 4.5 * Math.cos(dot.index * Math.PI / 4)
            y: mark.height / 2 - 1 + 4.5 * Math.sin(dot.index * Math.PI / 4)
            width: 2
            height: 2
            radius: 1
            color: mark.theme.muted
        }
    }
}
