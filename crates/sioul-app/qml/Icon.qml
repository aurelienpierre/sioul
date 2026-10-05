// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A desktop icon, by its freedesktop name, in its own colours. Decorative,
// unless it has a tip: then hovering it says what it stands for, and screen
// readers read the tip.

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Controls.impl
import QtQuick.Layouts

IconImage {
    id: icon

    required property string iconName
    property int size: 18
    // Said on hover, when set: what the icon stands for, in words.
    property string tip: ""

    ToolTip.visible: hover.hovered && icon.tip !== ""
    // A tip may hold what a message says of itself (the shield's checks): a
    // word joiner after each "<" keeps it plain text (Theme.plain).
    ToolTip.text: icon.tip.replace(/</g, "<\u2060")
    ToolTip.delay: 300

    name: iconName
    sourceSize: Qt.size(size, size)
    // Read on a thread: a page with many icons waited for each (half a second on a phone).
    asynchronous: true
    Layout.preferredWidth: size
    Layout.preferredHeight: size
    Accessible.ignored: icon.tip === ""
    Accessible.name: icon.tip

    HoverHandler {
        id: hover

        enabled: icon.tip !== ""
    }
}
