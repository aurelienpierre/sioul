// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A code that came by mail, on the Porch (CodeCard.qml), on a stand-in for
// Sioul: a click on the card opens its message, as the Porch's other cards
// do; Copy copies the code and opens nothing (23 October 2026). Run by
// tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 600
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

    CodeCard {
        id: card

        width: 500
        sioul: mock
        theme: testTheme
        // Made up: no real bank.
        card: ({ key: "/mail/inbox/cur/1791446400.code:2,S", title: "Code from Example Bank (verified)", code: "482913", validity: "valid about 10 minutes" })
    }

    SignalSpy {
        id: opened

        target: card
        signalName: "open"
    }
    SignalSpy {
        id: copied

        target: card
        signalName: "copy"
    }

    TestCase {
        name: "CodeCard"
        when: windowShown

        function find(item, name) {
            if (!item)
                return null
            if (item.objectName === name)
                return item
            const kids = item.children || []
            for (let i = 0; i < kids.length; ++i) {
                const found = find(kids[i], name)
                if (found)
                    return found
            }
            return item.contentItem && item.contentItem !== item ? find(item.contentItem, name) : null
        }

        function init() {
            opened.clear()
            copied.clear()
        }

        function test_a_click_opens_its_message() {
            mouseClick(card, 20, card.height - 10)
            compare(opened.count, 1)
            compare(opened.signalArguments[0][0], "/mail/inbox/cur/1791446400.code:2,S")
            compare(copied.count, 0)
        }

        function test_copy_copies_and_opens_nothing() {
            const copy = find(card, "codeCardCopy")
            verify(copy !== null)
            mouseClick(copy)
            compare(copied.count, 1)
            compare(copied.signalArguments[0][0], "482913")
            compare(opened.count, 0, "Copy opens nothing")
        }
    }
}
