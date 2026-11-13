// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The sounds' player (NoisePlayer.qml): a sound fades in as it starts and out
// as it stops, over five seconds; the volume slider changes the volume at
// once, during a fade in too, and never undoes a fade out. A sound made as it
// plays follows the same fades; where the output cannot play it, its file
// loops instead. No sound is played: the player is given a file that is not
// there, and a stand-in for Sioul's backend. Run by tools/qml-test.sh.

import QtQuick
import QtTest
import "../../qml"

Item {
    id: root

    width: 100
    height: 100

    NoisePlayer {
        id: noise
    }

    // A stand-in for Sioul's backend: what the player asked of it.
    QtObject {
        id: backend

        property bool takes: true
        property string started: ""
        property real volume: -1
        property int stops: 0
        property string asked: ""

        function liveSoundStart(kind, volume) {
            backend.started = kind
            backend.volume = volume
            return backend.takes
        }
        function liveSoundVolume(volume) {
            backend.volume = volume
        }
        function liveSoundStop() {
            backend.stops += 1
        }
        function noiseUrl(kind) {
            backend.asked = kind
            return "file:///nonexistent/sioul-test-" + kind + ".wav"
        }
    }

    NoisePlayer {
        id: live

        sioul: backend
    }

    TestCase {
        name: "NoisePlayer"
        when: windowShown

        readonly property url nothing: "file:///nonexistent/sioul-test-sound.ogg"

        function cleanup() {
            noise.stop()
            live.stop()
            backend.takes = true
            backend.started = ""
            backend.volume = -1
            backend.stops = 0
            backend.asked = ""
        }

        function test_a_sound_fades_in() {
            noise.play(nothing, 0.6)
            wait(300)
            verify(noise.level > 0 && noise.level < 0.2, "a few hundredths after 300 ms: " + noise.level)
        }

        function test_the_slider_moves_the_volume_at_once() {
            noise.play(nothing, 0.6)
            wait(100)
            noise.setVolume(0.8)
            compare(noise.level, 0.8, "at once, the fade in ended there")
            wait(200)
            compare(noise.level, 0.8, "and stays")
            noise.setVolume(0.3)
            compare(noise.level, 0.3)
        }

        function test_a_sound_fades_out_and_the_slider_leaves_it() {
            noise.play(nothing, 0.5)
            noise.setVolume(0.5)
            noise.fadeOut()
            verify(noise.fadingOut)
            wait(300)
            verify(noise.level < 0.5 && noise.level > 0.4, "fading: " + noise.level)
            noise.setVolume(0.9)
            verify(noise.fadingOut && noise.level < 0.5, "a slider moved while it fades out does not bring it back")
        }

        function test_a_sound_made_as_it_plays_follows_the_fades() {
            live.play("live:waves", 0.6)
            compare(backend.started, "waves")
            verify(live.live)
            wait(300)
            verify(backend.volume > 0 && backend.volume < 0.2, "fading in: " + backend.volume)
            live.setVolume(0.4)
            compare(backend.volume, 0.4)
            live.play("live:birds", 0.6)
            compare(backend.stops, 1, "the last one stopped first")
            compare(backend.started, "birds")
            live.stop()
            compare(backend.stops, 2)
            verify(!live.live)
        }

        function test_a_sound_the_output_cannot_play_loops_its_file() {
            backend.takes = false
            live.play("live:rain", 0.5)
            compare(backend.asked, "rain")
            verify(!live.live)
            wait(300)
            verify(live.level > 0, "its file fades in: " + live.level)
        }
    }
}
