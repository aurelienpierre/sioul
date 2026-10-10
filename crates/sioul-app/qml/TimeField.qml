// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A time of day, picked rather than typed: its hour and its minute, each a
// short list (every five minutes, and the minute set when it falls between).
// The keyboard reaches both: Tab, then the arrows, or the list opened with
// Space. "08:00" in, "08:00" out (`time`, `edited`).

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

RowLayout {
    id: field

    required property var sioul
    required property var theme
    // "08:00".
    property string time: "08:00"

    // Picked by hand (not when `time` is set from outside).
    signal edited

    function pad(n) {
        return n < 10 ? "0" + n : String(n)
    }

    readonly property int hour: {
        const h = parseInt(field.time.split(":")[0], 10)
        return isNaN(h) ? 8 : Math.max(0, Math.min(23, h))
    }
    readonly property int minute: {
        const m = parseInt((field.time.split(":")[1] || "0"), 10)
        return isNaN(m) ? 0 : Math.max(0, Math.min(59, m))
    }
    readonly property var hours: Array.from({ length: 24 }, (_, i) => field.pad(i))
    // Every five minutes, and the minute set when it is between two.
    readonly property var minutes: {
        const every = Array.from({ length: 12 }, (_, i) => i * 5)
        if (every.indexOf(field.minute) < 0)
            every.push(field.minute)
        return every.sort((a, b) => a - b).map(m => field.pad(m))
    }

    function pick(h, m) {
        field.time = field.pad(h) + ":" + field.pad(m)
        field.edited()
    }

    spacing: 2

    // As wide as "23" and its arrow, not the style's 140 pixels.
    ComboBox {
        id: hourBox

        implicitContentWidthPolicy: ComboBox.WidestText
        implicitWidth: implicitContentWidth + leftPadding + rightPadding
        model: field.hours.map(h => field.theme.plain(h))
        currentIndex: field.hour
        font.features: { "tnum": 1 }
        Accessible.name: field.sioul.text("health-take-hour")
        onActivated: index => field.pick(index, field.minute)
    }
    Label {
        text: ":"
        color: field.theme.muted
    }
    ComboBox {
        id: minuteBox

        implicitContentWidthPolicy: ComboBox.WidestText
        implicitWidth: implicitContentWidth + leftPadding + rightPadding
        model: field.minutes.map(m => field.theme.plain(m))
        currentIndex: field.minutes.indexOf(field.pad(field.minute))
        font.features: { "tnum": 1 }
        Accessible.name: field.sioul.text("health-take-minute")
        onActivated: index => field.pick(field.hour, parseInt(field.minutes[index], 10))
    }
}
