// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The row of tabs (Tabs.qml), alone, with an assistive tool listening
// (qml-test: accessibility; tools/qml-test.sh starts it so). Short names share
// the row equally; longer ones each take their width and share what is left,
// the row still filled; names wider than the row make it scroll sideways and
// say so: the side that has more fades out, the tab at that edge shows cut,
// never whole, a chevron there (on a computer, not under a finger) brings the
// next tab into view; the tab shown is kept in sight. A font changed later is
// measured again. Tab reaches the tab shown and leaves the row; Left, Right,
// Home and End choose a tab, around the ends, and the focus follows it. As a
// screen reader is told: a page tab list named as the page, each tab a page
// tab by its words, checkable, the one shown checked and selected. Each tab is
// named `<row>:<id>`.

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
    Theme {
        id: fingerTheme

        touch: true
        compact: true
    }

    Column {
        Tabs {
            id: tabs

            property var said: []

            objectName: "views"
            width: 600
            theme: testTheme
            name: "Tasks"
            onChosen: id => {
                tabs.said = tabs.said.concat([id])
                tabs.current = id
            }
        }
        // Where Tab goes after the row.
        TextField {
            id: after

            width: 200
        }
    }

    // A probe of the assistive tool: Qt writes a button's own state over what QML said of it
    // once a tool listens. Not checkable, said checkable: told so only while no tool listens.
    AbstractButton {
        id: probe

        y: 200
        text: "probe"
        Accessible.checkable: true
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
        // A part of the row by its name: the fade, a chevron (null when not made).
        function child(name) {
            const find = item => {
                if (item.objectName === name)
                    return item
                for (let i = 0; i < item.children.length; ++i) {
                    const found = find(item.children[i])
                    if (found)
                        return found
                }
                return null
            }
            return find(tabs)
        }
        function shows(name) {
            const item = child(name)
            return item !== null && item.visible
        }
        // How much of a tab shows within the row as it is scrolled, from 0 to 1.
        function seen(item) {
            const left = Math.max(item.x, tabs.strip.contentX)
            const right = Math.min(item.x + item.width, tabs.strip.contentX + tabs.strip.width)
            return Math.max(0, right - left) / item.width
        }
        function whole(item) {
            return seen(item) > 0.999
        }
        // The tab at the row's right edge, and at its left, as scrolled.
        function atRight() {
            const edge = tabs.strip.contentX + tabs.strip.width
            for (let i = 0; i < tabs.tabRepeater.count; ++i) {
                const item = tabs.tabRepeater.itemAt(i)
                if (item.x < edge - 0.5 && item.x + item.width > edge - 0.5)
                    return item
            }
            return null
        }
        function atLeft() {
            const edge = tabs.strip.contentX
            for (let i = 0; i < tabs.tabRepeater.count; ++i) {
                const item = tabs.tabRepeater.itemAt(i)
                if (item.x < edge + 0.5 && item.x + item.width > edge + 0.5)
                    return item
            }
            return null
        }
        function words(n, prefix) {
            const model = []
            for (let i = 0; i < n; ++i)
                model.push({ id: "t" + i, text: prefix + " " + i })
            return model
        }

        function init() {
            tabs.width = 600
            tabs.rightPadding = 0
            tabs.theme = testTheme
            tabs.font.pixelSize = 15
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
            verify(!shows("fade") && !shows("chevronAfter"), "nothing says more")
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

        // Names a little too wide for their room: their padding narrows before the row scrolls.
        function test_names_narrow_their_padding_before_scrolling() {
            tabs.model = [{ id: "a", text: "Overview" }, { id: "b", text: "Budgets" }, { id: "c", text: "Movements" }]
            waitForRendering(tabs)
            tabs.width = Math.ceil(tabs.leastTotal + 4)
            tabs.current = "a"
            waitForRendering(tabs)
            verify(tabs.naturalTotal > tabs.width, "too wide at their room")
            compare(tabs.fit, "tight")
            verify(!tabs.scrolls, "no scrolling")
            for (const id of ["a", "b", "c"])
                verify(tab(id).width >= tab(id).contentItem.implicitWidth, id + ": its words whole")
            fuzzyCompare(tab("c").x + tab("c").width, tabs.width, 0.5, "the row filled")
        }

        // A page's room at the row's end (a ⚙): the row, its fade and its chevrons keep out of it.
        function test_a_page_s_room_at_the_end_is_left_out() {
            tabs.model = words(12, "A rather long section")
            tabs.current = "t0"
            tabs.rightPadding = 46
            waitForRendering(tabs)
            compare(tabs.strip.width, 600 - 46, "the row less the room")
            const fade = child("fade")
            compare(fade.x + fade.width, 600 - 46, "the fade within the row")
            const chevron = child("chevronAfter")
            verify(chevron.mapToItem(tabs, chevron.width, 0).x <= 600 - 46 + 0.5, "the chevron within the row")
            tabs.rightPadding = 0
        }

        function test_a_row_that_scrolls_says_so_and_keeps_the_tab_in_sight() {
            tabs.model = words(12, "A rather long section")
            tabs.current = "t0"
            waitForRendering(tabs)
            compare(tabs.fit, "scroll")
            verify(tabs.scrolls && tabs.strip.interactive, "the row scrolls")
            for (let i = 0; i < 12; ++i)
                verify(tab("t" + i).width >= tab("t" + i).contentItem.implicitWidth, "t" + i + ": its words whole")
            compare(tabs.strip.contentX, 0, "from the start")
            // The edge that has more: faded, a chevron, the tab there cut, near its middle.
            verify(shows("fade") && tabs.moreAfter, "the end fades")
            verify(shows("chevronAfter"), "a chevron at the end")
            verify(!tabs.moreBefore && !shows("chevronBefore"), "nothing before the first")
            const cut = atRight()
            verify(cut !== null, "a tab at the edge")
            const part = seen(cut)
            verify(part > 0.25 && part < 0.85, "the tab at the edge shows cut: " + part.toFixed(2))
            // The last one chosen: brought into sight, the start now faded.
            tabs.current = "t11"
            tryVerify(() => whole(tab("t11")), 1000, "the last one in sight")
            verify(tabs.moreBefore && shows("chevronBefore"), "the start fades, with its chevron")
            verify(!tabs.moreAfter && !shows("chevronAfter"), "nothing after the last")
            verify(atLeft() !== null && !whole(atLeft()), "the tab at the start's edge cut")
            // One in the middle: in sight, both edges cut.
            tabs.current = "t5"
            tryVerify(() => whole(tab("t5")), 1000, "the sixth in sight")
            for (const edge of [atLeft(), atRight()])
                verify(edge === null || !whole(edge), "an edge's tab cut")
            // The row made narrower, the tab shown still in sight.
            tabs.width = 300
            tryVerify(() => whole(tab("t5")), 1000, "in sight in a narrower row")
        }

        function test_a_chevron_brings_the_next_tab() {
            tabs.model = words(12, "A rather long section")
            tabs.current = "t0"
            waitForRendering(tabs)
            const hidden = atRight()
            verify(hidden !== null && !whole(hidden), "a tab cut at the end")
            mouseClick(child("chevronAfter"))
            tryVerify(() => whole(hidden), 1000, "brought whole")
            compare(tabs.current, "t0", "the tab shown unchanged")
            verify(shows("chevronBefore"), "now one before too")
            const back = atLeft()
            verify(back !== null, "a tab cut at the start")
            mouseClick(child("chevronBefore"))
            tryVerify(() => whole(back), 1000, "brought back whole")
        }

        function test_under_a_finger_no_chevron() {
            tabs.theme = fingerTheme
            tabs.width = 392
            tabs.model = [{ id: "now", text: "Maintenant" }, { id: "day", text: "La journée" }, { id: "list", text: "Liste" }, { id: "board", text: "Tableau" }, { id: "gantt", text: "Gantt" }, { id: "search", text: "Chercher" }]
            tabs.current = "now"
            waitForRendering(tabs)
            verify(tabs.scrolls, "the French names are wider than a phone")
            verify(shows("fade") && tabs.moreAfter, "the end fades")
            verify(child("chevronAfter") === null, "no chevron under a finger, none made")
            const cut = atRight()
            verify(cut !== null && !whole(cut), "the tab at the edge cut")
        }

        function test_the_wheel_scrolls_the_row() {
            tabs.model = words(12, "A rather long section")
            tabs.current = "t0"
            waitForRendering(tabs)
            compare(tabs.strip.contentX, 0)
            mouseWheel(tabs, 100, 20, 0, -240)
            tryVerify(() => tabs.strip.contentX > 0, 1000, "the wheel down scrolls the row on")
        }

        function test_a_font_changed_later_is_measured_again() {
            tabs.model = [{ id: "a", text: "Display" }, { id: "b", text: "Hours" }, { id: "c", text: "Words" }]
            tabs.current = "a"
            waitForRendering(tabs)
            const before = tabs.naturals[0]
            tabs.font.pixelSize = 26
            tryVerify(() => tabs.naturals[0] > before, 1000, "measured again")
            for (const id of ["a", "b", "c"])
                tryVerify(() => tab(id).width >= tab(id).contentItem.implicitWidth, 1000, id + ": its words whole in the new font")
        }

        function test_the_focus_is_on_the_tab_shown_and_follows_the_arrows() {
            tabs.model = [{ id: "now", text: "Now" }, { id: "day", text: "The day" }, { id: "list", text: "List" }]
            tabs.current = "now"
            waitForRendering(tabs)
            tab("now").forceActiveFocus(Qt.TabFocusReason)
            compare(root.Window.activeFocusItem, tab("now"), "the tab shown has the focus")
            verify(tab("now").visualFocus, "and shows it")
            keyClick(Qt.Key_Right)
            compare(tabs.current, "day")
            compare(root.Window.activeFocusItem, tab("day"), "the focus follows")
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
            compare(root.Window.activeFocusItem, tab("list"))
            compare(tabs.said.join(" "), "day list now list now list")
            // Tab leaves the row for what follows, the other tabs skipped.
            keyClick(Qt.Key_Tab)
            compare(root.Window.activeFocusItem, after, "Tab leaves the row")
            keyClick(Qt.Key_Backtab)
            compare(root.Window.activeFocusItem, tab("list"), "Shift+Tab comes back to the tab shown")
        }

        function test_a_screen_reader_hears_the_tabs() {
            // Run with an assistive tool listening (tools/qml-test.sh), or this test proves nothing.
            verify(!probe.Accessible.checkable, "an assistive tool listens: Qt said the probe as it is")
            tabs.model = [{ id: "now", text: "Now" }, { id: "day", text: "The day" }]
            tabs.current = "now"
            waitForRendering(tabs)
            compare(tabs.Accessible.role, Accessible.PageTabList)
            compare(tabs.Accessible.name, "Tasks", "the list named as its page")
            compare(tab("now").Accessible.role, Accessible.PageTab)
            compare(tab("now").Accessible.name, "Now")
            verify(tab("now").Accessible.checkable && tab("day").Accessible.checkable, "each tab checkable")
            verify(tab("now").Accessible.checked, "the tab shown, checked")
            verify(tab("now").Accessible.selected, "and selected")
            verify(!tab("day").Accessible.checked && !tab("day").Accessible.selected, "the other, neither")
            mouseClick(tab("day"))
            compare(tabs.said.join(" "), "day")
            compare(tabs.current, "day")
            verify(tab("day").Accessible.checked && tab("day").Accessible.selected, "now the other")
            verify(!tab("now").Accessible.checked, "the first no more")
            // A click on the tab shown keeps it so.
            mouseClick(tab("day"))
            verify(tab("day").checked && tab("day").Accessible.checked, "still checked")
            verify(tab("day").contentItem.font.weight > tab("now").contentItem.font.weight, "strong words for the tab shown")
        }
    }
}
