// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A section of a device's card in Settings ▸ Devices (PhoneCard.qml): on a
// computer a heading and its frame (Section.qml), open; on a phone (412
// points) a line that folds and unfolds it ("▸ Calls", "▾ Calls"), so that a
// phone's card is a short list of what it holds until one is opened.

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
    // A phone's width: the section folds.
    property bool folds: false
    // Unfolded, on a phone; always on a computer.
    property bool open: false
    // What the frame holds, in a column.
    default property alias content: frame.content

    spacing: 4

    BareButton {
        id: foldLine

        objectName: "foldLine"
        visible: fold.folds
        Layout.fillWidth: true
        text: (fold.open ? "▾  " : "▸  ") + fold.title
        font.pixelSize: 16
        font.weight: Font.DemiBold
        Accessible.name: fold.title
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
    }
}
