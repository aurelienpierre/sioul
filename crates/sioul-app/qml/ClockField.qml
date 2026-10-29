// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A time of day typed "HH:MM" behind its mask: every form's one field for a
// time typed (the unified agenda, part 5; a medicine's takes are picked from
// lists, TimeField.qml). Up and Down move it by five minutes, to the next
// whole five minutes, around midnight; what does not read is left as typed,
// for its form to say. A phone offers its digits. Its form reads `text`, as
// from the field it replaces.

import QtQuick
import QtQuick.Controls.Basic

TextField {
    id: field

    // Five minutes later (`step` 1) or earlier (-1), from the time typed; "" stays "".
    function stepBy(step) {
        const parts = field.text.split(":")
        const hour = parseInt(parts[0], 10)
        const minute = parseInt(parts[1] || "0", 10)
        if (isNaN(hour) || isNaN(minute) || hour > 23 || minute > 59)
            return
        const now = hour * 60 + minute
        const snapped = step > 0 ? Math.floor(now / 5) * 5 + 5 : Math.ceil(now / 5) * 5 - 5
        const at = (snapped + 1440) % 1440
        const pad = n => n < 10 ? "0" + n : String(n)
        field.text = pad(Math.floor(at / 60)) + ":" + pad(at % 60)
        field.textEdited()
    }

    inputMask: "99:99"
    inputMethodHints: Qt.ImhPreferNumbers
    font.features: { "tnum": 1 }
    Keys.onUpPressed: field.stepBy(1)
    Keys.onDownPressed: field.stepBy(-1)
}
