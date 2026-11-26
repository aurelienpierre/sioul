// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// An action in a row of actions (a message's, the contacts' list): its icon
// and its words. In a row that runs short of room (`folds`, ActionFolds.qml)
// it waits in the row's ⋮ with its words, never as an icon alone; `compact`
// is for a lone sign the rules allow (◂ back, ⋮), its name on hover and to
// screen readers.

import QtQuick
import QtQuick.Controls.Basic

Button {
    id: action

    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme
    // Its icon, by its freedesktop name.
    required property string iconName
    // Its name: beside the icon, or, `compact`, said on hover and to screen readers.
    property string label: ""
    // Its icon alone: only for the signs that need no word (◂ back, ⋮).
    property bool compact: false
    // It applies here and now (its own condition): in the row, or in its ⋮.
    property bool offered: true
    // Its rank when the row runs short of room: the lowest stay the longest.
    property int rank: 0
    // Its row's folds (ActionFolds.qml), null for an action alone.
    property ActionFolds folds: null
    // No room for its words in its row: it waits in the row's ⋮.
    readonly property bool folded: action.folds !== null && action.folds.folded.indexOf(action) >= 0
    // Its width with its name beside its icon, whatever it shows now: what a
    // row of actions needs to show their names (ActionFolds.qml).
    readonly property real wideWidth: Math.ceil(nameMetrics.advanceWidth) + action.icon.width + action.spacing + action.leftPadding + action.rightPadding
    // Its width as its icon alone.
    readonly property real narrowWidth: action.icon.width + action.leftPadding + action.rightPadding

    visible: action.offered && !action.folded
    // As wide as its content: the style's buttons are 100 pixels at least.
    implicitWidth: action.implicitContentWidth + action.leftPadding + action.rightPadding
    text: action.theme.plain(action.label)
    icon.name: action.iconName
    icon.color: action.theme.text
    display: action.compact ? AbstractButton.IconOnly : AbstractButton.TextBesideIcon
    flat: true
    ToolTip.visible: action.compact && action.hovered
    ToolTip.text: action.theme.plain(action.label)
    ToolTip.delay: 300
    Accessible.name: action.label

    TextMetrics {
        id: nameMetrics

        font: action.font
        text: action.label
    }
}
