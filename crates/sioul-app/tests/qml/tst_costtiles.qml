// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The four costs and the gain as tiles (CostTiles.qml), by mouse, touch and
// keys: a tap sets a value and the same tap clears it, 0 is not unsaid; the
// digits, + and =, the arrows and Delete; the faint values taken by "Looks
// right", or overridden by a tap on one tile; "How was it?" with the forecast
// as a mark and never copied (FeltRatings.qml); one column when narrow, or
// when two would make a cell narrower than a finger. On a stand-in for Sioul.
// Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml" as App

Item {
    id: root

    width: 700
    height: 1400

    QtObject {
        id: words

        function text(key) { return key }
        function textWith(key, name, value) { return key + "(" + value + ")" }
        function textArgs(key, args) { return key + JSON.stringify(JSON.parse(args)) }
    }
    App.Theme {
        id: theme
    }
    QtObject {
        id: spy

        property var given: []
        property var taken: []
        property var felt: []
    }

    App.CostTiles {
        id: tiles

        width: 600
        sioul: words
        theme: theme
        values: ({ cognitive: null, emotional: null, anxiety: null, body: null, gain: null })
        // As a task panel does: the value given is the value said.
        onEdited: (name, value) => {
            spy.given.push([name, value])
            const next = Object.assign({}, tiles.values)
            next[name] = value
            tiles.values = next
        }
        onTaken: values => {
            spy.taken.push(values)
            tiles.values = Object.assign({}, tiles.values, values)
        }
    }

    App.FeltRatings {
        id: felt

        y: 700
        width: 600
        sioul: words
        theme: theme
        forecast: ({ cognitive: null, emotional: 2, anxiety: 7, body: null, gain: 0 })
        kept: null
        onGiven: values => {
            spy.felt.push(values)
            felt.kept = values
        }
    }

    TestCase {
        name: "CostTiles"
        when: windowShown

        function init() {
            spy.given = []
            spy.taken = []
            spy.felt = []
            tiles.width = 600
            theme.touch = false
            tiles.proposed = {}
            tiles.proposedFrom = ""
            tiles.values = { cognitive: null, emotional: null, anxiety: null, body: null, gain: null }
            felt.kept = null
            wait(20)
        }

        function tap(tile, index) {
            const p = tile.point(index)
            mouseClick(tile, p.x, p.y)
        }

        function findText(item, text) {
            if (item.text === text && item.visible)
                return item
            for (let i = 0; i < item.children.length; ++i) {
                const found = findText(item.children[i], text)
                if (found)
                    return found
            }
            return null
        }

        function test_a_tap_sets_and_the_same_tap_clears() {
            const t = tiles.tile("anxiety")
            verify(t !== null)
            compare(tiles.said("anxiety"), null)
            tap(t, 7)
            compare(spy.given.length, 1)
            compare(spy.given[0][0], "anxiety")
            compare(spy.given[0][1], 7)
            compare(tiles.said("anxiety"), 7)
            tap(t, 7)
            compare(spy.given[1][1], null, "the same tap: unsaid again")
            compare(tiles.said("anxiety"), null)
            tap(t, 3)
            tap(t, 5)
            compare(spy.given[3][1], 5, "another cell moves it")
        }

        function test_zero_is_not_unsaid() {
            const t = tiles.tile("body")
            tap(t, 0)
            compare(spy.given[0][1], 0, "the 0 cell gives 0")
            compare(tiles.said("body"), 0)
            compare(tiles.word("body", 0), "tile-cost-0")
            compare(tiles.word("body", null), "tile-unsaid")
            verify(t.Accessible.name.indexOf("tile-said{") === 0, "said aloud with its value: " + t.Accessible.name)
            tap(t, 0)
            compare(spy.given[1][1], null)
            compare(t.Accessible.name, "tile-said-none(tile-body)")
        }

        function test_a_touch_tap() {
            const t = tiles.tile("emotional")
            const p = t.point(4)
            const touch = touchEvent(t)
            touch.press(0, t, p.x, p.y).commit()
            touch.release(0, t, p.x, p.y).commit()
            wait(20)
            compare(spy.given.length, 1)
            compare(spy.given[0][1], 4)
        }

        function test_a_narrow_gauge_picks_the_nearest_cell() {
            tiles.width = 380
            wait(20)
            const t = tiles.tile("cognitive")
            const a = t.point(5)
            const b = t.point(6)
            // Between two cells, a little nearer the sixth; then past the last cell's end.
            mouseClick(t, (a.x + 2 * b.x) / 3, a.y)
            compare(spy.given[0][1], 6)
            const last = t.point(10)
            mouseClick(t, last.x + 3, last.y + 8)
            compare(spy.given[1][1], 10, "a little below the cells still counts")
            // The tile's margin before the 0 cell: 0.
            mouseClick(t, 3, a.y)
            compare(spy.given[2][1], 0, "the margin before the gauge gives 0")
        }

        function test_keys() {
            const t = tiles.tile("cognitive")
            t.forceActiveFocus()
            keyClick(Qt.Key_3)
            compare(spy.given[0][1], 3)
            keyClick(Qt.Key_0)
            compare(spy.given[1][1], 0, "0 is a value")
            keyClick(Qt.Key_Plus)
            compare(spy.given[2][1], 10, "+ gives 10")
            keyClick(Qt.Key_7)
            keyClick(Qt.Key_Equal)
            compare(spy.given[4][1], 10, "= gives 10, where + needs Shift")
            keyPress(Qt.Key_Left)
            compare(spy.given.length, 5, "an arrow gives nothing while the key is down")
            keyRelease(Qt.Key_Left)
            compare(spy.given[5][1], 9)
            keyClick(Qt.Key_Right)
            compare(spy.given[6][1], 10)
            keyClick(Qt.Key_Right)
            compare(spy.given.length, 7, "nothing past 10")
            keyClick(Qt.Key_Delete)
            compare(spy.given[7][1], null, "Delete: unsaid")
            keyClick(Qt.Key_Right)
            compare(spy.given[8][1], 5, "from unsaid, the first arrow gives the middle")
            keyClick(Qt.Key_Backspace)
            compare(spy.given[9][1], null, "Backspace: unsaid too")
        }

        function test_the_tiles_are_in_the_tab_order() {
            const first = tiles.tile("cognitive")
            first.forceActiveFocus()
            keyClick(Qt.Key_Tab)
            verify(tiles.tile("emotional").activeFocus, "Tab goes to the next tile")
            keyClick(Qt.Key_Tab)
            keyClick(Qt.Key_Tab)
            keyClick(Qt.Key_Tab)
            verify(tiles.tile("gain").activeFocus, "the gain after the four costs")
        }

        function test_looks_right_takes_the_faint_values() {
            tiles.values = { cognitive: 2, emotional: null, anxiety: null, body: null, gain: null }
            tiles.proposed = { cognitive: 6, emotional: 3.5, anxiety: 7, body: null, gain: 4 }
            tiles.proposedFrom = "item"
            wait(20)
            verify(tiles.offered)
            compare(spy.given.length, 0, "faint values give nothing by themselves")
            compare(tiles.tile("emotional").faintShown, true)
            compare(tiles.tile("cognitive").faintShown, false, "a value said hides the faint one")
            const button = findText(tiles, "tile-looks-right")
            verify(button !== null, "Looks right shows")
            mouseClick(button)
            compare(spy.taken.length, 1)
            const taken = spy.taken[0]
            compare(taken.cognitive, undefined, "a value said stays as said")
            compare(taken.emotional, 4, "a median between two, rounded")
            compare(taken.anxiety, 7)
            compare(taken.body, undefined, "no faint value, nothing taken")
            compare(taken.gain, 4)
            verify(!tiles.offered, "nothing faint left")
            compare(findText(tiles, "tile-looks-right"), null)
        }

        function test_a_tap_overrides_one_faint_tile() {
            tiles.proposed = { cognitive: 6, emotional: 3, anxiety: 7, body: 2, gain: 4 }
            tiles.proposedFrom = "kind"
            wait(20)
            tap(tiles.tile("anxiety"), 4)
            compare(spy.given.length, 1)
            compare(spy.given[0][0], "anxiety")
            compare(spy.given[0][1], 4, "the tap, not the faint 7")
            compare(spy.taken.length, 0, "the other faint values are not given")
            compare(tiles.said("cognitive"), null)
            verify(tiles.offered, "the others still wait for Looks right")
            // A tap on the faint value itself gives it, for that tile only.
            tap(tiles.tile("body"), 2)
            compare(spy.given[1][1], 2)
            compare(tiles.said("gain"), null)
            // The arrows start at the faint value.
            tiles.tile("gain").forceActiveFocus()
            keyClick(Qt.Key_Up)
            compare(spy.given[2][1], 4)
        }

        function test_how_was_it_marks_the_forecast_and_never_copies_it() {
            const t = felt.tiles.tile("anxiety")
            verify(t !== null)
            const mark = t.markAt()
            verify(mark !== null, "the forecast's mark shows")
            const cell = t.point(7)
            verify(Math.abs(mark.x - cell.x) < 2, "on the forecast's cell: " + mark.x + " against " + cell.x)
            verify(felt.tiles.tile("cognitive").markAt() === null, "no forecast, no mark")
            verify(felt.tiles.tile("gain").markAt() !== null, "a forecast of 0 is marked too")
            compare(t.faintShown, false, "the forecast is no faint value")
            verify(!felt.tiles.offered, "no Looks right in How was it?")
            compare(spy.felt.length, 0)
            tap(t, 4)
            compare(spy.felt.length, 1)
            compare(spy.felt[0].anxiety, 4)
            compare(spy.felt[0].emotional, null, "a forecast never copied")
            compare(spy.felt[0].gain, null, "a forecast of 0 never copied either")
            verify(t.markAt() !== null, "the mark stays beside what was felt")
            compare(felt.tiles.ask("gain"), "tile-gain-felt-ask", "the gain asked in the past")
        }

        function test_one_column_when_narrow() {
            tiles.width = 600
            wait(20)
            compare(tiles.columns, 2)
            const a = tiles.tile("cognitive")
            const b = tiles.tile("emotional")
            verify(b.x > a.x, "side by side")
            tiles.width = 340
            wait(20)
            compare(tiles.columns, 1, "under 360: one column")
            verify(Math.abs(tiles.tile("emotional").x - tiles.tile("cognitive").x) < 1, "one under the other")
            verify(tiles.tile("emotional").y > tiles.tile("cognitive").y)
        }

        function test_a_finger_gets_one_column_and_wide_cells() {
            theme.touch = true
            tiles.width = 372
            wait(20)
            compare(tiles.columns, 1, "two columns would make cells too narrow for a finger")
            const t = tiles.tile("cognitive")
            const width = t.point(2).x - t.point(1).x
            verify(width >= 30, "a cell a fingertip wide: " + width)
            tiles.width = 800
            wait(20)
            compare(tiles.columns, 2, "wide enough for two columns of finger-wide cells")
        }

        function test_the_words_follow_borg() {
            const costs = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10].map(v => tiles.word("cognitive", v))
            compare(costs.join(","), "tile-cost-0,tile-cost-1,tile-cost-2,tile-cost-3,tile-cost-3,tile-cost-5,tile-cost-5,tile-cost-7,tile-cost-7,tile-cost-7,tile-cost-10")
            const gains = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10].map(v => tiles.word("gain", v))
            compare(gains.join(","), "tile-gain-0,tile-gain-1,tile-gain-1,tile-gain-3,tile-gain-3,tile-gain-5,tile-gain-5,tile-gain-7,tile-gain-7,tile-gain-7,tile-gain-10")
        }
    }
}
