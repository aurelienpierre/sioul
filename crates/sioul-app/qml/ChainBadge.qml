// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A thing's family as a shape, with its kind's icon inside (the chain of
// ties, ChainNode.qml and ChainOutline.qml): time and doing a rounded
// square, messages a wide pill, people a circle, money a diamond, records a
// square with a folded corner, projects a hexagon. Each tinted with its
// family's colour, its outline in that colour at 3:1 against the page
// (Theme.qml's `familyColours`): the shape says the family without colour.

import QtQuick
import QtQuick.Shapes

Item {
    id: badge

    required property var theme
    // "time", "messages", "people", "money", "records", "projects" (`chain::family`).
    required property string family
    required property string kind

    readonly property color colour: badge.theme.familyColours[badge.family] || badge.theme.muted

    implicitWidth: 30
    implicitHeight: 30

    // Time and doing, messages, people: a rectangle of their own corners.
    Rectangle {
        visible: badge.family === "time" || badge.family === "messages" || badge.family === "people"
        anchors.centerIn: parent
        width: badge.family === "messages" ? 30 : 26
        height: badge.family === "messages" ? 20 : 26
        radius: badge.family === "time" ? 6 : height / 2
        color: Qt.alpha(badge.colour, 0.16)
        border.width: 1.5
        border.color: badge.colour
    }
    // Money: a diamond.
    Rectangle {
        visible: badge.family === "money"
        anchors.centerIn: parent
        width: 20
        height: 20
        rotation: 45
        color: Qt.alpha(badge.colour, 0.16)
        border.width: 1.5
        border.color: badge.colour
    }
    // Records: a page with its corner folded; projects: a hexagon.
    Shape {
        visible: badge.family === "records" || badge.family === "projects"
        anchors.fill: parent
        preferredRendererType: Shape.CurveRenderer

        ShapePath {
            strokeWidth: 1.5
            strokeColor: badge.colour
            fillColor: Qt.alpha(badge.colour, 0.16)
            joinStyle: ShapePath.RoundJoin

            PathSvg {
                path: badge.family === "projects" ? "M 8 3 L 22 3 L 29 15 L 22 27 L 8 27 L 1 15 Z" : "M 4 2 L 19 2 L 26 9 L 26 28 L 4 28 Z M 19 2 L 19 9 L 26 9"
            }
        }
    }
    Icon {
        anchors.centerIn: parent
        iconName: badge.theme.kindIcons[badge.kind] || "insert-link"
        size: 16
    }
}
