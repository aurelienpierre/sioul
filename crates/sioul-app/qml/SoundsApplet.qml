// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Sounds, in the status line, a button of the line (LineButton.qml): noise
// for focus (white, pink, brown), nature to rest by (waves, rain, wind in the
// trees, crickets, a distant storm), all made here, and your own recordings
// (the notes' `sounds` folder). One at a time, looping, fading in and out;
// never on by itself.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

LineButton {
    id: applet

    required property var sioul
    // Its pop-up under it, the line at the window's top (a computer's title bar); else above it.
    property bool below: false
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
        player.active = true
        player.item.volume = 0
        player.item.play(url)
        player.item.volume = applet.loudness
    }

    // Faded out, then stopped.
    function stop() {
        if (player.item)
            player.item.volume = 0
        stopping.start()
    }

    icon.name: applet.playing !== "" ? "audio-volume-medium" : "audio-volume-low"
    // What plays, beside its icon, where there is room.
    display: applet.playing !== "" && !applet.compact ? AbstractButton.TextBesideIcon : AbstractButton.IconOnly
    text: applet.playingTitle
    name: applet.sioul.text("sounds-title")
    // What plays, said in its tip: its icon alone on a narrow line.
    tip: applet.playing !== "" ? applet.name + "\n" + applet.playingTitle : applet.name
    menuOpen: popupLoader.item !== null && (popupLoader.item as Popup).opened
    onChosen: {
        applet.calm = JSON.parse(applet.sioul.calmSounds() || "[]")
        popupLoader.active = true
        popupLoader.item.open()
    }

    contentItem: RowLayout {
        spacing: 4

        Icon {
            iconName: applet.icon.name
            color: applet.theme.muted
            size: 16
        }
        Label {
            visible: applet.display === AbstractButton.TextBesideIcon && applet.text !== ""
            Layout.maximumWidth: 140
            text: applet.text
            textFormat: Text.PlainText
            elide: Text.ElideRight
            color: applet.theme.muted
        }
    }

    // The player, made the first time a sound plays (NoisePlayer.qml): Qt
    // Multimedia waits until then.
    Loader {
        id: player

        active: false
        source: "NoisePlayer.qml"
    }

    Timer {
        id: stopping

        interval: 5200
        onTriggered: {
            if (player.item)
                player.item.stop()
            applet.playing = ""
            applet.playingTitle = ""
        }
    }

    // Made the first time it opens, its rows with it.
    Loader {
        id: popupLoader

        active: false
        sourceComponent: Popup {
            id: popup

            y: applet.below ? applet.height + 6 : -height - 6
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
                                if (player.item)
                                    player.item.volume = value
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
}
