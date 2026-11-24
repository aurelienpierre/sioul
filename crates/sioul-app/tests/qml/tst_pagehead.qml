// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A page's head (PageHead.qml) at a phone's width, a narrow column and a
// computer's: the ⚙ at the right end of the first line meets no other item
// (Notes at 1100 points drew it over the memo's button, its three navigation
// buttons laid as one block), no two items meet, none runs past the head, an
// item wider than its line is narrowed to it and gets its width back once it
// fits, and on a phone the filters fold behind one button that says how many
// are set. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 1200
    height: 600

    Theme {
        id: appTheme
    }
    QtObject {
        id: mock

        function textCounted(key, n) {
            return n === 0 ? key : key + " (" + n + ")"
        }
    }

    Component {
        id: headKind

        PageHead {
            id: head

            property real wideFirst: 0

            sioul: mock
            theme: appTheme
            filtersSet: 2
            // As Notes: a button with words and two icons alone, about 205 points together.
            navigation: [
                Button {
                    objectName: "first"
                    implicitWidth: head.wideFirst > 0 ? head.wideFirst : 110
                    text: "New note"
                },
                Button {
                    objectName: "second"
                    implicitWidth: 40
                    text: "+"
                },
                Button {
                    objectName: "third"
                    implicitWidth: 40
                    text: "●"
                }
            ]
            settings: ToolButton {
                objectName: "gear"
                implicitWidth: 40
                implicitHeight: 40
                text: "⚙"
            }

            PlainComboBox {
                objectName: "filterA"
                visible: !head.filtersFolded
                width: 200
                model: ["Every project"]
            }
            PlainComboBox {
                objectName: "filterB"
                visible: !head.filtersFolded
                width: 190
                model: ["All kinds"]
            }
        }
    }

    TestCase {
        name: "PageHead"
        when: windowShown

        function made(width, props) {
            const head = createTemporaryObject(headKind, root, Object.assign({ width: width }, props || {}))
            verify(head !== null)
            tryVerify(() => !head.laying, 2000, "laid out")
            waitForRendering(head)
            return head
        }
        function find(item, name) {
            if (item.objectName === name)
                return item
            for (let i = 0; i < item.children.length; i++) {
                const found = find(item.children[i], name)
                if (found !== null)
                    return found
            }
            return null
        }
        function box(head, item) {
            const at = item.mapToItem(head, 0, 0)
            return { x: at.x, y: at.y, w: item.width, h: item.height, name: item.objectName || item.text }
        }
        function meet(a, b) {
            return a.x < b.x + b.w - 0.5 && b.x < a.x + a.w - 0.5 && a.y < b.y + b.h - 0.5 && b.y < a.y + a.h - 0.5
        }
        // Every item laid in the row and shown, as rectangles in the head.
        function laid(head) {
            return head.items().filter(i => i.visible).map(i => box(head, i))
        }

        function test_the_gear_meets_no_item_data() {
            return [{ tag: "phone", width: 200 }, { tag: "narrow", width: 360 }, { tag: "computer", width: 1100 }, { tag: "Notes' column", width: 215 }]
        }
        function test_the_gear_meets_no_item(data) {
            const head = made(data.width)
            const gear = box(head, find(head, "gear"))
            const items = laid(head)
            verify(items.length >= 3, "the navigation laid: " + items.length)
            for (const item of items) {
                verify(!meet(gear, item), "the ⚙ meets " + item.name + " at " + data.width + ": " + JSON.stringify([gear, item]))
                verify(item.x + item.w <= head.width + 0.5, item.name + " runs past the head at " + data.width)
                verify(item.x >= -0.5, item.name + " starts before the head")
            }
            for (let i = 0; i < items.length; i++)
                for (let j = i + 1; j < items.length; j++)
                    verify(!meet(items[i], items[j]), items[i].name + " meets " + items[j].name + " at " + data.width)
            // The ⚙ at the right end of the first line.
            verify(Math.abs(gear.x + gear.w - head.width) < 0.5, "at the right end")
            compare(gear.y, 0, "on the first line")
        }

        // The navigation wraps one button at a time: in Notes' column, the memo goes under, New note stays.
        function test_the_navigation_wraps_one_button_at_a_time() {
            const head = made(215)
            const first = box(head, find(head, "first"))
            const third = box(head, find(head, "third"))
            compare(first.y, 0, "New note on the first line")
            verify(third.y > 0, "the memo on the next line")
        }

        // A first item wider than the first line's room is narrowed to it, and given its width back once it fits.
        function test_a_wide_first_item_is_narrowed_to_its_line() {
            const head = made(200, { wideFirst: 260 })
            const first = find(head, "first")
            const gear = box(head, find(head, "gear"))
            verify(!meet(gear, box(head, first)), "the ⚙ meets the narrowed item")
            verify(first.width <= 200 - 40 - head.row.spacing + 0.5, "narrowed: " + first.width)
            head.width = 1100
            tryCompare(first, "width", 260, 2000, "its own width back")
        }

        // On a phone the filters fold behind one button, which says how many are set.
        function test_the_filters_fold_on_a_phone() {
            const head = made(360, { foldsFilters: true })
            const fold = find(head, "pageFilters")
            verify(fold.visible)
            compare(fold.text, "ui-filters (2)")
            verify(!find(head, "filterA").visible, "folded")
            mouseClick(fold)
            tryVerify(() => find(head, "filterA").visible, 1000, "unfolded")
            tryVerify(() => !head.laying, 1000)
            const gear = box(head, find(head, "gear"))
            for (const item of laid(head))
                verify(!meet(gear, item), "the ⚙ meets " + item.name + " once unfolded")
        }
    }
}
