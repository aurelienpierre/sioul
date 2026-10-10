// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A page that waits (quiet time, or while you sleep): one line over it,
// what may still be done then (a thought noted for later), and the page
// itself if you ask (docs/areas.md).

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Rectangle {
    id: cover

    // Sioul's backend (backend.rs): its words in your language and what it does.
    required property var sioul
    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme
    // The one line said over the page ("Sleep: nothing disturbs until 07:00.").
    required property string line
    // Shown under the line: a field to note a thought for later, say.
    default property alias content: more.data

    // "Show anyway" was pressed: the page is shown all the same.
    signal shown

    anchors.fill: parent
    color: cover.theme.background

    // Nothing under it takes a click.
    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.AllButtons
    }
    ColumnLayout {
        anchors.centerIn: parent
        width: Math.min(parent.width - 2 * cover.theme.gap, 480)
        spacing: 10

        Label {
            Layout.fillWidth: true
            text: cover.line
            textFormat: Text.PlainText
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.Wrap
            color: cover.theme.muted
        }
        ColumnLayout {
            id: more

            visible: more.children.length > 0
            Layout.fillWidth: true
            spacing: 6
        }
        Button {
            Layout.alignment: Qt.AlignHCenter
            flat: true
            text: cover.sioul.text("ui-show-anyway")
            onClicked: cover.shown()
        }
    }
}
