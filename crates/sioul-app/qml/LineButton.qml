// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A button of the status line (StatusLine.qml), its applets' too: its icon,
// and its words beside it where there is room; `compact` (a phone, a window
// as narrow), its icon alone, as wide as the icon and an even margin on each
// side. Its name is said to screen readers, when the pointer rests on it, and
// at a long press on a touch screen, which then does nothing else (unless
// `tipOnHold` is false: there, as for do-not-disturb and Free time, a long
// press opens a menu). Flat: a tint under the pointer, the accent's while it
// is on, a ring for the keyboard's focus; Undo, which waits ten seconds, is
// drawn as a button at rest too (`raised`).

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ToolButton {
    id: button

    required property var theme
    // Its icon alone.
    property bool compact: false
    // What it is, in words: said to screen readers, and its tip unless `tip` says more.
    property string name: button.text
    property string tip: button.name
    // On (Free time, do-not-disturb): its icon in the accent, the accent's tint under it.
    property bool switchedOn: false
    // Drawn as a button at rest too.
    property bool raised: false
    // Its words' colour.
    property color textColor: button.theme.text
    // Its own menu or pop-up is open: no tip over it.
    property bool menuOpen: false
    // A long press says its name; false where a long press opens a menu.
    property bool tipOnHold: true
    // Its name said after a long press, for three seconds.
    property bool saying: false
    // A long press said its name, or opened its menu: letting go does nothing.
    property bool held: false

    // Clicked, or Space or Enter, unless a long press said its name or opened its menu.
    signal chosen

    // As wide as what it shows: the style's tool buttons are 40 pixels at least,
    // their icon on the left, the rest empty.
    implicitWidth: button.implicitContentWidth + button.leftPadding + button.rightPadding
    implicitHeight: 28
    // Whole on a full line: the sentences around it give way first.
    Layout.minimumWidth: button.implicitWidth
    leftPadding: 6
    rightPadding: 6
    topPadding: 4
    bottomPadding: 4
    spacing: 6
    icon.width: 16
    icon.height: 16
    icon.color: button.switchedOn ? button.theme.accent : button.theme.muted
    display: button.compact || button.text === "" ? AbstractButton.IconOnly : AbstractButton.TextBesideIcon
    Accessible.name: button.name
    Accessible.description: button.tip !== button.name ? button.tip : ""
    ToolTip.visible: (button.hovered || button.saying) && button.tip !== "" && !button.menuOpen
    ToolTip.text: button.theme.plain(button.tip)
    ToolTip.delay: button.saying ? 0 : 600

    onPressed: button.held = false
    onClicked: {
        if (button.held)
            button.held = false
        else
            button.chosen()
    }
    Keys.onReturnPressed: button.chosen()
    Keys.onEnterPressed: button.chosen()

    // A touch screen has no pointer to rest: a long press says what it is.
    TapHandler {
        enabled: button.tipOnHold
        acceptedDevices: PointerDevice.TouchScreen
        onLongPressed: {
            button.held = true
            button.saying = true
            sayingFor.restart()
        }
    }
    Timer {
        id: sayingFor

        interval: 3000
        onTriggered: button.saying = false
    }

    // Its icon, then its words, cut short at the end when the line is full,
    // never drawn past the button: Qt's own IconLabel centres them and lets
    // them run over its neighbours, and a layout inside kept their width from
    // before the button was narrowed (both seen in the title bar). Placed by
    // bindings alone.
    contentItem: Item {
        id: content

        readonly property bool iconShown: button.display !== AbstractButton.TextOnly && button.icon.name !== ""
        readonly property bool wordsShown: button.display !== AbstractButton.IconOnly && button.text !== ""

        implicitWidth: (iconShown ? 16 : 0) + (iconShown && wordsShown ? button.spacing : 0) + (wordsShown ? words.implicitWidth : 0)
        implicitHeight: Math.max(16, wordsShown ? words.implicitHeight : 0)
        clip: true

        Icon {
            visible: content.iconShown
            width: 16
            height: 16
            anchors.verticalCenter: parent.verticalCenter
            iconName: button.icon.name
            color: button.icon.color
            size: 16
        }
        Label {
            id: words

            visible: content.wordsShown
            x: content.iconShown ? 16 + button.spacing : 0
            width: Math.max(0, content.width - words.x)
            anchors.verticalCenter: parent.verticalCenter
            text: button.text
            // Words from the backend, a server, a place's name: never read as rich text.
            textFormat: Text.PlainText
            elide: Text.ElideRight
            font: button.font
            color: button.textColor
        }
    }

    background: Rectangle {
        radius: button.theme.radius
        color: button.down ? button.theme.pressed : button.switchedOn ? Qt.rgba(button.theme.accent.r, button.theme.accent.g, button.theme.accent.b, button.theme.dark ? 0.24 : 0.16) : button.hovered ? button.theme.hover : button.raised ? button.theme.button : "transparent"
        border.width: button.visualFocus ? 2 : 0
        border.color: button.theme.focus
    }
}
