// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The sounds' player, in a file of its own (SoundsApplet.qml): Qt Multimedia
// and its decoders are loaded the first time a sound plays, not each time the
// window opens. A sound fades in as it starts and out as it stops, over five
// seconds; the volume slider meanwhile changes it at once.

import QtQuick
import QtMultimedia

Item {
    id: noise

    // The output's volume as it is now, mid-fade too (for the tests).
    readonly property alias level: output.volume
    // Fading out, before the sound stops.
    readonly property bool fadingOut: fade.running && fade.to === 0

    // `url` played in a loop, faded in from silence to `volume`.
    function play(url, volume) {
        fade.stop()
        output.volume = 0
        media.source = url
        media.play()
        noise.fadeTo(volume)
    }

    // The slider moved: the volume at once, a fade in under way ended there;
    // never while the sound fades out to stop.
    function setVolume(volume) {
        if (noise.fadingOut)
            return
        fade.stop()
        output.volume = volume
    }

    // Faded to silence; the sound itself stops with `stop`.
    function fadeOut() {
        noise.fadeTo(0)
    }

    function stop() {
        fade.stop()
        media.stop()
    }

    function fadeTo(volume) {
        fade.stop()
        fade.from = output.volume
        fade.to = volume
        fade.start()
    }

    NumberAnimation {
        id: fade

        target: output
        property: "volume"
        duration: 5000
    }

    MediaPlayer {
        id: media

        loops: MediaPlayer.Infinite
        audioOutput: AudioOutput {
            id: output

            volume: 0
        }
    }
}
