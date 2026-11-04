// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A code that came by mail, on the Porch: its row in Right now
// (NoticeRow.qml, as PorchPage.qml makes it), on a stand-in for Sioul: a
// click on the row opens its message, as the Porch's other rows do; Copy,
// its main action, copies the code and opens nothing (23 October 2026);
// Dismiss and Mark read beside it, quiet. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 700
    height: 300

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        function text(key) {
            return key
        }
    }

    NoticeRow {
        id: card

        width: 600
        sioul: mock
        theme: testTheme
        // Made up: no real bank.
        notice: ({ id: "mail:home:00000000000000c1", kind: "mail", who: "Code from Example Bank (verified)", what: "482913, valid about 10 minutes", when: "Wed 4 Nov 09:12", readable: true, state: { mark: "new" } })
        iconName: "security-high"
        mainText: mock.text("ui-copy")
        mainIcon: "edit-copy"
    }

    SignalSpy {
        id: opened

        target: card
        signalName: "opened"
    }
    SignalSpy {
        id: copied

        target: card
        signalName: "mainPressed"
    }

    TestCase {
        name: "CodeCard"
        when: windowShown

        function find(item, test) {
            if (!item)
                return null
            if (test(item))
                return item
            const kids = item.children || []
            for (let i = 0; i < kids.length; ++i) {
                const found = find(kids[i], test)
                if (found)
                    return found
            }
            return item.contentItem && item.contentItem !== item ? find(item.contentItem, test) : null
        }

        function init() {
            opened.clear()
            copied.clear()
        }

        function test_a_click_opens_its_message() {
            mouseClick(card, 20, card.height / 2)
            compare(opened.count, 1)
            compare(copied.count, 0)
        }

        function test_copy_copies_and_opens_nothing() {
            const copy = find(card, i => i.visible && i.text === "ui-copy" && i.clicked !== undefined)
            verify(copy !== null)
            mouseClick(copy)
            compare(copied.count, 1)
            compare(opened.count, 0, "Copy opens nothing")
        }

        function test_dismiss_and_mark_read_beside_it() {
            verify(find(card, i => i.visible && i.objectName === "dismissButton") !== null)
            verify(find(card, i => i.visible && i.objectName === "readButton") !== null)
        }
    }
}
