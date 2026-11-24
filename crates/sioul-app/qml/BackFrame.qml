// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The window's frame, the keys' last stop: the keyboard's focus rests here
// when nothing else holds it, and every page's key that nothing took comes up
// to it. A Back that reaches it found no popup to close it (one open beside a
// field that keeps the focus holds the window's Back shortcut back): it goes
// back as the shortcut would (`host.backTaken()`, main.qml), never leaving
// Sioul while something is open; with nothing open, it is left to Android,
// which puts Sioul in the background. tst_backkey.qml checks it.

import QtQuick

Rectangle {
    id: frame

    // The window: its `backTaken()`, true when it went back from something.
    required property var host

    focus: true
    Keys.onPressed: event => {
        if (event.key === Qt.Key_Back && frame.host.backTaken())
            event.accepted = true
    }
}
