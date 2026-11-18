// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Android's restricted settings, for Sioul installed from a file
// (docs/android.md, "What Android withholds"; `access::restricted`), in
// this phone's card in Settings ▸ Devices: three steps, each a button, each
// ticked when its outcome is seen. 1. Let Android show its notice (from
// Android 15 the texts' question, before the page where Sioul's greyed
// switch shows it); 2. open Sioul's page in Android, with a drawing of where
// "Allow restricted settings" is (⋮, at the top right), drawn here, never a
// picture of a screen; 3. back in Sioul, each access asked again by itself.
// Sioul cannot see the setting itself, only whether each access works: the
// card follows that, and says so.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: card

    // Sioul's backend (backend.rs): its words in your language.
    required property var sioul
    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme
    // What the core says: {intro, steps: [{step, said, button, done}], works, cannot}.
    property var view: ({ steps: [] })

    // A step's button pressed: "notice", "info", "back".
    signal step(string id)

    spacing: 10

    Label {
        Layout.fillWidth: true
        text: card.view.intro || ""
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: card.theme.text
    }

    Repeater {
        model: card.view.steps || []

        delegate: ColumnLayout {
            id: stepItem

            required property var modelData
            required property int index
            readonly property bool done: stepItem.modelData.done === true

            objectName: "restrictedStep-" + stepItem.modelData.step
            Layout.fillWidth: true
            spacing: 6

            RowLayout {
                Layout.fillWidth: true
                spacing: 10

                // Its number, ticked once its outcome is seen.
                Rectangle {
                    Layout.alignment: Qt.AlignTop
                    implicitWidth: 28
                    implicitHeight: 28
                    radius: 14
                    color: stepItem.done ? card.theme.accent : "transparent"
                    border.width: 1
                    border.color: stepItem.done ? card.theme.accent : card.theme.edge
                    Accessible.ignored: true

                    Label {
                        anchors.centerIn: parent
                        text: stepItem.done ? "✓" : String(stepItem.index + 1)
                        font.weight: Font.DemiBold
                        color: stepItem.done ? card.theme.surface : card.theme.text
                    }
                }
                Label {
                    objectName: "restrictedSaid"
                    Layout.fillWidth: true
                    text: stepItem.modelData.said
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: card.theme.text
                    Accessible.name: (stepItem.index + 1) + ". " + stepItem.modelData.said
                }
            }
            // Where "Allow restricted settings" is: Sioul's page in Android, its ⋮ at the top right open.
            Item {
                visible: stepItem.modelData.step === "info"
                Layout.leftMargin: 38
                implicitWidth: Math.min(300, card.width - 38)
                implicitHeight: 150
                Accessible.ignored: true

                Rectangle {
                    id: screen

                    anchors.fill: parent
                    radius: 10
                    color: card.theme.field
                    border.width: 1
                    border.color: card.theme.edge

                    // The page's bar: back, its title, ⋮.
                    Label {
                        x: 12
                        y: 12
                        text: "←"
                        font.pixelSize: 16
                        color: card.theme.muted
                    }
                    Label {
                        x: 40
                        y: 13
                        width: parent.width - 90
                        text: card.sioul.text("restricted-picture-title")
                        textFormat: Text.PlainText
                        elide: Text.ElideRight
                        font.pixelSize: 14
                        color: card.theme.text
                    }
                    Rectangle {
                        id: dots

                        x: parent.width - 38
                        y: 6
                        width: 30
                        height: 30
                        radius: 15
                        color: "transparent"
                        border.width: 2
                        border.color: card.theme.accent

                        Label {
                            anchors.centerIn: parent
                            text: "⋮"
                            font.pixelSize: 18
                            font.weight: Font.Bold
                            color: card.theme.accent
                        }
                    }
                    // The page itself, greyed lines.
                    Repeater {
                        model: 3

                        delegate: Rectangle {
                            required property int index

                            x: 14
                            y: 52 + index * 26
                            width: screen.width * (0.55 - index * 0.1)
                            height: 8
                            radius: 4
                            color: card.theme.line
                        }
                    }
                    // The menu ⋮ opens, its entry marked.
                    Rectangle {
                        x: parent.width - width - 10
                        y: 40
                        width: Math.min(220, parent.width - 40)
                        height: menuWords.implicitHeight + 20
                        radius: 6
                        color: card.theme.surface
                        border.width: 1
                        border.color: card.theme.edge

                        Rectangle {
                            anchors.fill: parent
                            anchors.margins: 4
                            radius: 4
                            color: "transparent"
                            border.width: 2
                            border.color: card.theme.accent
                        }
                        Label {
                            id: menuWords

                            anchors.verticalCenter: parent.verticalCenter
                            x: 12
                            width: parent.width - 24
                            text: card.sioul.text("restricted-picture-allow")
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            font.pixelSize: 13
                            color: card.theme.text
                        }
                    }
                }
            }
            Button {
                objectName: "restrictedButton"
                Layout.leftMargin: 38
                flat: stepItem.done
                text: card.theme.plain(stepItem.modelData.button)
                onClicked: card.step(stepItem.modelData.step)
            }
        }
    }
    Label {
        Layout.fillWidth: true
        text: card.view.cannot || ""
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.pixelSize: 13
        color: card.theme.muted
    }
}
