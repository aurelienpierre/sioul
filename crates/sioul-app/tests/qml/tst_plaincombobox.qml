// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The window's ComboBox (PlainComboBox.qml): its list shows a choice holding
// "&" as it is written. Qt's own list draws each choice on a button, which
// takes "&" for the mark of a shortcut and leaves it out: "R&D" was drawn
// "RD" (the review of 5 October 2026). What a line draws is told by its
// width, against the same words in a plain Text of the same font. Run by
// tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 400
    height: 400

    PlainComboBox {
        id: plain

        model: ["R&D", "Tom & Jerry"]
    }
    // Qt's own, for the difference.
    ComboBox {
        id: bare

        y: 200
        model: ["R&D"]
    }
    PlainComboBox {
        id: roles

        x: 200
        textRole: "name"
        model: [{ name: "Ponts & Chaussées" }]
    }

    Text {
        id: measure
    }

    TestCase {
        name: "PlainComboBox"
        when: windowShown

        // The text item a list line draws its words with.
        function label(item) {
            if (!item)
                return null
            if (String(item).indexOf("MnemonicLabel") >= 0 || String(item).indexOf("QQuickText") === 0)
                return item
            for (let i = 0; i < item.children.length; ++i) {
                const found = label(item.children[i])
                if (found)
                    return found
            }
            return null
        }

        // The width `words` take in the font of `item`.
        function widthOf(words, item) {
            measure.font = item.font
            measure.text = words
            return measure.implicitWidth
        }

        // The words line `index` of `box`'s list draws, by their width.
        function drawn(box, index) {
            box.popup.open()
            tryCompare(box.popup, "opened", true)
            const line = box.popup.contentItem.itemAtIndex(index)
            verify(line !== null)
            const text = label(line.contentItem)
            verify(text !== null)
            const width = text.implicitWidth
            box.popup.close()
            tryCompare(box.popup, "visible", false)
            return { width: width, item: text }
        }

        function test_an_ampersand_is_drawn_as_written() {
            const first = drawn(plain, 0)
            compare(first.width, widthOf("R&D", first.item))
            const second = drawn(plain, 1)
            compare(second.width, widthOf("Tom & Jerry", second.item))
            // The closed box shows the choice as it is.
            compare(plain.displayText, "R&D")
        }

        function test_qt_s_own_list_leaves_it_out() {
            const line = drawn(bare, 0)
            compare(line.width, widthOf("RD", line.item))
        }

        function test_a_role_is_read_the_same_way() {
            const line = drawn(roles, 0)
            compare(line.width, widthOf("Ponts & Chaussées", line.item))
            compare(roles.displayText, "Ponts & Chaussées")
        }
    }
}
