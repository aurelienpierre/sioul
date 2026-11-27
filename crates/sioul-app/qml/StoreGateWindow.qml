// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The screen before the window (storegate.rs; docs/store.md, "At start"),
// shown only when what Sioul keeps on this device is not open yet or plain
// files wait to be sealed. Waiting for the keyring, it says so and goes on by
// itself; **Use a way back…** opens it with what the person kept, or starts
// again from the servers and the other devices. The move shows its progress.
// At the first sealing, the ways back are offered once (WaysBack.qml), none
// needed; then the window opens.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import com.aurelienpierre.sioul

ApplicationWindow {
    id: window

    // The ways back shown under "Use a way back…".
    property bool waysShown: false
    // "Start again" asked: what it sets aside said, then confirmed.
    property bool againAsked: false
    // The screen's object (storegate.rs), named `sioul` as the backend is, which the ways back's component takes.
    readonly property var gateObject: sioul

    visible: true
    width: 560
    height: 640
    title: "Sioul"
    color: theme.background

    Theme {
        id: theme
    }

    StoreGate {
        id: sioul

        // Ready: the window opens (closing this one ends its loop, storegate.rs).
        onStageChanged: if (sioul.stage === "ready") window.close()
    }

    Component.onCompleted: sioul.begin()

    ScrollView {
        id: scroll

        anchors.fill: parent
        anchors.margins: 24
        contentWidth: availableWidth

        ColumnLayout {
            width: scroll.availableWidth
            spacing: 14

            Label {
                Layout.fillWidth: true
                text: sioul.text("store-gate-title")
                font.pixelSize: 22
                wrapMode: Text.WordWrap
                color: theme.text
            }
            Label {
                objectName: "storeGateSaid"
                Layout.fillWidth: true
                text: sioul.stage === "moving" ? sioul.text("store-gate-moving-help") : sioul.said
                textFormat: Text.PlainText
                wrapMode: Text.WordWrap
                lineHeight: 1.25
                color: theme.text
            }
            Label {
                visible: sioul.stage === "waiting"
                Layout.fillWidth: true
                text: sioul.text("store-gate-waiting-help")
                textFormat: Text.PlainText
                wrapMode: Text.WordWrap
                color: theme.text
            }

            // The move: how far, calmly.
            ProgressBar {
                objectName: "storeGateProgress"
                visible: sioul.stage === "moving"
                Layout.fillWidth: true
                from: 0
                to: Math.max(1, sioul.total)
                value: sioul.done
            }
            Label {
                visible: sioul.stage === "moving"
                Layout.fillWidth: true
                text: sioul.textCounted("store-gate-moving", sioul.total) + "  " + sioul.done + " / " + sioul.total
                textFormat: Text.PlainText
                wrapMode: Text.WordWrap
                color: theme.text
            }

            // Waiting, or the key lost: the ways back.
            Button {
                objectName: "storeGateUseWay"
                visible: (sioul.stage === "waiting" || sioul.stage === "lost") && !window.waysShown
                text: sioul.text("store-gate-use-way")
                onClicked: window.waysShown = true
            }
            ColumnLayout {
                visible: (sioul.stage === "waiting" || sioul.stage === "lost") && (window.waysShown || sioul.stage === "lost")
                Layout.fillWidth: true
                spacing: 8

                Label {
                    text: sioul.text("store-gate-passphrase")
                    color: theme.text
                }
                // Which passphrase: the one kept, on its day (the sharing's then, if it was).
                Label {
                    objectName: "storeGatePassphraseSaid"
                    readonly property string kept: sioul.passphraseSaid()

                    visible: kept !== ""
                    Layout.fillWidth: true
                    text: kept
                    textFormat: Text.PlainText
                    wrapMode: Text.WordWrap
                    color: theme.text
                }
                RowLayout {
                    Layout.fillWidth: true

                    TextField {
                        id: passphrase

                        Layout.fillWidth: true
                        echoMode: TextInput.Password
                        onAccepted: sioul.usePassphrase(passphrase.text)
                    }
                    Button {
                        enabled: passphrase.text !== "" && !sioul.busy
                        text: sioul.text("store-gate-use")
                        onClicked: sioul.usePassphrase(passphrase.text)
                    }
                }
                Label {
                    text: sioul.text("store-gate-code")
                    color: theme.text
                }
                RowLayout {
                    Layout.fillWidth: true

                    TextField {
                        id: code

                        Layout.fillWidth: true
                        font.family: "monospace"
                        onAccepted: sioul.useCode(code.text)
                    }
                    Button {
                        enabled: code.text !== "" && !sioul.busy
                        text: sioul.text("store-gate-use")
                        onClicked: sioul.useCode(code.text)
                    }
                }
                Label {
                    text: sioul.text("store-gate-card-pin")
                    color: theme.text
                }
                RowLayout {
                    Layout.fillWidth: true

                    TextField {
                        id: pin

                        Layout.fillWidth: true
                        echoMode: TextInput.Password
                        onAccepted: sioul.useCard(pin.text)
                    }
                    Button {
                        enabled: pin.text !== "" && !sioul.busy
                        text: sioul.text("store-gate-use-card")
                        onClicked: sioul.useCard(pin.text)
                    }
                }
                Label {
                    objectName: "storeGateAnswer"
                    visible: sioul.answer !== ""
                    Layout.fillWidth: true
                    text: sioul.answer
                    textFormat: Text.PlainText
                    wrapMode: Text.WordWrap
                    color: theme.warm
                }
                // Without any way back: from the servers and the other devices. Only
                // when the keyring holds no key (never while it is merely locked),
                // and only once what it sets aside is said and confirmed.
                Label {
                    visible: sioul.stage === "lost"
                    Layout.fillWidth: true
                    Layout.topMargin: 10
                    text: sioul.text("store-gate-start-again-help")
                    textFormat: Text.PlainText
                    wrapMode: Text.WordWrap
                    lineHeight: 1.25
                    color: theme.text
                }
                Button {
                    objectName: "storeGateStartAgain"
                    visible: sioul.stage === "lost" && !window.againAsked
                    enabled: !sioul.busy
                    text: sioul.text("store-gate-start-again")
                    onClicked: window.againAsked = true
                }
                Label {
                    objectName: "storeGateStartAgainConfirm"
                    visible: sioul.stage === "lost" && window.againAsked
                    Layout.fillWidth: true
                    text: sioul.text("store-gate-start-again-confirm")
                    textFormat: Text.PlainText
                    wrapMode: Text.WordWrap
                    lineHeight: 1.25
                    color: theme.warm
                }
                RowLayout {
                    visible: sioul.stage === "lost" && window.againAsked
                    spacing: 8

                    Button {
                        objectName: "storeGateStartAgainYes"
                        enabled: !sioul.busy
                        text: sioul.text("store-gate-start-again-yes")
                        onClicked: sioul.startAgain()
                    }
                    Button {
                        objectName: "storeGateStartAgainNo"
                        text: sioul.text("store-gate-start-again-no")
                        onClicked: window.againAsked = false
                    }
                }
            }

            // The first sealing done: the ways back offered once, then the window.
            WaysBack {
                visible: sioul.stage === "offer"
                Layout.fillWidth: true
                sioul: window.gateObject
                theme: theme
            }
            Button {
                objectName: "storeGateOpen"
                visible: sioul.stage === "offer" || sioul.stage === "failed"
                highlighted: true
                text: sioul.text(sioul.stage === "failed" ? "ui-close" : "store-gate-open")
                onClicked: window.close()
            }
        }
    }
}
