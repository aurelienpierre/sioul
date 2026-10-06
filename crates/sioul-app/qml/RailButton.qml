// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// One button of the places (Places.qml; in main.qml's status line, the one
// that shows them again): its icon, and its name beside it where names are
// shown (a phone's drawer; Settings ▸ Display ▸ "Show the places' names
// beside their icons"). Its name and its key are said when the pointer rests
// on it, when the keyboard reaches it, at a long press on a touch screen
// (which then opens nothing), and to screen readers. The place shown is marked
// quietly: a soft tint of the accent, its icon in the accent, a short bar on
// its left; the keyboard's focus by a ring. Light, as it is made with the
// window: an icon read on a thread, one line of text, empty unless names are
// shown; the places' tip is one for the whole window (main.qml's sayRailTip).

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Controls.impl

Button {
    id: rail

    required property var theme
    required property string iconName
    // What it is, in words: said in its tip and to screen readers, shown when `named`.
    required property string name
    // Its key ("Ctrl+2", "F9"), said with its name; "" for none.
    property string keys: ""
    // Its name shown beside its icon.
    property bool named: false
    // Written after its name when it is shown: "  ▾" for a menu.
    property string after: ""
    // A place: checked for screen readers while it is the one shown.
    property bool place: false
    // The place shown: marked, and checked.
    property bool current: false
    // New: filled with the accent, the window's one such button.
    property bool filled: false
    // What its tip says: its name, and its key.
    property string tip: rail.keys !== "" ? rail.name + " (" + rail.keys + ")" : rail.name
    // Where its tip is said: the window's function (button, shown, milliseconds),
    // at the right of the places; else Qt's own tip, above it.
    property var sayTip: null
    // A long press on a touch screen said its name: letting go opens nothing.
    property bool held: false
    readonly property alias glyph: glyph

    // Clicked, or Space or Enter, unless a long press only said its name.
    signal chosen

    function showTip(shown, ms) {
        if (rail.sayTip !== null)
            rail.sayTip(rail, shown, ms)
        else if (shown)
            rail.ToolTip.show(rail.tip, ms)
        else
            rail.ToolTip.hide()
    }

    // Its width where its name shows is the window's to give.
    implicitWidth: 44
    implicitHeight: 36
    padding: 0
    text: rail.name
    checkable: rail.place
    checked: rail.current
    Accessible.name: rail.name
    Accessible.description: rail.keys
    // Qt's own tip, where the window gives none: half a second, at once after a long press.
    ToolTip.delay: rail.held ? 0 : 500

    onHoveredChanged: rail.showTip(rail.hovered || rail.visualFocus, -1)
    onVisualFocusChanged: rail.showTip(rail.hovered || rail.visualFocus, -1)
    onPressed: rail.held = false
    onClicked: {
        // A click on the place shown would untick it: it stays as the window says.
        rail.checked = Qt.binding(() => rail.current)
        if (rail.held)
            rail.held = false
        else
            rail.chosen()
    }
    Keys.onReturnPressed: rail.chosen()
    Keys.onEnterPressed: rail.chosen()

    // A touch screen has no pointer to rest: a long press says what it is.
    TapHandler {
        acceptedDevices: PointerDevice.TouchScreen
        onLongPressed: {
            rail.held = true
            rail.showTip(true, 3000)
        }
    }

    background: Rectangle {
        radius: rail.theme.radius
        color: rail.filled ? (rail.down ? Qt.darker(rail.theme.accent, 1.15) : rail.hovered ? Qt.darker(rail.theme.accent, 1.07) : rail.theme.accent) : rail.current ? Qt.rgba(rail.theme.accent.r, rail.theme.accent.g, rail.theme.accent.b, rail.theme.dark ? 0.24 : 0.16) : rail.down ? rail.theme.pressed : rail.hovered ? rail.theme.hover : "transparent"
        border.width: rail.visualFocus ? 2 : 0
        border.color: rail.theme.focus

        // The place shown, by a shape as well as by a colour.
        Rectangle {
            visible: rail.current
            x: 3
            width: 3
            height: 16
            radius: 1.5
            anchors.verticalCenter: parent.verticalCenter
            color: rail.theme.accent
        }
    }

    contentItem: Item {
        IconImage {
            id: glyph

            x: rail.named ? 12 : Math.round((rail.width - 22) / 2)
            anchors.verticalCenter: parent.verticalCenter
            width: 22
            height: 22
            name: rail.iconName
            sourceSize: Qt.size(22, 22)
            // Read on a thread, as every icon of Sioul's (Icon.qml).
            asynchronous: true
            color: rail.filled ? rail.theme.accentText : !rail.enabled ? rail.theme.muted : rail.current ? rail.theme.accent : rail.theme.text
        }
        Text {
            id: label

            x: glyph.x + 22 + 10
            width: Math.max(0, rail.width - x - 6)
            anchors.verticalCenter: parent.verticalCenter
            visible: rail.named
            text: rail.named ? rail.name + rail.after : ""
            textFormat: Text.PlainText
            elide: Text.ElideRight
            font.pixelSize: 16
            color: rail.filled ? rail.theme.accentText : !rail.enabled ? rail.theme.muted : rail.theme.text
        }
    }
}
