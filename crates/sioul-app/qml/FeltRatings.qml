// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// How a task was, once done, if you want to say (docs/research/capacity-budget.md,
// criteria 6 and 7: rated after, the gap to the forecast learned): the four
// costs and the gain as tiles (CostTiles.qml), what was foreseen a thin mark
// on each gauge. Only what you tap is kept as felt; the rest stays unsaid,
// never copied from the forecast (it would bring its bias back: dread is
// foreseen higher than it is felt). Never asked twice, never counted, never praised.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: how

    // Sioul's backend (backend.rs): its words in your language and what it does.
    required property var sioul
    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme
    // What was foreseen: {cognitive, emotional, anxiety, body, gain}, each 0 to 10 or null.
    property var forecast: ({})
    // What was felt today, as kept so far; null before.
    property var kept: null
    // The four costs and the gain, by their names in a task's record.
    readonly property var names: ["cognitive", "emotional", "anxiety", "body", "gain"]
    // Whether anything was said of how it was.
    readonly property bool said: how.kept !== null && how.kept !== undefined && how.names.some(n => how.kept[n] !== null && how.kept[n] !== undefined)

    // The tiles themselves (for the tests).
    readonly property alias tiles: tiles

    // Today's five values to keep: the one just given, the others as kept so far.
    signal given(var values)

    // What was said of one of them: 0 to 10, or null when unsaid.
    function felt(name) {
        const value = how.kept ? how.kept[name] : null
        return value === undefined ? null : value
    }

    // One of them said (a tile tapped): kept as felt, the others as they were.
    function give(name, value) {
        const values = {}
        for (const n of how.names)
            values[n] = n === name ? value : how.felt(n)
        how.given(values)
    }

    // Its natural width, the tiles' (CostTiles.qml).
    readonly property real naturalWidth: tiles.naturalWidth

    spacing: 8

    Label {
        Layout.fillWidth: true
        text: how.said ? how.sioul.text("felt-kept") : how.sioul.text("felt-hint")
        wrapMode: Text.Wrap
        font.pixelSize: 13
        color: how.theme.muted
    }
    CostTiles {
        id: tiles

        Layout.fillWidth: true
        sioul: how.sioul
        theme: how.theme
        felt: true
        forecast: how.forecast || {}
        values: how.kept || {}
        onEdited: (name, value) => how.give(name, value)
    }
}
