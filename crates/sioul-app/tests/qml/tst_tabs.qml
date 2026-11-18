// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The row of tabs (Tabs.qml), alone: short names share the row equally;
// longer ones each take their width and share what is left, the row still
// filled; names wider than the row make it scroll sideways, with the tab
// shown kept in sight, by a choice and by the arrows; Left, Right, Home and
// End move between the tabs while the row has the focus, around the ends;
// a click chooses a tab; each tab is a page tab to screen readers, the one
// shown checked, and named `<row>:<id>`. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 900
    height: 300

    Theme {
        id: testTheme
    }

    Tabs {
        id: tabs

        property var said: []

        objectName: "views"
        width: 600
        theme: testTheme
        onChosen: id => {
            tabs.said = tabs.said.concat([id])
            tabs.current = id
        }
    }

    TestCase {
        name: "Tabs"
        when: windowShown

        function tab(id) {
            for (let i = 0; i < tabs.tabRepeater.count; ++i) {
                const item = tabs.tabRepeater.itemAt(i)
                if (item.objectName === "views:" + id)
                    return item
            }
            return null
        }
        // Whether a tab shows whole within the row as it is scrolled.
        function inSight(item) {
            return item.x >= tabs.strip.contentX - 0.5 && item.x + item.width <= tabs.strip.contentX + tabs.strip.width + 0.5
        }
        function words(n, prefix) {
            const model = []
            for (let i = 0; i < n; ++i)
                model.push({ id: "t" + i, text: prefix + " " + i })
            return model
        }

        function init() {
            tabs.width = 600
            tabs.said = []
            tabs.model = []
            tabs.current = ""
        }

        function test_short_names_share_the_row_equally() {
            tabs.model = [{ id: "now", text: "Now" }, { id: "list", text: "List" }, { id: "board", text: "Board" }]
            tabs.current = "list"
            waitForRendering(tabs)
            compare(tabs.fit, "equal")
            for (const id of ["now", "list", "board"])
                fuzzyCompare(tab(id).width, 200, 0.5, id + ": a third of the row")
            compare(tab("board").x + tab("board").width, 600, "the row filled")
            verify(!tabs.scrolls, "no scrolling")
        }

        function test_longer_names_keep_their_width_and_fill_the_row() {
            tabs.model = [{ id: "a", text: "Display" }, { id: "b", text: "Your folder and sharing, and a long tail" }, { id: "c", text: "AI" }]
            tabs.current = "a"
            waitForRendering(tabs)
            verify(tabs.naturals[1] > 200, "the long name does not fit a third")
            verify(tabs.naturalTotal <= 600, "all of them fit the row")
            compare(tabs.fit, "spread")
            for (const id of ["a", "b", "c"])
                verify(tab(id).width >= tab(id).contentItem.implicitWidth, id + ": its words whole")
            verify(tab("b").width > tab("a").width, "the long one wider")
            fuzzyCompare(tab("c").x + tab("c").width, 600, 0.5, "the row filled")
        }

        function test_names_wider_than_the_row_scroll_and_keep_the_tab_in_sight() {
            tabs.model = words(12, "A rather long section")
            tabs.current = "t0"
            waitForRendering(tabs)
            compare(tabs.fit, "scroll")
            verify(tabs.scrolls && tabs.strip.interactive, "the row scrolls")
            verify(tabs.strip.contentWidth > tabs.width, "wider than the row")
            for (let i = 0; i < 12; ++i)
                verify(tab("t" + i).width >= tab("t" + i).contentItem.implicitWidth, "t" + i + ": its words whole")
            compare(tabs.strip.contentX, 0, "from the start")
            // The last one chosen: brought into sight.
            tabs.current = "t11"
            tryVerify(() => inSight(tab("t11")), 1000, "the last one in sight")
            verify(tabs.strip.contentX > 0, "scrolled")
            // One in the middle: in sight too.
            tabs.current = "t5"
            tryVerify(() => inSight(tab("t5")), 1000, "the sixth in sight")
            // The row made narrower, the tab shown still in sight.
            tabs.width = 300
            tryVerify(() => inSight(tab("t5")), 1000, "in sight in a narrower row")
        }

        function test_the_wheel_scrolls_the_row() {
            tabs.model = words(12, "A rather long section")
            tabs.current = "t0"
            waitForRendering(tabs)
            compare(tabs.strip.contentX, 0)
            mouseWheel(tabs, 100, 20, 0, -240)
            tryVerify(() => tabs.strip.contentX > 0, 1000, "the wheel down scrolls the row on")
        }

        function test_arrows_move_between_tabs() {
            tabs.model = [{ id: "now", text: "Now" }, { id: "day", text: "The day" }, { id: "list", text: "List" }]
            tabs.current = "now"
            tabs.forceActiveFocus(Qt.TabFocusReason)
            verify(tabs.activeFocus, "the row has the focus")
            keyClick(Qt.Key_Right)
            compare(tabs.current, "day")
            keyClick(Qt.Key_Right)
            compare(tabs.current, "list")
            keyClick(Qt.Key_Right)
            compare(tabs.current, "now", "around the end")
            keyClick(Qt.Key_Left)
            compare(tabs.current, "list", "around the start")
            keyClick(Qt.Key_Home)
            compare(tabs.current, "now")
            keyClick(Qt.Key_End)
            compare(tabs.current, "list")
            compare(tabs.said.join(" "), "day list now list now list")
        }

        function test_arrows_keep_the_tab_in_sight() {
            tabs.model = words(12, "A rather long section")
            tabs.current = "t0"
            tabs.forceActiveFocus(Qt.TabFocusReason)
            for (let i = 1; i < 12; ++i) {
                keyClick(Qt.Key_Right)
                compare(tabs.current, "t" + i)
                tryVerify(() => inSight(tab("t" + i)), 1000, "t" + i + " in sight")
            }
        }

        function test_a_click_chooses_and_screen_readers_hear_page_tabs() {
            tabs.model = [{ id: "now", text: "Now" }, { id: "day", text: "The day" }]
            tabs.current = "now"
            waitForRendering(tabs)
            compare(tabs.Accessible.role, Accessible.PageTabList)
            compare(tab("now").Accessible.role, Accessible.PageTab)
            compare(tab("now").Accessible.name, "Now")
            verify(tab("now").Accessible.checked, "the tab shown, checked")
            verify(!tab("day").Accessible.checked, "the other, not")
            mouseClick(tab("day"))
            compare(tabs.said.join(" "), "day")
            compare(tabs.current, "day")
            verify(tab("day").Accessible.checked, "now checked")
            verify(tab("day").contentItem.font.weight > tab("now").contentItem.font.weight, "strong words for the tab shown")
        }
    }
}
