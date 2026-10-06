// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The window's edges, on a computer where Sioul draws its own title bar
// (main.qml, TitleBar.qml): pressed and dragged, an edge or a corner resizes
// the window as the system's borders would, the system doing the resizing
// (snapping, tiling); the pointer takes the resizing shape there. Six pixels
// inside each edge, the corners sixteen along each; nothing while the window
// is maximized or full screen, which have no edge to pull.

pragma ComponentBehavior: Bound

import QtQuick

Item {
    id: edges

    required property var window
    // How far in from each edge a press resizes, and how far along each edge a corner reaches.
    readonly property int grip: 6
    readonly property int corner: 16

    // One edge, its corners at its ends.
    component Edge: MouseArea {
        id: edge

        // The edge it pulls: Qt.TopEdge, Qt.BottomEdge, Qt.LeftEdge or Qt.RightEdge.
        required property int side
        readonly property bool across: edge.side === Qt.TopEdge || edge.side === Qt.BottomEdge
        // Where along the edge the pointer is: its start, its end, or between.
        readonly property real along: edge.across ? edge.mouseX : edge.mouseY
        readonly property real length: edge.across ? edge.width : edge.height

        // The edges a press there pulls: a corner's two near its ends.
        function sides(at: real): int {
            if (at < edges.corner)
                return edge.side | (edge.across ? Qt.LeftEdge : Qt.TopEdge)
            if (at > edge.length - edges.corner)
                return edge.side | (edge.across ? Qt.RightEdge : Qt.BottomEdge)
            return edge.side
        }

        hoverEnabled: true
        acceptedButtons: Qt.LeftButton
        cursorShape: {
            const pulled = edge.sides(edge.along)
            if (pulled === (Qt.TopEdge | Qt.LeftEdge) || pulled === (Qt.BottomEdge | Qt.RightEdge))
                return Qt.SizeFDiagCursor
            if (pulled === (Qt.TopEdge | Qt.RightEdge) || pulled === (Qt.BottomEdge | Qt.LeftEdge))
                return Qt.SizeBDiagCursor
            return edge.across ? Qt.SizeVerCursor : Qt.SizeHorCursor
        }
        onPressed: mouse => edges.window.startSystemResize(edge.sides(edge.across ? mouse.x : mouse.y))
    }

    Edge {
        side: Qt.TopEdge
        width: edges.width
        height: edges.grip
    }
    Edge {
        side: Qt.BottomEdge
        y: edges.height - edges.grip
        width: edges.width
        height: edges.grip
    }
    Edge {
        side: Qt.LeftEdge
        width: edges.grip
        height: edges.height
    }
    Edge {
        side: Qt.RightEdge
        x: edges.width - edges.grip
        width: edges.grip
        height: edges.height
    }
}
