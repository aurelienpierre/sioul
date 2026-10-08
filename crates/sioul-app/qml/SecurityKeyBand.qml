// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Your security key, asked where you are: a band at the bottom of the writing
// window when a message is signed at Send, or under the Reader's line when
// you open an encrypted message with it. What it says comes from Sioul
// (securitykey.rs): plug the key in (Sioul goes on as soon as it comes), its
// PIN in a password field with the tries left when some were lost, "Touch
// your security key" while it blinks, and what went wrong in plain words with
// what to do: Try again, Let GnuPG release it. No red, no pop-up; the PIN's
// field is emptied as soon as it is given.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Rectangle {
    id: band

    required property var sioul
    required property var theme
    // Whose band this is: a draft's id, or a message's key.
    property string context: ""
    // "sign" in the writing window, "open" in the Reader.
    property string purpose: "sign"
    // What Sioul says now: {state, line, tries, warm, action}.
    property var said: band.quiet
    readonly property var quiet: ({ state: "", line: "", tries: "", warm: false, action: "" })
    readonly property bool active: band.said.state !== "" && band.said.state !== "done"
    readonly property bool signing: band.purpose === "sign"

    // Go on, with the PIN typed ("" for the one Sioul holds).
    signal go(string pin)
    // "Send unsigned", in the writing window.
    signal unsigned
    // Signed and scheduled, or opened.
    signal done

    // Sioul asks the key: what it says comes back in the band.
    function start() {
        band.said = { state: "working", line: band.sioul.text("seckey-reading"), tries: "", warm: false, action: "" }
        band.go("")
    }

    // Shows what Sioul said before anything was asked (an expired key).
    function say(line) {
        band.said = { state: "problem", line: line, tries: "", warm: true, action: "" }
    }

    function reset() {
        pinField.text = ""
        band.said = band.quiet
    }

    function submit() {
        if (pinField.text === "")
            return
        const pin = pinField.text
        // The PIN leaves the field at once: nothing keeps it on screen.
        pinField.text = ""
        band.said = { state: "working", line: band.sioul.text(band.signing ? "seckey-signing" : "seckey-opening"), tries: "", warm: false, action: "" }
        band.go(pin)
    }

    function notNow() {
        band.sioul.securityKeyNotNow(band.context)
        band.reset()
    }

    function release() {
        const answer = JSON.parse(band.sioul.letGnupgRelease() || "{}")
        if (answer.done) {
            band.said = { state: "working", line: answer.line, tries: "", warm: false, action: "" }
            band.go("")
        } else {
            band.said = { state: "problem", line: answer.line || "", tries: "", warm: true, action: "retry" }
        }
    }

    visible: band.active
    implicitHeight: column.implicitHeight + 24
    color: band.theme.surface
    radius: band.theme.radius
    border.color: band.said.warm ? band.theme.warm : band.theme.line

    Connections {
        target: band.sioul

        function onSecurityKeyChanged(context, state) {
            if (context !== band.context)
                return
            const said = JSON.parse(state)
            band.said = said
            if (said.state === "pin")
                pinField.forceActiveFocus()
            else if (said.state === "done")
                band.done()
        }
    }

    ColumnLayout {
        id: column

        anchors.fill: parent
        anchors.margins: 12
        spacing: 8

        RowLayout {
            Layout.fillWidth: true
            spacing: 8

            // The key's sign, breathing slowly while the key waits for a finger.
            Icon {
                id: keyIcon

                iconName: "dialog-password"
                size: 16

                SequentialAnimation {
                    running: band.said.state === "touch"
                    loops: Animation.Infinite
                    onStopped: keyIcon.opacity = 1

                    NumberAnimation {
                        target: keyIcon
                        property: "opacity"
                        to: 0.3
                        duration: 700
                        easing.type: Easing.InOutSine
                    }
                    NumberAnimation {
                        target: keyIcon
                        property: "opacity"
                        to: 1
                        duration: 700
                        easing.type: Easing.InOutSine
                    }
                }
            }
            BusyIndicator {
                visible: band.said.state === "working"
                running: visible
                implicitWidth: 18
                implicitHeight: 18
            }
            // Its commands, if any, to copy (no smart card service, a locked key).
            CommandText {
                Layout.fillWidth: true
                sioul: band.sioul
                theme: band.theme
                text: band.said.line || ""
                color: band.said.warm ? band.theme.warm : band.theme.text
            }
        }
        Label {
            visible: band.said.tries !== undefined && band.said.tries !== ""
            Layout.fillWidth: true
            text: band.said.tries || ""
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: band.theme.warm
            font.pixelSize: 13
        }
        // The PIN, typed in Sioul's own field.
        RowLayout {
            visible: band.said.state === "pin"
            Layout.fillWidth: true
            spacing: 8

            PasswordField {
                id: pinField

                Layout.fillWidth: true
                Layout.maximumWidth: 260
                sioul: band.sioul
                placeholderText: band.sioul.text("seckey-pin-field")
                Accessible.name: band.sioul.text("seckey-pin")
                onAccepted: band.submit()
            }
            Button {
                text: band.sioul.text(band.signing ? "seckey-sign-and-send" : "seckey-open")
                highlighted: true
                enabled: pinField.text !== ""
                onClicked: band.submit()
            }
        }
        Flow {
            visible: band.said.state !== "working" && band.said.state !== "touch"
            Layout.fillWidth: true
            spacing: 8

            Button {
                visible: band.said.action === "release"
                text: band.sioul.text("seckey-release")
                highlighted: true
                onClicked: band.release()
            }
            Button {
                visible: band.said.action === "retry" || band.said.action === "release"
                text: band.sioul.text("seckey-try-again")
                onClicked: band.start()
            }
            Button {
                visible: band.signing
                flat: true
                text: band.sioul.text("seckey-send-unsigned")
                onClicked: {
                    band.notNow()
                    band.unsigned()
                }
            }
            Button {
                flat: true
                text: band.sioul.text("seckey-not-now")
                onClicked: band.notNow()
            }
        }
    }
}
