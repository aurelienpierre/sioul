// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The keys in the status line (KeysHint.qml): shown on a computer when the
// line has room, never on a phone, held upright or sideways, however wide
// (the owner's new phone, 16 October 2026: "Ctrl+1 to Ctrl+0 places…" on a
// phone held sideways, 2400 pixels wide). Run by tools/qml-test.sh.

import QtQuick
import QtTest
import "../../qml"

Item {
    id: root

    width: 2400
    height: 200

    QtObject {
        id: mock

        function text(key) {
            return key === "ui-keys" ? "Ctrl+1 to Ctrl+0 places · Ctrl+N new · Ctrl+Z undo · F5 refresh" : key
        }
    }

    Theme {
        id: computer
    }

    // A phone (Android), or the runner's phone size: main.qml says it by the platform.
    Theme {
        id: phone

        touch: true
    }

    KeysHint {
        id: onComputer

        width: root.width
        sioul: mock
        theme: computer
    }

    KeysHint {
        id: onPhone

        // Held sideways: as wide as a computer's line.
        width: root.width
        sioul: mock
        theme: phone
    }

    TestCase {
        name: "KeysHint"
        when: windowShown

        function test_a_computer_shows_the_keys_when_the_line_has_room() {
            onComputer.room = true
            verify(onComputer.visible)
            compare(onComputer.text, "Ctrl+1 to Ctrl+0 places · Ctrl+N new · Ctrl+Z undo · F5 refresh")
            onComputer.room = false
            verify(!onComputer.visible)
            onComputer.room = true
        }

        function test_a_phone_held_sideways_never_shows_them() {
            onPhone.room = true
            verify(onPhone.width >= 720, "as wide as a computer's line")
            verify(!onPhone.visible)
            onPhone.room = false
            verify(!onPhone.visible)
        }
    }
}
