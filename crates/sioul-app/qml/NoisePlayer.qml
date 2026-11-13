// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The sounds' player, in a file of its own (SoundsApplet.qml): Qt Multimedia
// and its decoders are loaded the first time a sound plays, not each time the
// window opens. The noises and the sounds of nature are made as they play, so
// that they never loop (Sioul.liveSoundStart, docs/sounds.md); where the
// system's audio output cannot play them, their files loop instead, as your
// recordings do. A sound fades in as it starts and out as it stops, over five
// seconds; the volume slider meanwhile changes it at once.

import QtQuick
import QtMultimedia

Item {
    id: noise

    // Sioul's backend, which makes the sounds as they play; none in the tests.
    property var sioul: null
    // The volume as it is now, mid-fade too (for the tests): the file's or the sound's made as it plays.
    property real level: 0
    // Fading out, before the sound stops.
    readonly property bool fadingOut: fade.running && fade.to === 0
    // A sound made as it plays, rather than a file.
    property bool live: false

    onLevelChanged: {
        output.volume = noise.level
        if (noise.live && noise.sioul)
            noise.sioul.liveSoundVolume(noise.level)
    }

    // `source` played, faded in from silence to `volume`: "live:waves" a sound
    // made here as it plays, else a file's URL, looping.
    function play(source, volume) {
        fade.stop()
        noise.quiet()
        noise.level = 0
        const text = String(source)
        if (text.startsWith("live:")) {
            const kind = text.slice(5)
            if (noise.sioul && noise.sioul.liveSoundStart(kind, 0)) {
                noise.live = true
            } else {
                media.source = noise.sioul ? noise.sioul.noiseUrl(kind) : ""
                media.play()
            }
        } else {
            media.source = source
            media.play()
        }
        noise.fadeTo(volume)
    }

    // The slider moved: the volume at once, a fade in under way ended there;
    // never while the sound fades out to stop.
    function setVolume(volume) {
        if (noise.fadingOut)
            return
        fade.stop()
        noise.level = volume
    }

    // Faded to silence; the sound itself stops with `stop`.
    function fadeOut() {
        noise.fadeTo(0)
    }

    function stop() {
        fade.stop()
        noise.quiet()
    }

    // Whatever plays, stopped at once.
    function quiet() {
        media.stop()
        if (noise.live && noise.sioul)
            noise.sioul.liveSoundStop()
        noise.live = false
    }

    function fadeTo(volume) {
        fade.stop()
        fade.from = noise.level
        fade.to = volume
        fade.start()
    }

    NumberAnimation {
        id: fade

        target: noise
        property: "level"
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
