// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// What the thing open is tied to, both ways: its notes, the mail it came
// from, what it waits for and frees, the people, the project. One row each,
// with how it is tied; a click opens it where it lives, a right click can
// undo the tie. Read again whenever a tie is made or undone.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: related

    // Sioul's backend (backend.rs): its words in your language and what it does. The
    // ties are read there (`related`).
    required property var sioul
    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme
    // The thing open, by its address.
    property string uri: ""
    // Ties shown elsewhere on the card, by how they read ("Step of"…).
    property var leaveOut: []
    // Its heading; "" for none.
    property string title: ""
    // The ties shown: each {uri, how, title, …}, those of the kinds left out
    // (`leaveOut`) taken out.
    property var items: []
    // Rows shown at most; "See them all" opens the chain of ties (ChainPage.qml).
    property int atMost: 12
    // The window that opens the chain, when this list is in Sioul's main window.
    readonly property var shownIn: related.Window.window
    readonly property var chainOpener: related.shownIn !== null && typeof related.shownIn.openChain === "function" ? related.shownIn : null

    // A tie clicked: the thing it names, to open where it lives.
    signal openThing(var item)

    // The ties read again, for `uri`.
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
        textFormat: Text.PlainText
        font.weight: Font.DemiBold
        color: related.theme.text
        Layout.bottomMargin: 2
    }

    Repeater {
        model: related.items.slice(0, related.atMost)

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

            // On a touch screen, the menu comes at a long press; letting go then opens nothing.
            onPressAndHold: {
                thing.Window.window.menuAt = thing.mapToItem(null, thing.pressX, thing.pressY)
                tieMenu.target = thing.modelData
                tieMenu.popup()
            }
            TapHandler {
                acceptedButtons: Qt.RightButton
                // A touch has no buttons: on a touch screen, the row's long press.
                acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
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

    // Every tie, and what those are tied to: the chain of ties from the thing open.
    Button {
        objectName: "relatedSeeAll"
        visible: related.chainOpener !== null && related.uri !== ""
        flat: true
        text: related.sioul.text("chain-see-all")
        onClicked: related.chainOpener.openChain(related.uri)
    }

    // Right click on a tie: open it, its own ties, or undo it.
    SioulMenu {
        id: tieMenu

        property var target: null

        MenuItem {
            enabled: tieMenu.target !== null && tieMenu.target.found
            text: related.sioul.text("ui-open")
            onTriggered: related.openThing(tieMenu.target)
        }
        MenuItem {
            visible: related.chainOpener !== null
            height: visible ? implicitHeight : 0
            enabled: tieMenu.target !== null
            text: related.sioul.text("chain-its-ties")
            onTriggered: related.chainOpener.openChain(tieMenu.target.uri)
        }
        MenuItem {
            text: related.sioul.text("link-undo")
            onTriggered: related.sioul.unlinkThings(related.uri, tieMenu.target.uri)
        }
    }
}
