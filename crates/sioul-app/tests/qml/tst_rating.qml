// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// One 0-to-10 rating (RatingSlider.qml), by touch, mouse and keys: a tap sets
// a value, a value is given once the finger or the key is up, a vertical
// swipe scrolls the page and gives nothing, Delete and the × make it unsaid
// again, a hint starts the arrows and is never given by itself. Run by
// tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml" as App

Item {
    id: root
    width: 412
    height: 500

    QtObject {
        id: words
        function text(key) { return key === "rating-none" ? "Not rated" : key }
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

    ScrollView {
        id: scroll
        anchors.fill: parent
        contentWidth: availableWidth
        Column {
            width: scroll.availableWidth
            spacing: 10
            Rectangle { width: parent.width; height: 300; color: "#eee" }
            App.RatingSlider {
                id: slider
                width: parent.width
                sioul: words
                theme: theme
                label: "Anxiety it raises"
                words: ["no dread", "dread before or after", "dread long before and after"]
                onEdited: v => { spy.given.push(v) }
            }
            Rectangle { width: parent.width; height: 900; color: "#ddd" }
        }
    }
    QtObject { id: spy; property var given: [] }

    TestCase {
        name: "RatingSlider"
        when: windowShown

        function track() {
            // The Slider inside: the second child of the ColumnLayout.
            for (let i = 0; i < slider.children.length; ++i)
                if (slider.children[i].toString().indexOf("Slider") >= 0) return slider.children[i]
            return null
        }
        function init() {
            spy.given = []
            slider.value = null
            slider.shown = null
            slider.held = null
            scroll.contentItem.contentY = 0
            wait(50)
        }

        function test_tap_sets_value() {
            const s = track()
            verify(s !== null, "slider found")
            compare(slider.said, false)
            // A touch tap at 70 % of the track.
            const x = s.leftPadding + 11 + 0.7 * (s.availableWidth - 22)
            const y = s.height / 2
            const touch = touchEvent(s)
            touch.press(0, s, x, y).commit()
            touch.release(0, s, x, y).commit()
            wait(20)
            compare(spy.given.length, 1, "one value given")
            compare(spy.given[0], 7)
            compare(slider.shown, 7)
        }

        function test_tap_at_middle_on_unsaid() {
            const s = track()
            const x = s.leftPadding + 11 + 0.5 * (s.availableWidth - 22)
            const touch = touchEvent(s)
            touch.press(0, s, x, s.height / 2).commit()
            touch.release(0, s, x, s.height / 2).commit()
            wait(20)
            compare(spy.given.length, 1)
            compare(spy.given[0], 5)
        }

        function test_horizontal_drag() {
            const s = track()
            const y = s.height / 2
            const x0 = s.leftPadding + 11 + 0.2 * (s.availableWidth - 22)
            const touch = touchEvent(s)
            touch.press(0, s, x0, y).commit()
            for (let k = 1; k <= 10; ++k) {
                touch.move(0, s, x0 + k * 20, y).commit()
                wait(5)
            }
            compare(spy.given.length, 0, "nothing given while the finger is down")
            touch.release(0, s, x0 + 200, y).commit()
            wait(20)
            compare(spy.given.length, 1, "given once, on release")
            const expected = Math.round(((x0 + 200) - s.leftPadding - 11) / (s.availableWidth - 22) * 10)
            compare(spy.given[0], expected)
        }

        function test_vertical_drag_scrolls() {
            const s = track()
            const y = s.height / 2
            const x0 = s.leftPadding + 11 + 0.3 * (s.availableWidth - 22)
            const before = scroll.contentItem.contentY
            const touch = touchEvent(s)
            touch.press(0, s, x0, y).commit()
            for (let k = 1; k <= 15; ++k) {
                touch.move(0, s, x0 + 1, y - k * 15).commit()
                wait(10)
            }
            touch.release(0, s, x0 + 1, y - 225).commit()
            wait(400)
            verify(scroll.contentItem.contentY > before + 50, "the page scrolled: " + scroll.contentItem.contentY)
            compare(spy.given.length, 0, "no value from a scroll")
            compare(slider.said, false)
        }

        function test_mouse_click_and_drag() {
            const s = track()
            const y = s.height / 2
            const x = s.leftPadding + 11 + 0.3 * (s.availableWidth - 22)
            mouseClick(s, x, y)
            wait(20)
            compare(spy.given.length, 1)
            compare(spy.given[0], 3)
            mousePress(s, x, y)
            mouseMove(s, x + 40, y)
            mouseMove(s, x + 80, y)
            mouseMove(s, x + 120, y)
            mouseRelease(s, x + 120, y)
            wait(20)
            compare(spy.given.length, 2)
            verify(spy.given[1] > 3)
        }

        function test_keys() {
            const s = track()
            s.forceActiveFocus()
            keyPress(Qt.Key_Right)
            compare(spy.given.length, 0, "not given while the key is down")
            keyRelease(Qt.Key_Right)
            compare(spy.given.length, 1)
            compare(spy.given[0], 5, "the first arrow starts at 5")
            keyClick(Qt.Key_Right)
            compare(spy.given[1], 6)
            keyClick(Qt.Key_Home)
            compare(spy.given[2], 0, "0 is said, not unsaid")
            compare(slider.said, true)
            keyClick(Qt.Key_Left)
            compare(spy.given.length, 3, "nothing below 0")
            keyClick(Qt.Key_Delete)
            compare(spy.given[3], null, "Delete: unsaid again")
            compare(slider.said, false)
            keyClick(Qt.Key_End)
            compare(spy.given[4], 10)
        }


        function test_scroll_over_said_keeps_it() {
            slider.value = 6
            const s = track()
            const y = s.height / 2
            const x0 = s.leftPadding + 11 + 0.9 * (s.availableWidth - 22)
            const touch = touchEvent(s)
            touch.press(0, s, x0, y).commit()
            for (let k = 1; k <= 15; ++k) {
                touch.move(0, s, x0, y - k * 15).commit()
                wait(10)
            }
            touch.release(0, s, x0, y - 225).commit()
            wait(400)
            compare(spy.given.length, 0)
            compare(slider.shown, 6)
        }

        function test_clear_button() {
            slider.value = 2
            let clear = null
            const walk = item => {
                for (let i = 0; i < item.children.length; ++i) {
                    const c = item.children[i]
                    if (c.text === "\u00d7") clear = c
                    walk(c)
                }
            }
            walk(slider)
            verify(clear !== null)
            mouseClick(clear)
            compare(spy.given.length, 1)
            compare(spy.given[0], null)
            compare(slider.said, false)
        }

        function test_tab_release_gives_nothing() {
            const s = track()
            s.forceActiveFocus()
            keyRelease(Qt.Key_Tab)
            compare(spy.given.length, 0)
        }


        function test_hint_starts_the_arrows_and_is_never_given() {
            slider.hint = 7
            const s = track()
            compare(slider.said, false)
            wait(20)
            compare(spy.given.length, 0, "a hint gives nothing by itself")
            s.forceActiveFocus()
            keyClick(Qt.Key_Left)
            compare(spy.given[0], 7, "the first arrow starts at the hint")
            keyClick(Qt.Key_Left)
            compare(spy.given[1], 6)
            slider.hint = null
        }

        function test_hint_tap_elsewhere() {
            slider.hint = 7
            const s = track()
            const x = s.leftPadding + 11 + 0.2 * (s.availableWidth - 22)
            const touch = touchEvent(s)
            touch.press(0, s, x, s.height / 2).commit()
            touch.release(0, s, x, s.height / 2).commit()
            wait(20)
            compare(spy.given.length, 1)
            compare(spy.given[0], 2)
            slider.hint = null
        }

        function test_value_from_outside() {
            slider.value = 4
            compare(slider.shown, 4)
            compare(slider.said, true)
            slider.value = null
            compare(slider.said, false)
            compare(spy.given.length, 0)
        }
    }
}
