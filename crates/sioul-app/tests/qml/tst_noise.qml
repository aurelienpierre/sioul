// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The sounds' player (NoisePlayer.qml): a sound fades in as it starts and out
// as it stops, over five seconds; the volume slider changes the volume at
// once, during a fade in too, and never undoes a fade out. No sound is
// played: the player is given a file that is not there. Run by
// tools/qml-test.sh.

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

    TestCase {
        name: "NoisePlayer"
        when: windowShown

        readonly property url nothing: "file:///nonexistent/sioul-test-sound.ogg"

        function cleanup() {
            noise.stop()
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
    }
}
