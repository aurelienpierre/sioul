// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A level's mark (docs/attention.md, §1.8): ● at once (★ on the Always
// through row: whatever their list), ◑ shown, not told, ◐ when its event
// falls then, ◎ at the gathered times, ○ later, – not at all, = as their
// own list, ☆ through do-not-disturb at their own times. Drawn, not a
// font's glyph: the same on every system. Always beside words (the grid's
// legend, a cell's sentence), never alone.

import QtQuick
import QtQuick.Controls.impl

Item {
    id: mark

    // The level it marks: "now", "quiet" (shown, not told), "event", "gathered",
    // "later", "never", "as" (as their own list), "through" (docs/attention.md).
    property string value: ""
    // On the Always through row: at once is a star.
    property bool always: false
    // Its colour.
    property color tint: "black"
    // The page's colour, to hollow the star of ☆.
    property color hole: "white"
    // Drawn as a star: at once on the Always through row, or through do-not-disturb.
    readonly property bool star: (mark.value === "now" && mark.always) || mark.value === "through"
    // Drawn hollow: later, at the gathered times, when its event falls, or shown
    // without a word.
    readonly property bool ring: mark.value === "later" || mark.value === "gathered" || mark.value === "event" || mark.value === "quiet"

    implicitWidth: 16
    implicitHeight: 16

    Rectangle {
        visible: mark.value === "now" && !mark.always
        anchors.centerIn: parent
        width: 10
        height: 10
        radius: 5
        color: mark.tint
    }
    Rectangle {
        visible: mark.ring
        anchors.centerIn: parent
        width: 12
        height: 12
        radius: 6
        color: "transparent"
        border.color: mark.tint
        border.width: 1.5
    }
    Rectangle {
        visible: mark.value === "gathered"
        anchors.centerIn: parent
        width: 4
        height: 4
        radius: 2
        color: mark.tint
    }
    // Half the ring filled: the left half for ◐, the right half for ◑.
    Item {
        visible: mark.value === "event" || mark.value === "quiet"
        anchors.centerIn: parent
        width: 12
        height: 12

        Item {
            x: mark.value === "quiet" ? 6 : 0
            width: 6
            height: 12
            clip: true

            Rectangle {
                x: mark.value === "quiet" ? -6 : 0
                width: 12
                height: 12
                radius: 6
                color: mark.tint
            }
        }
    }
    Rectangle {
        visible: mark.value === "never"
        anchors.centerIn: parent
        width: 10
        height: 2
        radius: 1
        color: mark.tint
    }
    // =: two short bars.
    Column {
        visible: mark.value === "as"
        anchors.centerIn: parent
        spacing: 3

        Rectangle {
            width: 10
            height: 2
            radius: 1
            color: mark.tint
        }
        Rectangle {
            width: 10
            height: 2
            radius: 1
            color: mark.tint
        }
    }
    IconImage {
        visible: mark.star
        anchors.centerIn: parent
        name: "emblem-favorite"
        color: mark.tint
        sourceSize: Qt.size(15, 15)
    }
    // ☆: the star hollowed by a smaller one, of the page's colour.
    IconImage {
        visible: mark.value === "through"
        anchors.centerIn: parent
        anchors.verticalCenterOffset: 1
        name: "emblem-favorite"
        color: mark.hole
        sourceSize: Qt.size(7, 7)
    }
}
