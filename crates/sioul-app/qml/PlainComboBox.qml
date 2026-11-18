// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A ComboBox whose list shows its choices as they are written. Basic's own
// list draws each choice on a button, and a button reads "&" as the mark of
// its shortcut key: "R&D" showed "RD", "Tom & Jerry" lost its "&" (the review
// of 5 October 2026). Each "&" is doubled for the list alone ("&&" is one);
// the closed box is a text field, which shows the text as it is. Every
// ComboBox of the window is one of these; one that brings its own delegate
// doubles the "&" itself (TaskPanel's list of task lists). Drawn as a field,
// not as a button: the theme's `field` and its firmer `edge`, as every text
// field (qml/style/StyleTextField.qml); Basic's size.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic

ComboBox {
    id: box

    // Its list kept inside the window's safe area, never under the system's
    // bars, 6 points within it as Basic's own (review of 4c77d0c, L9).
    readonly property var safeArea: box.Overlay.overlay ? box.Overlay.overlay.SafeArea.margins : null

    popup.topMargin: 6 + (box.safeArea ? box.safeArea.top : 0)
    popup.bottomMargin: 6 + (box.safeArea ? box.safeArea.bottom : 0)
    popup.leftMargin: box.safeArea ? box.safeArea.left : 0
    popup.rightMargin: box.safeArea ? box.safeArea.right : 0

    // Basic's background, as a field: the palette's base (the theme's `field`,
    // SioulWindow.qml) and the window's theme's `edge`; pressed, Basic's mid tone.
    background: Rectangle {
        // The window's theme (SioulWindow.qml); a window of Qt's own has none: its palette's border then.
        readonly property var shownIn: box.Window.window
        readonly property var theme: shownIn ? shownIn.theme : undefined

        implicitWidth: 140
        implicitHeight: 40
        visible: !box.flat || box.down
        color: box.down ? box.palette.mid : box.palette.base
        border.color: box.visualFocus ? box.palette.highlight : theme ? theme.edge : box.palette.mid
        border.width: box.visualFocus ? 2 : 1
    }

    // Basic's delegate, but for the "&" doubled.
    delegate: ItemDelegate {
        id: choice

        required property var model
        required property int index

        width: ListView.view.width
        // Theme's `plain` too, written out (a word joiner after each "<"): a
        // button guesses HTML from a "<", and the choices may be words from outside.
        text: String(choice.model[box.textRole] ?? "").replace(/&/g, "&&").replace(/</g, "<\u2060")
        palette.text: box.palette.text
        palette.highlightedText: box.palette.highlightedText
        font.weight: box.currentIndex === choice.index ? Font.DemiBold : Font.Normal
        highlighted: box.highlightedIndex === choice.index
        hoverEnabled: box.hoverEnabled
    }
}
