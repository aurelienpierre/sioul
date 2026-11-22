// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The outline of a text note (NotesPage.qml): its headings in order, each
// indented by its level under the highest one, each a flat line that leads
// to its place in the note (`chosen`). A heading's words are the note's,
// words from outside: plain text, on two lines at most, cut with "…", within
// the width the column gives (260 points beside the note), never widening it.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Controls.impl
import QtQuick.Layouts

ColumnLayout {
    id: outline

    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme
    // The headings, as Rust gives them: [{ level, text, line, drawn, block }].
    property var headings: []
    // The heading being read, the one the block at the top of the note is
    // under (NotesPage.qml, `readHeading`): its line tinted as the rail's
    // current place is. -1 for none.
    property int current: -1
    // The highest level among them: its headings are not indented.
    readonly property int highest: outline.headings.reduce((most, h) => Math.min(most, h.level), 6)

    // A heading pressed: the note goes to it.
    signal chosen(var heading)

    spacing: 0

    Repeater {
        model: outline.headings

        delegate: BareButton {
            id: entry

            required property var modelData
            required property int index
            readonly property bool current: entry.index === outline.current

            objectName: "outlineHeading"
            Layout.fillWidth: true
            // The words wrap in the width given; their own width asks for none.
            implicitWidth: entry.leftPadding + entry.rightPadding
            leftPadding: 6 + 14 * Math.min(4, entry.modelData.level - outline.highest)
            topPadding: 4
            bottomPadding: 4
            // Its words drawn as plain text below, never as a button's (which reads "&" and "<").
            Accessible.name: entry.modelData.text
            onClicked: outline.chosen(entry.modelData)

            // BareButton's ground (pressed, the keyboard's ring), and the heading being read tinted.
            background: Rectangle {
                implicitWidth: 100
                implicitHeight: 40
                visible: entry.down || entry.current || entry.visualFocus
                color: entry.down ? Color.blend(entry.palette.button, entry.palette.mid, 0.5) : entry.current ? Qt.rgba(outline.theme.accent.r, outline.theme.accent.g, outline.theme.accent.b, outline.theme.dark ? 0.24 : 0.16) : "transparent"
                radius: entry.current && !entry.down ? outline.theme.radius : 0
                border.color: entry.palette.highlight
                border.width: entry.visualFocus ? 2 : 0
            }

            contentItem: Label {
                text: entry.modelData.text
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                maximumLineCount: 2
                elide: Text.ElideRight
                horizontalAlignment: Text.AlignLeft
                font.weight: entry.modelData.level === outline.highest ? Font.DemiBold : Font.Normal
                color: outline.theme.accent
            }
        }
    }
}
