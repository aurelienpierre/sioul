// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// An action on a message: its icon and its name; narrow, the icon alone, its
// name said on hover and to screen readers.

import QtQuick
import QtQuick.Controls.Basic

Button {
    id: action

    required property var theme
    required property string iconName
    property string label: ""
    property bool compact: false
    // Its width with its name beside its icon, whatever it shows now: what a
    // row of actions needs to show their names (the Reader's `compact`).
    readonly property real wideWidth: Math.ceil(nameMetrics.advanceWidth) + action.icon.width + action.spacing + action.leftPadding + action.rightPadding

    // As wide as its content: the style's buttons are 100 pixels at least.
    implicitWidth: action.implicitContentWidth + action.leftPadding + action.rightPadding
    text: action.label
    icon.name: action.iconName
    icon.color: action.theme.text
    display: action.compact ? AbstractButton.IconOnly : AbstractButton.TextBesideIcon
    flat: true
    ToolTip.visible: action.compact && action.hovered
    ToolTip.text: action.label
    ToolTip.delay: 300
    Accessible.name: action.label

    TextMetrics {
        id: nameMetrics

        font: action.font
        text: action.label
    }
}
