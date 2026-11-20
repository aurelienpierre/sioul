// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A part of a device's card in Settings ▸ Devices that folds: the card's
// Details (DevicesTab.qml), and each section of a phone's card inside them
// (PhoneCard.qml). A flat line folds and unfolds it ("▸ Calls", "▾ Calls"),
// at every width, so that a card is a short list of what it holds until one
// is opened; the line is a button for the keyboard (Tab, Space) and for a
// screen reader, which says its name. Under the line, a section's heading
// and frame (Section.qml), or, for the card's Details, what it holds flat on
// the card. What it holds is made once it is opened first (`made`, a
// Loader's `active`) and kept when it folds again: what was typed there
// stays. Unfolded, it stays as it was left until folded by hand.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: fold

    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme
    // The section's name.
    property string title: ""
    // What a screen reader says of its line: its name, unless something more is needed
    // to tell it apart (a card's Details: "Details of Work desk").
    property string name: fold.title
    // It folds; never folding, what it holds shows under its heading.
    property bool folds: true
    // Unfolded.
    property bool open: false
    // In a frame, under its heading (a section of a phone's card); else flat (a card's Details).
    property bool framed: true
    // The size of its line's words.
    property int titleSize: 16
    // Opened once, or never folding: what it holds is made, and kept folded again
    // (a Loader inside, `active: fold.made`). Set by the fold itself.
    property bool made: false
    // What the frame holds, in a column.
    default property alias content: frame.content

    spacing: 4
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
        text: (fold.open ? "▾  " : "▸  ") + fold.title
        font.pixelSize: fold.titleSize
        font.weight: Font.DemiBold
        Accessible.name: fold.name
        Accessible.role: Accessible.Button
        checkable: false
        onClicked: fold.open = !fold.open

        contentItem: Label {
            text: foldLine.text
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            font: foldLine.font
            color: fold.theme.accent
        }
    }
    Section {
        id: frame

        visible: !fold.folds || fold.open
        Layout.fillWidth: true
        theme: fold.theme
        title: fold.folds ? "" : fold.title
        framed: fold.framed
    }
}
