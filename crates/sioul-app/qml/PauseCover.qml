// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The pause's screen (docs/pauses.md, P11–P17): over Sioul's whole window,
// Sioul's content only (on a phone the system stays free: calls, other apps,
// emergency dialling). A few literal lines about Sioul, never about you;
// your own list of what helps, folded; your line and the breathing guide if
// you set them; the emergency number and the crisis line of your country,
// one tap each; one button to come back, nothing asked. Nothing moves unless
// you start it: no count, no countdown, no question. Coming back, the same
// calm ground says it in a few words, with the one offer, then goes.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Rectangle {
    id: cover

    required property var sioul
    required property var theme
    // A try-out from the setup: the screen as it will be, nothing held.
    property bool trial: false
    // After "Come back": the return's words ({title, day, tomorrow, lighten, go}), else null.
    property var back: null
    // Read again each time the cover shows: the setup may have changed.
    property var screen: ({ doses: false, helps: [], breathing: false, pace: 6, grounding: "", numbers: [], more: [], country: "" })
    property bool listOpen: false
    property bool numbersOpen: false
    property bool lightened: false
    // The breathing guide moving (its button, a tap on it).
    property alias guideRunning: guide.running

    // "Come back" pressed: the cover stays for the return's words.
    signal coming
    // The try-out ended, or the return was read.
    signal done

    function reload() {
        cover.screen = JSON.parse(cover.sioul.pauseScreen() || "null") || cover.screen
        cover.listOpen = false
        cover.numbersOpen = false
        cover.lightened = false
        guide.running = false
    }

    // Back from the pause: the return's words, the same ground under them.
    function comeBack() {
        if (cover.trial) {
            cover.done()
            return
        }
        guide.running = false
        cover.coming()
        cover.back = JSON.parse(cover.sioul.comeBack() || "null")
        if (cover.back === null)
            cover.done()
    }

    color: cover.theme.background
    focus: true
    Component.onCompleted: {
        cover.reload()
        cover.forceActiveFocus()
    }
    onVisibleChanged: {
        if (visible) {
            cover.reload()
            cover.forceActiveFocus()
        }
    }

    // Nothing under it takes a click, a wheel or a finger.
    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.AllButtons
        onWheel: wheel => wheel.accepted = true
    }

    Flickable {
        id: flick

        anchors.fill: parent
        contentWidth: width
        contentHeight: Math.max(height, column.implicitHeight + 2 * cover.theme.gap)
        clip: true
        boundsBehavior: Flickable.StopAtBounds

        ColumnLayout {
            id: column

            // In the middle of the window, its lines never wider than a reader's.
            width: Math.min(flick.width - 2 * cover.theme.gap, 520)
            x: (flick.width - width) / 2
            y: Math.max(cover.theme.gap, (flick.height - implicitHeight) / 2)
            spacing: 14

            // The try-out says it is one.
            Label {
                visible: cover.trial
                Layout.fillWidth: true
                text: cover.sioul.text("pause-try")
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                horizontalAlignment: Text.AlignHCenter
                color: cover.theme.muted
            }

            // ---- During the pause.
            Label {
                visible: cover.back === null
                Layout.fillWidth: true
                text: cover.sioul.text("pause-title")
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 28
                color: cover.theme.text
            }
            Label {
                visible: cover.back === null
                Layout.fillWidth: true
                text: cover.sioul.text("pause-text")
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 18
                lineHeight: 1.3
                color: cover.theme.text
            }
            Label {
                visible: cover.back === null && cover.screen.doses
                Layout.fillWidth: true
                text: cover.sioul.text("pause-doses-come")
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: cover.theme.muted
            }
            // Your own line, in your words.
            Label {
                visible: cover.back === null && cover.screen.grounding !== ""
                Layout.fillWidth: true
                text: cover.screen.grounding
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 18
                color: cover.theme.text
            }

            // What helps you, your list, folded until you open it.
            Button {
                visible: cover.back === null && cover.screen.helps.length > 0
                flat: true
                text: cover.sioul.text("pause-helps") + (cover.listOpen ? "  ▴" : "  ▾")
                Accessible.name: cover.sioul.text("pause-helps")
                onClicked: cover.listOpen = !cover.listOpen
            }
            Repeater {
                model: cover.back === null && cover.listOpen ? cover.screen.helps : []

                delegate: ColumnLayout {
                    id: help

                    required property var modelData

                    Layout.fillWidth: true
                    Layout.leftMargin: 12
                    spacing: 0

                    Label {
                        visible: help.modelData.open === ""
                        Layout.fillWidth: true
                        // In line with the words of a line that opens something (a flat button's padding).
                        leftPadding: 8
                        text: help.modelData.text
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        font.pixelSize: 17
                        color: cover.theme.text
                    }
                    // What it opens, opened by the system: a playlist, a folder of photos, an app.
                    Button {
                        id: opener

                        visible: help.modelData.open !== ""
                        Layout.maximumWidth: help.width
                        flat: true
                        text: help.modelData.text + "  ↗"
                        Accessible.name: help.modelData.text
                        onClicked: Qt.openUrlExternally(help.modelData.open)

                        contentItem: Label {
                            text: opener.text
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            font.pixelSize: 17
                            color: cover.theme.text
                        }
                    }
                }
            }

            // The breathing guide, only when set up: still until tapped, and a tap stops it.
            Button {
                visible: cover.back === null && cover.screen.breathing
                flat: true
                text: cover.sioul.text("pause-breathing")
                checkable: true
                checked: guide.running
                onClicked: guide.running = !guide.running
            }
            Item {
                id: guide

                property bool running: false
                // Breathing in two fifths of the breath, out three fifths: the breath out longer.
                readonly property int breath: Math.round(60000 / Math.max(3, cover.screen.pace || 6))

                visible: cover.back === null && cover.screen.breathing && guide.running
                Layout.alignment: Qt.AlignHCenter
                Layout.preferredWidth: 160
                Layout.preferredHeight: 160

                Rectangle {
                    id: shape

                    anchors.centerIn: parent
                    width: 150
                    height: 150
                    radius: width / 2
                    scale: 0.55
                    color: cover.theme.line
                    border.color: cover.theme.button
                    border.width: 1
                }
                SequentialAnimation {
                    running: guide.running && guide.visible
                    loops: Animation.Infinite
                    onStopped: shape.scale = 0.55

                    NumberAnimation {
                        target: shape
                        property: "scale"
                        from: 0.55
                        to: 1
                        duration: guide.breath * 2 / 5
                        easing.type: Easing.InOutSine
                    }
                    NumberAnimation {
                        target: shape
                        property: "scale"
                        from: 1
                        to: 0.55
                        duration: guide.breath * 3 / 5
                        easing.type: Easing.InOutSine
                    }
                }
                TapHandler {
                    onTapped: guide.running = false
                }
            }

            // The emergency number and the crisis line of your country: one quiet line, one tap to call.
            Flow {
                id: numbers

                visible: cover.back === null && cover.screen.numbers.length > 0
                Layout.fillWidth: true
                Layout.topMargin: 10
                spacing: 4

                Repeater {
                    model: cover.screen.numbers

                    delegate: Button {
                        id: number

                        required property var modelData

                        width: Math.min(implicitWidth, numbers.width)
                        flat: true
                        text: number.modelData.number + "  " + number.modelData.label
                        Accessible.name: number.modelData.label + " " + number.modelData.number
                        ToolTip.visible: hovered && number.modelData.about !== ""
                        ToolTip.text: number.modelData.about
                        ToolTip.delay: 800
                        onClicked: Qt.openUrlExternally(number.modelData.url)

                        contentItem: Label {
                            text: number.text
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            color: cover.theme.muted
                        }
                    }
                }
            }
            // The others (medical emergency, in writing, urgent care), unfolded on demand.
            Button {
                visible: cover.back === null && cover.screen.more.length > 0
                flat: true
                text: cover.sioul.text("pause-numbers-more") + (cover.numbersOpen ? "  ▴" : "  ▾")
                Accessible.name: cover.sioul.text("pause-numbers-more")
                onClicked: cover.numbersOpen = !cover.numbersOpen
            }
            Repeater {
                model: cover.back === null && cover.numbersOpen ? cover.screen.more : []

                delegate: Button {
                    id: other

                    required property var modelData

                    Layout.fillWidth: true
                    flat: true
                    text: other.modelData.number + "  " + other.modelData.label + (other.modelData.about !== "" ? " · " + other.modelData.about : "")
                    Accessible.name: other.modelData.label + " " + other.modelData.number
                    onClicked: Qt.openUrlExternally(other.modelData.url)

                    contentItem: Label {
                        text: other.text
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: cover.theme.muted
                    }
                }
            }

            // One way back, always in view; nothing asked.
            Button {
                visible: cover.back === null
                Layout.topMargin: 10
                Layout.alignment: Qt.AlignHCenter
                padding: 14
                font.pixelSize: 17
                text: cover.sioul.text("pause-back")
                onClicked: cover.comeBack()
            }

            // ---- Coming back: a few lines, the one offer (default: no), then on.
            Label {
                visible: cover.back !== null
                Layout.fillWidth: true
                text: cover.back ? cover.back.title : ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 24
                color: cover.theme.text
            }
            Label {
                visible: cover.back !== null && cover.back.day !== ""
                Layout.fillWidth: true
                text: cover.back ? cover.back.day : ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 17
                color: cover.theme.text
            }
            RowLayout {
                visible: cover.back !== null && cover.back.tomorrow !== ""
                Layout.fillWidth: true
                spacing: 8

                Label {
                    Layout.fillWidth: true
                    text: cover.lightened ? cover.sioul.text("pause-tomorrow-lighter") : (cover.back ? cover.back.tomorrow : "")
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: cover.theme.muted
                }
                Button {
                    visible: !cover.lightened
                    flat: true
                    text: cover.back ? cover.back.lighten : ""
                    onClicked: {
                        cover.sioul.lightenTomorrow()
                        cover.lightened = true
                    }
                }
            }
            Button {
                visible: cover.back !== null
                Layout.topMargin: 10
                Layout.alignment: Qt.AlignHCenter
                padding: 12
                text: cover.back ? cover.back.go : ""
                onClicked: {
                    cover.back = null
                    cover.done()
                }
            }
        }
    }

    // Escape is no way out of the pause; on the return's words, it goes on.
    Keys.onEscapePressed: event => {
        if (cover.back !== null || cover.trial) {
            cover.back = null
            cover.done()
        }
        event.accepted = true
    }
}
