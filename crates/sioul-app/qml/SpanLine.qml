// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// One event over several days, as a list draws it through a day (What comes,
// a phone's Week and Month lists): a thin line in its calendar's colour down
// the list's left edge, in its lane (`ongoing::Line`, from Rust: the list
// computes no lane), from a dot at its row on its first day shown to a short
// bar across, its end mark, on its last day. A tap opens the event.

pragma ComponentBehavior: Bound

import QtQuick

Item {
    id: spanLine

    objectName: "spanLine"

    required property var theme
    // {id, uid, colour, lane, first, last}.
    required property var span
    // Where it starts and ends in its day: its row's middle on its first day
    // (else the day's top, through the gap above), its end mark's on its last
    // (else the day's bottom).
    property real fromY: 0
    property real toY: 0
    // The event's title, for a screen reader.
    property string title: ""

    signal open

    readonly property color tint: spanLine.span.colour ? spanLine.span.colour : spanLine.theme.muted

    // A hand's width to tap, the line itself three points wide.
    x: 4 + spanLine.span.lane * 7 - 5
    y: spanLine.fromY
    width: 13
    height: Math.max(0, spanLine.toY - spanLine.fromY)
    Accessible.role: Accessible.Button
    Accessible.name: spanLine.title
    Accessible.onPressAction: spanLine.open()

    Rectangle {
        x: 5
        width: 3
        height: spanLine.height
        radius: 1.5
        color: spanLine.tint
    }
    // Its first day: a dot at its row.
    Rectangle {
        visible: spanLine.span.first === true
        x: 3
        y: -3.5
        width: 7
        height: 7
        radius: 3.5
        color: spanLine.tint
    }
    // Its last day: the end mark, a short bar across.
    Rectangle {
        objectName: "spanEnd"
        visible: spanLine.span.last === true
        x: 1
        y: spanLine.height - 1.5
        width: 11
        height: 3
        radius: 1.5
        color: spanLine.tint
    }
    TapHandler {
        onTapped: spanLine.open()
    }
}
