// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The one fold of the window (docs/qt-quick.md, "The look of each kind of
// thing", rule 8): a line with its arrow before its words, "▸ More details"
// folded and "▾ More details" unfolded, the whole line reacting to a click,
// a tap, Space, Enter, or the Right and Left arrows; a screen reader hears a
// button by its name (`name`), checked while it is unfolded. Under the line,
// what it holds, one step in (the theme's `indent`, under its words), or in
// a section's frame (`framed`: a phone's card in Settings ▸ Devices), whose
// heading the line then is, in the heading's look. What it holds is made the
// first time it unfolds (a Loader inside: a page full of folds costs nothing
// until one opens) and kept when it folds again, so that what was typed there
// stays. Never folding (`folds: false`), it shows what it holds under its
// words, made at once.
//
//     Fold {
//         Layout.fillWidth: true
//         theme: page.theme
//         title: page.sioul.text("ui-reasons")
//
//         ColumnLayout { … }   // made when first unfolded
//     }
//
// What it holds is one item (a column for several), written inside it: it is
// a Component, so its ids are seen inside it only; the page reaches it
// through `item` once it is made. A page that keeps whether it is unfolded
// (to fold it again for the next thing shown) either reaches `open` (an
// alias to it), or, when the fold is inside a component of its own, sets
// `controlled`, binds `open` and follows `toggled`.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: fold

    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme
    // Its words, fixed (a translation) or made plain.
    property string title: ""
    // What a screen reader says of its line: its words, unless something more is
    // needed to tell it apart (a card's Details: "Details of Work desk").
    property string name: fold.title
    // It folds; never folding, what it holds shows under its words.
    property bool folds: true
    // Unfolded.
    property bool open: false
    // Its state is the page's: the page binds `open` and follows `toggled`, and the
    // fold never sets `open` itself (a page that folds it again from elsewhere).
    property bool controlled: false
    // What it holds in a section's frame, the line as that section's heading;
    // else flat, one step in, under the line's words.
    property bool framed: false
    // The size of its line's words; 0: the heading's size when framed, else the window's own.
    property int titleSize: 0
    // Opened once, or never folding: what it holds is made, and kept folded again.
    // Set by the fold itself.
    property bool made: false
    // What it holds, made the first time it unfolds.
    default property Component body
    // What it holds once made, else null.
    readonly property alias item: holder.item
    // Its line, for a page that gives it the focus.
    readonly property alias line: foldLine

    // Its line pressed (or its keys): unfolded or folded, as asked.
    signal toggled(bool open)

    // Unfolded (true) or folded, as its line asks.
    function want(open: bool) {
        if (open === fold.open)
            return
        if (!fold.controlled)
            fold.open = open
        fold.toggled(open)
    }

    spacing: fold.framed ? fold.theme.labelGap : 0
    onOpenChanged: {
        if (fold.open)
            fold.made = true
    }
    onFoldsChanged: {
        if (!fold.folds)
            fold.made = true
    }
    Component.onCompleted: {
        if (fold.open || !fold.folds)
            fold.made = true
    }

    BareButton {
        id: foldLine

        objectName: "foldLine"
        visible: fold.folds
        Layout.fillWidth: true
        leftPadding: 4
        // Its words and arrow, as tests and screen readers read them; drawn below.
        text: (fold.open ? "▾  " : "▸  ") + fold.title
        font.pixelSize: fold.titleSize > 0 ? fold.titleSize : fold.framed ? 17 : foldLine.Window.window && foldLine.Window.window.font ? foldLine.Window.window.font.pixelSize : 15
        font.weight: Font.DemiBold
        Accessible.name: fold.name
        Accessible.role: Accessible.Button
        Accessible.checkable: true
        Accessible.checked: fold.open
        Accessible.onPressAction: fold.want(!fold.open)
        onClicked: fold.want(!fold.open)
        Keys.onReturnPressed: fold.want(!fold.open)
        Keys.onEnterPressed: fold.want(!fold.open)
        Keys.onRightPressed: fold.want(true)
        Keys.onLeftPressed: fold.want(false)

        // The arrow in a column of its own, so that the words do not move as it turns;
        // the words where what it holds begins, one step in.
        contentItem: RowLayout {
            spacing: 0

            Label {
                Layout.preferredWidth: fold.theme.indent - foldLine.leftPadding
                Layout.alignment: Qt.AlignTop
                text: fold.open ? "▾" : "▸"
                font: foldLine.font
                color: words.color
            }
            Label {
                id: words

                Layout.fillWidth: true
                text: fold.title
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font: foldLine.font
                // A section's heading in the accent; any other fold in the words' colour, never
                // a link's (rule 7): the arrow and the whole line's tint under the pointer say it folds.
                color: !foldLine.enabled ? fold.theme.muted : fold.framed ? fold.theme.accent : fold.theme.text
            }
        }
    }
    Section {
        visible: !fold.folds || fold.open
        Layout.fillWidth: true
        Layout.leftMargin: fold.framed ? 0 : fold.theme.indent
        theme: fold.theme
        title: fold.folds ? "" : fold.title
        framed: fold.framed

        Loader {
            id: holder

            objectName: "foldBody"
            active: fold.made
            visible: holder.status === Loader.Ready
            Layout.fillWidth: true
            sourceComponent: fold.body
        }
    }
}
