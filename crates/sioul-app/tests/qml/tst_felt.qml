// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// "How was it?" (FeltRatings.qml): of its five sliders, only those moved are
// given; a forecast is never copied into the answer, not even a forecast of 0.
// Run by tools/qml-test.sh.

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
    }
    QtObject {
        id: theme
        property color line: "#e2dccf"
        property color muted: "#6b655c"
        property color text: "#2d2a26"
        property color accent: "#4c6b5c"
        property color focus: "#2f4a3d"
        property color hover: "#e5ded1"
        property color surface: "#fbfaf7"
    }
    QtObject { id: spy; property var given: [] }

    App.FeltRatings {
        id: felt
        width: parent.width
        sioul: words
        theme: theme
        forecast: ({ cognitive: null, emotional: 2, anxiety: 7, body: null, gain: 0 })
        kept: null
        onGiven: values => { spy.given.push(values); felt.kept = values }
    }

    TestCase {
        name: "FeltRatings"
        when: windowShown

        function sliders() {
            const found = []
            const walk = item => {
                for (let i = 0; i < item.children.length; ++i) {
                    const c = item.children[i]
                    if (c.from === 0 && c.to === 10 && c.stepSize === 1)
                        found.push(c)
                    else
                        walk(c)
                }
            }
            walk(felt)
            return found
        }

        function test_only_what_is_moved() {
            const all = sliders()
            compare(all.length, 5, "five sliders")
            // Anxiety (the third), first arrow: starts at the forecast, 7; then down to 6.
            all[2].forceActiveFocus()
            keyClick(Qt.Key_Left)
            compare(spy.given.length, 1)
            compare(spy.given[0].anxiety, 7)
            compare(spy.given[0].emotional, null, "a forecast never copied")
            compare(spy.given[0].gain, null, "a forecast of 0 never copied either")
            keyClick(Qt.Key_Left)
            compare(spy.given[1].anxiety, 6)
            // The gain (the fifth), tapped at 60 %: both kept, the others still null.
            const g = all[4]
            const x = g.leftPadding + 11 + 0.6 * (g.availableWidth - 22)
            mouseClick(g, x, g.height / 2)
            const last = spy.given[spy.given.length - 1]
            compare(last.anxiety, 6)
            compare(last.gain, 6)
            compare(last.cognitive, null)
            compare(last.emotional, null)
            compare(last.body, null)
        }
    }
}
