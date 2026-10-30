// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Recents' picture of Sioul kept out while a health card is open (the
// unified agenda's part 8): on Android 13 and later the overview of apps then
// shows Sioul's icon and name, not a medicine (`sioul.recentsPicture`,
// android/main.cpp); back once it closes. Rust does nothing on Android 12
// and earlier, whose only way (FLAG_SECURE) would stop your own screenshots
// too, nor on a computer. main.qml says when a health card is open: the
// Health page shown, a dose's question (DoseTaken.qml), a prescription's
// date in the card host.

import QtQml

QtObject {
    id: guard

    required property var sioul
    property bool health: false
    // The picture as last said; Android's own at start: shown.
    property bool shown: true

    // Said once at each change, however the change came: Sioul opened on a
    // health card (a dose's notification brings Health) has its binding made
    // true at start, which says it at once (tst_recents.qml).
    function say() {
        if (guard.shown === !guard.health)
            return
        guard.shown = !guard.health
        guard.sioul.recentsPicture(guard.shown)
    }

    onHealthChanged: guard.say()
}
