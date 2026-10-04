// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A sound of the notes folder, an audio memo most often: play, pause, where
// it is, how long it lasts. No autoplay. Loaded only when a sound opens.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import QtMultimedia

ColumnLayout {
    id: player

    required property url source
    required property var sioul
    required property var theme

    function clock(ms) {
        const seconds = Math.floor(ms / 1000)
        const pad = n => n < 10 ? "0" + n : String(n)
        return Math.floor(seconds / 60) + ":" + pad(seconds % 60)
    }

    spacing: 8

    MediaPlayer {
        id: media

        source: player.source
        audioOutput: AudioOutput {}
    }

    RowLayout {
        Layout.fillWidth: true
        spacing: 10

        Button {
            text: media.playbackState === MediaPlayer.PlayingState ? player.sioul.text("audio-pause") : player.sioul.text("audio-play")
            icon.name: media.playbackState === MediaPlayer.PlayingState ? "media-playback-pause" : "media-playback-start"
            icon.color: player.theme.text
            highlighted: media.playbackState !== MediaPlayer.PlayingState
            onClicked: media.playbackState === MediaPlayer.PlayingState ? media.pause() : media.play()
        }
        Slider {
            Layout.fillWidth: true
            from: 0
            to: Math.max(1, media.duration)
            value: media.position
            enabled: media.seekable
            Accessible.name: player.sioul.text("audio-position")
            onMoved: media.position = value
        }
        Label {
            text: player.clock(media.position) + " / " + player.clock(media.duration)
            textFormat: Text.PlainText
            color: player.theme.muted
            font.features: { "tnum": 1 }
        }
    }
    // A sentence in your language; Qt's own words, in English, after it.
    Label {
        visible: media.error !== MediaPlayer.NoError
        Layout.fillWidth: true
        text: visible ? player.sioul.textWith("audio-cannot-play", "why", media.errorString) : ""
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: player.theme.warm
    }
}
