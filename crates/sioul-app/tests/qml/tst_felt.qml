// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// "How was it?" (FeltRatings.qml): of its five tiles, only those tapped are
// given; a forecast is never copied into the answer, not even a forecast of 0,
// and it offers no faint values to take. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml" as App

Item {
    id: root

    width: 412
    height: 900

    QtObject {
        id: words

        function text(key) { return key }
        function textWith(key, name, value) { return key + ":" + value }
        function textArgs(key, args) { return key }
    }
    App.Theme {
        id: theme
    }
    QtObject {
        id: spy

        property var given: []
    }

    App.FeltRatings {
        id: felt

        width: parent.width
        sioul: words
        theme: theme
        forecast: ({ cognitive: null, emotional: 2, anxiety: 7, body: null, gain: 0 })
        kept: null
        onGiven: values => {
            spy.given.push(values)
            felt.kept = values
        }
    }

    TestCase {
        name: "FeltRatings"
        when: windowShown

        function test_only_what_is_tapped() {
            const tiles = felt.tiles
            verify(!tiles.offered, "nothing faint to take")
            // Worry, first arrow: it starts at the forecast, 7, given only by the key; then down to 6.
            const worry = tiles.tile("anxiety")
            worry.forceActiveFocus()
            keyClick(Qt.Key_Left)
            compare(spy.given.length, 1)
            compare(spy.given[0].anxiety, 7)
            compare(spy.given[0].emotional, null, "a forecast never copied")
            compare(spy.given[0].gain, null, "a forecast of 0 never copied either")
            keyClick(Qt.Key_Left)
            compare(spy.given[1].anxiety, 6)
            // The gain, tapped at 6: both kept, the others still null.
            const gain = tiles.tile("gain")
            const p = gain.point(6)
            mouseClick(gain, p.x, p.y)
            const last = spy.given[spy.given.length - 1]
            compare(last.anxiety, 6)
            compare(last.gain, 6)
            compare(last.cognitive, null)
            compare(last.emotional, null)
            compare(last.body, null)
            verify(felt.said)
        }
    }
}
