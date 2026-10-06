// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// How a task was, once done, if you want to say (docs/research/capacity-budget.md,
// criteria 6 and 7: rated after, the gap to the forecast learned): the four
// costs and the gain, what was foreseen shown as pale marks. Only what you
// move is kept as felt; the rest stays unsaid, never copied from the forecast
// (it would bring its bias back: dread is foreseen higher than it is felt).
// Never asked twice, never counted, never praised.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: how

    required property var sioul
    required property var theme
    // What was foreseen: {cognitive, emotional, anxiety, body, gain}, each 0 to 10 or null.
    property var forecast: ({})
    // What was felt today, as kept so far; null before.
    property var kept: null
    readonly property var names: ["cognitive", "emotional", "anxiety", "body", "gain"]
    readonly property bool said: how.kept !== null && how.kept !== undefined && how.names.some(n => how.kept[n] !== null && how.kept[n] !== undefined)

    // Today's five values to keep: the one just given, the others as kept so far.
    signal given(var values)

    function felt(name) {
        const value = how.kept ? how.kept[name] : null
        return value === undefined ? null : value
    }

    function foreseen(name) {
        const value = how.forecast ? how.forecast[name] : null
        return value === undefined ? null : value
    }

    function give(name, value) {
        const values = {}
        for (const n of how.names)
            values[n] = n === name ? value : how.felt(n)
        how.given(values)
    }

    spacing: 4

    Label {
        Layout.fillWidth: true
        text: how.said ? how.sioul.text("felt-kept") : how.sioul.text("felt-hint")
        wrapMode: Text.Wrap
        font.pixelSize: 13
        color: how.theme.muted
    }
    RatingSlider {
        Layout.fillWidth: true
        Layout.topMargin: 4
        sioul: how.sioul
        theme: how.theme
        label: how.sioul.text("task-field-cognitive")
        words: [how.sioul.text("rating-cognitive-0"), how.sioul.text("rating-cognitive-5"), how.sioul.text("rating-cognitive-10")]
        value: how.felt("cognitive")
        hint: how.foreseen("cognitive")
        onEdited: given => how.give("cognitive", given)
    }
    RatingSlider {
        Layout.fillWidth: true
        sioul: how.sioul
        theme: how.theme
        label: how.sioul.text("task-field-emotional")
        words: [how.sioul.text("rating-emotional-0"), how.sioul.text("rating-emotional-5"), how.sioul.text("rating-emotional-10")]
        value: how.felt("emotional")
        hint: how.foreseen("emotional")
        onEdited: given => how.give("emotional", given)
    }
    RatingSlider {
        Layout.fillWidth: true
        sioul: how.sioul
        theme: how.theme
        label: how.sioul.text("task-field-anxiety")
        words: [how.sioul.text("rating-anxiety-0"), how.sioul.text("rating-anxiety-5"), how.sioul.text("rating-anxiety-10")]
        value: how.felt("anxiety")
        hint: how.foreseen("anxiety")
        onEdited: given => how.give("anxiety", given)
    }
    RatingSlider {
        Layout.fillWidth: true
        sioul: how.sioul
        theme: how.theme
        label: how.sioul.text("task-field-body")
        words: [how.sioul.text("rating-body-0"), how.sioul.text("rating-body-5"), how.sioul.text("rating-body-10")]
        value: how.felt("body")
        hint: how.foreseen("body")
        onEdited: given => how.give("body", given)
    }
    RatingSlider {
        Layout.fillWidth: true
        sioul: how.sioul
        theme: how.theme
        label: how.sioul.text("task-field-gain")
        words: [how.sioul.text("rating-gain-0"), how.sioul.text("rating-gain-5"), how.sioul.text("rating-gain-10")]
        value: how.felt("gain")
        hint: how.foreseen("gain")
        onEdited: given => how.give("gain", given)
    }
}
