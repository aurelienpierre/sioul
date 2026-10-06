// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The calls Sioul declined on this phone, on the Porch (docs/porch.md, "Calls
// declined"; docs/android.md, "Calls"): each shown at the times its caller
// may reach you, by phone or in writing; never counted, no badge. Who, when,
// the doubt said plainly ("They may have left a message"), and what you may
// do: call back or text back (the phone's own apps, the number filled in:
// nothing is sent by Sioul), add to your contacts (Android's own form), block,
// listen to the voicemail Free mailed; "Seen" takes a line away.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: section

    required property var sioul
    required property var theme
    required property var window

    // calls.rs's `view`: {lines: [{ids, text, number, dial, known, hidden, doubt, message: {text, path, sound}, why}]}.
    property var shown: ({ lines: [] })
    property string said: ""
    // The voicemail being played: its line's first id, and its sound's address.
    property string playing: ""
    property string playingUrl: ""
    // Lines whose "Why?" is open, by their first id.
    property var whyShown: ({})

    function reload() {
        section.shown = JSON.parse(section.sioul.callsView() || "null") || ({ lines: [] })
    }

    function act(verb, args) {
        const answer = JSON.parse(section.sioul.callsAction(verb, JSON.stringify(args || {})) || "null")
        section.said = answer && answer.said ? answer.said : ""
        return answer
    }

    spacing: 6
    visible: section.shown.lines.length > 0
    Component.onCompleted: section.reload()

    // At each minute's turn (a caller's time may have come), and when the Porch is sorted again.
    Connections {
        target: section.window

        function onNowChanged() {
            section.reload()
        }
    }
    Connections {
        target: section.sioul

        function onPorchChanged() {
            section.reload()
        }
    }

    Panel {
        Layout.fillWidth: true
        theme: section.theme

        ColumnLayout {
            anchors.fill: parent
            spacing: 10

            Label {
                Layout.fillWidth: true
                text: section.sioul.text("calls-title")
                font.weight: Font.DemiBold
                wrapMode: Text.Wrap
                color: section.theme.text
            }
            Label {
                visible: section.said !== ""
                Layout.fillWidth: true
                text: section.said
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: section.theme.muted
            }

            Repeater {
                model: section.shown.lines

                delegate: ColumnLayout {
                    id: line

                    required property var modelData
                    readonly property string key: line.modelData.ids.length > 0 ? line.modelData.ids[0] : ""

                    Layout.fillWidth: true
                    spacing: 4

                    // Who called, when: what the network and your contacts say, never more.
                    Label {
                        Layout.fillWidth: true
                        text: line.modelData.text
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: section.theme.text
                    }
                    Label {
                        visible: line.modelData.number !== ""
                        Layout.fillWidth: true
                        text: line.modelData.number
                        textFormat: Text.PlainText
                        font.features: { "tnum": 1 }
                        color: section.theme.muted
                    }
                    // What Sioul cannot know, said; or the message Free mailed.
                    Label {
                        visible: line.modelData.doubt !== ""
                        Layout.fillWidth: true
                        text: line.modelData.doubt
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        font.pixelSize: 13
                        color: section.theme.muted
                    }
                    Label {
                        visible: !!line.modelData.message
                        Layout.fillWidth: true
                        text: line.modelData.message ? line.modelData.message.text : ""
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: section.theme.text
                    }
                    Label {
                        visible: section.whyShown[line.key] === true
                        Layout.fillWidth: true
                        text: line.modelData.why
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        font.pixelSize: 13
                        color: section.theme.muted
                    }
                    // The voicemail's sound, played here once asked (AudioPlayer.qml).
                    Loader {
                        active: section.playing === line.key && section.playingUrl !== ""
                        visible: active
                        Layout.fillWidth: true

                        sourceComponent: Component {
                            AudioPlayer {
                                source: section.playingUrl
                                sioul: section.sioul
                                theme: section.theme
                            }
                        }
                    }
                    Flow {
                        Layout.fillWidth: true
                        spacing: 6

                        Button {
                            visible: !!line.modelData.message && line.modelData.message.sound !== null && line.modelData.message.sound !== undefined && section.playing !== line.key
                            width: Math.min(implicitWidth, parent ? parent.width : implicitWidth)
                            text: section.sioul.text("calls-listen")
                            icon.name: "media-playback-start"
                            icon.color: section.theme.text
                            onClicked: {
                                const answer = section.act("listen", { path: line.modelData.message.path, sound: line.modelData.message.sound })
                                if (answer && answer.url) {
                                    section.playingUrl = answer.url
                                    section.playing = line.key
                                }
                            }
                        }
                        // Written first: a text before a call.
                        Button {
                            visible: line.modelData.dial !== ""
                            width: Math.min(implicitWidth, parent ? parent.width : implicitWidth)
                            text: section.sioul.text("calls-text-back")
                            icon.name: "mail-message-new"
                            icon.color: section.theme.text
                            onClicked: Qt.openUrlExternally("sms:" + line.modelData.dial)
                        }
                        Button {
                            visible: line.modelData.dial !== ""
                            width: Math.min(implicitWidth, parent ? parent.width : implicitWidth)
                            text: section.sioul.text("calls-call-back")
                            icon.name: "call-start"
                            icon.color: section.theme.text
                            onClicked: Qt.openUrlExternally("tel:" + line.modelData.dial)
                        }
                        Button {
                            visible: line.modelData.dial !== "" && !line.modelData.known
                            width: Math.min(implicitWidth, parent ? parent.width : implicitWidth)
                            flat: true
                            text: section.sioul.text("calls-add-contact")
                            icon.name: "contact-new"
                            icon.color: section.theme.text
                            onClicked: section.act("add-contact", { number: line.modelData.dial })
                        }
                        Button {
                            visible: line.modelData.dial !== ""
                            width: Math.min(implicitWidth, parent ? parent.width : implicitWidth)
                            flat: true
                            text: section.sioul.text("calls-block")
                            icon.name: "dialog-cancel"
                            icon.color: section.theme.text
                            onClicked: {
                                blockAsk.number = line.modelData.dial
                                blockAsk.ask(section.sioul.text("calls-block-title"), section.sioul.textWith("calls-block-ask", "number", line.modelData.number), section.sioul.text("calls-block"))
                            }
                        }
                        Button {
                            width: Math.min(implicitWidth, parent ? parent.width : implicitWidth)
                            flat: true
                            text: section.sioul.text("calls-why")
                            onClicked: {
                                const next = Object.assign({}, section.whyShown)
                                next[line.key] = !next[line.key]
                                section.whyShown = next
                            }
                        }
                        Button {
                            width: Math.min(implicitWidth, parent ? parent.width : implicitWidth)
                            flat: true
                            text: section.sioul.text("calls-seen")
                            onClicked: {
                                section.act("seen", { ids: line.modelData.ids })
                                if (section.playing === line.key)
                                    section.playing = ""
                                section.reload()
                            }
                        }
                    }
                }
            }
            // Blocking is asked once: their calls go to voicemail for good, listed nowhere.
            ConfirmDialog {
                id: blockAsk

                property string number: ""

                sioul: section.sioul
                theme: section.theme
                onConfirmed: {
                    section.act("block", { number: blockAsk.number })
                    section.reload()
                }
            }
            // Android's own blocked numbers: calls and texts, for every app.
            Button {
                flat: true
                Layout.alignment: Qt.AlignLeft
                text: section.sioul.text("calls-open-blocked")
                onClicked: section.act("open-blocked", {})
            }
        }
    }
}
