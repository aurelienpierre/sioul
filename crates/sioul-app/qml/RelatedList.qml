// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// What the thing open is tied to, both ways: its notes, the mail it came
// from, what it waits for and frees, the people, the case. One row each,
// with how it is tied; a click opens it where it lives, a right click can
// undo the tie. Read again whenever a tie is made or undone.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: related

    required property var sioul
    required property var theme
    // The thing open, by its address.
    property string uri: ""
    // Ties shown elsewhere on the card, by how they read ("Step of"…).
    property var leaveOut: []
    property string title: ""
    property var items: []

    signal openThing(var item)

    function reload() {
        const all = related.uri !== "" ? JSON.parse(related.sioul.related(related.uri) || "[]") : []
        related.items = all.filter(r => related.leaveOut.indexOf(r.how) < 0)
    }

    spacing: 2
    visible: related.items.length > 0
    onUriChanged: related.reload()
    Component.onCompleted: related.reload()

    Connections {
        target: related.sioul

        function onLinksChanged() {
            related.reload()
        }

        function onTasksChanged() {
            related.reload()
        }
    }

    Label {
        visible: related.title !== ""
        text: related.title
        font.weight: Font.DemiBold
        color: related.theme.text
        Layout.bottomMargin: 2
    }

    Repeater {
        model: related.items

        delegate: ItemDelegate {
            id: thing

            required property var modelData

            Layout.fillWidth: true
            padding: 4
            enabled: thing.modelData.found
            onClicked: related.openThing(thing.modelData)
            Accessible.name: thing.modelData.how + ": " + thing.modelData.title

            background: Rectangle {
                color: thing.hovered ? related.theme.surface : "transparent"
                radius: related.theme.radius
                border.color: thing.visualFocus ? related.theme.focus : "transparent"
            }

            TapHandler {
                acceptedButtons: Qt.RightButton
                onTapped: {
                    tieMenu.target = thing.modelData
                    tieMenu.popup()
                }
            }

            contentItem: RowLayout {
                spacing: 8

                Icon {
                    iconName: related.theme.kindIcons[thing.modelData.kind] || "insert-link"
                    size: 16
                    opacity: thing.modelData.found ? 1 : 0.4
                }
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 0

                    // A message's subject, an event's title from an invitation: plain text.
                    Label {
                        Layout.fillWidth: true
                        text: thing.modelData.title
                        textFormat: Text.PlainText
                        elide: Text.ElideRight
                        color: thing.modelData.found ? related.theme.text : related.theme.muted
                    }
                    Label {
                        Layout.fillWidth: true
                        text: [thing.modelData.how, thing.modelData.detail, thing.modelData.when].filter(t => t !== "").join("  ·  ")
                        textFormat: Text.PlainText
                        elide: Text.ElideRight
                        font.pixelSize: 12
                        color: related.theme.muted
                    }
                }
            }
        }
    }

    // Right click on a tie: open it, or undo it.
    SioulMenu {
        id: tieMenu

        property var target: null

        MenuItem {
            enabled: tieMenu.target !== null && tieMenu.target.found
            text: related.sioul.text("ui-open")
            onTriggered: related.openThing(tieMenu.target)
        }
        MenuItem {
            text: related.sioul.text("link-undo")
            onTriggered: related.sioul.unlinkThings(related.uri, tieMenu.target.uri)
        }
    }
}
