// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The keys in the status line (KeysHint.qml): shown when the line has room
// and a hardware keyboard is attached (`sioul.keyboard`): on a computer, on
// a tablet with its keyboard, touch screen or not; never on a phone or a
// tablet without one, held upright or sideways, however wide (16 October
// 2026: "Ctrl+1 to Ctrl+0 places…" on a phone held sideways, 2400 pixels
// wide). Run by tools/qml-test.sh.

import QtQuick
import QtTest
import "../../qml"

Item {
    id: root

    width: 2400
    height: 200

    component Sioul: QtObject {
        // Whether a hardware keyboard is attached (devicekind.rs).
        property bool keyboard: true

        function text(key) {
            return key === "ui-keys" ? "Ctrl+1 to Ctrl+0 places · Ctrl+N new · Ctrl+Z undo · F5 refresh" : key
        }
    }

    Sioul {
        id: computerSioul
    }

    Sioul {
        id: phoneSioul

        keyboard: false
    }

    Sioul {
        id: tabletSioul

        keyboard: true
    }

    Theme {
        id: computer
    }

    // A touch screen: a phone's, a tablet's (Android).
    Theme {
        id: touchScreen

        touch: true
    }

    KeysHint {
        id: onComputer

        width: root.width
        sioul: computerSioul
        theme: computer
    }

    KeysHint {
        id: onPhone

        // Held sideways: as wide as a computer's line.
        width: root.width
        sioul: phoneSioul
        theme: touchScreen
    }

    KeysHint {
        id: onTablet

        width: root.width
        sioul: tabletSioul
        theme: touchScreen
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

        // A tablet with its keyboard shows them, touch screen and all; taken
        // away (the keyboard folded back, a Bluetooth one gone), they go.
        function test_a_tablet_shows_them_while_its_keyboard_is_attached() {
            onTablet.room = true
            verify(onTablet.visible, "a keyboard attached")
            tabletSioul.keyboard = false
            verify(!onTablet.visible, "taken away")
            tabletSioul.keyboard = true
            verify(onTablet.visible, "back")
        }
    }
}
