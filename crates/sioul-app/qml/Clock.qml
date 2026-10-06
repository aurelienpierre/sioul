// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Now, for the whole window (main.qml): one clock that the line at now in the
// day views, today's marks and the pages that read again as time passes all
// follow. `now` is the minute (Unix seconds at its turn), `today` the day
// ("2026-10-06"); each changes only when it turns, today first, so that a
// page reading again at the minute sees the new day. One timer, set again at
// each minute's turn from the wall clock: a timer counts only while the
// computer runs (asleep, it waits), and Qt counts a Timer on its animation
// clock, so the time is read at each turn, never counted. Read again at once
// when the window or the app comes back (`read`). Held while `paused` (a
// phone that put Sioul away), read again as soon as it is not.

import QtQuick

QtObject {
    id: clock

    // The time it reads, in milliseconds since 1970: the system's, unless a test gives its own.
    property var source: () => Date.now()
    property bool paused: false
    // The system's time as it is made, its source's once made, then at each turn.
    property real now: Math.floor(Date.now() / 60000) * 60
    property string today: clock.dayOf(new Date())
    // The last minute said turned (minutes since 1970).
    property real turnedAt: 0

    // Each minute's turn, paused or not: the backend's minute rides on it.
    signal turned

    // "2026-10-06": a day as the pages and the views write it.
    function dayOf(d) {
        const pad = n => n < 10 ? "0" + n : String(n)
        return d.getFullYear() + "-" + pad(d.getMonth() + 1) + "-" + pad(d.getDate())
    }

    // The time read again: what changed is said, nothing else.
    function read() {
        if (clock.paused)
            return
        const at = new Date(clock.source())
        const day = clock.dayOf(at)
        if (day !== clock.today)
            clock.today = day
        const minute = Math.floor(at.getTime() / 60000) * 60
        if (minute !== clock.now)
            clock.now = minute
    }

    // To the next minute's turn, a little after it: set again at each turn,
    // so that a turn come early or late (the computer woken) is not carried on.
    function arm() {
        clock.turn.interval = 60000 - clock.source() % 60000 + 50
        clock.turn.restart()
    }

    onPausedChanged: clock.read()
    Component.onCompleted: {
        const at = new Date(clock.source())
        clock.today = clock.dayOf(at)
        clock.now = Math.floor(at.getTime() / 60000) * 60
        clock.turnedAt = Math.floor(at.getTime() / 60000)
        clock.arm()
    }

    readonly property Timer turn: Timer {
        onTriggered: {
            clock.arm()
            const minute = Math.floor(clock.source() / 60000)
            if (minute !== clock.turnedAt) {
                clock.turnedAt = minute
                clock.turned()
            }
            clock.read()
        }
    }
}
