// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A heading and a frame around a group of fields, for cards and dialogs
// (docs/qt-quick.md, "Sections"): the heading in strong words, in the accent,
// said as a heading to screen readers; under it the group, on a card's surface
// with its thin border, so that the fields on it stand out (Theme.qml's
// `field`). What it holds goes in its column, one under the other. Without
// its frame (`framed`), the group lies flat on what holds it: a device's
// Details on its card (Fold.qml).

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: section

    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme
    // The heading's words, fixed (a translation) or made plain; none: the frame alone.
    property string title: ""
    // An icon before the heading, by its freedesktop name (a card's: the password manager's); none by default.
    property string iconName: ""
    // What the frame holds, in a column.
    default property alias content: body.data
    // The column inside the frame, for its spacing.
    readonly property alias body: body
    // Its frame drawn: a card's surface, its thin border, room inside; else the group alone.
    property bool framed: true

    // Between its heading and its frame: a label's gap (Theme.qml).
    spacing: section.theme.labelGap

    RowLayout {
        visible: section.title !== ""
        Layout.fillWidth: true
        spacing: 8

        Loader {
            active: section.iconName !== ""
            visible: active

            sourceComponent: Component {
                Icon {
                    iconName: section.iconName
                }
            }
        }
        Label {
            Layout.fillWidth: true
            text: section.title
            textFormat: Text.PlainText
            font.pixelSize: 17
            font.weight: Font.DemiBold
            wrapMode: Text.Wrap
            color: section.theme.accent
            Accessible.role: Accessible.Heading
            Accessible.name: section.title
        }
    }
    Pane {
        id: frame

        Layout.fillWidth: true
        padding: section.framed ? section.theme.cardPadding : 0

        background: Rectangle {
            visible: section.framed
            color: section.theme.surface
            radius: section.theme.radius
            border.color: section.theme.line
            border.width: 1
        }

        ColumnLayout {
            id: body

            width: frame.availableWidth
            spacing: section.theme.rowGap
        }
    }
}
