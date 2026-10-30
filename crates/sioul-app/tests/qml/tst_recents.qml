// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Recents' picture of Sioul (RecentsGuard.qml): off while a health card is
// open, back once it closes, said once at each change; said at once when
// Sioul opens on one; nothing said while none is. Run by tools/qml-test.sh.

import QtQuick
import QtTest
import "../../qml" as App

Item {
    id: root

    QtObject {
        id: stand
        property var said: []
        function recentsPicture(shown) {
            stand.said = stand.said.concat([shown])
        }
    }

    Component {
        id: guardComponent
        App.RecentsGuard {
            sioul: stand
        }
    }
    // As main.qml makes it: its value written in, true from the start.
    Component {
        id: openedOnHealth
        App.RecentsGuard {
            sioul: stand
            health: root.healthAtStart
        }
    }
    property bool healthAtStart: true

    TestCase {
        name: "RecentsGuard"

        function init() {
            stand.said = []
        }

        function test_off_while_a_health_card_is_open() {
            const guard = createTemporaryObject(guardComponent, root, { health: false })
            compare(stand.said, [], "nothing said while no health card is open")
            guard.health = true
            compare(stand.said, [false], "off as one opens")
            guard.health = true
            compare(stand.said, [false], "said once")
            guard.health = false
            compare(stand.said, [false, true], "back once it closes")
        }

        function test_opened_on_a_health_card() {
            createTemporaryObject(guardComponent, root, { health: true })
            compare(stand.said, [false], "off from the start, said once")
            stand.said = []
            const bound = createTemporaryObject(openedOnHealth, root)
            compare(stand.said, [false], "bound to a health card from the start: off, said once")
            root.healthAtStart = false
            compare(stand.said, [false, true], "back once it closes")
            root.healthAtStart = true
        }
    }
}
