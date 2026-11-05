// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// One thing of the chain of ties (ChainPage.qml), as the drawing shows it:
// its family's shape with its kind's icon, its title and a line beneath
// (its kind, its date or detail), and "+N" for what was left out under it
// ("−" once opened). The entry point has a double outline; a thing not
// found is dimmed. Its family is said by its shape and its colour, its kind
// by its icon and in words: never by colour alone.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

FocusScope {
    id: node

    required property var sioul
    required property var theme
    // One row of the chain (`chain::ChainNode`, flattened by ChainPage.qml).
    required property var thing
    // Its place in the outline's order, which the keys follow.
    required property int place
    property bool current: false
    property bool entry: false
    // The tie's words on its second line: where a column is too narrow for them on the edge (a phone).
    property bool howOnLine: false
    // The row, or an empty one while a removed row's item goes.
    readonly property var t: node.thing ? node.thing : ({ uri: "", kind: "other", family: "records", kindWords: "", howWords: "", title: "", detail: "", date: "", found: true, more: 0, moreWords: "", opened: 0, name: "" })

    signal chosen(int place)
    signal opened(int place)
    signal plus(int place)
    signal minus(int place)
    signal menu(int place, var at)

    implicitWidth: node.theme.touch ? 200 : 220
    implicitHeight: 44
    opacity: node.t.found ? 1 : 0.5

    Accessible.role: Accessible.Button
    Accessible.name: node.t.name
    Accessible.description: node.t.more > 0 ? node.sioul.text("chain-folded") : node.t.opened > 0 ? node.sioul.text("chain-unfolded") : ""
    Accessible.onPressAction: node.opened(node.place)

    // The entry point's second outline, outside the first.
    Rectangle {
        visible: node.entry
        anchors.fill: parent
        anchors.margins: -4
        radius: node.theme.radius + 3
        color: "transparent"
        border.width: 1
        border.color: node.theme.familyColours[node.t.family] || node.theme.muted
    }
    Rectangle {
        id: box

        anchors.fill: parent
        radius: node.theme.radius
        color: hover.hovered ? node.theme.hover : node.theme.surface
        border.width: node.current ? 2 : 1
        border.color: node.current ? node.theme.focus : (node.theme.familyColours[node.t.family] || node.theme.line)
    }

    HoverHandler {
        id: hover
    }
    TapHandler {
        acceptedButtons: Qt.LeftButton
        onTapped: {
            node.chosen(node.place)
            node.opened(node.place)
        }
        onLongPressed: {
            node.chosen(node.place)
            node.menu(node.place, node.mapToItem(null, point.position.x, point.position.y))
        }
    }
    TapHandler {
        acceptedButtons: Qt.RightButton
        acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
        onTapped: {
            node.chosen(node.place)
            node.menu(node.place, null)
        }
    }

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: 6
        anchors.rightMargin: 4
        spacing: 6

        ChainBadge {
            theme: node.theme
            family: node.t.family
            kind: node.t.kind
        }
        ColumnLayout {
            Layout.fillWidth: true
            Layout.preferredWidth: 0
            spacing: 0

            // A message's subject, a contact's name: plain text.
            Label {
                Layout.fillWidth: true
                text: node.t.title
                textFormat: Text.PlainText
                elide: Text.ElideRight
                font.weight: node.entry ? Font.DemiBold : Font.Normal
                color: node.theme.text
            }
            Label {
                Layout.fillWidth: true
                text: [node.howOnLine ? node.t.howWords || "" : "", node.t.kindWords, node.t.date !== "" ? node.t.date : node.t.detail].filter(t => t !== "").join(" · ")
                textFormat: Text.PlainText
                elide: Text.ElideRight
                font.pixelSize: 11
                color: node.theme.muted
            }
        }
        // "+N" for what is left out under it; "−" folds back what "+" opened.
        ToolButton {
            visible: node.t.more > 0 || node.t.opened > 0
            Layout.preferredWidth: Math.max(node.theme.touch ? 48 : 36, implicitWidth)
            Layout.fillHeight: true
            flat: true
            focusPolicy: Qt.NoFocus
            text: node.theme.plain(node.t.more > 0 ? "+" + node.t.more : "−")
            Accessible.name: node.t.more > 0 ? node.t.moreWords : node.sioul.text("chain-minus")
            onClicked: node.t.more > 0 ? node.plus(node.place) : node.minus(node.place)
            ToolTip.visible: hovered && node.t.more > 0
            ToolTip.text: node.theme.plain(node.t.moreWords)
        }
    }
}
