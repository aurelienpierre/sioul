// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A person's picture, in a soft frame; their initials while it loads, or when
// the card has none. No shader: it shows the same on every graphics card.

import QtQuick
import QtQuick.Controls.Basic

Item {
    id: avatar

    required property var theme
    property string source: ""
    property string name: ""
    property int size: 40
    // A word's first letter as a whole character: an emoji or a rare letter is two UTF-16 halves.
    readonly property string initials: avatar.name.split(/[\s.@_-]+/).filter(w => w !== "").slice(0, 2).map(w => Array.from(w)[0].toUpperCase()).join("")
    readonly property int corner: Math.round(avatar.size / 5)

    implicitWidth: avatar.size
    implicitHeight: avatar.size

    Rectangle {
        anchors.fill: parent
        visible: picture.status !== Image.Ready
        radius: avatar.corner
        color: avatar.theme.button

        Label {
            anchors.centerIn: parent
            text: avatar.initials
            textFormat: Text.PlainText
            font.pixelSize: Math.round(avatar.size * 0.38)
            color: avatar.theme.muted
        }
    }
    Image {
        id: picture

        anchors.fill: parent
        anchors.margins: 1
        source: avatar.source
        fillMode: Image.PreserveAspectCrop
        sourceSize.width: avatar.size * 2
        sourceSize.height: avatar.size * 2
        asynchronous: true
        smooth: true
    }
    // The frame: the picture's corners softened by the page's colour.
    Rectangle {
        anchors.fill: parent
        visible: picture.status === Image.Ready
        color: "transparent"
        radius: avatar.corner
        border.width: 2
        border.color: avatar.theme.surface
    }
}
