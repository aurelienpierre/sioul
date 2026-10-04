// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The sounds' player, in a file of its own (SoundsApplet.qml): Qt Multimedia
// and its decoders are loaded the first time a sound plays, not each time the
// window opens. Its volume fades in and out over five seconds.

import QtQuick
import QtMultimedia

Item {
    id: noise

    property alias volume: output.volume

    function play(url) {
        media.source = url
        media.play()
    }

    function stop() {
        media.stop()
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
}
