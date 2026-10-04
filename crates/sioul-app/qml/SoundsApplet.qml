// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Sounds, in the status line: noise for focus (white, pink, brown), nature to
// rest by (waves, rain, wind in the trees, crickets, a distant storm), all
// made here, and your own recordings (the notes' `sounds` folder).
// One at a time, looping, fading in and out; never on by itself.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import QtMultimedia

ToolButton {
    id: applet

    required property var sioul
    required property var theme
    property string playing: ""
    property string playingTitle: ""
    property var calm: []
    property real loudness: 0.5

    function play(url, title) {
        if (url === "")
            return
        // A sound chosen while the last one fades out: the fade's stop would stop it too.
        stopping.stop()
        applet.playing = url
        applet.playingTitle = title
        output.volume = 0
        media.source = url
        media.play()
        output.volume = applet.loudness
    }

    // Faded out, then stopped.
    function stop() {
        output.volume = 0
        stopping.start()
    }

    implicitHeight: 28
    icon.name: applet.playing !== "" ? "audio-volume-medium" : "audio-volume-low"
    icon.color: applet.theme.muted
    display: applet.playing !== "" ? AbstractButton.TextBesideIcon : AbstractButton.IconOnly
    text: applet.playingTitle
    Accessible.name: applet.sioul.text("sounds-title")
    ToolTip.visible: hovered && !popup.opened
    ToolTip.text: applet.sioul.text("sounds-title")
    ToolTip.delay: 600
    onClicked: {
        applet.calm = JSON.parse(applet.sioul.calmSounds() || "[]")
        popup.open()
    }

    contentItem: RowLayout {
        spacing: 4

        Icon {
            iconName: applet.icon.name
            color: applet.theme.muted
            size: 16
        }
        Label {
            visible: applet.text !== ""
            Layout.maximumWidth: 140
            text: applet.text
            textFormat: Text.PlainText
            elide: Text.ElideRight
            color: applet.theme.muted
        }
    }

    MediaPlayer {
        id: media

        loops: MediaPlayer.Infinite
        audioOutput: AudioOutput {
            id: output

            volume: 0

            Behavior on volume {
                NumberAnimation {
                    duration: 5000
                }
            }
        }
    }

    Timer {
        id: stopping

        interval: 5200
        onTriggered: {
            media.stop()
            applet.playing = ""
            applet.playingTitle = ""
        }
    }

    Popup {
        id: popup

        y: -height - 6
        x: Math.min(0, applet.parent ? applet.parent.width - applet.x - width : 0)
        width: 300
        padding: 12
        closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside

        background: Rectangle {
            color: applet.theme.surface
            radius: applet.theme.radius
            border.color: applet.theme.line
        }

        contentItem: ColumnLayout {
            spacing: 4

            Label {
                text: applet.sioul.text("sounds-focus")
                font.weight: Font.DemiBold
                color: applet.theme.text
            }
            Repeater {
                model: ["white", "pink", "brown"]

                delegate: ItemDelegate {
                    id: noise

                    required property string modelData
                    readonly property string title: applet.sioul.text("sounds-" + noise.modelData)

                    Layout.fillWidth: true
                    text: noise.title
                    highlighted: applet.playingTitle === noise.title
                    onClicked: applet.play(applet.sioul.noiseUrl(noise.modelData), noise.title)
                }
            }
            Label {
                Layout.topMargin: 6
                text: applet.sioul.text("sounds-nature")
                font.weight: Font.DemiBold
                color: applet.theme.text
            }
            Repeater {
                model: ["waves", "rain", "wind", "crickets", "storm"]

                delegate: ItemDelegate {
                    id: nature

                    required property string modelData
                    readonly property string title: applet.sioul.text("sounds-" + nature.modelData)

                    Layout.fillWidth: true
                    text: nature.title
                    highlighted: applet.playingTitle === nature.title
                    onClicked: applet.play(applet.sioul.noiseUrl(nature.modelData), nature.title)
                }
            }
            Label {
                Layout.topMargin: 6
                text: applet.sioul.text("sounds-calm")
                font.weight: Font.DemiBold
                color: applet.theme.text
            }
            Repeater {
                model: applet.calm

                delegate: ItemDelegate {
                    id: recording

                    required property var modelData

                    Layout.fillWidth: true
                    text: applet.theme.plain(recording.modelData.title).replace(/&/g, "&&")
                    highlighted: applet.playing === recording.modelData.url
                    onClicked: applet.play(recording.modelData.url, recording.modelData.title)
                }
            }
            Label {
                visible: applet.calm.length === 0
                Layout.fillWidth: true
                text: applet.sioul.text("sounds-calm-none")
                wrapMode: Text.Wrap
                font.pixelSize: 12
                color: applet.theme.muted
            }
            RowLayout {
                Layout.topMargin: 6
                Layout.fillWidth: true
                spacing: 6

                Icon {
                    iconName: "audio-volume-low"
                    color: applet.theme.muted
                    size: 16
                }
                Slider {
                    Layout.fillWidth: true
                    from: 0
                    to: 1
                    value: applet.loudness
                    Accessible.name: applet.sioul.text("sounds-volume")
                    onMoved: {
                        applet.loudness = value
                        if (applet.playing !== "")
                            output.volume = value
                    }
                }
            }
            Button {
                visible: applet.playing !== ""
                Layout.fillWidth: true
                text: applet.sioul.text("sounds-stop")
                icon.name: "media-playback-stop"
                icon.color: applet.theme.text
                onClicked: applet.stop()
            }
        }
    }
}
