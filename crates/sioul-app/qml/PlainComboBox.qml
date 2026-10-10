// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A ComboBox whose list shows its choices as they are written. Basic's own
// list draws each choice on a button, and a button reads "&" as the mark of
// its shortcut key: "R&D" showed "RD", "Tom & Jerry" lost its "&" (the review
// of 5 October 2026). Each "&" is doubled for the list alone ("&&" is one);
// the closed box is a text field, which shows the text as it is. Every
// ComboBox of the window is one of these; one that brings its own delegate
// doubles the "&" itself (TaskPanel's list of task lists).

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic

ComboBox {
    id: box

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
