// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// One event in a day: its calendar's colour, its time, its title and place.
// A cancelled one is struck through; a tentative one is in grey.

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ItemDelegate {
    id: row

    required property var event
    required property var theme
    property bool compact: false
    property bool selected: false

    signal open
    signal menu

    padding: row.compact ? 4 : 8
    highlighted: row.selected
    onClicked: row.open()
    Keys.onReturnPressed: row.open()

    background: Rectangle {
        color: row.highlighted || row.hovered ? row.theme.surface : "transparent"
        radius: row.theme.radius
        border.color: row.visualFocus ? row.theme.focus : row.highlighted ? row.theme.line : "transparent"
    }

    // On a touch screen, the menu comes at a long press; letting go then opens nothing.
    onPressAndHold: {
        row.Window.window.menuAt = row.mapToItem(null, row.pressX, row.pressY)
        row.menu()
    }
    TapHandler {
        acceptedButtons: Qt.RightButton
        // A touch has no buttons: on a touch screen, the row's long press.
        acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
        onTapped: row.menu()
    }

    contentItem: GridLayout {
        columns: row.compact ? 1 : 3
        columnSpacing: 10
        rowSpacing: 1

        RowLayout {
            spacing: 6

            Rectangle {
                Layout.preferredWidth: 8
                Layout.preferredHeight: 8
                radius: 4
                color: row.event.color ? row.event.color : row.theme.accent
            }
            Label {
                Layout.preferredWidth: row.compact ? -1 : 110
                text: row.event.when
                textFormat: Text.PlainText
                color: row.theme.muted
                font.pixelSize: row.compact ? 12 : 14
            }
        }
        Label {
            Layout.fillWidth: true
            text: row.event.summary
            textFormat: Text.PlainText
            elide: Text.ElideRight
            font.strikeout: row.event.cancelled
            font.pixelSize: row.compact ? 13 : 15
            color: row.event.tentative ? row.theme.muted : row.theme.text
        }
        Label {
            visible: !row.compact && row.event.location !== ""
            Layout.maximumWidth: 220
            text: row.event.location
            textFormat: Text.PlainText
            elide: Text.ElideRight
            color: row.theme.muted
        }
    }
}
