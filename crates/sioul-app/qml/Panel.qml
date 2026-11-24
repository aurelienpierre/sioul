// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A card: a soft surface with a thin border. With `accent`, a firmer border,
// for the one thing that cannot wait (a code).

import QtQuick
import QtQuick.Controls.Basic

Pane {
    id: panel

    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme
    // A firmer border, in the accent: the one thing that cannot wait.
    property bool accent: false

    // Inside a card, on every side: one padding for every card and section (Theme.qml).
    padding: theme.cardPadding

    background: Rectangle {
        color: panel.theme.surface
        radius: panel.theme.radius
        border.color: panel.accent ? panel.theme.accent : panel.theme.line
        border.width: panel.accent ? 2 : 1
    }
}
