// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// One rating from 0 to 10, or none said: what a task or an event costs, or
// what it gives back (docs/research/capacity-budget.md, criteria 1 to 5;
// wellbeing-gain.md, G1). Its name above, its number (or "Not rated") at the
// right, the track the whole width, words at 0, 5 and 10 under it.
//
// Unsaid is not 0: the handle stays hidden until a tap, a drag or a key
// gives a value (an arrow starts at 5, Home at 0, End at 10), and × (or
// Delete) puts it back to unsaid. A hint (what was foreseen, when saying how
// a task was) shows as a pale mark meanwhile, and the arrows start there;
// it is never given by itself. A value is given once the hand lets go or
// the key is released, never at each step of a drag. A drag sideways moves
// it; a drag up or down scrolls the page it is in, and gives nothing.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: rating

    required property var sioul
    required property var theme
    property string label: ""
    // The rating kept: 0 to 10, or null when unsaid.
    property var value: null
    // The words under the track, at 0, 5 and 10.
    property var words: ["", "", ""]
    // A pale mark while nothing is said: what was foreseen; null for none.
    property var hint: null
    readonly property bool hinted: rating.hint !== null && rating.hint !== undefined
    // What shows: the rating kept, or the one just given until it is kept.
    property var shown: null
    // A value on its way, while a finger, the mouse or a key is still down.
    property var held: null
    readonly property bool said: rating.shown !== null && rating.shown !== undefined
    readonly property bool marked: rating.held !== null || rating.said

    // A value given, 0 to 10, or null: unsaid again.
    signal edited(var value)

    function give(given) {
        rating.held = null
        if (given === rating.shown)
            return
        rating.shown = given
        rating.edited(given)
    }

    onValueChanged: rating.shown = rating.value === undefined ? null : rating.value
    Component.onCompleted: rating.shown = rating.value === undefined ? null : rating.value

    spacing: 0

    RowLayout {
        Layout.fillWidth: true
        spacing: 4

        Label {
            Layout.fillWidth: true
            text: rating.label
            wrapMode: Text.Wrap
            color: rating.theme.muted
        }
        Label {
            text: rating.marked ? String(rating.held !== null ? rating.held : rating.shown) : rating.hinted ? rating.sioul.textWith("rating-foreseen", "value", String(rating.hint)) : rating.sioul.text("rating-none")
            color: rating.marked ? rating.theme.text : rating.theme.muted
            font.weight: rating.marked ? Font.DemiBold : Font.Normal
        }
        // Back to unsaid: there while something is said, its room kept otherwise.
        ToolButton {
            implicitWidth: 28
            implicitHeight: 28
            opacity: rating.said ? 1 : 0
            enabled: rating.said && rating.enabled
            text: "×"
            Accessible.name: rating.sioul.text("rating-clear") + " · " + rating.label
            ToolTip.visible: hovered && rating.said
            ToolTip.text: rating.sioul.text("rating-clear")
            ToolTip.delay: 600
            onClicked: rating.give(null)
        }
    }

    Slider {
        id: slider

        // A handle's width, for the ticks and the words to line up with it.
        readonly property real knob: 22

        Layout.fillWidth: true
        from: 0
        to: 10
        stepSize: 1
        snapMode: Slider.SnapAlways
        // Unsaid: between two steps, where no tap or step lands, so that the
        // first one sets a value wherever it is.
        value: rating.held !== null ? rating.held : rating.said ? rating.shown : 5.5
        topPadding: 9
        bottomPadding: 9
        Accessible.name: rating.label
        Accessible.description: rating.marked ? "" : rating.sioul.text("rating-none")

        // A press says nothing yet: a drag up or down may be the page scrolling.
        onPressedChanged: {
            if (slider.pressed)
                rating.held = null
            else if (rating.held !== null)
                rating.give(rating.held)
        }
        // Sideways past a few pixels, or a tap let go: a value, kept on release.
        onMoved: rating.held = Math.round(slider.value)

        Keys.onPressed: event => {
            const now = rating.held !== null ? rating.held : rating.said ? rating.shown : null
            // The first press gives where the arrows start: the hint, else the middle.
            const start = rating.hinted ? rating.hint : 5
            let next = null
            if (event.key === Qt.Key_Left || event.key === Qt.Key_Down)
                next = now === null ? start : Math.max(0, now - 1)
            else if (event.key === Qt.Key_Right || event.key === Qt.Key_Up)
                next = now === null ? start : Math.min(10, now + 1)
            else if (event.key === Qt.Key_PageDown)
                next = now === null ? start : Math.max(0, now - 3)
            else if (event.key === Qt.Key_PageUp)
                next = now === null ? start : Math.min(10, now + 3)
            else if (event.key === Qt.Key_Home)
                next = 0
            else if (event.key === Qt.Key_End)
                next = 10
            else if ((event.key === Qt.Key_Delete || event.key === Qt.Key_Backspace) && rating.said) {
                rating.give(null)
                event.accepted = true
                return
            } else
                return
            rating.held = next
            event.accepted = true
        }
        // Kept when the key comes up, not at each repeat while it is held.
        Keys.onReleased: event => {
            if (!event.isAutoRepeat && rating.held !== null && !slider.pressed) {
                rating.give(rating.held)
                event.accepted = true
            }
        }

        background: Item {
            x: slider.leftPadding
            y: slider.topPadding + slider.availableHeight / 2 - height / 2
            implicitWidth: 200
            implicitHeight: 22
            width: slider.availableWidth
            height: implicitHeight

            // The track; with the keyboard on it and nothing said, ringed.
            Rectangle {
                x: slider.knob / 2 - 3
                width: parent.width - slider.knob + 6
                anchors.verticalCenter: parent.verticalCenter
                height: 6
                radius: 3
                color: rating.theme.line
                border.width: slider.visualFocus && !rating.marked ? 2 : 0
                border.color: rating.theme.focus

                // From 0 to the value, once there is one.
                Rectangle {
                    visible: rating.marked
                    width: 3 + slider.visualPosition * (parent.width - 6) + 3
                    height: parent.height
                    radius: 3
                    color: rating.theme.accent
                }
            }
            // What was foreseen, pale: while nothing is said, and beside what is
            // said when it differs (dread felt lighter than foreseen, shown, never pushed).
            Rectangle {
                visible: rating.hinted && (!rating.marked || rating.hint !== (rating.held !== null ? rating.held : rating.shown))
                x: rating.hinted ? rating.hint / 10 * (slider.availableWidth - slider.knob) : 0
                anchors.verticalCenter: parent.verticalCenter
                width: slider.knob
                height: slider.knob
                radius: width / 2
                color: "transparent"
                border.width: 2
                border.color: rating.theme.accent
                opacity: 0.35
            }
            // A mark at each step, where a tap lands.
            Repeater {
                model: 11

                delegate: Rectangle {
                    required property int index

                    x: slider.knob / 2 + index / 10 * (slider.availableWidth - slider.knob) - width / 2
                    anchors.verticalCenter: parent.verticalCenter
                    width: 2
                    height: index % 5 === 0 ? 12 : 8
                    radius: 1
                    color: rating.theme.muted
                    opacity: 0.5
                }
            }
        }

        handle: Rectangle {
            x: slider.leftPadding + slider.visualPosition * (slider.availableWidth - width)
            y: slider.topPadding + slider.availableHeight / 2 - height / 2
            implicitWidth: slider.knob
            implicitHeight: slider.knob
            radius: width / 2
            visible: rating.marked
            color: slider.pressed ? rating.theme.hover : rating.theme.surface
            border.width: slider.visualFocus ? 3 : 2
            border.color: slider.visualFocus ? rating.theme.focus : rating.theme.accent
        }
    }

    // The words at 0, 5 and 10, each under its place on the track.
    Item {
        id: words

        readonly property real third: (width - slider.knob) / 3

        Layout.fillWidth: true
        // Room before the next rating's name.
        Layout.bottomMargin: 8
        implicitHeight: Math.max(atNone.implicitHeight, atHalf.implicitHeight, atFull.implicitHeight)

        Label {
            id: atNone

            x: 0
            width: words.third + slider.knob / 2
            text: rating.words[0]
            // A long word cut rather than run into its neighbour.
            wrapMode: Text.WrapAtWordBoundaryOrAnywhere
            font.pixelSize: 12
            color: rating.theme.muted
        }
        Label {
            id: atHalf

            x: slider.knob / 2 + words.third
            width: words.third
            text: rating.words[1]
            horizontalAlignment: Text.AlignHCenter
            // A long word cut rather than run into its neighbour.
            wrapMode: Text.WrapAtWordBoundaryOrAnywhere
            font.pixelSize: 12
            color: rating.theme.muted
        }
        Label {
            id: atFull

            x: slider.knob / 2 + 2 * words.third
            width: words.third + slider.knob / 2
            text: rating.words[2]
            horizontalAlignment: Text.AlignRight
            // A long word cut rather than run into its neighbour.
            wrapMode: Text.WrapAtWordBoundaryOrAnywhere
            font.pixelSize: 12
            color: rating.theme.muted
        }
    }
}
