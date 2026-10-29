// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A time of day typed "HH:MM" behind its mask: every form's one field for a
// time typed (the unified agenda, part 5; a medicine's takes are picked from
// lists, TimeField.qml). Up and Down move it by five minutes, to the next
// whole five minutes, around midnight, but in an end field (`end`) only
// from 00:00 to 23:55; what does not read is left as typed, for its form to
// say. A step edits the text as typing does, so a `text:` binding holds. A
// phone offers its digits. Its form reads `text`, as from the field it
// replaces.

import QtQuick
import QtQuick.Controls.Basic

TextField {
    id: field

    // An end of something (its "to", its "until"): Up stops at 23:55 and Down at 00:00,
    // never around midnight to the same day's start (review of 21436bf, later 10).
    property bool end: false

    // Five minutes later (`step` 1) or earlier (-1), from the time typed; what does not
    // read whole ("9:", "25:00") stays as typed.
    function stepBy(step) {
        const parts = field.text.split(":")
        if (parts.length !== 2 || parts[0].trim().length !== 2 || parts[1].trim().length !== 2)
            return
        const hour = parseInt(parts[0], 10)
        const minute = parseInt(parts[1], 10)
        if (isNaN(hour) || isNaN(minute) || hour > 23 || minute > 59)
            return
        const now = hour * 60 + minute
        const snapped = step > 0 ? Math.floor(now / 5) * 5 + 5 : Math.ceil(now / 5) * 5 - 5
        const at = field.end ? Math.max(0, Math.min(23 * 60 + 55, snapped)) : (snapped + 1440) % 1440
        const pad = n => n < 10 ? "0" + n : String(n)
        // Through the field's own editing, never an assignment: a `text:` binding holds, and
        // the field follows its model when that changes (review of 21436bf, later 10). The
        // edit says itself as typed (`textEdited`).
        field.selectAll()
        field.insert(0, pad(Math.floor(at / 60)) + ":" + pad(at % 60))
        field.deselect()
    }

    inputMask: "99:99"
    inputMethodHints: Qt.ImhPreferNumbers
    font.features: { "tnum": 1 }
    Keys.onUpPressed: field.stepBy(1)
    Keys.onDownPressed: field.stepBy(-1)
}
