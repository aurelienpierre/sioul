// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The one link of the window (docs/qt-quick.md, "The look of each kind of
// thing", rule 7): words that take you to another thing or place (the task
// a step belongs to, the time a task is pinned to, "The whole day"). In the
// accent and underlined, always, since a phone has no pointer to hover with;
// darker (light) or lighter (dark) while pressed; a ring with the keyboard's
// focus, which Tab gives it, and Enter, Return or Space follows it. A screen
// reader hears a link by its words, or by `name`. It says `activated()`; the
// page goes there. Nothing else in running text is underlined or in the
// accent. Its words are plain text: a title from outside stays words.
//
//     Link {
//         Layout.fillWidth: true
//         theme: page.theme
//         text: page.sioul.textWith("task-step-of", "title", parent_title)
//         elide: Text.ElideRight
//         onActivated: page.open(parent)
//     }
//
// A link may hold handlers of its own beside it (a right click's menu); a
// long press says `held`, where a page opens a menu on a touch screen.

import QtQuick
import QtQuick.Controls.Basic

Label {
    id: link

    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme
    // What a screen reader says of it: its words, unless they need more.
    property string name: link.text
    // Held down: by a pointer, a finger, or a key.
    readonly property bool pressed: tap.pressed || link.keyDown

    property bool keyDown: false

    // Followed: by a click, a tap, Enter, Return, Space, or a screen reader.
    signal activated
    // Held down a while (a long press): where, for a menu the page opens there.
    signal held(point at)

    textFormat: Text.PlainText
    wrapMode: Text.Wrap
    color: !link.enabled ? link.theme.muted : link.pressed ? link.theme.focusEdge : link.theme.accent
    font.underline: true
    activeFocusOnTab: link.enabled
    Accessible.role: Accessible.Link
    Accessible.name: link.name
    Accessible.focusable: true
    Accessible.onPressAction: link.activated()
    Keys.onPressed: event => {
        if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
            link.keyDown = true
            event.accepted = true
        }
    }
    Keys.onReleased: event => {
        if (link.keyDown && (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space)) {
            link.keyDown = false
            event.accepted = true
            link.activated()
        }
    }
    onActiveFocusChanged: link.keyDown = false

    HoverHandler {
        enabled: link.enabled
        cursorShape: Qt.PointingHandCursor
    }
    TapHandler {
        id: tap

        enabled: link.enabled
        // The left button, a finger, a pen: a right click is for a menu the page may add.
        acceptedButtons: Qt.LeftButton
        gesturePolicy: TapHandler.ReleaseWithinBounds
        onTapped: link.activated()
        onLongPressed: link.held(tap.point.scenePosition)
    }
    // The keyboard's focus: a ring around its words, the focused fields' colour.
    Rectangle {
        visible: link.activeFocus
        x: -3
        y: -2
        width: Math.min(link.width, link.contentWidth) + 6
        height: link.height + 4
        radius: 3
        color: "transparent"
        border.color: link.theme.focusEdge
        border.width: 2
    }
}
