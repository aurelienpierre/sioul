// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A page's column (docs/qt-quick.md, "A page's column"): text and forms keep
// a legible width, the theme's `readable` (720 points), centred in the room
// the page gives; tables, grids and cards in several columns (`wide`) take
// the whole room. Settings' sections, Papers, Letters and Tasks ▸ Now lay
// their content in one.

import QtQuick
import QtQuick.Layouts

ColumnLayout {
    id: column

    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme
    // The width the page gives: its ScrollView's `availableWidth`, or its own.
    required property real room
    // A table, a grid or cards side by side: the whole room.
    property bool wide: false

    width: column.wide ? column.room : Math.min(column.room, column.theme.readable)
    x: Math.max(0, Math.floor((column.room - column.width) / 2))
}
