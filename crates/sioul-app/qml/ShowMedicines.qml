// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// "Show to a doctor or pharmacist": a prescription's medicines, or all those
// taken now, over the whole window, read-only, to hold out across a counter
// (`health::for_professional`). Large type, one medicine a block: its generic
// name and strength first, then its brand name, its takes with their amounts,
// how long it has been taken, who prescribed it. Nothing to tap by mistake:
// one button, Close; Escape and Back close it too (HealthPage.qml).

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Popup {
    id: view

    required property var sioul
    required property var theme
    // The view (`ForProfessional`): {title, about, medicines, empty}.
    property var shown: ({ title: "", about: [], medicines: [], empty: "" })

    function show(json) {
        view.shown = JSON.parse(json || "{}")
        list.contentY = 0
        view.open()
    }

    parent: Overlay.overlay
    x: 0
    y: 0
    width: parent ? parent.width : 400
    height: parent ? parent.height : 600
    modal: true
    focus: true
    padding: 0
    closePolicy: Popup.CloseOnEscape

    background: Rectangle {
        color: view.theme.surface
    }

    contentItem: ColumnLayout {
        spacing: 0

        Flickable {
            id: list

            Layout.fillWidth: true
            Layout.fillHeight: true
            contentWidth: width
            contentHeight: column.implicitHeight + 2 * view.theme.gap
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar {}

            ColumnLayout {
                id: column

                x: view.theme.gap
                y: view.theme.gap
                width: list.width - 2 * view.theme.gap
                spacing: 18

                Label {
                    Layout.fillWidth: true
                    text: view.shown.title || ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 26
                    font.weight: Font.DemiBold
                    color: view.theme.text
                }
                Repeater {
                    model: view.shown.about || []

                    delegate: Label {
                        required property string modelData

                        Layout.fillWidth: true
                        Layout.topMargin: -12
                        text: modelData
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        font.pixelSize: 19
                        color: view.theme.muted
                    }
                }
                Label {
                    visible: (view.shown.empty || "") !== ""
                    Layout.fillWidth: true
                    text: view.shown.empty || ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 22
                    color: view.theme.text
                }
                // One medicine a block, a line between them.
                Repeater {
                    model: view.shown.medicines || []

                    delegate: ColumnLayout {
                        id: medicine

                        required property var modelData
                        required property int index

                        Layout.fillWidth: true
                        spacing: 4

                        Rectangle {
                            visible: medicine.index > 0
                            Layout.fillWidth: true
                            Layout.bottomMargin: 12
                            implicitHeight: 1
                            color: view.theme.line
                        }
                        // Its generic name and strength, what a professional reads first.
                        Label {
                            Layout.fillWidth: true
                            text: medicine.modelData.molecule
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            font.pixelSize: 28
                            font.weight: Font.DemiBold
                            color: view.theme.text
                        }
                        Label {
                            visible: text !== ""
                            Layout.fillWidth: true
                            text: medicine.modelData.brand
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            font.pixelSize: 21
                            color: view.theme.text
                        }
                        Repeater {
                            model: medicine.modelData.takes

                            delegate: Label {
                                required property string modelData

                                Layout.fillWidth: true
                                text: modelData
                                textFormat: Text.PlainText
                                wrapMode: Text.Wrap
                                font.pixelSize: 21
                                font.features: { "tnum": 1 }
                                color: view.theme.text
                            }
                        }
                        Repeater {
                            model: [medicine.modelData.since, medicine.modelData.prescriber].concat(medicine.modelData.notes).filter(t => t !== "")

                            delegate: Label {
                                required property string modelData

                                Layout.fillWidth: true
                                text: modelData
                                textFormat: Text.PlainText
                                wrapMode: Text.Wrap
                                font.pixelSize: 19
                                color: view.theme.muted
                            }
                        }
                    }
                }
            }
        }
        // The one button, wide, at the bottom: nothing else to tap.
        Rectangle {
            Layout.fillWidth: true
            implicitHeight: close.implicitHeight + 2 * view.theme.gap
            color: view.theme.surface

            Rectangle {
                anchors.top: parent.top
                width: parent.width
                height: 1
                color: view.theme.line
            }
            Button {
                id: close

                anchors.centerIn: parent
                width: Math.min(parent.width - 2 * view.theme.gap, 420)
                font.pixelSize: 19
                text: view.sioul.text("ui-close")
                onClicked: view.close()
            }
        }
    }
}
